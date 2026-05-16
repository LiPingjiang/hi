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
#   4. Removes stale copies of `hi` from other locations (e.g. ~/.cargo/bin)
#   5. Installs the binary to HI_INSTALL
#   6. Verifies `which hi` resolves to the newly installed binary

set -eu

REPO="LiPingjiang/hi"
BINARY="hi"

# ── Helpers ────────────────────────────────────────────────────────────────────

# step N/TOTAL "description"  — numbered progress header
STEP_CURRENT=0
STEP_TOTAL=6

step() {
    STEP_CURRENT=$((STEP_CURRENT + 1))
    printf '\033[1;36m[%d/%d]\033[0m \033[1m%s\033[0m\n' "$STEP_CURRENT" "$STEP_TOTAL" "$*"
}

say() {
    printf '\033[1;32m  ✓ \033[0m%s\n' "$*"
}

info() {
    printf '\033[0;37m    %s\033[0m\n' "$*"
}

warn() {
    printf '\033[1;33m  ⚠ \033[0m%s\n' "$*" >&2
}

err() {
    printf '\033[1;31m  ✗ error:\033[0m %s\n' "$*" >&2
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
    info "Querying GitHub API for latest release... (may be slow in China)"
    LATEST=$(curl -fsSL --connect-timeout 15 --max-time 30 \
        "https://api.github.com/repos/${REPO}/releases/latest" \
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

# ── Remove stale hi binaries that would shadow the new install ─────────────────
# Common locations: ~/.cargo/bin (cargo install), ~/go/bin, ~/.local/bin, etc.
# We only remove copies that are NOT in the target install directory.

cleanup_old_versions() {
    TARGET_DIR="$1"
    TARGET_PATH="${TARGET_DIR}/${BINARY}"

    # Well-known directories where package managers drop binaries
    KNOWN_DIRS="${HOME}/.cargo/bin ${HOME}/.local/bin /usr/local/bin /usr/bin ${HOME}/go/bin ${HOME}/.bin ${HOME}/bin"

    for DIR in $KNOWN_DIRS; do
        CANDIDATE="${DIR}/${BINARY}"

        # Skip the directory we're installing into
        [ "$DIR" = "$TARGET_DIR" ] && continue

        # Skip if no hi binary exists there
        [ -f "$CANDIDATE" ] || continue

        # Skip if it's a symlink (likely managed by a package manager like brew)
        [ -L "$CANDIDATE" ] && continue

        # Found a stale copy — remove it
        if [ -w "$CANDIDATE" ]; then
            say "Removing old hi at ${CANDIDATE}"
            rm -f "$CANDIDATE"
        elif [ -w "$DIR" ]; then
            say "Removing old hi at ${CANDIDATE}"
            rm -f "$CANDIDATE"
        else
            warn "Found old hi at ${CANDIDATE} but cannot remove (no write permission)."
            warn "Please remove it manually: rm ${CANDIDATE}"
        fi
    done

    # Also handle cargo specifically: if cargo is available and hi is installed
    # via cargo, uninstall it cleanly so cargo's metadata stays consistent.
    if command -v cargo > /dev/null 2>&1; then
        if cargo install --list 2>/dev/null | grep -q "^hi v"; then
            info "Found hi installed via cargo, uninstalling... (this may take a moment)"
            cargo uninstall hi 2>/dev/null || true
            say "Removed cargo-installed hi"
        fi
    fi
}

# ── Post-install verification ──────────────────────────────────────────────────
# Make sure `which hi` points to the binary we just installed.

verify_install() {
    TARGET_DIR="$1"
    TARGET_PATH="${TARGET_DIR}/${BINARY}"
    EXPECTED_VERSION="$2"

    # Check which hi the shell would find
    RESOLVED=$(command -v "$BINARY" 2>/dev/null || true)

    if [ -z "$RESOLVED" ]; then
        warn "hi is not in your PATH. See instructions below."
        return
    fi

    # Normalize paths for comparison (resolve symlinks)
    RESOLVED_REAL=$(cd "$(dirname "$RESOLVED")" && pwd -P)/$(basename "$RESOLVED")
    TARGET_REAL=$(cd "$(dirname "$TARGET_PATH")" && pwd -P)/$(basename "$TARGET_PATH")

    if [ "$RESOLVED_REAL" != "$TARGET_REAL" ]; then
        warn "Another hi binary shadows the new install:"
        warn "  which hi  → ${RESOLVED}"
        warn "  installed → ${TARGET_PATH}"
        warn "Remove the old one: rm ${RESOLVED}"
        warn "Or move ${TARGET_DIR} earlier in your PATH."
        return
    fi

    # Verify version matches
    ACTUAL_VERSION=$("$TARGET_PATH" --version 2>/dev/null | awk '{print $NF}' || true)
    CLEAN_EXPECTED=$(echo "$EXPECTED_VERSION" | sed 's/^v//')

    if [ "$ACTUAL_VERSION" = "$CLEAN_EXPECTED" ]; then
        say "Verified: hi ${ACTUAL_VERSION} ✓"
    else
        warn "Version mismatch: expected ${CLEAN_EXPECTED}, got ${ACTUAL_VERSION}"
    fi
}

# ── Main ───────────────────────────────────────────────────────────────────────

main() {
    need_cmd curl
    need_cmd tar

    # ── Step 1: Detect environment ──
    step "Detecting environment"
    OS=$(detect_os)
    ARCH=$(detect_arch)
    say "OS: ${OS}, Arch: ${ARCH}"

    # ── Step 2: Resolve version ──
    step "Resolving version"
    VERSION=$(resolve_version)
    SUFFIX=$(archive_suffix "$OS" "$ARCH")
    INSTALL_DIR=$(resolve_install_dir)
    say "Version: ${VERSION}"
    info "Install dir: ${INSTALL_DIR}"

    ARCHIVE_NAME="${BINARY}-${VERSION}-${SUFFIX}.tar.gz"
    BASE_URL="https://github.com/${REPO}/releases/download/${VERSION}"
    ARCHIVE_URL="${BASE_URL}/${ARCHIVE_NAME}"
    CHECKSUM_URL="${ARCHIVE_URL}.sha256"

    # ── Step 3: Clean up old versions ──
    step "Cleaning up old installations"
    cleanup_old_versions "$INSTALL_DIR"

    # ── Step 4: Download ──
    step "Downloading binary"
    info "Source: ${ARCHIVE_URL}"
    info "Note: GitHub downloads may be slow in China — please wait..."

    TMP_DIR=$(mktemp -d)
    trap 'rm -rf "$TMP_DIR"' EXIT

    curl -fL --progress-bar --connect-timeout 15 --retry 3 --retry-delay 2 \
        "$ARCHIVE_URL" -o "${TMP_DIR}/${ARCHIVE_NAME}"
    curl -fsSL --connect-timeout 15 --retry 3 --retry-delay 2 \
        "$CHECKSUM_URL" -o "${TMP_DIR}/${ARCHIVE_NAME}.sha256"
    say "Download complete"

    # ── Step 5: Verify and extract ──
    step "Verifying checksum & extracting"
    verify_checksum "${TMP_DIR}/${ARCHIVE_NAME}" "${TMP_DIR}/${ARCHIVE_NAME}.sha256"
    tar -xzf "${TMP_DIR}/${ARCHIVE_NAME}" -C "$TMP_DIR"
    say "Extraction OK"

    # ── Step 6: Install ──
    step "Installing"
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

    # Verify the install is clean
    verify_install "$INSTALL_DIR" "$VERSION"

    printf '\n\033[1;32m✓ All done!\033[0m Run: hi --version\n\n'
}

main "$@"
