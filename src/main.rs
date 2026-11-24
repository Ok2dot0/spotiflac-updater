#![forbid(unsafe_code)]

use std::collections::HashMap;
use std::ffi::OsString;
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use clap::Parser;
use proton_drive_rs::{Node, NodeKind, ProtonDrivePublicLinkClient};
use proton_sdk::config::ProtonClientConfiguration;
use proton_sdk::ids::NodeUid;
use sha2::{Digest, Sha256};
use zip::ZipArchive;

const APP_VERSION: &str = "external-drive-spotiflac@1.0.0";
const DEFAULT_TARGET: &str = "AppImages/spotiflacnext.appimage";

/// Update SpotiFLAC-Next from a Proton Drive share link.
#[derive(Debug, Parser)]
#[command(version, about)]
struct Cli {
    /// Proton Drive share URL, for example
    /// https://drive.proton.me/urls/XXXXXXXXXX#YYYYYYYYYYY
    ///
    /// If omitted, the URL is read from standard input.
    url: Option<String>,

    /// AppImage file to replace
    #[arg(short, long, value_name = "PATH")]
    target: Option<PathBuf>,

    /// List the share contents without downloading anything
    #[arg(long)]
    list: bool,
}

#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("error: {error:#}");
        std::process::exit(1);
    }
}

async fn run() -> Result<()> {
    let cli = Cli::parse();
    let url = match cli.url {
        Some(url) => clean_url(url),
        None => clean_url(prompt_url()?),
    };

    if !url.contains('#') {
        bail!(
            "the share URL has no #password fragment; paste the complete link, \
             including everything after the #"
        );
    }

    let config = ProtonClientConfiguration::new(APP_VERSION);
    let info = ProtonDrivePublicLinkClient::info(config.clone(), &url)
        .await
        .context("could not read the Proton Drive share")?;

    if info.is_legacy {
        bail!("this is a legacy Proton Drive share link that is no longer supported");
    }
    if info.vendor_type != 0 {
        bail!("this link belongs to another Proton app, not to Proton Drive");
    }

    let custom_password = if info.is_custom_password_protected {
        Some(
            rpassword::prompt_password("Share password: ")
                .context("could not read the share password")?,
        )
    } else {
        None
    };

    let client = ProtonDrivePublicLinkClient::open(config, &url, custom_password.as_deref())
        .await
        .context("could not open the Proton Drive share (wrong password?)")?;
    let root = client
        .get_root_node()
        .await
        .context("could not read the root of the share")?;

    println!("Share: {}", root.name);

    if cli.list {
        list_share(&client, &root).await?;
        return Ok(());
    }

    let nodes = if root.is_folder() {
        client
            .enumerate_subtree(&root.uid)
            .await
            .context("could not list the files in the share")?
    } else {
        Vec::new()
    };

    let choice = choose_candidate(&root, &nodes)?;
    println!("Using: {} ({})", choice.path, human_size(choice.size));

    let temp = tempfile::Builder::new()
        .prefix("spotiflac-update-")
        .tempdir()
        .context("could not create a temporary directory")?;

    let new_appimage = match choice.kind {
        FileKind::AppImage => {
            let path = temp.path().join("download.AppImage");
            download(&client, &choice.node, &path).await?;
            path
        }
        FileKind::Zip => {
            let archive_path = temp.path().join("download.zip");
            download(&client, &choice.node, &archive_path).await?;

            let extracted = extract_appimages(&archive_path, temp.path())?;
            if extracted.is_empty() {
                bail!("{} does not contain an .AppImage file", choice.node.name);
            }
            let selected = pick_appimage(&extracted)?;
            println!(
                "Extracted: {}",
                selected.file_name().unwrap_or_default().to_string_lossy()
            );
            selected
        }
    };

    validate_appimage(&new_appimage)?;

    let target = match cli.target {
        Some(target) => target,
        None => default_target()?,
    };

    install(&new_appimage, &target)?;
    Ok(())
}

fn prompt_url() -> Result<String> {
    print!("Proton Drive share URL: ");
    io::stdout().flush().context("could not write the prompt")?;

    let mut line = String::new();
    io::stdin()
        .read_line(&mut line)
        .context("could not read the URL")?;
    if line.trim().is_empty() {
        bail!("no URL given");
    }
    Ok(line)
}

