# This file is a template used by .github/workflows/release.yml
# to auto-update the Homebrew tap at:
#   https://github.com/LiPingjiang/homebrew-tap/blob/main/Formula/hi.rb
#
# PLACEHOLDER_* values are replaced by the release workflow with real sha256 hashes.

class Hi < Formula
  desc "A modern, fast terminal text editor written in Rust"
  homepage "https://github.com/LiPingjiang/hi"
  version "0.1.0"
  license "MIT"

  on_macos do
    on_arm do
      url "https://github.com/LiPingjiang/hi/releases/download/v#{version}/hi-v#{version}-aarch64-apple-darwin.tar.gz"
      sha256 "PLACEHOLDER_AARCH64_APPLE"
    end
    on_intel do
      url "https://github.com/LiPingjiang/hi/releases/download/v#{version}/hi-v#{version}-x86_64-apple-darwin.tar.gz"
      sha256 "PLACEHOLDER_X86_64_APPLE"
    end
  end

  on_linux do
    on_arm do
      url "https://github.com/LiPingjiang/hi/releases/download/v#{version}/hi-v#{version}-aarch64-linux-gnu.tar.gz"
      sha256 "PLACEHOLDER_AARCH64_LINUX"
    end
    on_intel do
      url "https://github.com/LiPingjiang/hi/releases/download/v#{version}/hi-v#{version}-x86_64-linux-musl.tar.gz"
      sha256 "PLACEHOLDER_X86_64_LINUX"
    end
  end

  def install
    bin.install "hi"
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/hi --version")
  end
end
