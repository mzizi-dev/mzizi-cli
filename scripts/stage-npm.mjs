#!/usr/bin/env node
// Stage the ONE npm package, with every platform binary bundled. Publishes nothing.
//
//   node scripts/stage-npm.mjs --version 0.14.2 --bin-dir dist --out stage [--checksums dist/SHA256SUMS]
//
// <bin-dir> holds release assets named mz-<triple>[.exe]. Each present asset is copied to
// <out>/vendor/<triple>/mz[.exe]; the launcher (npm/) is copied beside it and its version set.
// With --checksums, every binary must match its SHA256SUMS line or staging fails. With
// --require-all, a missing platform fails too.
import { createHash } from "node:crypto";
import {
  chmodSync,
  copyFileSync,
  cpSync,
  existsSync,
  mkdirSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { createRequire } from "node:module";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const require = createRequire(import.meta.url);
const here = dirname(fileURLToPath(import.meta.url));
const { PLATFORMS, binFile } = require("../npm/lib/platforms.js");

export function stage({ version, binDir, out, checksums, requireAll = false }) {
  if (!/^\d+\.\d+\.\d+(-[0-9A-Za-z.-]+)?$/.test(version ?? ""))
    throw new Error(`bad --version ${version}`);
  const sums = new Map();
  if (checksums) {
    for (const line of readFileSync(checksums, "utf8").split("\n")) {
      const m = /^([0-9a-f]{64})\s+\*?(\S+)\s*$/i.exec(line.trim());
      if (m) sums.set(m[2], m[1].toLowerCase());
    }
  }
  rmSync(out, { recursive: true, force: true });
  cpSync(resolve(here, "../npm"), out, {
    recursive: true,
    filter: (s) => !/[\\/](test|node_modules|vendor)$/.test(s),
  });
  const staged = [];
  const missing = [];
  const shas = {};
  for (const row of PLATFORMS) {
    const asset = `mz-${row.triple}${row.os === "win32" ? ".exe" : ""}`;
    const src = join(binDir, asset);
    if (!existsSync(src)) {
      missing.push(row.triple);
      continue;
    }
    const sha = createHash("sha256").update(readFileSync(src)).digest("hex");
    if (checksums && sums.get(asset) !== sha)
      throw new Error(`${asset}: sha256 ${sha} does not match ${checksums}`);
    const dest = join(out, "vendor", row.triple, binFile(row.os));
    mkdirSync(dirname(dest), { recursive: true });
    copyFileSync(src, dest);
    chmodSync(dest, 0o755);
    shas[row.triple] = sha;
    staged.push(row.triple);
  }
  // The launcher checks a cache copy against these when it cannot chmod the bundled binary.
  mkdirSync(join(out, "vendor"), { recursive: true });
  writeFileSync(join(out, "vendor", "checksums.json"), `${JSON.stringify(shas, null, 2)}\n`);
  if (requireAll && missing.length) throw new Error(`missing binaries for: ${missing.join(", ")}`);
  const pj = join(out, "package.json");
  const p = JSON.parse(readFileSync(pj, "utf8"));
  p.version = version;
  delete p.scripts.test;
  writeFileSync(pj, `${JSON.stringify(p, null, 2)}\n`);
  return staged;
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const a = process.argv.slice(2);
  const get = (k) => (a.includes(k) ? a[a.indexOf(k) + 1] : undefined);
  try {
    const staged = stage({
      version: get("--version"),
      binDir: get("--bin-dir"),
      out: get("--out"),
      checksums: get("--checksums"),
      requireAll: a.includes("--require-all"),
    });
    process.stdout.write(
      `staged one package with ${staged.length} binaries: ${staged.join(", ")}\n`,
    );
  } catch (e) {
    process.stderr.write(`stage-npm: ${e.message}\n`);
    process.exit(1);
  }
}
