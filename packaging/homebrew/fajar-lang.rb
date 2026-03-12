class FajarLang < Formula
  desc "Systems programming language for embedded ML + OS integration"
  homepage "https://github.com/fajarkraton/fajar-lang"
  version "3.0.0"
  license "MIT"

  on_macos do
    on_arm do
      url "https://github.com/fajarkraton/fajar-lang/releases/download/v#{version}/fj-v#{version}-aarch64-apple-darwin.tar.gz"
      # sha256 "PLACEHOLDER"
    end
    on_intel do
      url "https://github.com/fajarkraton/fajar-lang/releases/download/v#{version}/fj-v#{version}-x86_64-apple-darwin.tar.gz"
      # sha256 "PLACEHOLDER"
    end
  end

  on_linux do
    on_arm do
      url "https://github.com/fajarkraton/fajar-lang/releases/download/v#{version}/fj-v#{version}-aarch64-unknown-linux-gnu.tar.gz"
      # sha256 "PLACEHOLDER"
    end
    on_intel do
      url "https://github.com/fajarkraton/fajar-lang/releases/download/v#{version}/fj-v#{version}-x86_64-unknown-linux-gnu.tar.gz"
      # sha256 "PLACEHOLDER"
    end
  end

  def install
    bin.install "fj"
  end

  test do
    assert_match "Fajar Lang", shell_output("#{bin}/fj --version")
  end
end