fn clean_url(mut url: String) -> String {
    if let Some(rest) = url.trim().strip_prefix('\'') {
        url = rest.trim_end_matches('\'').to_string();
    } else if let Some(rest) = url.trim().strip_prefix('"') {
        url = rest.trim_end_matches('"').to_string();
    }
    url.trim().to_string()
}

fn default_target() -> Result<PathBuf> {
    let home = std::env::var_os("HOME").context("HOME is not set; pass --target")?;
    Ok(PathBuf::from(home).join(DEFAULT_TARGET))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FileKind {
    AppImage,
    Zip,
}

#[derive(Debug, Clone)]
struct Choice {
    node: Node,
    kind: FileKind,
    path: String,
    size: u64,
    score: u8,
}

fn choose_candidate(root: &Node, nodes: &[Node]) -> Result<Choice> {
    let paths = build_paths(root, nodes);
    let mut files: Vec<&Node> = nodes.iter().filter(|node| node.is_file()).collect();
    if root.is_file() {
        files.push(root);
    }

    if files.is_empty() {
        bail!("the share contains no files");
    }

    let mut candidates: Vec<Choice> = Vec::new();
    for node in &files {
        let lower = node.name.to_ascii_lowercase();
        let (kind, score) = if lower.ends_with(".appimage") {
            (FileKind::AppImage, 2u8)
        } else if let Some(score) = linux_zip_score(&lower) {
            (FileKind::Zip, score)
        } else {
            continue;
        };

        candidates.push(Choice {
            node: (*node).clone(),
            kind,
            path: paths
                .get(&node.uid)
                .cloned()
                .unwrap_or_else(|| node.name.clone()),
            size: file_size(node),
            score,
        });
    }

    candidates.sort_by_key(|choice| {
        (
            std::cmp::Reverse(choice.score),
            std::cmp::Reverse(choice.node.modification_time),
        )
    });

    if let Some(choice) = candidates.into_iter().next() {
        return Ok(choice);
    }

    let names: Vec<&str> = files.iter().map(|node| node.name.as_str()).collect();
    bail!(
        "no Linux AppImage or Linux .zip archive found in the share\n\
         files present: {}",
        names.join(", ")
    );
}

fn linux_zip_score(lower_name: &str) -> Option<u8> {
    if !lower_name.ends_with(".zip") {
        return None;
    }

    let arch_patterns: &[&str] = match std::env::consts::ARCH {
        "x86_64" => &["linux-x64", "linux-x86_64", "linux-x86-64", "linux-amd64"],
        "aarch64" => &["linux-arm64", "linux-aarch64", "linux-arm"],
        _ => &[],
    };

    if arch_patterns
        .iter()
        .any(|pattern| lower_name.contains(pattern))
    {
        Some(2)
    } else if lower_name.contains("linux") {
        Some(1)
    } else {
        None
    }
}

fn build_paths(root: &Node, nodes: &[Node]) -> HashMap<NodeUid, String> {
    let by_uid: HashMap<&NodeUid, &Node> = nodes.iter().map(|node| (&node.uid, node)).collect();

    let mut paths = HashMap::new();
    paths.insert(root.uid.clone(), root.name.clone());

    for node in nodes {
        let mut parts = vec![node.name.clone()];
        let mut parent = node.parent_uid.as_ref();
        let mut depth = 0;

        while let Some(parent_uid) = parent {
            if *parent_uid == root.uid {
                parts.push(root.name.clone());
                break;
            }
            let Some(parent_node) = by_uid.get(parent_uid) else {
                break;
            };
            parts.push(parent_node.name.clone());
            parent = parent_node.parent_uid.as_ref();

            depth += 1;
            if depth > 512 {
                break;
            }
        }

        parts.reverse();
        paths.insert(node.uid.clone(), parts.join("/"));
    }

    paths
}

fn file_size(node: &Node) -> u64 {
    match &node.kind {
        NodeKind::File {
            total_size_on_storage,
            claimed_size,
            ..
        } => claimed_size
            .filter(|size| *size >= 0)
            .map(|size| size as u64)
            .unwrap_or_else(|| (*total_size_on_storage).max(0) as u64),
        NodeKind::Folder => 0,
    }
}

async fn list_share(client: &ProtonDrivePublicLinkClient, root: &Node) -> Result<()> {
    let nodes = if root.is_folder() {
        client
            .enumerate_subtree(&root.uid)
            .await
            .context("could not list the files in the share")?
    } else {
        Vec::new()
    };

    let paths = build_paths(root, &nodes);
    let mut entries: Vec<&Node> = nodes.iter().collect();
    if root.is_file() {
        entries.push(root);
    }

    for node in entries {
        let path = paths
            .get(&node.uid)
            .cloned()
            .unwrap_or_else(|| node.name.clone());

        if node.is_folder() {
            println!("DIR  {path}");
        } else {
            println!(
                "FILE {path}  {}  {}",
                human_size(file_size(node)),
                format_time(node.modification_time)
            );
        }
    }

    Ok(())
}

async fn download(
    client: &ProtonDrivePublicLinkClient,
    node: &Node,
    destination: &Path,
) -> Result<()> {
    let size = file_size(node);
    println!("Downloading {} ({})...", node.name, human_size(size));

    let file = File::create(destination).context("could not create the download file")?;
    let mut writer = ProgressWriter::new(file, size);

    client
        .download_file_to(&node.uid, &mut writer)
        .await
        .with_context(|| format!("could not download {}", node.name))?;

    writer.finish();
    Ok(())
}

fn extract_appimages(archive_path: &Path, destination: &Path) -> Result<Vec<PathBuf>> {
    let file = File::open(archive_path).context("could not open the downloaded archive")?;
    let mut archive = ZipArchive::new(file).context("could not read the downloaded zip archive")?;

    let mut checksums: HashMap<String, String> = HashMap::new();
    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .with_context(|| format!("could not read zip entry {index}"))?;
        let name = entry
            .name()
            .context("zip entry has an invalid name")?
            .into_owned();
        let base = Path::new(&name)
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default();

        if base.eq_ignore_ascii_case("SHA256SUMS.txt") {
            let mut contents = String::new();
            entry
                .read_to_string(&mut contents)
                .context("could not read SHA256SUMS.txt")?;

            for line in contents.lines() {
                let mut fields = line.split_whitespace();
                let (Some(hash), Some(file_name)) = (fields.next(), fields.next()) else {
                    continue;
                };
                if hash.len() != 64 {
                    continue;
                }
                let file_name = file_name
                    .trim_start_matches('*')
                    .trim_start_matches("./")
                    .to_string();
                checksums.insert(file_name, hash.to_ascii_lowercase());
            }
        }
    }

    let mut extracted = Vec::new();
    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .with_context(|| format!("could not read zip entry {index}"))?;
        if entry.is_dir() {
            continue;
        }

        let name = entry
            .name()
            .context("zip entry has an invalid name")?
            .into_owned();
        if !name.to_ascii_lowercase().ends_with(".appimage") {
            continue;
        }

        let file_name = Path::new(&name)
            .file_name()
            .and_then(|name| name.to_str())
            .context("the AppImage inside the zip has an invalid name")?
            .to_string();
        let path = destination.join(&file_name);

        let mut output =
            File::create(&path).with_context(|| format!("could not extract {file_name}"))?;
        io::copy(&mut entry, &mut output)
            .with_context(|| format!("could not extract {file_name}"))?;
        output.sync_all().ok();

        if let Some(expected) = checksums.get(&file_name) {
            let actual = sha256_file(&path)?;
            if !actual.eq_ignore_ascii_case(expected) {
                bail!("checksum mismatch for {file_name}: expected {expected}, got {actual}");
            }
            println!("Checksum verified: {file_name}");
        }

        extracted.push(path);
    }

    Ok(extracted)
}

