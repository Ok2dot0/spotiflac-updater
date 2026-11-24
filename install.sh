#!/usr/bin/env bash
set -euo pipefail

APP_NAME="spotiflac-updater"
SHORT_NAME="spotiflac-update"
REPO="Ok2dot0/spotiflac-updater"
if ! ROOT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]:-$0}")" 2>/dev/null && pwd)"; then
    ROOT_DIR="$(pwd)"
fi
BIN_DIR="${BIN_DIR:-$HOME/.local/bin}"
ALIAS_MARKER="# SpotiFLAC-Next updater"
BUILD_MODE=0
TAG="${TAG:-}"

say() {
    printf '%s\n' "$*"
}

die() {
    printf 'error: %s\n' "$*" >&2
    exit 1
}

usage() {
    cat <<EOF
Install $APP_NAME.

Usage: install.sh [options]

Options:
  --build         Build from source instead of downloading a release
  --tag TAG       Install a specific release tag (for example v1.0.0)
  -h, --help      Show this help

Environment:
  BIN_DIR         Install directory (default: ~/.local/bin)
  INSTALL_RUST=1  Allow automatic Rust installation when building from source

Examples:
  bash install.sh
  bash install.sh --tag v1.0.0
  BIN_DIR="\$HOME/bin" bash install.sh
  bash install.sh --build
EOF
}

while [ "$#" -gt 0 ]; do
    case "$1" in
        --build)
            BUILD_MODE=1
            shift
            ;;
        --tag)
            [ "$#" -ge 2 ] || die "--tag requires a value"
            TAG="$2"
            shift 2
            ;;
        --tag=*)
            TAG="${1#*=}"
            shift
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        *)
            die "unknown option: $1 (try --help)"
            ;;
    esac
done

[ -n "$TAG" ] && [ "$BUILD_MODE" -eq 1 ] && die "--build and --tag cannot be combined"

case "$TAG" in
    *[!A-Za-z0-9._-]*) die "invalid release tag: $TAG" ;;
esac

case "$(uname -s)" in
    Linux) ;;
    *) die "this installer only supports Linux" ;;
esac

case "$(uname -m)" in
    x86_64|amd64) TARGET="x86_64-unknown-linux-gnu" ;;
    aarch64|arm64) TARGET="aarch64-unknown-linux-gnu" ;;
    *)
        die "unsupported architecture: $(uname -m) (supported: x86_64, aarch64)"
        ;;
esac

command -v install >/dev/null 2>&1 || die "the 'install' command is required"

TMP_DIR="$(mktemp -d)"
trap 'rm -rf "$TMP_DIR"' EXIT

add_shell_setup() {
    local rc="$1"
    [ -f "$rc" ] || return 0
    local changed=0

    if ! grep -qF "alias $SHORT_NAME=" "$rc"; then
        {
            printf '\n%s\n' "$ALIAS_MARKER"
            printf "alias %s='%s/%s'\n" "$SHORT_NAME" "$BIN_DIR" "$APP_NAME"
        } >>"$rc"
        say "Added $SHORT_NAME alias to $rc"
        changed=1
    fi

    if [ "$BIN_DIR" = "$HOME/.local/bin" ]; then
        if ! grep -qF '.local/bin' "$rc"; then
            # shellcheck disable=SC2016 # write a literal $HOME/$PATH into the rc file
            printf '\nexport PATH="$HOME/.local/bin:$PATH"\n' >>"$rc"
            say "Added ~/.local/bin to PATH in $rc"
            changed=1
        fi
    elif ! grep -qF "$BIN_DIR" "$rc"; then
        # shellcheck disable=SC2016 # write a literal $PATH into the rc file
        printf '\nexport PATH="%s:$PATH"\n' "$BIN_DIR" >>"$rc"
        say "Added $BIN_DIR to PATH in $rc"
        changed=1
    fi

    if [ "$changed" -eq 1 ]; then
        say "Open a new terminal or run: source $rc"
    fi
}

