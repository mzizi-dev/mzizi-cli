#!/bin/sh
# Tests install.sh against a local fake release (file:// base). No network.
set -eu
here=$(cd "$(dirname "$0")/.." && pwd)
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
fail() { printf 'FAIL: %s\n' "$*" >&2; exit 1; }

# The triple install.sh will pick on this machine.
os=$(uname -s); arch=$(uname -m)
case "$arch" in arm64 | aarch64) arch=aarch64 ;; *) arch=x86_64 ;; esac
case "$os" in
  Darwin) triple="$arch-apple-darwin" ;;
  Linux) if ls /lib/ld-musl-* >/dev/null 2>&1; then triple="$arch-unknown-linux-musl"; else triple="$arch-unknown-linux-gnu"; fi ;;
  *) echo "skipped: install.sh supports macOS and Linux only"; exit 0 ;;
esac
asset="mz-$triple"
rel="$work/rel/v9.9.9"; mkdir -p "$rel"
printf '#!/bin/sh\necho fake-mz\n' > "$rel/$asset"
if command -v sha256sum >/dev/null 2>&1; then sum=$(sha256sum "$rel/$asset" | cut -d' ' -f1); else sum=$(shasum -a 256 "$rel/$asset" | cut -d' ' -f1); fi
printf '%s  %s\n' "$sum" "$asset" > "$rel/SHA256SUMS"
export MZIZI_DOWNLOAD_BASE="file://$work/rel" MZIZI_VERSION=9.9.9

# 1. good download installs, is executable, and runs
MZIZI_INSTALL_DIR="$work/bin" sh "$here/install.sh" >/dev/null || fail "good install failed"
[ "$("$work/bin/mz")" = "fake-mz" ] || fail "installed binary does not run"

# 2. checksum mismatch refuses and installs nothing
printf '%s  %s\n' "$(printf 0 | cut -c1 | sed 's/0/0000000000000000000000000000000000000000000000000000000000000000/')" "$asset" > "$rel/SHA256SUMS"
if MZIZI_INSTALL_DIR="$work/bin2" sh "$here/install.sh" >"$work/out" 2>&1; then fail "mismatch was accepted"; fi
grep -q "checksum mismatch" "$work/out" || fail "no mismatch message"
[ ! -e "$work/bin2/mz" ] || fail "binary installed despite mismatch"

# 3. missing SHA256SUMS entry refuses
printf '%s  other\n' "$sum" > "$rel/SHA256SUMS"
if MZIZI_INSTALL_DIR="$work/bin3" sh "$here/install.sh" >/dev/null 2>&1; then fail "missing entry accepted"; fi

# 4. a base without a version is refused
if env -u MZIZI_VERSION MZIZI_INSTALL_DIR="$work/bin4" sh "$here/install.sh" >/dev/null 2>&1; then fail "base without version accepted"; fi
echo "install.sh: 4 checks passed"