fn pick_appimage(appimages: &[PathBuf]) -> Result<PathBuf> {
    if appimages.len() == 1 {
        return Ok(appimages[0].clone());
    }

    let matching: Vec<&PathBuf> = appimages
        .iter()
        .filter(|path| {
            let name = path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_ascii_lowercase();
            name.contains("spotby") || name.contains("spotiflac")
        })
        .collect();

    if matching.len() == 1 {
        return Ok(matching[0].clone());
    }

    let names: Vec<String> = appimages
        .iter()
        .map(|path| {
            path.file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string()
        })
        .collect();
    bail!(
        "the archive contains more than one AppImage and none is clearly the app:\n  {}",
        names.join("\n  ")
    );
}

fn validate_appimage(path: &Path) -> Result<()> {
    let mut file = File::open(path).context("could not open the downloaded AppImage")?;
    let mut magic = [0u8; 12];
    file.read_exact(&mut magic)
        .context("the downloaded file is too small to be an AppImage")?;

    if &magic[0..4] != b"\x7fELF" {
        bail!("the downloaded file is not an ELF executable");
    }
    if &magic[8..10] != b"AI" {
        eprintln!("warning: the downloaded file is missing the AppImage magic bytes");
    }

    let size = file
        .metadata()
        .context("could not inspect the download")?
        .len();
    if size < 1_000_000 {
        bail!(
            "the downloaded AppImage is suspiciously small ({})",
            human_size(size)
        );
    }

    Ok(())
}

