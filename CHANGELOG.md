# Changelog

All notable changes to this project are documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Changed

- The installer now uses quiet downloads and explicitly confirms checksum
  verification.

## [1.0.0] - 2026-11-24

### Added

- Initial public release.
- Update SpotiFLAC-Next/SpotBye from a Proton Drive public share link.
- Automatic selection of direct AppImages or Linux `x86_64`/`aarch64` archives.
- SHA-256 verification of extracted AppImages when `SHA256SUMS.txt` is present.
- Atomic installation with a `.bak` backup of the previous AppImage.
- `--list` mode, custom `--target` support, and custom-password share support.
- Prebuilt release installer with checksum verification and source-build fallback.
- CI for formatting, clippy, tests, MSRV, and installer checks.
- Tag-driven release workflow for `x86_64` and `aarch64` binaries.

[Unreleased]: https://github.com/Ok2dot0/spotiflac-updater/compare/v1.0.0...HEAD
[1.0.0]: https://github.com/Ok2dot0/spotiflac-updater/releases/tag/v1.0.0