print_summary() {
    local installed_version
    installed_version="$("$BIN_DIR/$APP_NAME" --version 2>/dev/null || true)"
    say ""
    say "Installed: ${installed_version:-$BIN_DIR/$APP_NAME}"
    say "Binary:    $BIN_DIR/$APP_NAME"
    say "Command:   $SHORT_NAME"
    say ""
    say "Update SpotiFLAC-Next with:"
    say "  $SHORT_NAME 'https://drive.proton.me/urls/XXXXXXXXXX#YYYYYYYYYYY'"
    say ""
    say "The default target is ~/AppImages/spotiflacnext.appimage."
    say "Use --target PATH to update a different AppImage."
}

install_binary() {
    local source="$1"
    install -d "$BIN_DIR"
    install -m 755 "$source" "$BIN_DIR/$APP_NAME"
    ln -sfn "$BIN_DIR/$APP_NAME" "$BIN_DIR/$SHORT_NAME"

    add_shell_setup "$HOME/.bashrc"
    if [ -f "$HOME/.zshrc" ]; then
        add_shell_setup "$HOME/.zshrc"
    fi

    print_summary
}

download_release() {
    command -v curl >/dev/null 2>&1 || return 1
    command -v tar >/dev/null 2>&1 || return 1
    command -v sha256sum >/dev/null 2>&1 || return 1

    local base asset expected actual extracted
    if [ -n "$TAG" ]; then
        base="https://github.com/$REPO/releases/download/$TAG"
    else
        base="https://github.com/$REPO/releases/latest/download"
    fi

    asset="$APP_NAME-$TARGET.tar.gz"
    say "Downloading $asset..."
    if ! curl -fL --retry 3 --connect-timeout 15 \
        -o "$TMP_DIR/$asset" "$base/$asset"; then
        return 1
    fi

    if ! curl -fL --retry 3 --connect-timeout 15 \
        -o "$TMP_DIR/SHA256SUMS" "$base/SHA256SUMS"; then
        return 1
    fi

    expected="$(awk -v file="$asset" '$2 == file { print $1 }' "$TMP_DIR/SHA256SUMS")"
    [ -n "$expected" ] || die "SHA256SUMS does not list $asset"
    actual="$(sha256sum "$TMP_DIR/$asset" | awk '{ print $1 }')"
    [ "$expected" = "$actual" ] || die "checksum mismatch for $asset; nothing installed"

    tar -xzf "$TMP_DIR/$asset" -C "$TMP_DIR"
    extracted="$TMP_DIR/${APP_NAME}-${TARGET}/$APP_NAME"
    [ -x "$extracted" ] || die "the release archive does not contain $APP_NAME"

    install_binary "$extracted"
    return 0
}

install_rust() {
    command -v curl >/dev/null 2>&1 || die "curl is required to install Rust"
    say "Installing Rust with rustup..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
        | sh -s -- -y --profile minimal
    # shellcheck disable=SC1091
    . "$HOME/.cargo/env"
}

build_from_source() {
    local build_dir
    build_dir="$ROOT_DIR"
    if [ ! -f "$build_dir/Cargo.toml" ]; then
        command -v git >/dev/null 2>&1 \
            || die "git is required to download the source (or use the release installer)"
        build_dir="$TMP_DIR/source"
        say "Cloning $REPO..."
        git clone --depth 1 "https://github.com/$REPO.git" "$build_dir"
    fi

    if ! command -v cargo >/dev/null 2>&1 && [ -f "$HOME/.cargo/env" ]; then
        # shellcheck disable=SC1091
        . "$HOME/.cargo/env"
    fi

    if ! command -v cargo >/dev/null 2>&1; then
        if [ -t 0 ]; then
            printf 'Rust is not installed. Install it with rustup? [Y/n] '
            read -r answer
            case "${answer:-Y}" in
                y|Y|yes|YES) ;;
                *) die "Rust is required. Install it from https://rustup.rs and re-run." ;;
            esac
        fi
        install_rust
    fi

    say "Building $APP_NAME from source..."
    cargo build --release --locked --manifest-path "$build_dir/Cargo.toml"
    install_binary "$build_dir/target/release/$APP_NAME"
}

if [ "$BUILD_MODE" -eq 1 ]; then
    build_from_source
elif download_release; then
    :
elif [ -n "$TAG" ]; then
    die "could not download release $TAG for $TARGET"
else
    say "No prebuilt release is available for $TARGET; building from source."
    build_from_source
fi