fn install(source: &Path, target: &Path) -> Result<()> {
    let new_hash = sha256_file(source)?;
    let old_hash = if target.exists() {
        Some(sha256_file(target)?)
    } else {
        None
    };

    if old_hash.as_deref() == Some(new_hash.as_str()) {
        println!(
            "Already up to date ({}), nothing changed.",
            human_size(fs::metadata(target)?.len())
        );
        return Ok(());
    }

    let parent = target
        .parent()
        .context("the target AppImage path has no parent directory")?;
    fs::create_dir_all(parent).with_context(|| format!("could not create {}", parent.display()))?;

    let mut temporary = tempfile::Builder::new()
        .prefix(".spotiflac-update-")
        .suffix(".tmp")
        .tempfile_in(parent)
        .context("could not create a temporary file next to the target AppImage")?;

    {
        let mut input = File::open(source).context("could not open the downloaded AppImage")?;
        io::copy(&mut input, &mut temporary)
            .context("could not copy the new AppImage into place")?;
        temporary
            .as_file()
            .sync_all()
            .context("could not flush the new AppImage to disk")?;
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        temporary
            .as_file()
            .set_permissions(fs::Permissions::from_mode(0o755))
            .context("could not make the new AppImage executable")?;
    }

    let backup = backup_path(target);
    if target.exists() {
        if backup.exists() {
            fs::remove_file(&backup)
                .with_context(|| format!("could not replace backup {}", backup.display()))?;
        }
        fs::rename(target, &backup)
            .with_context(|| format!("could not back up {}", target.display()))?;
    }

    if let Err(error) = temporary.persist(target) {
        if backup.exists() && !target.exists() {
            let _ = fs::rename(&backup, target);
        }
        return Err(
            anyhow::Error::new(error.error).context("could not move the new AppImage into place")
        );
    }

    if let Ok(directory) = File::open(parent) {
        let _ = directory.sync_all();
    }

    let new_size = fs::metadata(target)?.len();
    println!("Updated {}", target.display());
    if let Some(old_hash) = old_hash {
        println!(
            "  old: {}  {old_hash}",
            human_size(fs::metadata(&backup)?.len())
        );
        println!("  new: {}  {new_hash}", human_size(new_size));
        println!("  backup: {}", backup.display());
    } else {
        println!("  new: {}  {new_hash}", human_size(new_size));
    }
    println!("Restart SpotiFLAC-Next to use the new version.");

    Ok(())
}

fn backup_path(target: &Path) -> PathBuf {
    let mut name: OsString = target.as_os_str().to_os_string();
    name.push(".bak");
    PathBuf::from(name)
}

