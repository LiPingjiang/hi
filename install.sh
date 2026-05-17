#!/usr/bin/env sh
# install.sh — hi editor installer
#
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/LiPingjiang/hi/main/install.sh | sh
#
# Options (via environment variables):
#   HI_VERSION   — install a specific version, e.g. HI_VERSION=v0.1.0
#   HI_INSTALL   — install directory, default: /usr/local/bin (falls back to ~/.local/bin)
#   HI_MIRROR    — force a specific download mirror:
#                    "github"   — direct GitHub (default, tries mirrors on failure)
#                    "ghproxy"  — https://ghfast.top
#                    "mirror"   — https://hub.gitmirror.com
#
# The script:
#   1. Detects OS and CPU architecture
#   2. Resolves the latest version (with CN-friendly API fallback)
#   3. Downloads the matching pre-built binary (tries mirrors on slow/failed connections)
#   4. Verifies the SHA256 checksum
#   5. Removes stale copies of `hi` from other locations (e.g. ~/.cargo/bin)
#   6. Installs the binary to HI_INSTALL
#   7. Verifies `which hi` resolves to the newly installed binary

set -eu

REPO="LiPingjiang/hi"
BINARY="hi"

# ── Mirror configuration ────────────────────────────────────────────────────────
# Each mirror wraps a GitHub Release URL.
# Usage: mirror_url <mirror_name> <original_github_url>
#
# Supported mirrors (in priority order when auto-detecting):
#   ghfast     https://ghfast.top/           — fast, reliable CN proxy
#   gitmirror  https://hub.gitmirror.com/    — gitmirror.com proxy
#   github     https://github.com/           — direct (last resort in CN)

MIRROR_GHFAST="ghfast"
MIRROR_GITMIRROR="gitmirror"
MIRROR_DIRECT="github"

# Build a download URL for a given mirror and original GitHub release URL
mirror_url() {
    _MIRROR="$1"
    _ORIG="$2"   # full https://github.com/... URL
    case "$_MIRROR" in
        ghfast)     echo "https://ghfast.top/${_ORIG}" ;;
        gitmirror)  echo "https://hub.gitmirror.com/${_ORIG}" ;;
        github)     echo "${_ORIG}" ;;
        *)          echo "${_ORIG}" ;;
    esac
}

# ── Helpers ────────────────────────────────────────────────────────────────────

STEP_CURRENT=0
STEP_TOTAL=7

step() {
    STEP_CURRENT=$((STEP_CURRENT + 1))
    printf '\033[1;36m[%d/%d]\033[0m \033[1m%s\033[0m\n' "$STEP_CURRENT" "$STEP_TOTAL" "$*" >&2
}

say() {
    printf '\033[1;32m  ✓ \033[0m%s\n' "$*" >&2
}

info() {
    printf '\033[0;37m    %s\033[0m\n' "$*" >&2
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

archive_suffix() {
    OS="$1"
    ARCH="$2"
    case "${OS}-${ARCH}" in
        macos-aarch64)  echo "aarch64-apple-darwin" ;;
        macos-x86_64)   echo "x86_64-apple-darwin" ;;
        linux-x86_64)   echo "x86_64-linux-musl" ;;
        linux-aarch64)  echo "aarch64-linux-gnu" ;;
        *)              err "no pre-built binary for ${OS}-${ARCH}" ;;
    esac
}

# ── Query a GitHub releases/latest API endpoint and extract tag_name ──────────
# Top-level helper (POSIX sh does not support nested function definitions).
# Usage: _query_api <url>
# Prints the tag_name string (e.g. "v0.1.4") or nothing on failure.

_query_api() {
    curl -fsSL --connect-timeout 10 --max-time 20 "$1" 2>/dev/null \
        | grep '"tag_name"' \
        | sed 's/.*"tag_name"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/' \
        | tr -d '[:space:]'
}

# ── Resolve version ────────────────────────────────────────────────────────────
# Try GitHub API first; if it fails or returns empty, fall back to
# CN-accessible mirror APIs.

