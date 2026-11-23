#!/bin/sh
# Install the Mzizi CLI (`mz`) from a GitHub release, verifying its sha256.
#
#   curl -fsSL https://raw.githubusercontent.com/mzizi-dev/mzizi-cli/main/install.sh | sh
#
# Environment:
#   MZ_VERSION        a version such as 0.14.1 (default: the latest release)
#   MZ_INSTALL_DIR    where to put mz (default: $HOME/.local/bin). Never uses sudo.
#   MZ_DOWNLOAD_BASE  release host, for mirrors and tests. Assets are read from
#                        <base>/v<version>/ (needs MZ_VERSION). file:// works.
set -eu

REPO="mzizi-dev/mzizi-cli"

say() { printf '%s\n' "$*"; }
die() { printf 'install.sh: %s\n' "$*" >&2; exit 1; }

command -v curl >/dev/null 2>&1 || die "curl is required"

# What are we running on?
os=$(uname -s)
arch=$(uname -m)
case "$arch" in
  x86_64 | amd64) arch=x86_64 ;;
  arm64 | aarch64) arch=aarch64 ;;
  *) die "no prebuilt binary for architecture $arch; try: cargo install mzizi-cli" ;;
esac
case "$os" in
  Darwin) triple="$arch-apple-darwin" ;;
  Linux)
    libc=gnu
    if ls /lib/ld-musl-* /usr/lib/ld-musl-* >/dev/null 2>&1; then
      libc=musl
    elif command -v ldd >/dev/null 2>&1 && ldd --version 2>&1 | grep -qi musl; then
      libc=musl
    fi
    triple="$arch-unknown-linux-$libc"
    ;;
  *) die "no installer for $os; on Windows use: npm i -g @nyuchi/mzizi-cli, or cargo install mzizi-cli" ;;
esac
asset="mz-$triple"

# Where do the release files live?
if [ -n "${MZ_DOWNLOAD_BASE:-}" ]; then
  [ -n "${MZ_VERSION:-}" ] || die "MZ_DOWNLOAD_BASE needs MZ_VERSION"
  url="${MZ_DOWNLOAD_BASE%/}/v${MZ_VERSION#v}"
elif [ -n "${MZ_VERSION:-}" ]; then
  url="https://github.com/$REPO/releases/download/v${MZ_VERSION#v}"
else
  url="https://github.com/$REPO/releases/latest/download"
fi

dir="${MZ_INSTALL_DIR:-$HOME/.local/bin}"
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT INT TERM

say "Downloading $asset from $url"
curl -fsSL "$url/SHA256SUMS" -o "$work/SHA256SUMS" || die "could not fetch SHA256SUMS from $url"
curl -fsSL "$url/$asset" -o "$work/$asset" || die "could not fetch $asset from $url"

# Verify before anything is installed.
want=$(awk -v f="$asset" '$2 == f || $2 == "*" f { print $1; exit }' "$work/SHA256SUMS")
[ -n "$want" ] || die "SHA256SUMS has no entry for $asset"
if command -v sha256sum >/dev/null 2>&1; then
  got=$(sha256sum "$work/$asset" | awk '{ print $1 }')
elif command -v shasum >/dev/null 2>&1; then
  got=$(shasum -a 256 "$work/$asset" | awk '{ print $1 }')
else
  die "need sha256sum or shasum to verify the download"
fi
[ "$got" = "$want" ] || die "checksum mismatch for $asset: expected $want, got $got. Nothing was installed."
say "sha256 verified: $got"

mkdir -p "$dir" || die "cannot create $dir (set MZ_INSTALL_DIR to a directory you own)"
cp "$work/$asset" "$dir/mz.new" || die "cannot write to $dir (set MZ_INSTALL_DIR to a directory you own)"
chmod 755 "$dir/mz.new"
mv "$dir/mz.new" "$dir/mz"
say "Installed $dir/mz"
case ":$PATH:" in
  *":$dir:"*) ;;
  *) say "Add $dir to your PATH to run \`mz\`." ;;
esac
