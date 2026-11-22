"use strict";
const test = require("node:test");
const assert = require("node:assert/strict");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const crypto = require("node:crypto");

test("stage-npm bundles every binary into one package, checksum-verified", async () => {
  const { stage } = await import("../../scripts/stage-npm.mjs");
  const d = fs.mkdtempSync(path.join(os.tmpdir(), "mz-stage-"));
  const bins = path.join(d, "dist");
  fs.mkdirSync(bins);
  const assets = [
    "mz-aarch64-apple-darwin",
    "mz-x86_64-unknown-linux-musl",
    "mz-x86_64-pc-windows-msvc.exe",
  ];
  const lines = [];
  for (const a of assets) {
    const b = Buffer.from(`binary ${a}`);
    fs.writeFileSync(path.join(bins, a), b);
    lines.push(`${crypto.createHash("sha256").update(b).digest("hex")}  ${a}`);
  }
  fs.writeFileSync(path.join(bins, "SHA256SUMS"), `${lines.join("\n")}\n`);
  const out = path.join(d, "stage");
  const staged = stage({
    version: "1.2.3",
    binDir: bins,
    out,
    checksums: path.join(bins, "SHA256SUMS"),
  });
  assert.deepEqual(staged.sort(), [
    "aarch64-apple-darwin",
    "x86_64-pc-windows-msvc",
    "x86_64-unknown-linux-musl",
  ]);
  assert.ok(fs.existsSync(path.join(out, "vendor", "x86_64-pc-windows-msvc", "mz.exe")));
  assert.ok(fs.statSync(path.join(out, "vendor", "aarch64-apple-darwin", "mz")).mode & 0o111);
  const p = JSON.parse(fs.readFileSync(path.join(out, "package.json"), "utf8"));
  assert.equal(p.name, "@nyuchi/mzizi-cli");
  assert.equal(p.version, "1.2.3");
  assert.equal(p.optionalDependencies, undefined);
  assert.equal(p.scripts.postinstall, undefined);
  assert.ok(!fs.existsSync(path.join(out, "test")));
  // --require-all refuses a partial set
  assert.throws(
    () => stage({ version: "1.2.3", binDir: bins, out, requireAll: true }),
    /missing binaries/,
  );
  // a binary that does not match its checksum line must stop staging
  fs.writeFileSync(path.join(bins, "mz-aarch64-apple-darwin"), "tampered");
  assert.throws(
    () => stage({ version: "1.2.3", binDir: bins, out, checksums: path.join(bins, "SHA256SUMS") }),
    /does not match/,
  );
});
