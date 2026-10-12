// Acceptance tests 14 to 19 of docs/spec/delivery.md section 6 for the distribution check.
import assert from 'node:assert/strict'
import { spawnSync } from 'node:child_process'
import { mkdir, mkdtemp, readFile, rm, symlink, truncate, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { dirname, join } from 'node:path'
import { test } from 'node:test'
import { fileURLToPath } from 'node:url'

import { EXPECTED_HEADERS, PAGES_FILE_LIMIT, checkDist, oversizedFiles } from './check-dist.mjs'

const scriptPath = fileURLToPath(new URL('check-dist.mjs', import.meta.url))
const publicHeadersPath = fileURLToPath(new URL('../public/_headers', import.meta.url))

// The cross-origin isolation headers exactly as section 2.7 gives them.
const isolationHeaders =
  '/*\n  Cross-Origin-Opener-Policy: same-origin\n  Cross-Origin-Embedder-Policy: require-corp\n'

const usageLine = 'usage: node scripts/check-dist.mjs [directory]'

/** The section 4 message for a file over the Cloudflare Pages limit. */
function oversizedMessage(directory, path, size) {
  return `${directory}/${path} is ${size} bytes, over the Cloudflare Pages limit of 26214400 bytes (25 MiB). Split the core into lazily loaded modules or serve the file from R2 (docs/spec/delivery.md section 3.5).`
}

/** The section 4 message for a missing `_headers` file. */
function missingHeadersMessage(directory) {
  return `${directory}/_headers is missing. It must set Cross-Origin-Opener-Policy and Cross-Origin-Embedder-Policy (docs/spec/delivery.md section 2.7).`
}

/** The section 4 message for a `_headers` file with other contents. */
function mismatchedHeadersMessage(directory) {
  return `${directory}/_headers does not match the cross-origin isolation headers of docs/spec/delivery.md section 2.7.`
}

/** The section 4 message for a directory that does not exist. */
function missingDirectoryMessage(directory) {
  return `${directory} does not exist. Run pnpm build first.`
}

/** Creates a temporary directory that is removed when the test ends. */
async function temporaryDirectory(context) {
  const directory = await mkdtemp(join(tmpdir(), 'partsmith-check-dist-'))
  context.after(() => rm(directory, { recursive: true, force: true }))
  return directory
}

/** Writes a file, creating its parent folders. */
async function writeFileAt(path, contents) {
  await mkdir(dirname(path), { recursive: true })
  await writeFile(path, contents)
}

/** Writes a sparse file of `size` bytes, so large sizes cost no disk space. */
async function writeSparseFileAt(path, size) {
  await writeFileAt(path, '')
  await truncate(path, size)
}

/** Runs `node scripts/check-dist.mjs` with `commandArguments` in `workingDirectory`. */
function runCommand(workingDirectory, commandArguments) {
  const result = spawnSync(process.execPath, [scriptPath, ...commandArguments], {
    cwd: workingDirectory,
    encoding: 'utf8',
  })
  return { status: result.status, stdout: result.stdout, stderr: result.stderr }
}

/** Writes a directory that is ready for Pages: three files, the largest 4096 bytes. */
async function writeReadyDistribution(directory) {
  await writeFileAt(join(directory, '_headers'), isolationHeaders)
  await writeFileAt(join(directory, 'index.html'), '<!doctype html>\n')
  await writeFileAt(join(directory, 'core', 'partsmith_core.wasm'), Buffer.alloc(4096))
}

test('14: PAGES_FILE_LIMIT is 26214400', () => {
  assert.equal(PAGES_FILE_LIMIT, 26_214_400)
})

test('14: EXPECTED_HEADERS equals the bytes of ui/public/_headers', async () => {
  assert.deepEqual(await readFile(publicHeadersPath), Buffer.from(EXPECTED_HEADERS, 'utf8'))
})

test('15: a 10-byte file and a file of exactly the limit are not oversized', async (context) => {
  const directory = await temporaryDirectory(context)
  await writeFileAt(join(directory, 'index.html'), '0123456789')
  await writeSparseFileAt(join(directory, 'core', 'partsmith_core.wasm'), 26_214_400)

  assert.deepEqual(await oversizedFiles(directory), [])
})

test('16: a file one byte over the limit at core/big.wasm is reported with its size', async (context) => {
  const directory = await temporaryDirectory(context)
  await writeFileAt(join(directory, 'index.html'), '<!doctype html>\n')
  await writeSparseFileAt(join(directory, 'core', 'big.wasm'), 26_214_401)

  assert.deepEqual(await oversizedFiles(directory), [{ path: 'core/big.wasm', size: 26_214_401 }])
})

test('16: two oversized files in nested folders come back sorted by path', async (context) => {
  const directory = await temporaryDirectory(context)
  // The shallower file sorts last, so a traversal that lists files before folders is caught.
  await writeSparseFileAt(join(directory, 'core', 'partsmith_core.wasm'), 30_000_000)
  await writeSparseFileAt(join(directory, 'core', 'modules', 'kernel.wasm'), 26_214_401)

  assert.deepEqual(await oversizedFiles(directory), [
    { path: 'core/modules/kernel.wasm', size: 26_214_401 },
    { path: 'core/partsmith_core.wasm', size: 30_000_000 },
  ])
})

test('17: a symbolic link to an oversized file outside the directory is not reported', async (context) => {
  const workspace = await temporaryDirectory(context)
  const directory = join(workspace, 'dist')
  const outsideFile = join(workspace, 'outside', 'big.wasm')
  await writeSparseFileAt(outsideFile, 26_214_401)
  await writeFileAt(join(directory, 'index.html'), '<!doctype html>\n')
  await mkdir(join(directory, 'core'))
  await symlink(outsideFile, join(directory, 'core', 'big.wasm'))

  assert.deepEqual(await oversizedFiles(directory), [])
})

test('17: a symbolic link to a folder outside the directory is not followed', async (context) => {
  const workspace = await temporaryDirectory(context)
  const directory = join(workspace, 'dist')
  const outsideFolder = join(workspace, 'outside')
  await writeSparseFileAt(join(outsideFolder, 'big.wasm'), 26_214_401)
  await writeFileAt(join(directory, 'index.html'), '<!doctype html>\n')
  await symlink(outsideFolder, join(directory, 'core'), 'dir')

  assert.deepEqual(await oversizedFiles(directory), [])
})

test('18: checkDist finds no problem in a small file next to the exact _headers', async (context) => {
  const directory = await temporaryDirectory(context)
  await writeFileAt(join(directory, 'index.html'), '<!doctype html>\n')
  await writeFileAt(join(directory, '_headers'), isolationHeaders)

  assert.deepEqual(await checkDist(directory), [])
})

test('18: checkDist reports a missing _headers', async (context) => {
  const directory = await temporaryDirectory(context)
  await writeFileAt(join(directory, 'index.html'), '<!doctype html>\n')

  assert.deepEqual(await checkDist(directory), [missingHeadersMessage(directory)])
})

test('18: checkDist reports a _headers without its trailing newline as different', async (context) => {
  const directory = await temporaryDirectory(context)
  await writeFileAt(join(directory, 'index.html'), '<!doctype html>\n')
  await writeFileAt(join(directory, '_headers'), isolationHeaders.slice(0, -1))

  assert.deepEqual(await checkDist(directory), [mismatchedHeadersMessage(directory)])
})

test('19: the command on a ready directory exits 0 and prints the ready line', async (context) => {
  const workspace = await temporaryDirectory(context)
  await writeReadyDistribution(join(workspace, 'dist'))

  assert.deepEqual(runCommand(workspace, ['dist']), {
    status: 0,
    stdout:
      'dist is ready for Cloudflare Pages: 3 files, the largest is core/partsmith_core.wasm (4096 bytes).\n',
    stderr: '',
  })
})

test('19: the command without an argument checks dist in the working directory', async (context) => {
  const workspace = await temporaryDirectory(context)
  await writeReadyDistribution(join(workspace, 'dist'))

  assert.deepEqual(runCommand(workspace, []), {
    status: 0,
    stdout:
      'dist is ready for Cloudflare Pages: 3 files, the largest is core/partsmith_core.wasm (4096 bytes).\n',
    stderr: '',
  })
})

test('19: the command on a directory with an oversized file exits 1 and names it', async (context) => {
  const workspace = await temporaryDirectory(context)
  const directory = join(workspace, 'dist')
  await writeFileAt(join(directory, '_headers'), isolationHeaders)
  await writeFileAt(join(directory, 'index.html'), '<!doctype html>\n')
  await writeSparseFileAt(join(directory, 'core', 'big.wasm'), 26_214_401)

  assert.deepEqual(runCommand(workspace, ['dist']), {
    status: 1,
    stdout: '',
    stderr: `${oversizedMessage('dist', 'core/big.wasm', 26_214_401)}\n`,
  })
})

test('19: the command on a missing directory exits 1 with the missing message', async (context) => {
  const workspace = await temporaryDirectory(context)

  assert.deepEqual(runCommand(workspace, ['dist']), {
    status: 1,
    stdout: '',
    stderr: `${missingDirectoryMessage('dist')}\n`,
  })
})

test('19: the command with two arguments exits 2 with the usage line', async (context) => {
  const workspace = await temporaryDirectory(context)
  await writeReadyDistribution(join(workspace, 'dist'))

  assert.deepEqual(runCommand(workspace, ['dist', 'public']), {
    status: 2,
    stdout: '',
    stderr: `${usageLine}\n`,
  })
})
