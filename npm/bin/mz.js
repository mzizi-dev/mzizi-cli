#!/usr/bin/env node
"use strict";
// Thin launcher: finds the Rust `mz` binary and runs it. No CLI logic lives here.
// Args, stdio, exit code and fatal signals pass through exactly.
const { spawnSync } = require("node:child_process");
const { resolveBinary } = require("../lib/resolve.js");
const { findPlatform } = require("../lib/platforms.js");

function fail(message) {
  process.stderr.write(
    `mz: ${message}\n` +
      "Install the binary another way: cargo install mzizi-cli, the install.sh from\n" +
      "https://github.com/mzizi-dev/mzizi-cli, or point MZ_BIN at an mz binary.\n",
  );
  process.exit(1);
}

function main() {
  const found = resolveBinary();
  if (!found.path) return fail(found.reason);
  const typed = require("node:path")
    .basename(process.argv[1] || "mz")
    .replace(/\.(c?js|cmd|exe)$/i, "");
  const invoked = typed === "mzizi" || typed === "fundi" ? typed : "mz";
  const r = spawnSync(found.path, process.argv.slice(2), {
    stdio: "inherit",
    windowsHide: false,
    env: { ...process.env, MZ_INVOKED_AS: invoked },
  });
  if (r.error) return fail(`could not run ${found.path}: ${r.error.message}`);
  if (r.signal) {
    process.kill(process.pid, r.signal);
    return;
  }
  process.exit(r.status === null ? 1 : r.status);
}

if (require.main === module) main();
module.exports = { main, findPlatform };
