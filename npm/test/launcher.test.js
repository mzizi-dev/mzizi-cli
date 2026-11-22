"use strict";
const test = require("node:test");
const assert = require("node:assert/strict");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const { spawnSync } = require("node:child_process");
const { PLATFORMS, findPlatform, detectLibc } = require("../lib/platforms.js");
const { resolveBinary } = require("../lib/resolve.js");

const SHIM = path.join(__dirname, "..", "bin", "mz.js");
const tmp = () => fs.mkdtempSync(path.join(os.tmpdir(), "mz-npm-"));

const MATRIX = [
  ["darwin", "arm64", undefined, "aarch64-apple-darwin"],
  ["darwin", "x64", undefined, "x86_64-apple-darwin"],
  ["linux", "x64", "glibc", "x86_64-unknown-linux-gnu"],
  ["linux", "x64", "musl", "x86_64-unknown-linux-musl"],
  ["linux", "arm64", "glibc", "aarch64-unknown-linux-gnu"],
  ["linux", "arm64", "musl", "aarch64-unknown-linux-musl"],
  ["win32", "x64", undefined, "x86_64-pc-windows-msvc"],
];

test("every platform maps to its own bundled target", () => {
  for (const [platform, arch, libc, triple] of MATRIX) {
    assert.equal(
      findPlatform({ platform, arch, libc })?.triple,
      triple,
      `${platform}/${arch}/${libc}`,
    );
  }
  assert.equal(new Set(PLATFORMS.map((p) => p.triple)).size, MATRIX.length);
});

test("unsupported platforms resolve to nothing, with a reason", () => {
  assert.equal(findPlatform({ platform: "freebsd", arch: "x64" }), undefined);
  assert.equal(findPlatform({ platform: "win32", arch: "arm64" }), undefined);
  const r = resolveBinary({ env: {}, platform: "freebsd", arch: "x64" });
  assert.equal(r.path, null);
  assert.match(r.reason, /no prebuilt mz binary for freebsd\/x64/);
});

test("libc detection reads the Node report", () => {
  assert.equal(detectLibc({ header: { glibcVersionRuntime: "2.39" } }), "glibc");
  assert.equal(detectLibc({ header: {} }), "musl");
});

test("resolution finds the bundled binary for each platform", () => {
  for (const [platform, arch, libc, triple] of MATRIX) {
    const root = tmp();
    const file = platform === "win32" ? "mz.exe" : "mz";
    fs.mkdirSync(path.join(root, "vendor", triple), { recursive: true });
    fs.writeFileSync(path.join(root, "vendor", triple, file), "");
    const r = resolveBinary({ env: {}, platform, arch, libc, root });
    assert.equal(r.path, path.join(root, "vendor", triple, file));
    assert.equal(r.source, "bundled");
  }
});

test("a missing bundled binary is a clear error naming the target", () => {
  const r = resolveBinary({ env: {}, platform: "darwin", arch: "arm64", root: tmp() });
  assert.equal(r.path, null);
  assert.match(r.reason, /aarch64-apple-darwin is missing/);
});

test("MZIZI_BIN is a deprecated alias of MZ_BIN", () => {
  const d = tmp();
  const bin = path.join(d, "my-mz");
  fs.writeFileSync(bin, "");
  assert.equal(resolveBinary({ env: { MZIZI_BIN: bin } }).path, bin);
});

test("MZ_BIN wins, and a missing MZ_BIN is an error, not a fallthrough", () => {
  const d = tmp();
  const bin = path.join(d, "my-mz");
  fs.writeFileSync(bin, "");
  assert.equal(resolveBinary({ env: { MZ_BIN: bin }, platform: "freebsd", arch: "x64" }).path, bin);
  const r = resolveBinary({ env: { MZ_BIN: path.join(d, "nope") } });
  assert.equal(r.path, null);
  assert.match(r.reason, /MZ_BIN is set to .*nope/);
});

// A fake `mz`: prints its argv as JSON and exits with the code in FAKE_EXIT.
function fakeBin() {
  const d = tmp();
  const f = path.join(d, process.platform === "win32" ? "mz.cmd" : "mz");
  if (process.platform === "win32")
    fs.writeFileSync(f, "@echo off\r\necho %*\r\nexit /b %FAKE_EXIT%\r\n");
  else
    fs.writeFileSync(f, '#!/bin/sh\nprintf "%s\\n" "$@"\nexit "${FAKE_EXIT:-0}"\n', {
      mode: 0o755,
    });
  return f;
}
const runShim = (args, env) =>
  spawnSync(process.execPath, [SHIM, ...args], {
    env: { ...process.env, ...env },
    encoding: "utf8",
  });

test("args and stdout pass through, exit 0", () => {
  const r = runShim(["add", "button", "--type=tsx", "a b"], { MZ_BIN: fakeBin() });
  assert.equal(r.status, 0);
  assert.deepEqual(r.stdout.trim().split("\n"), ["add", "button", "--type=tsx", "a b"]);
});

test("exit codes pass through exactly", { skip: process.platform === "win32" }, () => {
  for (const code of [1, 2, 3, 101]) {
    assert.equal(runShim(["x"], { MZ_BIN: fakeBin(), FAKE_EXIT: String(code) }).status, code);
  }
});

test("no binary available: a clear error, exit 1", () => {
  const r = runShim([], { MZ_BIN: path.join(tmp(), "nope") });
  assert.equal(r.status, 1);
  assert.match(r.stderr, /^mz: MZ_BIN is set to/);
  assert.match(r.stderr, /cargo install/);
});

test(
  "the shim tells the binary which old name was typed",
  { skip: process.platform === "win32" },
  () => {
    const d = tmp();
    const link = path.join(d, "fundi");
    fs.symlinkSync(SHIM, link);
    const bin = path.join(d, "show");
    fs.writeFileSync(bin, '#!/bin/sh\nprintf "%s" "$MZ_INVOKED_AS"\n', { mode: 0o755 });
    const r = spawnSync(process.execPath, [link], {
      env: { ...process.env, MZ_BIN: bin },
      encoding: "utf8",
    });
    assert.equal(r.stdout, "fundi");
  },
);

// npx-style: the real Rust binary, when `cargo build` has produced one.
const REAL = path.join(
  __dirname,
  "..",
  "..",
  "target",
  "debug",
  process.platform === "win32" ? "mz.exe" : "mz",
);
test(
  "the shim runs the real Rust binary: --version",
  { skip: !fs.existsSync(REAL) && `skipped: build it first (cargo build): ${REAL} is absent` },
  () => {
    const want = /"version": "([^"]+)"/.exec(
      fs.readFileSync(path.join(__dirname, "..", "package.json"), "utf8"),
    )[1];
    const r = runShim(["--version"], { MZ_BIN: REAL });
    assert.equal(r.status, 0);
    assert.equal(r.stdout.trim(), want);
  },
);
