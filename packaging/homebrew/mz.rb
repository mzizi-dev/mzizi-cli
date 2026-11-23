# Homebrew formula for the Mzizi CLI, with the sha256 values of the v0.14.1 release (from its
# SHA256SUMS). Not published anywhere: it belongs in a tap that does not exist yet.
#
# The tap would be mzizi-dev/homebrew-tap, Formula/mz.rb. Each release changes `version` and
# the four sha256 values (copy them from that release's SHA256SUMS). `brew audit --strict --new
# mz` must pass before it ships.
class Mz < Formula
  desc "The Mzizi CLI: add registry components, check the Bundu floor, plan with fundi"
  homepage "https://mzizi.dev"
  version "0.14.1"
  license "Apache-2.0"

  base = "https://github.com/mzizi-dev/mzizi-cli/releases/download/v#{version}"

  on_macos do
    on_arm do
      url "#{base}/mz-aarch64-apple-darwin"
      sha256 "86f2e411eee308eb3fb81e8a2d88e194438eb3328dd1fa16b72b766ef40b40fa"
    end
    on_intel do
      url "#{base}/mz-x86_64-apple-darwin"
      sha256 "67d6b452ae7bc303c247a1419fa05c101caa48bbad8fa47e66a280ca7d9fc52d"
    end
  end

  on_linux do
    on_arm do
      url "#{base}/mz-aarch64-unknown-linux-gnu"
      sha256 "5db9e1833c0f465cabee7ac5a81d43478fcf1f026051e8aa334d4deafc736c59"
    end
    on_intel do
      url "#{base}/mz-x86_64-unknown-linux-gnu"
      sha256 "4dbd40978c50c263f5f262ddfcaed351f57d670febe5b0b1fadc95e70efa70c4"
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
