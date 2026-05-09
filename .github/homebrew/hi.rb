# This formula is auto-generated. Copy it to your homebrew-tap repo:
#   https://github.com/LiPingjiang/homebrew-tap/blob/main/Formula/hi.rb
#
# After GitHub Actions builds v0.1.0, update the sha256 values with:
#   curl -sL <url> | shasum -a 256

class Hi < Formula
  desc "A modal text editor with native AI assistance"
  homepage "https://github.com/LiPingjiang/hi"
  version "0.1.0"
  license "Apache-2.0"

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
