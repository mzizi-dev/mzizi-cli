#!/usr/bin/env bash
# Parity check for `add`: run the TypeScript CLI (@nyuchi/mzizi-cli 0.13.0) and the Rust binary
# against the SAME recorded registry fixtures, in identical fresh project directories, and diff
# stdout, stderr, the exit code and every file written.
#
# Usage:  TS_CLI=/path/to/agent-tools/mzizi-cli  scripts/parity-add.sh [path/to/mz]
#   TS_CLI  the TypeScript package directory, already built (`pnpm build` -> dist/cli.js).
#   $1      the Rust binary (default: target/debug/mz, built if missing).
#
# No live network: a local server (python3) serves tests/fixtures/recorded/. The TypeScript CLI
# reaches it through a preload that rewrites fetch(); the Rust binary through MZ_REGISTRY_BASE
# and MZ_CRATES_BASE. Two intended differences are normalised, and only these:
#   - the user-facing command name: "mzizi add" -> "mz add" (owner decision, the command is mz)
#   - nothing else. Any other diff is a failure.
set -euo pipefail
here="$(cd "$(dirname "$0")/.." && pwd)"
: "${TS_CLI:?set TS_CLI to the built TypeScript package directory}"
bin="${1:-$here/target/debug/mz}"
[ -x "$bin" ] || (cd "$here" && cargo build --quiet && true)
work="$(mktemp -d)"; trap 'kill $srv 2>/dev/null || true; rm -rf "$work"' EXIT

cat > "$work/server.py" <<'PY'
import http.server, os, sys, urllib.parse
root = sys.argv[1]
class H(http.server.BaseHTTPRequestHandler):
    def log_message(self, *a): pass
    def do_GET(self):
        p = urllib.parse.urlsplit(self.path).path
        f = None
        if p.startswith("/__crates/api/v1/crates/"):
            f = os.path.join(root, "crates", p.rsplit("/", 1)[1] + ".json")
        elif p.startswith("/v1/"):
            parts = p[len("/v1/"):].split("/")
            if len(parts) == 2 and all(x and "%" not in x and ".." not in x for x in parts):
                f = os.path.join(root, "v1", parts[0], parts[1] + ".json")
        if f and os.path.isfile(f):
            b = open(f, "rb").read(); self.send_response(200)
        else:
            b = b'{"error":"not found"}'; self.send_response(404)
        self.send_header("content-type", "application/json"); self.send_header("content-length", str(len(b)))
        self.end_headers(); self.wfile.write(b)
s = http.server.ThreadingHTTPServer(("127.0.0.1", 0), H)
print(s.server_address[1], flush=True); s.serve_forever()
PY
python3 "$work/server.py" "$here/tests/fixtures/recorded" > "$work/port" &
srv=$!
for _ in $(seq 50); do [ -s "$work/port" ] && break; sleep 0.1; done
base="http://127.0.0.1:$(cat "$work/port")"

cat > "$work/preload.mjs" <<JS
const base = "$base"
const real = globalThis.fetch
globalThis.fetch = (u, o) =>
  real(String(u).replace("https://api.mzizi.dev", base).replace("https://crates.io", base + "/__crates"), o)
JS

fail=0; n=0
# scenario <name> <setup-shell> -- <args...>
scenario() {
  local name="$1" setup="$2"; shift 3; n=$((n + 1))
  for who in ts rs; do
    d="$work/$name/$who"; mkdir -p "$d"; (cd "$d" && eval "$setup")
    set +e
    if [ $who = ts ]; then
      (cd "$d" && node --import "$work/preload.mjs" "$TS_CLI/dist/cli.js" "$@") >"$work/$name/$who.out" 2>"$work/$name/$who.err"
    else
      (cd "$d" && MZ_REGISTRY_BASE="$base" MZ_CRATES_BASE="$base/__crates" "$bin" "$@") >"$work/$name/$who.out" 2>"$work/$name/$who.err"
    fi
    echo $? >"$work/$name/$who.code"; set -e
    # The old bins print a deprecation note; `mz` does not. Normalise the command name only.
    sed -i.bak -e 's/^mzizi add /mz add /' -e 's/`mzizi add button`/`mz add button`/' "$work/$name/$who.out" "$work/$name/$who.err"
    sed -i.bak -e 's/^error: //' "$work/$name/$who.err"
  done
  ok=1
  for f in out err code; do diff -u "$work/$name/ts.$f" "$work/$name/rs.$f" >"$work/$name/$f.diff" || { ok=0; echo "  DIFF in $f ($name)"; head -20 "$work/$name/$f.diff"; }; done
  diff -r "$work/$name/ts" "$work/$name/rs" >"$work/$name/tree.diff" 2>&1 || true
  if [ -s "$work/$name/tree.diff" ]; then ok=0; echo "  DIFF in written files ($name)"; cat "$work/$name/tree.diff"; fi
  if [ $ok = 1 ]; then echo "ok   $name (exit $(cat "$work/$name/rs.code"), $(wc -c <"$work/$name/rs.out") B stdout, $(find "$work/$name/rs" -type f | wc -l | tr -d " ") files)"; else echo "FAIL $name"; fail=$((fail + 1)); fi
}
react='echo "{}" > components.json'
scenario button-dry-run      "$react" -- add button --dry-run
scenario button-write        "$react" -- add button
scenario button-rewrite-skip "$react; mkdir -p components/ui; echo MINE > components/ui/button.tsx" -- add button
scenario button-overwrite    "$react; mkdir -p components/ui; echo MINE > components/ui/button.tsx" -- add button --overwrite
scenario address-input       "$react" -- add address-input
scenario button-mzizi-dry    'printf "[package]\nname=\"x\"\n" > Cargo.toml' -- add button --type rust --dry-run
scenario button-mzizi-write  'printf "[package]\nname=\"x\"\n" > Cargo.toml' -- add button --type=mzizi
scenario button-astro        'echo "export default {}" > astro.config.mjs; echo "{}" > package.json' -- add button
scenario button-ambiguous    ':' -- add button
scenario button-bad-type     ':' -- add button --type dioxus
scenario button-lacks-type   ':' -- add button --type kotlin
scenario no-names            ':' -- add --dry-run
scenario swift-only          ':' -- add mzizi-tokens-swift
scenario missing-component   ':' -- add no-such-thing
echo "$n scenarios, $fail failed"
[ "$fail" = 0 ]
