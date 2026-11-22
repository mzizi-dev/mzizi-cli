#!/usr/bin/env node
// Stage the npm packages from built binaries. Publishes nothing.
//
//   node scripts/stage-npm.mjs --version 0.14.0 --bin-dir dist --out stage [--checksums dist/SHA256SUMS]
//
// <bin-dir> holds release assets named mz-<triple>[.exe]. For each platform whose asset is
// present this writes <out>/<key>/{package.json,bin/mz[.exe],README.md}, and the launcher
// to <out>/launcher with every optionalDependency pinned to --version. With --checksums,
// every binary must match its SHA256SUMS line or staging fails.
import { createHash } from 'node:crypto'
import { chmodSync, copyFileSync, cpSync, existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs'
import { createRequire } from 'node:module'
import { dirname, join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const require = createRequire(import.meta.url)
const here = dirname(fileURLToPath(import.meta.url))
const { PLATFORMS, binFile } = require('../npm/lib/platforms.js')

export function stage({ version, binDir, out, checksums }) {
  if (!/^\d+\.\d+\.\d+(-[0-9A-Za-z.-]+)?$/.test(version ?? '')) throw new Error(`bad --version ${version}`)
  const sums = new Map()
  if (checksums) {
    for (const line of readFileSync(checksums, 'utf8').split('\n')) {
      const m = /^([0-9a-f]{64})\s+\*?(\S+)\s*$/i.exec(line.trim())
      if (m) sums.set(m[2], m[1].toLowerCase())
    }
  }
  rmSync(out, { recursive: true, force: true })
  const staged = []
  for (const row of PLATFORMS) {
    const asset = `mz-${row.triple}${row.os === 'win32' ? '.exe' : ''}`
    const src = join(binDir, asset)
    if (!existsSync(src)) continue
    const sha = createHash('sha256').update(readFileSync(src)).digest('hex')
    if (checksums && sums.get(asset) !== sha) throw new Error(`${asset}: sha256 ${sha} does not match ${checksums}`)
    const dir = join(out, row.key)
    mkdirSync(join(dir, 'bin'), { recursive: true })
    const dest = join(dir, 'bin', binFile(row.os))
    copyFileSync(src, dest)
    chmodSync(dest, 0o755)
    const pkg = {
      name: row.pkg,
      version,
      description: `The ${row.key} binary of @nyuchi/mzizi-cli (the Mzizi CLI, \`mz\`).`,
      license: 'Apache-2.0',
      repository: { type: 'git', url: 'git+https://github.com/mzizi-dev/mzizi-cli.git' },
      os: [row.os],
      cpu: [row.cpu],
      ...(row.libc ? { libc: [row.libc === 'glibc' ? 'glibc' : 'musl'] } : {}),
      files: ['bin'],
      publishConfig: { access: 'public' },
      mzizi: { triple: row.triple, asset, sha256: sha },
    }
    writeFileSync(join(dir, 'package.json'), `${JSON.stringify(pkg, null, 2)}\n`)
    writeFileSync(join(dir, 'README.md'), `# ${row.pkg}\n\nPlatform binary for [@nyuchi/mzizi-cli](https://www.npmjs.com/package/@nyuchi/mzizi-cli). Install that package, not this one.\n`)
    staged.push(row.key)
  }
  const launcher = join(out, 'launcher')
  cpSync(resolve(here, '../npm'), launcher, { recursive: true, filter: (s) => !/[\\/](test|node_modules|vendor)$/.test(s) })
  const lp = JSON.parse(readFileSync(join(launcher, 'package.json'), 'utf8'))
  lp.version = version
  lp.optionalDependencies = Object.fromEntries(PLATFORMS.map((r) => [r.pkg, version]))
  delete lp.scripts.test
  writeFileSync(join(launcher, 'package.json'), `${JSON.stringify(lp, null, 2)}\n`)
  return staged
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const a = process.argv.slice(2)
  const get = (k) => (a.includes(k) ? a[a.indexOf(k) + 1] : undefined)
  try {
    const staged = stage({ version: get('--version'), binDir: get('--bin-dir'), out: get('--out'), checksums: get('--checksums') })
    process.stdout.write(`staged launcher + ${staged.length} platform package(s): ${staged.join(', ')}\n`)
  } catch (e) {
    process.stderr.write(`stage-npm: ${e.message}\n`)
    process.exit(1)
  }
}
