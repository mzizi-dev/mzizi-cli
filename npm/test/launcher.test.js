'use strict'
const test = require('node:test')
const assert = require('node:assert/strict')
const fs = require('node:fs')
const os = require('node:os')
const path = require('node:path')
const crypto = require('node:crypto')
const { spawnSync } = require('node:child_process')
const { PLATFORMS, findPlatform, detectLibc } = require('../lib/platforms.js')
const { resolveBinary } = require('../lib/resolve.js')
const { downloadBinary, parseSums } = require('../lib/download.js')

const SHIM = path.join(__dirname, '..', 'bin', 'mz.js')
const tmp = () => fs.mkdtempSync(path.join(os.tmpdir(), 'mz-npm-'))

const MATRIX = [
  ['darwin', 'arm64', undefined, '@nyuchi/mzizi-cli-darwin-arm64'],
  ['darwin', 'x64', undefined, '@nyuchi/mzizi-cli-darwin-x64'],
  ['linux', 'x64', 'glibc', '@nyuchi/mzizi-cli-linux-x64-gnu'],
  ['linux', 'x64', 'musl', '@nyuchi/mzizi-cli-linux-x64-musl'],
  ['linux', 'arm64', 'glibc', '@nyuchi/mzizi-cli-linux-arm64-gnu'],
  ['linux', 'arm64', 'musl', '@nyuchi/mzizi-cli-linux-arm64-musl'],
  ['win32', 'x64', undefined, '@nyuchi/mzizi-cli-win32-x64'],
]

test('every platform maps to its own package', () => {
  for (const [platform, arch, libc, pkg] of MATRIX) {
    assert.equal(findPlatform({ platform, arch, libc })?.pkg, pkg, `${platform}/${arch}/${libc}`)
  }
  assert.equal(new Set(PLATFORMS.map((p) => p.pkg)).size, MATRIX.length)
})

test('unsupported platforms resolve to nothing, with a reason', () => {
  assert.equal(findPlatform({ platform: 'freebsd', arch: 'x64' }), undefined)
  assert.equal(findPlatform({ platform: 'win32', arch: 'arm64' }), undefined)
  const r = resolveBinary({ env: {}, platform: 'freebsd', arch: 'x64' })
  assert.equal(r.path, null)
  assert.match(r.reason, /no prebuilt mz binary for freebsd\/x64/)
})

test('libc detection reads the Node report', () => {
  assert.equal(detectLibc({ header: { glibcVersionRuntime: '2.39' } }), 'glibc')
  assert.equal(detectLibc({ header: {} }), 'musl')
})

test('resolution finds the binary inside the platform package', () => {
  for (const [platform, arch, libc, pkg] of MATRIX) {
    const root = tmp()
    const file = platform === 'win32' ? 'mz.exe' : 'mz'
    fs.mkdirSync(path.join(root, 'p', 'bin'), { recursive: true })
    fs.writeFileSync(path.join(root, 'p', 'package.json'), '{}')
    fs.writeFileSync(path.join(root, 'p', 'bin', file), '')
    const r = resolveBinary({
      env: {}, platform, arch, libc,
      resolvePackage: (id) => {
        assert.equal(id, `${pkg}/package.json`)
        return path.join(root, 'p', 'package.json')
      },
    })
    assert.equal(r.path, path.join(root, 'p', 'bin', file))
    assert.equal(r.source, pkg)
  }
})

test('MZ_BIN wins, and a missing MZ_BIN is an error, not a fallthrough', () => {
  const d = tmp()
  const bin = path.join(d, 'my-mz')
  fs.writeFileSync(bin, '')
  assert.equal(resolveBinary({ env: { MZ_BIN: bin }, platform: 'freebsd', arch: 'x64' }).path, bin)
  const r = resolveBinary({ env: { MZ_BIN: path.join(d, 'nope') } })
  assert.equal(r.path, null)
  assert.match(r.reason, /MZ_BIN is set to .*nope/)
})

