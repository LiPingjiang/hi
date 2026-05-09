#!/usr/bin/env sh
# install.sh — hi editor installer
#
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/LiPingjiang/hi/main/install.sh | sh
#
# Options (via environment variables):
#   HI_VERSION   — install a specific version, e.g. HI_VERSION=v0.1.0
#   HI_INSTALL   — install directory, default: /usr/local/bin (falls back to ~/.local/bin)
#
# The script:
#   1. Detects OS and CPU architecture
#   2. Downloads the matching pre-built binary from GitHub Releases
#   3. Verifies the SHA256 checksum
#   4. Installs the binary to HI_INSTALL

set -eu

REPO="LiPingjiang/hi"
BINARY="hi"

# ── Helpers ────────────────────────────────────────────────────────────────────

say() {
    printf '\033[1;32m==> \033[0m%s\n' "$*"
}

warn() {
    printf '\033[1;33mwarn:\033[0m %s\n' "$*" >&2
}

err() {
    printf '\033[1;31merror:\033[0m %s\n' "$*" >&2
    exit 1
}

need_cmd() {
    if ! command -v "$1" > /dev/null 2>&1; then
        err "required command not found: $1"
    fi
}

# ── Detect OS ──────────────────────────────────────────────────────────────────

detect_os() {
    case "$(uname -s)" in
        Linux)  echo "linux" ;;
        Darwin) echo "macos" ;;
        *)      err "unsupported OS: $(uname -s)" ;;
    esac
}

# ── Detect architecture ────────────────────────────────────────────────────────

detect_arch() {
    case "$(uname -m)" in
        x86_64 | amd64)   echo "x86_64" ;;
        aarch64 | arm64)  echo "aarch64" ;;
        *)                err "unsupported architecture: $(uname -m)" ;;
    esac
}

# ── Map (os, arch) → archive suffix used in release filenames ─────────────────
# Filenames follow the pattern: hi-<version>-<suffix>.tar.gz

archive_suffix() {
    OS="$1"
    ARCH="$2"
    case "${OS}-${ARCH}" in
        macos-aarch64)  echo "aarch64-apple-darwin" ;;
        macos-x86_64)   echo "x86_64-apple-darwin" ;;
        linux-x86_64)   echo "x86_64-linux-musl" ;;   # static musl = widest compat
        linux-aarch64)  echo "aarch64-linux-gnu" ;;
        *)              err "no pre-built binary for ${OS}-${ARCH}" ;;
    esac
}

# ── Resolve version ────────────────────────────────────────────────────────────

resolve_version() {
    if [ -n "${HI_VERSION:-}" ]; then
        echo "$HI_VERSION"
        return
    fi
    need_cmd curl
    # Query GitHub API for the latest release tag
    LATEST=$(curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" \
        | grep '"tag_name"' \
        | sed 's/.*"tag_name": *"\([^"]*\)".*/\1/')
    if [ -z "$LATEST" ]; then
        err "could not determine latest version (GitHub API rate limit?). Set HI_VERSION=vX.Y.Z and retry."
    fi
    echo "$LATEST"
}

# ── Resolve install directory ──────────────────────────────────────────────────

resolve_install_dir() {
    if [ -n "${HI_INSTALL:-}" ]; then
        echo "$HI_INSTALL"
        return
    fi
    # Prefer /usr/local/bin if writable, otherwise ~/.local/bin
    if [ -w "/usr/local/bin" ]; then
        echo "/usr/local/bin"
    else
        echo "${HOME}/.local/bin"
    fi
}

# ── Verify SHA256 ──────────────────────────────────────────────────────────────

verify_checksum() {
    ARCHIVE="$1"
    CHECKSUM_FILE="$2"

    EXPECTED=$(awk '{print $1}' "$CHECKSUM_FILE")

    if command -v sha256sum > /dev/null 2>&1; then
        ACTUAL=$(sha256sum "$ARCHIVE" | awk '{print $1}')
    elif command -v shasum > /dev/null 2>&1; then
        ACTUAL=$(shasum -a 256 "$ARCHIVE" | awk '{print $1}')
    else
        warn "sha256sum / shasum not found — skipping checksum verification"
        return
    fi

    if [ "$ACTUAL" != "$EXPECTED" ]; then
        err "checksum mismatch!\n  expected: $EXPECTED\n  actual:   $ACTUAL"
    fi
    say "Checksum OK"
}

# ── Main ───────────────────────────────────────────────────────────────────────

main() {
    need_cmd curl
    need_cmd tar

    OS=$(detect_os)
    ARCH=$(detect_arch)
    VERSION=$(resolve_version)
    SUFFIX=$(archive_suffix "$OS" "$ARCH")
    INSTALL_DIR=$(resolve_install_dir)

    ARCHIVE_NAME="${BINARY}-${VERSION}-${SUFFIX}.tar.gz"
    BASE_URL="https://github.com/${REPO}/releases/download/${VERSION}"
    ARCHIVE_URL="${BASE_URL}/${ARCHIVE_NAME}"
    CHECKSUM_URL="${ARCHIVE_URL}.sha256"

    say "Installing hi ${VERSION} for ${OS}/${ARCH}"
    say "Downloading ${ARCHIVE_NAME} ..."

    TMP_DIR=$(mktemp -d)
    trap 'rm -rf "$TMP_DIR"' EXIT

    # Download archive and checksum
    curl -fsSL --progress-bar "$ARCHIVE_URL"  -o "${TMP_DIR}/${ARCHIVE_NAME}"
    curl -fsSL "$CHECKSUM_URL" -o "${TMP_DIR}/${ARCHIVE_NAME}.sha256"

    # Verify
    verify_checksum "${TMP_DIR}/${ARCHIVE_NAME}" "${TMP_DIR}/${ARCHIVE_NAME}.sha256"

    # Extract
    tar -xzf "${TMP_DIR}/${ARCHIVE_NAME}" -C "$TMP_DIR"

    # Install
    mkdir -p "$INSTALL_DIR"
    install -m 755 "${TMP_DIR}/${BINARY}" "${INSTALL_DIR}/${BINARY}"

    say "Installed to ${INSTALL_DIR}/${BINARY}"

    # Warn if install dir is not in PATH
    case ":${PATH}:" in
        *":${INSTALL_DIR}:"*) ;;
        *)
            warn "${INSTALL_DIR} is not in your PATH."
            warn "Add the following line to your shell profile (~/.bashrc, ~/.zshrc, etc.):"
            warn "  export PATH=\"${INSTALL_DIR}:\$PATH\""
            ;;
    esac

    say "Done! Run: hi --version"
}

main "$@"
