# Contributing

Thanks for taking the time to improve SpotiFLAC-Next Updater.

## Before you start

- Search existing issues and pull requests first.
- For larger changes, open an issue to discuss the approach before writing code.
- Never commit real Proton Drive share URLs, AppImages, backups, or other
  secrets. Use placeholders in tests and documentation.

## Development setup

Requirements:

- Rust 1.96 or newer (the MSRV is checked in CI)
- Linux, for integration testing
- `shellcheck`, if you change `install.sh`

Build and test:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --locked
cargo build --release --locked
shellcheck install.sh
```

Test the installer without downloading a release:

```bash
BIN_DIR="$PWD/target/install-test" ./install.sh --build
"$PWD/target/install-test/spotiflac-updater" --version
```

Live Proton Drive tests require a real share link. Do not add network-dependent
tests to the test suite or CI.

## Pull requests

- Keep changes focused and explain the motivation in the PR description.
- Add or update tests for behavior changes.
- Update `README.md` and `CHANGELOG.md` when user-facing behavior changes.
- Make sure CI is green.
- By contributing, you agree that your changes are licensed under the MIT
  license used by this project.

## Commit messages

Short, imperative subject lines are preferred, for example:

```text
Add checksum verification for direct AppImages
Fix backup path handling on targets without an extension
```

## Releasing

The maintainer updates the version in `Cargo.toml` and `CHANGELOG.md`, runs the
checks above, and pushes a `vX.Y.Z` tag. The release workflow builds both
supported architectures, writes `SHA256SUMS`, and publishes the GitHub release.
