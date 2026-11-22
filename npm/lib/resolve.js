"use strict";
const fs = require("node:fs");
const path = require("node:path");
const { detectLibc, findPlatform, binFile } = require("./platforms.js");

const PKG_ROOT = path.resolve(__dirname, "..");

/**
 * Find the Rust binary: MZ_BIN, else the copy bundled in this package for the platform.
 * Returns { path, source } or { path: null, reason }. Every input is injectable so the
 * matrix is testable without the real platform.
 */
function resolveBinary(opts = {}) {
  let env = opts.env || process.env;
  const platform = opts.platform || process.platform;
  const arch = opts.arch || process.arch;
  const libc = opts.libc || (platform === "linux" ? detectLibc(opts.report) : undefined);
  const exists = opts.existsSync || fs.existsSync;
  const root = opts.root || PKG_ROOT;

  if (!env.MZ_BIN && env.MZIZI_BIN) {
    // Deprecated name, honoured for one release.
    process.stderr.write("warning: MZIZI_BIN is deprecated, use MZ_BIN\n");
    env = { ...env, MZ_BIN: env.MZIZI_BIN };
  }
  if (env.MZ_BIN) {
    if (!exists(env.MZ_BIN)) {
      return { path: null, reason: `MZ_BIN is set to ${env.MZ_BIN}, which does not exist` };
    }
    return { path: env.MZ_BIN, source: "MZ_BIN" };
  }

  const row = findPlatform({ platform, arch, libc });
  if (!row) {
    return {
      path: null,
      reason: `no prebuilt mz binary for ${platform}/${arch}${libc ? `/${libc}` : ""}`,
    };
  }
  const bundled = path.join(root, "vendor", row.triple, binFile(platform));
  if (exists(bundled)) return { path: bundled, source: "bundled", row };
  return {
    path: null,
    reason: `the bundled binary for ${row.triple} is missing from this install`,
    row,
  };
}

module.exports = { resolveBinary, PKG_ROOT };
