'use strict'
// Fallback: fetch the release binary from GitHub, verify its sha256 against the
// release's SHA256SUMS, and keep it in vendor/. Used by postinstall and on first run.
// Never required: the per-platform optional dependency is the normal route.
const crypto = require('node:crypto')
const fs = require('node:fs')
const path = require('node:path')
const https = require('node:https')
const { binFile } = require('./platforms.js')
const { vendorDir } = require('./resolve.js')

const DEFAULT_BASE = 'https://github.com/mzizi-dev/mzizi-cli/releases/download'

function fetchBytes(url, redirects = 5) {
  return new Promise((resolve, reject) => {
    if (url.startsWith('file://')) {
      fs.readFile(new URL(url), (e, b) => (e ? reject(e) : resolve(b)))
      return
    }
    https
      .get(url, { headers: { 'user-agent': 'mzizi-cli-npm' } }, (res) => {
        if ([301, 302, 303, 307, 308].includes(res.statusCode) && res.headers.location) {
          res.resume()
          if (redirects === 0) return reject(new Error('too many redirects'))
          return resolve(fetchBytes(new URL(res.headers.location, url).toString(), redirects - 1))
        }
        if (res.statusCode !== 200) {
          res.resume()
          return reject(new Error(`GET ${url} -> HTTP ${res.statusCode}`))
        }
        const chunks = []
        res.on('data', (c) => chunks.push(c))
        res.on('end', () => resolve(Buffer.concat(chunks)))
        res.on('error', reject)
      })
      .on('error', reject)
  })
}

/** Parse `<hex>  <file>` lines (sha256sum format). */
function parseSums(text) {
  const map = new Map()
  for (const line of text.split('\n')) {
    const m = /^([0-9a-f]{64})\s+\*?(\S+)\s*$/i.exec(line.trim())
    if (m) map.set(m[2], m[1].toLowerCase())
  }
  return map
}

/**
 * Download and verify. Throws on any failure, including a checksum mismatch (the file
 * is not written in that case). Returns the installed path.
 */
async function downloadBinary({ version, row, platform, env = process.env, root, fetch = fetchBytes }) {
  const base = (env.MZIZI_DOWNLOAD_BASE || DEFAULT_BASE).replace(/\/$/, '')
  const dir = `${base}/v${version}`
  const asset = `mz-${row.triple}${platform === 'win32' ? '.exe' : ''}`
  const sums = parseSums((await fetch(`${dir}/SHA256SUMS`)).toString('utf8'))
  const want = sums.get(asset)
  if (!want) throw new Error(`SHA256SUMS for v${version} has no entry for ${asset}`)
  const bytes = await fetch(`${dir}/${asset}`)
  const got = crypto.createHash('sha256').update(bytes).digest('hex')
  if (got !== want) throw new Error(`checksum mismatch for ${asset}: expected ${want}, got ${got}`)
  const out = path.join(vendorDir(root), binFile(platform))
  fs.mkdirSync(path.dirname(out), { recursive: true })
  const tmp = `${out}.${process.pid}.tmp`
  fs.writeFileSync(tmp, bytes, { mode: 0o755 })
  fs.renameSync(tmp, out)
  return out
}

module.exports = { downloadBinary, parseSums, fetchBytes }
