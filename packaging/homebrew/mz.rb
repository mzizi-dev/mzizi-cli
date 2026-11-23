# Homebrew formula for the Mzizi CLI, with the sha256 values of the v0.15.0 release (from its
# SHA256SUMS). The published copy is mzizi-dev/homebrew-tap, Formula/mz.rb; this one is its source
# of truth in this repo. Each release changes `version` and the four sha256 values (copy them from
# that release's SHA256SUMS). `brew audit --strict` must pass before it ships.
class Mz < Formula
  desc "The Mzizi CLI: add registry components, check the Bundu floor, plan with fundi"
  homepage "https://mzizi.dev"
  version "0.15.0"
  license "Apache-2.0"

  base = "https://github.com/mzizi-dev/mzizi-cli/releases/download/v#{version}"

  on_macos do
    on_arm do
      url "#{base}/mz-aarch64-apple-darwin"
      sha256 "482a9a288db35f37b520c454b82e1c68394af3238982627453f81cf2a775b38a"
    end
    on_intel do
      url "#{base}/mz-x86_64-apple-darwin"
      sha256 "16c1adf19294bb200ab10b21aa8856a0a003dd9f6010fbcc6b6974d706beb499"
    end
  end

  on_linux do
    on_arm do
      url "#{base}/mz-aarch64-unknown-linux-gnu"
      sha256 "f37cc2bbb42966e99b6f267ece51e4680e7cb796e3971779d8faa460ed19cc3e"
    end
    on_intel do
      url "#{base}/mz-x86_64-unknown-linux-gnu"
      sha256 "7453dd989f2c49556a3282767df601b4a685ded70129eee749d5039668a79764"
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
