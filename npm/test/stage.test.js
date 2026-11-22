"use strict";
const test = require("node:test");
const assert = require("node:assert/strict");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const crypto = require("node:crypto");

test("stage-npm stages platform packages with verified checksums and pins the launcher", async () => {
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
  assert.deepEqual(staged.sort(), ["darwin-arm64", "linux-x64-musl", "win32-x64"]);
  const mac = JSON.parse(fs.readFileSync(path.join(out, "darwin-arm64", "package.json"), "utf8"));
  assert.deepEqual(
    [mac.name, mac.version, mac.os, mac.cpu],
    ["@nyuchi/mzizi-cli-darwin-arm64", "1.2.3", ["darwin"], ["arm64"]],
  );
  const musl = JSON.parse(
    fs.readFileSync(path.join(out, "linux-x64-musl", "package.json"), "utf8"),
  );
  assert.deepEqual(musl.libc, ["musl"]);
  assert.ok(fs.existsSync(path.join(out, "win32-x64", "bin", "mz.exe")));
  assert.ok(fs.statSync(path.join(out, "darwin-arm64", "bin", "mz")).mode & 0o111);
  const l = JSON.parse(fs.readFileSync(path.join(out, "launcher", "package.json"), "utf8"));
  assert.equal(l.version, "1.2.3");
  assert.ok(Object.values(l.optionalDependencies).every((v) => v === "1.2.3"));
  assert.equal(Object.keys(l.optionalDependencies).length, 7);
  assert.ok(!fs.existsSync(path.join(out, "launcher", "test")));
  // a binary that does not match its checksum line must stop staging
  fs.writeFileSync(path.join(bins, "mz-aarch64-apple-darwin"), "tampered");
  assert.throws(
    () => stage({ version: "1.2.3", binDir: bins, out, checksums: path.join(bins, "SHA256SUMS") }),
    /does not match/,
  );
});
