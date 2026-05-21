# Installation

## One-line installer (Linux & macOS)

```bash
curl -fsSL https://raw.githubusercontent.com/LiPingjiang/hi/main/install.sh | sh
```

Detects your OS and architecture, downloads the matching pre-built binary from GitHub Releases, verifies the SHA256 checksum, and installs to `/usr/local/bin` (or `~/.local/bin` if you don't have write access).

### 国内加速

脚本会自动按优先级尝试多个镜像（ghproxy → ghfast → ghproxy.net → homeboyc → gitmirror → gitclone → 直连），无需手动配置。也可以直接用镜像拉取脚本本身：

```bash
# 推荐：ghproxy.com（稳定，支持大文件）
curl -fsSL https://ghproxy.com/https://raw.githubusercontent.com/LiPingjiang/hi/main/install.sh | sh

# 备选：ghfast.top（简洁快速）
curl -fsSL https://ghfast.top/https://raw.githubusercontent.com/LiPingjiang/hi/main/install.sh | sh

# 备选：ghproxy.net（支持断点续传）
curl -fsSL https://ghproxy.net/https://raw.githubusercontent.com/LiPingjiang/hi/main/install.sh | sh
```

强制指定某个镜像下载二进制：

```bash
HI_MIRROR=ghproxy curl -fsSL https://ghproxy.com/https://raw.githubusercontent.com/LiPingjiang/hi/main/install.sh | sh
```

可用镜像名：`ghproxy` · `ghfast` · `ghproxy_net` · `homeboyc` · `gitmirror` · `gitclone` · `github`

### Options

```bash
# Install a specific version
HI_VERSION=v0.1.2 curl -fsSL .../install.sh | sh

# Install to a custom directory
HI_INSTALL=~/.bin curl -fsSL .../install.sh | sh

# Force a specific mirror
HI_MIRROR=ghproxy curl -fsSL .../install.sh | sh
```

## Homebrew (macOS)

The easiest way to install and keep `hi` up to date on macOS.

```bash
brew tap LiPingjiang/tap
brew install hi
```

To upgrade to the latest version later:

```bash
brew upgrade hi
```

Supports both Apple Silicon (arm64) and Intel (x86_64) — Homebrew picks the right binary automatically.

## cargo install (requires Rust toolchain)

```bash
cargo install hi
```

## Download manually

Pre-built binaries for every release are available on the [Releases page](https://github.com/LiPingjiang/hi/releases):

| Platform | Archive |
|---|---|
| macOS Apple Silicon | `hi-<version>-aarch64-apple-darwin.tar.gz` |
| macOS Intel | `hi-<version>-x86_64-apple-darwin.tar.gz` |
| Linux x86\_64 (static) | `hi-<version>-x86_64-linux-musl.tar.gz` |
| Linux x86\_64 (glibc) | `hi-<version>-x86_64-linux-gnu.tar.gz` |
| Linux ARM64 | `hi-<version>-aarch64-linux-gnu.tar.gz` |

Each archive includes a `.sha256` checksum file.

## Terminal Requirements

> **Important:** `hi` requires a modern terminal emulator with true-color (24-bit) support.

The built-in macOS Terminal.app only supports 256 colors and will show incorrect colors. Use one of these instead:

| Terminal | Platform | Notes |
|---|---|---|
| **iTerm2** | macOS | Recommended. Full 24-bit color, ligatures, GPU rendering. |
| **Kitty** | macOS / Linux | GPU-accelerated, excellent performance. |
| **Alacritty** | macOS / Linux / Windows | Minimal, fast, Rust-based. |
| **WezTerm** | macOS / Linux / Windows | Feature-rich, Lua-configurable. |
| **Windows Terminal** | Windows | Default choice on Windows 11. |
| **Ghostty** | macOS / Linux | New, fast, native platform integration. |

To verify your terminal supports true-color:

```bash
printf "\x1b[38;2;255;100;0mTRUE COLOR\x1b[0m\n"
```

If you see "TRUE COLOR" in orange, you're good to go.
