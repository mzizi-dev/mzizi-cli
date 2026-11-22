"use strict";
// The platform matrix. One row per bundled binary, kept at vendor/<triple>/mz[.exe]
// inside this one package. `triple` is the Rust target and the suffix of the GitHub
// release asset.
const PLATFORMS = [
  { key: "darwin-arm64", triple: "aarch64-apple-darwin", os: "darwin", cpu: "arm64" },
  { key: "darwin-x64", triple: "x86_64-apple-darwin", os: "darwin", cpu: "x64" },
  {
    key: "linux-x64-gnu",
    triple: "x86_64-unknown-linux-gnu",
    os: "linux",
    cpu: "x64",
    libc: "glibc",
  },
  {
    key: "linux-x64-musl",
    triple: "x86_64-unknown-linux-musl",
    os: "linux",
    cpu: "x64",
    libc: "musl",
  },
  {
    key: "linux-arm64-gnu",
    triple: "aarch64-unknown-linux-gnu",
    os: "linux",
    cpu: "arm64",
    libc: "glibc",
  },
  {
    key: "linux-arm64-musl",
    triple: "aarch64-unknown-linux-musl",
    os: "linux",
    cpu: "arm64",
    libc: "musl",
  },
  { key: "win32-x64", triple: "x86_64-pc-windows-msvc", os: "win32", cpu: "x64" },
];

/** glibc or musl, from Node's own report (no child process). Injectable for tests. */
function detectLibc(report) {
  try {
    const r = report === undefined ? process.report && process.report.getReport() : report;
    const header = r && (typeof r === "string" ? JSON.parse(r) : r).header;
    if (header && header.glibcVersionRuntime) return "glibc";
    return "musl";
  } catch {
    return "glibc";
  }
}

/** The matrix row for a platform, or undefined when no binary is shipped for it. */
function findPlatform({ platform, arch, libc }) {
  return PLATFORMS.find(
    (p) => p.os === platform && p.cpu === arch && (p.os !== "linux" || p.libc === libc),
  );
}

const binFile = (platform, name = "mz") => (platform === "win32" ? `${name}.exe` : name);

module.exports = { PLATFORMS, detectLibc, findPlatform, binFile };
