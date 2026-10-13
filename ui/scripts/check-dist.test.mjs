// Acceptance tests 14 to 19 and 26 to 33 of docs/spec/delivery.md section 6 for the distribution check.
import assert from 'node:assert/strict'
import { spawnSync } from 'node:child_process'
import { randomBytes } from 'node:crypto'
import { lstat, mkdir, mkdtemp, readFile, rm, symlink, truncate, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { dirname, join } from 'node:path'
import { test } from 'node:test'
import { fileURLToPath } from 'node:url'
import { gzipSync } from 'node:zlib'
import { resolveConfig } from 'vite'

import {
  EXPECTED_HEADERS,
  INITIAL_JAVASCRIPT_LIMIT,
  PAGES_FILE_LIMIT,
  checkDist,
  initialJavaScript,
  initialJavaScriptProblems,
  oversizedFiles,
} from './check-dist.mjs'

const scriptPath = fileURLToPath(new URL('check-dist.mjs', import.meta.url))
const publicHeadersPath = fileURLToPath(new URL('../public/_headers', import.meta.url))
const uiDirectory = fileURLToPath(new URL('..', import.meta.url))

// The cross-origin isolation headers exactly as section 2.7 gives them.
const isolationHeaders =
  '/*\n  Cross-Origin-Opener-Policy: same-origin\n  Cross-Origin-Embedder-Policy: require-corp\n'

const usageLine = 'usage: node scripts/check-dist.mjs [directory]'

// The tag Vite writes in index.html for the entry script assets/index-a1.js.
const entryScriptTag = '<script type="module" crossorigin src="/assets/index-a1.js"></script>'

// Standard output of the command on the directory that writeReadyDistribution writes.
const readyOutput =
  'dist is ready for Cloudflare Pages: 3 files, the largest is core/partsmith_core.wasm (4096 bytes).\n' +
  'Initial JavaScript: 0 bytes gzipped, 0.0 percent of the limit of 250000 bytes.\n'

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

/** The section 4 message for a script or preload of `index.html` that is not a file of the build. */
function unresolvedMessage(directory, reference) {
  return `${directory}/index.html loads ${reference}, which is not a file in ${directory}. Make it the path of a file of the build, with no scheme, query or fragment, and keep base in vite.config.ts as '/' or './'.`
}

/** The section 4 message for initial JavaScript over `limit`, with `files` as `{ path, gzippedSize }` in path order. */
function overLimitMessage(directory, total, limit, files) {
  const sizes = files.map((file) => `${file.path} (${file.gzippedSize} bytes)`).join(', ')
  return `${directory} has ${total} bytes of initial JavaScript gzipped, over the limit of ${limit} bytes (docs/spec/ui.md section 5): ${sizes}. Move code that startup does not need into modules loaded with import() (docs/spec/delivery.md section 2.6).`
}

/** The second line the command prints on a ready directory, for a total of `total` gzipped bytes. */
function initialJavaScriptLine(total) {
  return `Initial JavaScript: ${total} bytes gzipped, ${((total * 100) / 250_000).toFixed(1)} percent of the limit of 250000 bytes.`
}

/** The length of `contents` compressed by `gzipSync` at `level`. */
function gzippedLength(contents, level = 9) {
  return gzipSync(contents, { level }).length
}

/** The file at `path` holding the bytes `contents`, as `initialJavaScript` lists an initial file. */
function initialFile(path, contents) {
  return { path, size: contents.length, gzippedSize: gzippedLength(contents) }
}

/** The sum of the `gzippedSize` values of `files`. */
function totalOf(files) {
  return files.reduce((total, file) => total + file.gzippedSize, 0)
}

/** The text of test 28: `const value{i} = {(i * 7919) % 10007};` for `i` from 0 to 9999, one per line. */
function generatedScript() {
  return Array.from(
    { length: 10_000 },
    (_, i) => `const value${i} = ${(i * 7919) % 10_007};\n`,
  ).join('')
}

/** An `index.html` shaped like Vite's whose `<head>` holds `tags`, in order. */
function pageWith(tags) {
  const head = tags.map((tag) => `    ${tag}\n`).join('')
  return `<!doctype html>\n<html lang="en">\n  <head>\n    <meta charset="UTF-8" />\n${head}  </head>\n  <body>\n    <div id="root"></div>\n  </body>\n</html>\n`
}

let buildConfiguration

/** Vite's configuration for `pnpm build`, resolved from `ui/vite.config.ts` once for all tests. */
function resolvedBuildConfiguration() {
  buildConfiguration ??= resolveConfig(
    { root: uiDirectory, configFile: join(uiDirectory, 'vite.config.ts'), logLevel: 'silent' },
    'build',
  )
  return buildConfiguration
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

/**
 * Writes the exact `_headers`, an `index.html` naming `/assets/index-a1.js`, that script with `entryContents` and the
 * worker script `assets/core.worker-e5.js` with `workerContents`; returns the files as `initialJavaScript` lists them.
 */
async function writeInitialScripts(directory, entryContents, workerContents) {
  await writeFileAt(join(directory, '_headers'), isolationHeaders)
  await writeFileAt(join(directory, 'index.html'), pageWith([entryScriptTag]))
  await writeFileAt(join(directory, 'assets', 'index-a1.js'), entryContents)
  await writeFileAt(join(directory, 'assets', 'core.worker-e5.js'), workerContents)
  return [
    initialFile('assets/core.worker-e5.js', workerContents),
    initialFile('assets/index-a1.js', entryContents),
  ]
}

/**
 * Writes the directory of test 29: the exact `_headers`, `assets/index-a1.js`, `assets/folder/x.js`, a symbolic link
 * `assets/link.js` to a file outside it, and an `index.html` naming none of them as a file of the build.
 */
async function writeUnresolvedDistribution(workspace) {
  const directory = join(workspace, 'dist')
  const outsideFile = join(workspace, 'outside', 'link.js')
  await writeFileAt(outsideFile, 'export const outside = 1\n')
  await writeFileAt(join(directory, '_headers'), isolationHeaders)
  await writeFileAt(join(directory, 'assets', 'index-a1.js'), 'export const entry = 1\n')
  await writeFileAt(join(directory, 'assets', 'folder', 'x.js'), 'export const nested = 1\n')
  await symlink(outsideFile, join(directory, 'assets', 'link.js'))
  await writeFileAt(
    join(directory, 'index.html'),
    pageWith([
      '<script type="module" src="/assets/missing.js"></script>',
      '<script type="module" src="https://example.com/remote.js"></script>',
      '<link rel="modulepreload" href="/assets/folder">',
      '<link rel="modulepreload" href="/assets/link.js">',
      '<script type="module" src="/assets/index-a1.js?v=1"></script>',
      '<script type="module" src="/assets/missing.js"></script>',
    ]),
  )
  return directory
}

// The distinct references of test 29's index.html, in order of first appearance.
const unresolvedReferences = [
  '/assets/missing.js',
  'https://example.com/remote.js',
  '/assets/folder',
  '/assets/link.js',
  '/assets/index-a1.js?v=1',
]

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

test('19: the command on a ready directory exits 0 and prints the ready line and the initial JavaScript line', async (context) => {
  const workspace = await temporaryDirectory(context)
  await writeReadyDistribution(join(workspace, 'dist'))

  assert.deepEqual(runCommand(workspace, ['dist']), { status: 0, stdout: readyOutput, stderr: '' })
})

test('19: the command without an argument checks dist in the working directory', async (context) => {
  const workspace = await temporaryDirectory(context)
  await writeReadyDistribution(join(workspace, 'dist'))

  assert.deepEqual(runCommand(workspace, []), { status: 0, stdout: readyOutput, stderr: '' })
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

test('26: INITIAL_JAVASCRIPT_LIMIT is 250000', () => {
  assert.equal(INITIAL_JAVASCRIPT_LIMIT, 250_000)
})

test('27: initialJavaScript counts scripts, modulepreload links and the core worker once each, sorted by path', async (context) => {
  const directory = await temporaryDirectory(context)
  await writeFileAt(
    join(directory, 'index.html'),
    pageWith([
      entryScriptTag,
      '<link rel="modulepreload" crossorigin href="/assets/vendor-b2.js">',
      '<link rel="modulepreload" crossorigin href="./assets/vendor-b2.js">',
      '<link rel="stylesheet" crossorigin href="/assets/index-c3.css">',
      '<script type="module">import \'./assets/inline-f6.js\'</script>',
    ]),
  )
  const paths = [
    'assets/index-a1.js',
    'assets/index-a1.js.map',
    'assets/vendor-b2.js',
    'assets/index-c3.css',
    'assets/inline-f6.js',
    'assets/lazy-d4.js',
    'assets/core.worker-e5.js',
    'assets/core.worker-e5.js.map',
    'workers/core.worker-g7.js',
    'core/partsmith_core.mjs',
  ]
  // Each file holds a different text, so a size taken from the wrong file shows.
  const contents = new Map(
    paths.map((path) => [path, Buffer.from(`export const source = '${path}'\n`)]),
  )
  await Promise.all([...contents].map(([path, text]) => writeFileAt(join(directory, path), text)))
  const counted = (path) => initialFile(path, contents.get(path))

  assert.deepEqual(await initialJavaScript(directory), {
    files: [
      counted('assets/core.worker-e5.js'),
      counted('assets/index-a1.js'),
      counted('assets/vendor-b2.js'),
    ],
    unresolved: [],
  })
})

test('27: initialJavaScript counts the core worker script of a directory without index.html', async (context) => {
  const directory = await temporaryDirectory(context)
  const worker = Buffer.from('self.onmessage = () => {}\n')
  await writeFileAt(join(directory, 'assets', 'core.worker-e5.js'), worker)

  assert.deepEqual(await initialJavaScript(directory), {
    files: [initialFile('assets/core.worker-e5.js', worker)],
    unresolved: [],
  })
})

test('27: initialJavaScript does not count a script in a folder named like the core worker', async (context) => {
  const directory = await temporaryDirectory(context)
  await writeFileAt(
    join(directory, 'assets', 'core.worker-e5', 'nested.js'),
    'export const nested = 1\n',
  )

  assert.deepEqual(await initialJavaScript(directory), { files: [], unresolved: [] })
})

test('27: initialJavaScript does not read tags inside a comment or an inline script', async (context) => {
  const directory = await temporaryDirectory(context)
  const entry = Buffer.from('export const entry = 1\n')
  await writeFileAt(
    join(directory, 'index.html'),
    pageWith([
      '<!-- <script type="module" src="/assets/commented-h8.js"></script> -->',
      '<script type="module">console.log(\'<link rel="modulepreload" href="/assets/text-i9.js">\')</script>',
      entryScriptTag,
    ]),
  )
  await writeFileAt(join(directory, 'assets', 'index-a1.js'), entry)
  await writeFileAt(join(directory, 'assets', 'commented-h8.js'), 'export const commented = 1\n')
  await writeFileAt(join(directory, 'assets', 'text-i9.js'), 'export const text = 1\n')

  assert.deepEqual(await initialJavaScript(directory), {
    files: [initialFile('assets/index-a1.js', entry)],
    unresolved: [],
  })
})

test('28: gzippedSize is the gzip length at level 9', async (context) => {
  const script = Buffer.from(generatedScript())
  const levelLengths = [1, 6, 9].map((level) => gzippedLength(script, level))
  // The text tells the three levels apart, so a size taken at another level shows.
  assert.equal(script.length, 237_788)
  assert.equal(new Set(levelLengths).size, 3)

  const directory = await temporaryDirectory(context)
  await writeFileAt(join(directory, 'index.html'), pageWith([entryScriptTag]))
  await writeFileAt(join(directory, 'assets', 'index-a1.js'), script)

  assert.deepEqual(await initialJavaScript(directory), {
    files: [{ path: 'assets/index-a1.js', size: 237_788, gzippedSize: levelLengths[2] }],
    unresolved: [],
  })
})

test('29: initialJavaScript lists each reference that is not a file of the build once, in order', async (context) => {
  const directory = await writeUnresolvedDistribution(await temporaryDirectory(context))

  assert.deepEqual(await initialJavaScript(directory), {
    files: [],
    unresolved: unresolvedReferences,
  })
})

test('29: checkDist reports each unresolved reference, in order', async (context) => {
  const directory = await writeUnresolvedDistribution(await temporaryDirectory(context))

  assert.deepEqual(
    await checkDist(directory),
    unresolvedReferences.map((reference) => unresolvedMessage(directory, reference)),
  )
})

test('29: initialJavaScript leaves a reference with a scheme, a query or a fragment unresolved when a file has its name', async (context) => {
  const directory = await temporaryDirectory(context)
  // Each reference names a file of this directory character for character, which a browser would not request.
  const references = [
    'data:text/javascript,1',
    '/assets/index-a1.js?v=1',
    '/assets/index-a1.js#top',
  ]
  await writeFileAt(
    join(directory, 'index.html'),
    pageWith(references.map((reference) => `<script type="module" src="${reference}"></script>`)),
  )
  await Promise.all(
    references.map((reference) =>
      writeFileAt(join(directory, reference.replace(/^\//, '')), 'export const named = 1\n'),
    ),
  )

  assert.deepEqual(await initialJavaScript(directory), { files: [], unresolved: references })
})

test('30: checkDist reports initial JavaScript over the limit with each file and its gzipped size', async (context) => {
  const directory = await temporaryDirectory(context)
  const files = await writeInitialScripts(directory, randomBytes(260_000), randomBytes(1_000))
  const total = totalOf(files)
  assert.ok(total > 250_000)

  assert.deepEqual(await checkDist(directory), [overLimitMessage(directory, total, 250_000, files)])
})

test('30: initialJavaScriptProblems passes a total equal to the limit', async (context) => {
  const directory = await temporaryDirectory(context)
  const files = await writeInitialScripts(directory, randomBytes(260_000), randomBytes(1_000))

  assert.deepEqual(await initialJavaScriptProblems(directory, totalOf(files)), [])
})

test('30: initialJavaScriptProblems reports a total one byte over the limit', async (context) => {
  const directory = await temporaryDirectory(context)
  const files = await writeInitialScripts(directory, randomBytes(260_000), randomBytes(1_000))
  const total = totalOf(files)

  assert.deepEqual(await initialJavaScriptProblems(directory, total - 1), [
    overLimitMessage(directory, total, total - 1, files),
  ])
})

test('30: initialJavaScriptProblems without a limit uses INITIAL_JAVASCRIPT_LIMIT', async (context) => {
  const directory = await temporaryDirectory(context)
  const files = await writeInitialScripts(directory, randomBytes(260_000), randomBytes(1_000))

  assert.deepEqual(await initialJavaScriptProblems(directory), [
    overLimitMessage(directory, totalOf(files), 250_000, files),
  ])
})

test('30: checkDist finds no problem in initial JavaScript under the limit', async (context) => {
  const directory = await temporaryDirectory(context)
  const files = await writeInitialScripts(directory, randomBytes(200_000), randomBytes(1_000))
  assert.ok(totalOf(files) <= 250_000)

  assert.deepEqual(await checkDist(directory), [])
})

test('30: checkDist reports oversized files, then _headers, then unresolved references, then the limit', async (context) => {
  const directory = await temporaryDirectory(context)
  const files = await writeInitialScripts(directory, randomBytes(260_000), randomBytes(1_000))
  await rm(join(directory, '_headers'))
  await writeSparseFileAt(join(directory, 'core', 'big.wasm'), 26_214_401)
  await writeFileAt(
    join(directory, 'index.html'),
    pageWith([
      entryScriptTag,
      '<script type="module" crossorigin src="/assets/missing.js"></script>',
    ]),
  )

  assert.deepEqual(await checkDist(directory), [
    oversizedMessage(directory, 'core/big.wasm', 26_214_401),
    missingHeadersMessage(directory),
    unresolvedMessage(directory, '/assets/missing.js'),
    overLimitMessage(directory, totalOf(files), 250_000, files),
  ])
})

test('31: the command on a ready directory prints the initial JavaScript total and its percent of the limit', async (context) => {
  const workspace = await temporaryDirectory(context)
  const directory = join(workspace, 'dist')
  const script = Buffer.from(generatedScript())
  await writeFileAt(join(directory, '_headers'), isolationHeaders)
  await writeFileAt(join(directory, 'index.html'), pageWith([entryScriptTag]))
  await writeFileAt(join(directory, 'assets', 'index-a1.js'), script)

  assert.deepEqual(runCommand(workspace, ['dist']), {
    status: 0,
    stdout:
      'dist is ready for Cloudflare Pages: 3 files, the largest is assets/index-a1.js (237788 bytes).\n' +
      `${initialJavaScriptLine(gzippedLength(script))}\n`,
    stderr: '',
  })
})

test('31: the command on a directory over the initial JavaScript limit exits 1 with the over-limit message', async (context) => {
  const workspace = await temporaryDirectory(context)
  const files = await writeInitialScripts(
    join(workspace, 'dist'),
    randomBytes(260_000),
    randomBytes(1_000),
  )

  assert.deepEqual(runCommand(workspace, ['dist']), {
    status: 1,
    stdout: '',
    stderr: `${overLimitMessage('dist', totalOf(files), 250_000, files)}\n`,
  })
})

test("32: the build keeps Vite's default build.modulePreload, so every static chunk has a modulepreload link", async () => {
  const configuration = await resolvedBuildConfiguration()

  assert.deepEqual(configuration.build.modulePreload, { polyfill: true })
})

test("32: the build keeps Vite's name assets/[name]-[hash].js for the worker script", async () => {
  const configuration = await resolvedBuildConfiguration()

  assert.equal(configuration.build.assetsDir, 'assets')
  assert.equal(configuration.worker.rolldownOptions.output, undefined)
})

test('32: src/core/core.worker.ts is a regular file', async () => {
  const worker = await lstat(join(uiDirectory, 'src', 'core', 'core.worker.ts'))

  assert.ok(worker.isFile())
})

test('32: transport.ts passes the URL of core.worker.ts to new Worker', async () => {
  const transport = await readFile(join(uiDirectory, 'src', 'core', 'transport.ts'), 'utf8')

  assert.match(transport, /new Worker\(\s*new URL\('\.\/core\.worker\.ts', import\.meta\.url\)/)
})

test('33: initialJavaScript reads the attributes of a tag in any order', async (context) => {
  const directory = await temporaryDirectory(context)
  const entry = Buffer.from('export const entry = 1\n')
  const vendor = Buffer.from('export const vendor = 1\n')
  await writeFileAt(
    join(directory, 'index.html'),
    pageWith([
      '<link crossorigin href="/assets/vendor-b2.js" rel="modulepreload">',
      '<script src="/assets/index-a1.js" type="module"></script>',
    ]),
  )
  await writeFileAt(join(directory, 'assets', 'index-a1.js'), entry)
  await writeFileAt(join(directory, 'assets', 'vendor-b2.js'), vendor)

  assert.deepEqual(await initialJavaScript(directory), {
    files: [initialFile('assets/index-a1.js', entry), initialFile('assets/vendor-b2.js', vendor)],
    unresolved: [],
  })
})
