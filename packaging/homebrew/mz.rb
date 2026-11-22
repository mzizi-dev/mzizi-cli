# DRAFT Homebrew formula for the Mzizi CLI. Not published anywhere.
#
# It belongs in a tap (for example mzizi-dev/homebrew-tap, Formula/mz.rb), which does not
# exist yet. Every sha256 below is a PLACEHOLDER: after a real release, copy the values from
# that release's SHA256SUMS. `brew audit --strict --new mz` must pass before it ships.
class Mz < Formula
  desc "The Mzizi CLI: add registry components, check the Bundu floor, plan with fundi"
  homepage "https://mzizi.dev"
  version "0.14.0"
  license "Apache-2.0"

  base = "https://github.com/mzizi-dev/mzizi-cli/releases/download/v#{version}"

  on_macos do
    on_arm do
      url "#{base}/mz-aarch64-apple-darwin"
      sha256 "0000000000000000000000000000000000000000000000000000000000000000" # PLACEHOLDER
    end
    on_intel do
      url "#{base}/mz-x86_64-apple-darwin"
      sha256 "0000000000000000000000000000000000000000000000000000000000000000" # PLACEHOLDER
    end
  end

  on_linux do
    on_arm do
      url "#{base}/mz-aarch64-unknown-linux-gnu"
      sha256 "0000000000000000000000000000000000000000000000000000000000000000" # PLACEHOLDER
    end
    on_intel do
      url "#{base}/mz-x86_64-unknown-linux-gnu"
      sha256 "0000000000000000000000000000000000000000000000000000000000000000" # PLACEHOLDER
    end
  end

  def install
    # The release asset is the bare binary, named mz-<triple>.
    bin.install Dir["mz-*"].first => "mz"
  end

  test do
    assert_equal version.to_s, shell_output("#{bin}/mz --version").strip
  end
end
