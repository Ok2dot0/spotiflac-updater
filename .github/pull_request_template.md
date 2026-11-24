## Summary

Describe the change and why it is needed.

## Testing

- [ ] `cargo fmt --check`
- [ ] `cargo clippy --all-targets --all-features -- -D warnings`
- [ ] `cargo test --all-targets --locked`
- [ ] `cargo build --release --locked`
- [ ] `shellcheck install.sh` (if the installer changed)
- [ ] Manually tested against a Proton Drive share (if applicable)

## Checklist

- [ ] The change is focused and does not include unrelated formatting or refactors.
- [ ] Tests were added or updated for behavior changes.
- [ ] `README.md` and `CHANGELOG.md` were updated when user-facing behavior changed.
- [ ] No Proton Drive URLs, decryption keys, AppImages, or other secrets are included.
