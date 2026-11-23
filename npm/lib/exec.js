"use strict";
// Make the bundled binary runnable. A package manager (or an artifact upload before publish) can
// leave files in node_modules without the execute bit: 0.14.0 shipped every vendored binary as
// 0644 and `mz` failed with EACCES. The launcher repairs that at run time:
//   1. already executable: use it;
//   2. else chmod it 0755 (it is the user's own install);
//   3. else (a read-only package directory, a store owned by someone else) copy it to a per-user
//      cache directory (0700), checking its sha256 against the checksums bundled in the package,
//      and run the copy.
const crypto = require("node:crypto");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");

const sha256 = (file, fsx = fs) =>
  crypto.createHash("sha256").update(fsx.readFileSync(file)).digest("hex");

function cacheRoot(env = process.env, home = os.homedir()) {
  const base = (env.XDG_CACHE_HOME || "").trim() || path.join(home, ".cache");
  return path.join(base, "mz", "bin");
}

/** The sha256 the package says this target's binary has, or null when it ships no checksums. */
function bundledSha(root, triple) {
  try {
    const sums = JSON.parse(fs.readFileSync(path.join(root, "vendor", "checksums.json"), "utf8"));
    return typeof sums[triple] === "string" ? sums[triple].toLowerCase() : null;
  } catch {
    return null;
  }
}

/**
 * Return a path to a runnable copy of `file`. Options: platform, triple, root (the package root),
 * env, home, fs (injectable for tests). Throws an Error with a plain message when it cannot.
 */
function ensureExecutable(file, opts = {}) {
  const fsx = opts.fs || fs;
  if ((opts.platform || process.platform) === "win32") return file;
  const runnable = (f) => {
    try {
      fsx.accessSync(f, fs.constants.X_OK);
      return true;
    } catch {
      return false;
    }
  };
  if (runnable(file)) return file;
  try {
    fsx.chmodSync(file, 0o755);
    if (runnable(file)) return file;
  } catch {
    // fall through to the cache copy
  }

  const expected = bundledSha(opts.root, opts.triple);
  if (!expected) {
    throw new Error(
      `${file} is not executable and cannot be made so, and this package has no checksums to verify a copy against`,
    );
  }
  const actual = sha256(file, fsx);
  if (actual !== expected) {
    throw new Error(`refusing to run ${file}: its sha256 ${actual} is not the bundled ${expected}`);
  }
  const dir = path.join(cacheRoot(opts.env, opts.home), actual.slice(0, 16));
  const dest = path.join(dir, "mz");
  const good = () => runnable(dest) && sha256(dest, fsx) === actual;
  if (good()) return dest;
  fsx.mkdirSync(dir, { recursive: true, mode: 0o700 });
  fsx.chmodSync(dir, 0o700);
  const tmp = path.join(dir, `mz.${process.pid}.${crypto.randomBytes(4).toString("hex")}`);
  try {
    fsx.copyFileSync(file, tmp);
    fsx.chmodSync(tmp, 0o700);
    if (sha256(tmp, fsx) !== actual)
      throw new Error("the copy does not match the bundled checksum");
    fsx.renameSync(tmp, dest);
  } catch (e) {
    try {
      fsx.rmSync(tmp, { force: true });
    } catch {
      // nothing to clean
    }
    throw new Error(`could not make ${file} runnable (${e.message})`);
  }
  return dest;
}

module.exports = { ensureExecutable, cacheRoot, bundledSha, sha256 };