resolve_version() {
    if [ -n "${HI_VERSION:-}" ]; then
        echo "$HI_VERSION"
        return
    fi
    need_cmd curl

    # 1st attempt: direct GitHub API
    # NOTE: info/warn must be called OUTSIDE $(...) to avoid polluting the result.
    info "Querying GitHub API for latest release..."
    LATEST=$(_query_api "https://api.github.com/repos/${REPO}/releases/latest" || true)

    # 2nd attempt: ghfast mirror API (CN-friendly)
    if [ -z "$LATEST" ]; then
        warn "GitHub API unreachable, trying ghfast mirror..."
        LATEST=$(_query_api "https://ghfast.top/https://api.github.com/repos/${REPO}/releases/latest" || true)
    fi

    # 3rd attempt: gitmirror API
    if [ -z "$LATEST" ]; then
        warn "ghfast mirror failed, trying gitmirror..."
        LATEST=$(_query_api "https://hub.gitmirror.com/https://api.github.com/repos/${REPO}/releases/latest" || true)
    fi

    if [ -z "$LATEST" ]; then
        err "Could not determine latest version from any source.
  GitHub API may be rate-limited or blocked.
  Fix: set HI_VERSION=vX.Y.Z and retry, e.g.:
    HI_VERSION=v0.1.4 curl -fsSL https://raw.githubusercontent.com/LiPingjiang/hi/main/install.sh | sh"
    fi

    echo "$LATEST"
}

# ── Resolve install directory ──────────────────────────────────────────────────

resolve_install_dir() {
    if [ -n "${HI_INSTALL:-}" ]; then
        echo "$HI_INSTALL"
        return
    fi
    if [ -w "/usr/local/bin" ]; then
        echo "/usr/local/bin"
    else
        echo "${HOME}/.local/bin"
    fi
}

# ── Download with mirror fallback ─────────────────────────────────────────────
# download_with_fallback <output_file> <github_url>
#
# Mirror priority:
#   If HI_MIRROR is set → only try that mirror (no fallback)
#   Otherwise           → ghfast → gitmirror → direct GitHub
#
# Each attempt uses a 30-second timeout; on failure we move to the next mirror.

download_with_fallback() {
    _OUT="$1"
    _GITHUB_URL="$2"   # original https://github.com/... URL

    # Build the ordered list of mirrors to try
    if [ -n "${HI_MIRROR:-}" ]; then
        _MIRRORS="$HI_MIRROR"
    else
        _MIRRORS="${MIRROR_GHFAST} ${MIRROR_GITMIRROR} ${MIRROR_DIRECT}"
    fi

    for _M in $_MIRRORS; do
        _URL=$(mirror_url "$_M" "$_GITHUB_URL")
        info "Trying [${_M}]: ${_URL}"
        if curl -fL --progress-bar \
                --connect-timeout 15 --max-time 120 \
                --retry 2 --retry-delay 3 \
                "$_URL" -o "$_OUT" 2>/dev/null; then
            say "Downloaded via [${_M}]"
            return 0
        fi
        warn "Mirror [${_M}] failed, trying next..."
    done

    err "All download mirrors failed for: ${_GITHUB_URL}
  You can:
    1. Set HI_MIRROR=github and use a VPN/proxy
    2. Download manually from https://github.com/${REPO}/releases
       and place the binary in your PATH"
}

# Silent variant for small files (checksums) — no progress bar
download_silent_with_fallback() {
    _OUT="$1"
    _GITHUB_URL="$2"

    if [ -n "${HI_MIRROR:-}" ]; then
        _MIRRORS="$HI_MIRROR"
    else
        _MIRRORS="${MIRROR_GHFAST} ${MIRROR_GITMIRROR} ${MIRROR_DIRECT}"
    fi

    for _M in $_MIRRORS; do
        _URL=$(mirror_url "$_M" "$_GITHUB_URL")
        if curl -fsSL \
                --connect-timeout 15 --max-time 30 \
                --retry 2 --retry-delay 3 \
                "$_URL" -o "$_OUT" 2>/dev/null; then
            return 0
        fi
    done

    err "Failed to download checksum from all mirrors: ${_GITHUB_URL}"
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
        err "Checksum mismatch — the downloaded file may be corrupted or tampered with.
  expected: $EXPECTED
  actual:   $ACTUAL"
    fi
    say "Checksum OK"
}

