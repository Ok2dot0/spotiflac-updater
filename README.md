# SpotiFLAC-Next Updater

[![CI](https://github.com/Ok2dot0/spotiflac-updater/actions/workflows/ci.yml/badge.svg)](https://github.com/Ok2dot0/spotiflac-updater/actions/workflows/ci.yml)
[![Latest release](https://img.shields.io/github/v/release/Ok2dot0/spotiflac-updater)](https://github.com/Ok2dot0/spotiflac-updater/releases/latest)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![MSRV](https://img.shields.io/badge/MSRV-1.96-blue.svg)](Cargo.toml)

A small Linux CLI that updates the **SpotiFLAC-Next** (now shipped as **SpotBye**)
AppImage from a Proton Drive share link.

Point it at a Proton Drive share; it finds the correct Linux archive, verifies the
published checksum, and atomically replaces the installed AppImage. Your desktop
entry and launcher stay untouched.

> This is an unofficial community tool. It is not affiliated with SpotiFLAC,
> SpotBye, Proton AG, or Spotify.

## Features

- Opens Proton Drive public share links, including the `#password` fragment.
- Picks a direct `.AppImage` or the Linux archive for the current architecture
  (`x86_64` or `aarch64`).
- Verifies `SHA256SUMS.txt` from the release archive when one is present.
- Replaces `~/AppImages/spotiflacnext.appimage` atomically.
- Keeps the previous version as `spotiflacnext.appimage.bak`.
- Cleans up every temporary download.
- Supports custom-password shares and custom target paths.
- Has a `--list` mode for inspecting a share without downloading anything.

## Requirements

- Linux on `x86_64` or `aarch64`
- An installed SpotiFLAC-Next/SpotBye AppImage at
  `~/AppImages/spotiflacnext.appimage` (or a custom path via `--target`)
- Rust/Cargo 1.96 or newer **only when building from source**
- The prebuilt release binaries target glibc 2.35+ (Ubuntu 22.04 and newer).
  On older systems, build from source.

## Installation

### Prebuilt release (recommended)

Review the installer first, then run it:

```bash
curl -fsSL https://raw.githubusercontent.com/Ok2dot0/spotiflac-updater/main/install.sh -o install.sh
less install.sh
bash install.sh
```

Or run it directly:

```bash
curl -fsSL https://raw.githubusercontent.com/Ok2dot0/spotiflac-updater/main/install.sh | bash
```

The installer downloads the latest release for your architecture, verifies its
SHA-256 checksum, installs `spotiflac-updater` to `~/.local/bin`, creates the
`spotiflac-update` command, and adds a shell alias.

Install a specific release or force a source build:

```bash
bash install.sh --tag v1.0.0
bash install.sh --build
BIN_DIR="$HOME/bin" bash install.sh
```

### From source

```bash
git clone https://github.com/Ok2dot0/spotiflac-updater.git
cd spotiflac-updater
./install.sh --build
```

If Rust is missing, the installer offers to install it with rustup.

### Manual release install

Download the archive for your architecture from the
[releases page](https://github.com/Ok2dot0/spotiflac-updater/releases/latest),
verify it against `SHA256SUMS`, extract it, and copy the binary somewhere on your
`PATH`:

```bash
tar -xzf spotiflac-updater-x86_64-unknown-linux-gnu.tar.gz
install -m 755 spotiflac-updater-x86_64-unknown-linux-gnu/spotiflac-updater ~/.local/bin/
```

## Usage

The simplest form prompts for the share URL:

```bash
spotiflac-update
```

Or pass the URL directly:

```bash
spotiflac-update 'https://drive.proton.me/urls/XXXXXXXXXX#YYYYYYYYYYY'
```

The URL after `#` is the decryption key. Prefer the interactive prompt on shared
machines so the key does not end up in your shell history.

Common options:

```bash
# Inspect the share without downloading anything
spotiflac-update --list 'https://drive.proton.me/urls/XXXXXXXXXX#YYYYYYYYYYY'

# Update a different AppImage path
spotiflac-update --target ~/Applications/SpotBye.AppImage '<url>'

# Show all options
spotiflac-update --help
```

After an update, restart SpotiFLAC-Next. If the app is running during the update,
the running process keeps using the old version until it is restarted.

### Exit status

| Code | Meaning |
| ---- | ------- |
| `0`  | Update installed, already up to date, or listing succeeded |
| `1`  | Any error (invalid link, download failure, checksum mismatch, write failure, ...) |

## How it works

1. Reads the Proton Drive public share metadata and decrypts it with the key in
   the URL fragment.
2. Walks the share and selects a direct `.AppImage`, or the Linux `.zip` matching
   the current architecture. The newest and best-matching candidate wins.
3. Downloads into a temporary directory. For `.zip` releases, extracts the
   AppImage and verifies it against `SHA256SUMS.txt` when available.
4. Validates that the result is an ELF AppImage of a plausible size.
5. Writes the new file next to the target, flushes it, makes it executable, and
   renames it into place. The previous file is moved to `.bak`.
6. Deletes all temporary files.

The desktop entry is never modified. If the update fails before the final rename,
the existing AppImage is left untouched.

## Troubleshooting

### `the share URL has no #password fragment`

Copy the complete Proton Drive link, including everything after `#`. The fragment
is not optional.

### `could not open the Proton Drive share (wrong password?)`

The link may be expired or the share may use a custom password. The updater asks
for the password when the share requires one.

### `no Linux AppImage or Linux .zip archive found in the share`

The share layout changed. Run `spotiflac-update --list '<url>'` to inspect the
files and open an issue with the file names (never the URL).

### `checksum mismatch`

The downloaded AppImage did not match the checksum published in the archive. The
existing installation was not changed. Retry; if it persists, do not install the
file and report the issue.

### The app still starts the old version

Restart SpotiFLAC-Next. The updater replaces the file on disk, but an already
running process cannot switch binaries.

### Permission denied during install

Make sure `~/.local/bin` exists and is writable, or choose another location with
`BIN_DIR="$HOME/bin" ./install.sh`.

## Security

- The Proton share URL contains a decryption key after `#`. Treat it as a secret.
  It is not logged by this tool, but shell history and process listings may expose
  it. Use the interactive prompt when possible.
- The updater talks directly to Proton Drive and GitHub (for installer downloads).
  It has no telemetry.
- Release archives are verified against `SHA256SUMS` from the same GitHub release.
  This protects against corrupt downloads, not against a compromised release.
- Release binaries are built from tagged source by GitHub Actions.
- Please report vulnerabilities privately through
  [GitHub Security Advisories](https://github.com/Ok2dot0/spotiflac-updater/security/advisories/new)
  instead of opening a public issue. See [SECURITY.md](SECURITY.md).

## Development

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test --locked
cargo build --release --locked
shellcheck install.sh
```

Test the installer from a checkout without touching the release feed:

```bash
BIN_DIR="$PWD/target/install-test" ./install.sh --build
"$PWD/target/install-test/spotiflac-updater" --version
```

See [CONTRIBUTING.md](CONTRIBUTING.md) for contribution guidelines.

## Releasing

1. Update the version in `Cargo.toml` and `CHANGELOG.md`.
2. Run the checks above.
3. Commit, tag, and push:

   ```bash
   git tag -a v1.0.0 -m 'v1.0.0'
   git push origin main --follow-tags
   ```

The [release workflow](.github/workflows/release.yml) builds `x86_64` and
`aarch64` archives, writes `SHA256SUMS`, and publishes a GitHub release.

## License

MIT. See [LICENSE](LICENSE).
