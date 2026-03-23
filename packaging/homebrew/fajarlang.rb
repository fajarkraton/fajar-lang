# Homebrew formula for Fajar Lang
# Install: brew install fajarlang/tap/fajarlang
# Usage:   fj run program.fj

class Fajarlang < Formula
  desc "Systems programming language for embedded AI + OS integration"
  homepage "https://fajarlang.org"
  url "https://github.com/fajarkraton/fajar-lang/archive/refs/tags/v4.2.0.tar.gz"
  sha256 "PLACEHOLDER_SHA256"
  license "MIT"
  head "https://github.com/fajarkraton/fajar-lang.git", branch: "main"

  depends_on "rust" => :build

  def install
    system "cargo", "build", "--release", "--features", "native"
    bin.install "target/release/fj"

    # Install stdlib and examples
    (share/"fj/stdlib").install Dir["stdlib/*.fj"]
    (share/"fj/examples").install Dir["examples/*.fj"]
  end

  test do
    # Test basic compilation
    (testpath/"hello.fj").write <<~FJ
      fn main() {
          println("Hello from Fajar Lang!")
      }
    FJ
    assert_match "Hello from Fajar Lang!", shell_output("#{bin}/fj run #{testpath}/hello.fj")
  end
end