# ── Remove stale hi binaries ───────────────────────────────────────────────────

cleanup_old_versions() {
    TARGET_DIR="$1"

    KNOWN_DIRS="${HOME}/.cargo/bin ${HOME}/.local/bin /usr/local/bin /usr/bin ${HOME}/go/bin ${HOME}/.bin ${HOME}/bin"

    for DIR in $KNOWN_DIRS; do
        CANDIDATE="${DIR}/${BINARY}"
        [ "$DIR" = "$TARGET_DIR" ] && continue
        [ -f "$CANDIDATE" ] || continue
        [ -L "$CANDIDATE" ] && continue

        if [ -w "$CANDIDATE" ] || [ -w "$DIR" ]; then
            say "Removing old hi at ${CANDIDATE}"
            rm -f "$CANDIDATE"
        else
            warn "Found old hi at ${CANDIDATE} but cannot remove (no write permission)."
            warn "Please remove it manually: rm ${CANDIDATE}"
        fi
    done

    if command -v cargo > /dev/null 2>&1; then
        if cargo install --list 2>/dev/null | grep -q "^hi v"; then
            info "Found hi installed via cargo, uninstalling..."
            cargo uninstall hi 2>/dev/null || true
            say "Removed cargo-installed hi"
        fi
    fi
}

# ── Post-install verification ──────────────────────────────────────────────────

verify_install() {
    TARGET_DIR="$1"
    TARGET_PATH="${TARGET_DIR}/${BINARY}"
    EXPECTED_VERSION="$2"

    RESOLVED=$(command -v "$BINARY" 2>/dev/null || true)

    if [ -z "$RESOLVED" ]; then
        warn "hi is not in your PATH. See instructions below."
        return
    fi

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

    # Sanity-check: VERSION must not contain whitespace
    case "$VERSION" in
        *[[:space:]]*) err "Resolved version contains whitespace: '${VERSION}'. Set HI_VERSION=vX.Y.Z and retry." ;;
    esac

    ARCHIVE_NAME="${BINARY}-${VERSION}-${SUFFIX}.tar.gz"
    GITHUB_BASE="https://github.com/${REPO}/releases/download/${VERSION}"
    ARCHIVE_GITHUB_URL="${GITHUB_BASE}/${ARCHIVE_NAME}"
    CHECKSUM_GITHUB_URL="${ARCHIVE_GITHUB_URL}.sha256"

    # ── Step 3: Clean up old versions ──
    step "Cleaning up old installations"
    cleanup_old_versions "$INSTALL_DIR"

    # ── Step 4: Download ──
    step "Downloading binary"
    info "Package: ${ARCHIVE_NAME}"
    if [ -n "${HI_MIRROR:-}" ]; then
        info "Mirror: ${HI_MIRROR} (forced via HI_MIRROR)"
    else
        info "Mirror: auto (ghfast → gitmirror → github)"
    fi

    TMP_DIR=$(mktemp -d)
    trap 'rm -rf "$TMP_DIR"' EXIT

    download_with_fallback \
        "${TMP_DIR}/${ARCHIVE_NAME}" \
        "$ARCHIVE_GITHUB_URL"

    download_silent_with_fallback \
        "${TMP_DIR}/${ARCHIVE_NAME}.sha256" \
        "$CHECKSUM_GITHUB_URL"

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

    case ":${PATH}:" in
        *":${INSTALL_DIR}:"*) ;;
        *)
            warn "${INSTALL_DIR} is not in your PATH."
            warn "Add the following line to your shell profile (~/.bashrc, ~/.zshrc, etc.):"
            warn "  export PATH=\"${INSTALL_DIR}:\$PATH\""
            ;;
    esac

    # ── Step 7: Verify install ──
    step "Verifying installation"
    verify_install "$INSTALL_DIR" "$VERSION"

    printf '\n\033[1;32m✓ All done!\033[0m Run: hi --version\n\n'
}

main "$@"