test('a vendored download is used when the package is absent', () => {
  const root = tmp()
  fs.mkdirSync(path.join(root, 'vendor'))
  fs.writeFileSync(path.join(root, 'vendor', 'mz'), '')
  const r = resolveBinary({ env: {}, platform: 'darwin', arch: 'arm64', root, resolvePackage: () => { throw new Error('absent') } })
  assert.equal(r.source, 'download')
})

// A fake `mz`: prints its argv as JSON and exits with the code in FAKE_EXIT.
function fakeBin() {
  const d = tmp()
  const f = path.join(d, process.platform === 'win32' ? 'mz.cmd' : 'mz')
  if (process.platform === 'win32') fs.writeFileSync(f, '@echo off\r\necho %*\r\nexit /b %FAKE_EXIT%\r\n')
  else fs.writeFileSync(f, '#!/bin/sh\nprintf "%s\\n" "$@"\nexit "${FAKE_EXIT:-0}"\n', { mode: 0o755 })
  return f
}
const runShim = (args, env) =>
  spawnSync(process.execPath, [SHIM, ...args], { env: { ...process.env, ...env }, encoding: 'utf8' })

test('args and stdout pass through, exit 0', () => {
  const r = runShim(['add', 'button', '--type=tsx', 'a b'], { MZ_BIN: fakeBin() })
  assert.equal(r.status, 0)
  assert.deepEqual(r.stdout.trim().split('\n'), ['add', 'button', '--type=tsx', 'a b'])
})

test('exit codes pass through exactly', { skip: process.platform === 'win32' }, () => {
  for (const code of [1, 2, 3, 101]) {
    assert.equal(runShim(['x'], { MZ_BIN: fakeBin(), FAKE_EXIT: String(code) }).status, code)
  }
})

test('no binary and no download: a clear error, exit 1', () => {
  const r = runShim([], { MZ_BIN: '', MZ_SKIP_DOWNLOAD: '1', MZ_DOWNLOAD_BASE: 'file:///nonexistent' })
  if (r.status === 0) return // a platform package happens to be installed here
  assert.equal(r.status, 1)
  assert.match(r.stderr, /^mz: /)
  assert.match(r.stderr, /MZ_BIN|cargo install/)
})

test('download verifies the checksum and refuses a mismatch', async () => {
  const base = tmp()
  const row = findPlatform({ platform: 'linux', arch: 'x64', libc: 'glibc' })
  const asset = `mz-${row.triple}`
  const bytes = Buffer.from('#!/bin/sh\necho hi\n')
  const dir = path.join(base, 'v9.9.9')
  fs.mkdirSync(dir)
  fs.writeFileSync(path.join(dir, asset), bytes)
  const good = crypto.createHash('sha256').update(bytes).digest('hex')
  fs.writeFileSync(path.join(dir, 'SHA256SUMS'), `${good}  ${asset}\n`)
  const env = { MZ_DOWNLOAD_BASE: `file://${base}` }
  const root = tmp()
  const out = await downloadBinary({ version: '9.9.9', row, platform: 'linux', env, root })
  assert.equal(fs.readFileSync(out).toString(), bytes.toString())
  fs.writeFileSync(path.join(dir, 'SHA256SUMS'), `${'0'.repeat(64)}  ${asset}\n`)
  const root2 = tmp()
  await assert.rejects(downloadBinary({ version: '9.9.9', row, platform: 'linux', env, root: root2 }), /checksum mismatch/)
  assert.equal(fs.existsSync(path.join(root2, 'vendor', 'mz')), false)
  assert.equal(parseSums(`${good}  a\nnoise\n`).get('a'), good)
})

// npx-style: the real Rust binary, when `cargo build` has produced one.
const REAL = path.join(__dirname, '..', '..', 'target', 'debug', process.platform === 'win32' ? 'mz.exe' : 'mz')
test('the shim runs the real Rust binary: --version', { skip: !fs.existsSync(REAL) && `skipped: build it first (cargo build): ${REAL} is absent` }, () => {
  const want = /"version": "([^"]+)"/.exec(fs.readFileSync(path.join(__dirname, '..', 'package.json'), 'utf8'))[1]
  const r = runShim(['--version'], { MZ_BIN: REAL })
  assert.equal(r.status, 0)
  assert.equal(r.stdout.trim(), want)
})
