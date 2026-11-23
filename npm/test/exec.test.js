"use strict";
const test = require("node:test");
const assert = require("node:assert/strict");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const { spawnSync } = require("node:child_process");
const { ensureExecutable, sha256 } = require("../lib/exec.js");
const { findPlatform, detectLibc } = require("../lib/platforms.js");

const unix = process.platform !== "win32";
const tmp = () => fs.mkdtempSync(path.join(os.tmpdir(), "mz-exec-"));
const SCRIPT = '#!/bin/sh\necho "ran $*"\n';

function pkgWith(triple, mode) {
  const root = tmp();
  const dir = path.join(root, "vendor", triple);
  fs.mkdirSync(dir, { recursive: true });
  const file = path.join(dir, "mz");
  fs.writeFileSync(file, SCRIPT);
  fs.chmodSync(file, mode);
  fs.writeFileSync(
    path.join(root, "vendor", "checksums.json"),
    JSON.stringify({ [triple]: sha256(file) }),
  );
  return { root, file };
}

test("a 0644 bundled binary is chmodded and runs", { skip: !unix }, () => {
  const { root, file } = pkgWith("t-1", 0o644);
  const out = ensureExecutable(file, { triple: "t-1", root });
  assert.equal(out, file);
  assert.ok(fs.statSync(file).mode & 0o100);
  assert.equal(spawnSync(out, ["x"], { encoding: "utf8" }).stdout, "ran x\n");
});

test(
  "when chmod is refused, a checksum-verified copy in a 0700 cache dir runs",
  { skip: !unix },
  () => {
    const { root, file } = pkgWith("t-2", 0o644);
    const cache = tmp();
    const denying = {
      ...fs,
      chmodSync: (p, m) =>
        p === file
          ? (() => {
              throw new Error("EPERM");
            })()
          : fs.chmodSync(p, m),
    };
    const out = ensureExecutable(file, {
      triple: "t-2",
      root,
      fs: denying,
      env: { XDG_CACHE_HOME: cache },
    });
    assert.notEqual(out, file);
    assert.ok(out.startsWith(cache));
    assert.equal(fs.statSync(path.dirname(out)).mode & 0o777, 0o700);
    assert.equal(spawnSync(out, ["y"], { encoding: "utf8" }).stdout, "ran y\n");
    // the second call reuses the verified copy
    assert.equal(
      ensureExecutable(file, { triple: "t-2", root, fs: denying, env: { XDG_CACHE_HOME: cache } }),
      out,
    );
  },
);

test(
  "a binary that does not match the bundled checksum is not copied or run",
  { skip: !unix },
  () => {
    const { root, file } = pkgWith("t-3", 0o644);
    fs.writeFileSync(file, "#!/bin/sh\necho tampered\n");
    fs.chmodSync(file, 0o644);
    const denying = {
      ...fs,
      chmodSync: () => {
        throw new Error("EPERM");
      },
    };
    assert.throws(
      () =>
        ensureExecutable(file, {
          triple: "t-3",
          root,
          fs: denying,
          env: { XDG_CACHE_HOME: tmp() },
        }),
      /not the bundled/,
    );
  },
);

test("no bundled checksums means no cache copy", { skip: !unix }, () => {
  const { root, file } = pkgWith("t-4", 0o644);
  fs.rmSync(path.join(root, "vendor", "checksums.json"));
  const denying = {
    ...fs,
    chmodSync: () => {
      throw new Error("EPERM");
    },
  };
  assert.throws(() => ensureExecutable(file, { triple: "t-4", root, fs: denying }), /no checksums/);
});

test(
  "the shim runs a staged package whose binary lost its exec bit (0.14.0)",
  { skip: !unix },
  async () => {
    const row = findPlatform({
      platform: process.platform,
      arch: process.arch,
      libc: process.platform === "linux" ? detectLibc() : undefined,
    });
    if (!row) return;
    const { stage } = await import("../../scripts/stage-npm.mjs");
    const d = tmp();
    fs.mkdirSync(path.join(d, "dist"));
    fs.writeFileSync(path.join(d, "dist", `mz-${row.triple}`), SCRIPT, { mode: 0o755 });
    const out = path.join(d, "pkg");
    stage({ version: "1.2.3", binDir: path.join(d, "dist"), out });
    const bin = path.join(out, "vendor", row.triple, "mz");
    fs.chmodSync(bin, 0o644);
    const env = { ...process.env };
    delete env.MZ_BIN;
    delete env.MZIZI_BIN;
    const r = spawnSync(process.execPath, [path.join(out, "bin", "mz.js"), "--version"], {
      env,
      encoding: "utf8",
    });
    assert.equal(r.status, 0, r.stderr);
    assert.equal(r.stdout, "ran --version\n");
  },
);
