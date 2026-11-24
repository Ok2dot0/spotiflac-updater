# Security Policy

## Supported versions

Security fixes are provided for the latest release. Older versions are supported
on a best-effort basis.

## Reporting a vulnerability

Please report vulnerabilities privately through
[GitHub Security Advisories](https://github.com/Ok2dot0/spotiflac-updater/security/advisories/new).

Do **not** open a public issue for a suspected vulnerability.

Please include:

- A description of the issue and its impact.
- Steps to reproduce, or a proof of concept.
- The affected version and your Linux distribution/architecture.

Never include a real Proton Drive share URL, its `#password` fragment, or any
other secret in a report. Replace them with placeholders.

## Response

This is a volunteer-maintained project. Reports are handled on a best-effort
basis. Please allow reasonable time for a fix and coordinated disclosure before
publishing details.

## Security model

- The Proton Drive share URL contains a decryption key after `#`. Treat it as a
  secret.
- The updater downloads release archives directly from Proton Drive and verifies
  the extracted AppImage against `SHA256SUMS.txt` when the archive provides one.
- Release binaries are built by GitHub Actions from the tagged source and
  published with a `SHA256SUMS` file. This detects corrupt downloads; it is not a
  substitute for independent code signing.
- The installer verifies downloaded release archives against the release's
  `SHA256SUMS` before installing them.
- No telemetry or analytics data is collected or sent.
