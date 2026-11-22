'use strict'
// Best effort, never fatal: if the platform package is missing (npm --omit=optional,
// an unsupported mirror), fetch the verified release binary now instead of at first run.
const { resolveBinary } = require('../lib/resolve.js')
const pkg = require('../package.json')

async function run() {
  if (process.env.MZ_SKIP_DOWNLOAD || process.env.MZ_BIN) return
  const found = resolveBinary()
  if (found.path || !found.row) return
  const { downloadBinary } = require('../lib/download.js')
  await downloadBinary({ version: pkg.version, row: found.row, platform: process.platform })
}

run().catch((e) => {
  process.stderr.write(`mzizi-cli: could not download the binary now (${e.message}); it will be tried again on first run.\n`)
})
