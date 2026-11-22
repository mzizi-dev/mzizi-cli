'use strict'
const fs = require('node:fs')
const path = require('node:path')
const { detectLibc, findPlatform, binFile } = require('./platforms.js')

const PKG_ROOT = path.resolve(__dirname, '..')

/** Where a first-run/postinstall download is kept: inside the launcher package. */
const vendorDir = (root = PKG_ROOT) => path.join(root, 'vendor')

/**
 * Find the Rust binary. Order: MZIZI_BIN, the per-platform optional dependency,
 * a previously downloaded copy. Returns { path, source } or { path: null, reason }.
 * Every input is injectable so the matrix is testable without the real platform.
 */
function resolveBinary(opts = {}) {
  const env = opts.env || process.env
  const platform = opts.platform || process.platform
  const arch = opts.arch || process.arch
  const libc = opts.libc || (platform === 'linux' ? detectLibc(opts.report) : undefined)
  const exists = opts.existsSync || fs.existsSync
  const resolvePkg = opts.resolvePackage || ((id) => require.resolve(id, { paths: [PKG_ROOT] }))
  const root = opts.root || PKG_ROOT

  if (env.MZIZI_BIN) {
    if (!exists(env.MZIZI_BIN)) {
      return { path: null, reason: `MZIZI_BIN is set to ${env.MZIZI_BIN}, which does not exist` }
    }
    return { path: env.MZIZI_BIN, source: 'MZIZI_BIN' }
  }

  const row = findPlatform({ platform, arch, libc })
  if (!row) {
    return {
      path: null,
      reason: `no prebuilt mz binary for ${platform}/${arch}${libc ? `/${libc}` : ''}`,
    }
  }
  try {
    const pkgJson = resolvePkg(`${row.pkg}/package.json`)
    const bin = path.join(path.dirname(pkgJson), 'bin', binFile(platform))
    if (exists(bin)) return { path: bin, source: row.pkg, row }
  } catch {
    // optional dependency not installed (npm --omit=optional, or an unsupported mirror)
  }
  const vendored = path.join(vendorDir(root), binFile(platform))
  if (exists(vendored)) return { path: vendored, source: 'download', row }
  return { path: null, reason: `${row.pkg} is not installed and nothing was downloaded`, row }
}

module.exports = { resolveBinary, vendorDir, PKG_ROOT }