fn sha256_file(path: &Path) -> Result<String> {
    let mut file =
        File::open(path).with_context(|| format!("could not open {}", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 1024 * 1024];

    loop {
        let read = file
            .read(&mut buffer)
            .with_context(|| format!("could not read {}", path.display()))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }

    Ok(hex::encode(hasher.finalize()))
}

fn human_size(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut value = bytes as f64;
    let mut unit = 0;

    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }

    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

fn format_time(epoch_seconds: i64) -> String {
    chrono::DateTime::from_timestamp(epoch_seconds, 0)
        .map(|time| {
            time.with_timezone(&chrono::Local)
                .format("%Y-%m-%d %H:%M")
                .to_string()
        })
        .unwrap_or_else(|| epoch_seconds.to_string())
}

struct ProgressWriter<W> {
    inner: W,
    total: u64,
    written: u64,
    next_report: u64,
    step: u64,
}

impl<W> ProgressWriter<W> {
    fn new(inner: W, total: u64) -> Self {
        let step = (total / 20).max(256 * 1024);
        Self {
            inner,
            total,
            written: 0,
            next_report: step,
            step,
        }
    }

    fn finish(&mut self) {
        if self.total > 0 {
            eprint!("\r  downloaded {:>5.1}%\n", 100.0);
            let _ = io::stderr().flush();
        }
    }
}

impl<W: Write> Write for ProgressWriter<W> {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        let written = self.inner.write(buffer)?;
        self.written += written as u64;

        if self.total > 0 && self.written >= self.next_report {
            let percent = (self.written as f64 / self.total as f64 * 100.0).min(100.0);
            eprint!("\r  downloaded {percent:>5.1}%");
            let _ = io::stderr().flush();
            while self.written >= self.next_report {
                self.next_report = self.next_report.saturating_add(self.step);
            }
        }

        Ok(written)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clean_url_strips_quotes_and_whitespace() {
        assert_eq!(
            clean_url("  'https://drive.proton.me/urls/abc#def'  ".into()),
            "https://drive.proton.me/urls/abc#def"
        );
        assert_eq!(
            clean_url("\"https://drive.proton.me/urls/abc#def\"\n".into()),
            "https://drive.proton.me/urls/abc#def"
        );
        assert_eq!(
            clean_url("https://drive.proton.me/urls/abc#def".into()),
            "https://drive.proton.me/urls/abc#def"
        );
    }

    #[test]
    fn human_size_uses_binary_units() {
        assert_eq!(human_size(0), "0 B");
        assert_eq!(human_size(1023), "1023 B");
        assert_eq!(human_size(1024), "1.0 KiB");
        assert_eq!(human_size(1536), "1.5 KiB");
        assert_eq!(human_size(1024 * 1024), "1.0 MiB");
    }

    #[test]
    fn backup_path_appends_bak() {
        assert_eq!(
            backup_path(Path::new("/tmp/app.AppImage")),
            PathBuf::from("/tmp/app.AppImage.bak")
        );
    }

    #[test]
    fn linux_zip_score_matches_architecture() {
        assert_eq!(linux_zip_score("release-linux.zip"), Some(1));
        assert_eq!(linux_zip_score("release-windows-x64.zip"), None);
        assert_eq!(linux_zip_score("release-macos-universal.zip"), None);

        let arch_name = match std::env::consts::ARCH {
            "x86_64" => "release-linux-x64.zip",
            "aarch64" => "release-linux-arm64.zip",
            _ => return,
        };
        assert_eq!(linux_zip_score(arch_name), Some(2));
    }

    #[test]
    fn sha256_file_hashes_contents() {
        let temp = tempfile::tempdir().expect("tempdir");
        let path = temp.path().join("data");
        fs::write(&path, b"hello").expect("write test file");

        assert_eq!(
            sha256_file(&path).expect("hash"),
            "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
        );
    }

    #[test]
    fn pick_appimage_selects_the_only_file() {
        let temp = tempfile::tempdir().expect("tempdir");
        let appimage = temp.path().join("SpotBye.AppImage");
        fs::write(&appimage, b"x").expect("write test file");

        assert_eq!(
            pick_appimage(std::slice::from_ref(&appimage)).expect("pick"),
            appimage
        );
    }

    #[test]
    fn pick_appimage_prefers_spotiflac_name() {
        let temp = tempfile::tempdir().expect("tempdir");
        let other = temp.path().join("other.AppImage");
        let spotiflac = temp.path().join("SpotBye-1.0.0.AppImage");
        fs::write(&other, b"x").expect("write test file");
        fs::write(&spotiflac, b"x").expect("write test file");

        assert_eq!(
            pick_appimage(&[other, spotiflac.clone()]).expect("pick"),
            spotiflac
        );
    }

    #[test]
    fn pick_appimage_rejects_ambiguous_archives() {
        let temp = tempfile::tempdir().expect("tempdir");
        let first = temp.path().join("first.AppImage");
        let second = temp.path().join("second.AppImage");
        fs::write(&first, b"x").expect("write test file");
        fs::write(&second, b"x").expect("write test file");

        assert!(pick_appimage(&[first, second]).is_err());
    }
}
