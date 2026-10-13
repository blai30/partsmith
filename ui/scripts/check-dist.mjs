// Checks that a web build can deploy to Cloudflare Pages and keeps its initial JavaScript within the limit of
// docs/spec/ui.md section 5 (docs/spec/delivery.md section 2.6).
// Usage: node scripts/check-dist.mjs [directory]
import { lstat, readFile, readdir, realpath, stat } from 'node:fs/promises'
import { join } from 'node:path'
import process from 'node:process'
import { fileURLToPath } from 'node:url'
import { gzipSync } from 'node:zlib'

/** Cloudflare Pages' per-file limit: 25 MiB. */
export const PAGES_FILE_LIMIT = 26_214_400

/** The initial JavaScript limit of ui.md section 5: 250 KB gzipped, with 1 KB = 1,000 bytes. */
export const INITIAL_JAVASCRIPT_LIMIT = 250_000

/** The exact text of `_headers` (2.7). */
export const EXPECTED_HEADERS =
  '/*\n  Cross-Origin-Opener-Policy: same-origin\n  Cross-Origin-Embedder-Policy: require-corp\n'

const USAGE = 'usage: node scripts/check-dist.mjs [directory]'

// The attributes of a start tag as Vite writes them: lowercase names, values in double quotes.
const ATTRIBUTES = String.raw`((?:\s+[a-z][a-z0-9-]*(?:="[^"]*")?)*)\s*\/?`

// A comment, a script element with its content, or a link tag; text in a comment or a script is never read as a tag.
const TAG_PATTERN = new RegExp(
  `<!--.*?-->|<script${ATTRIBUTES}>.*?</script\\s*>|<link${ATTRIBUTES}>`,
  'gs',
)

// One attribute of an ATTRIBUTES match, with its value when it has one.
const ATTRIBUTE_PATTERN = /([a-z][a-z0-9-]*)(?:="([^"]*)")?/g

// A reference with a scheme, a query or a fragment, which never names a file of the build (2.6).
const NOT_A_PATH_PATTERN = /^[a-z][a-z0-9+.-]*:|[?#]/i

// The name Vite gives the core worker script: `assets/[name]-[hash].js` with the name `core.worker`.
const CORE_WORKER_PATTERN = /^assets\/core\.worker-[^/]*\.js$/

/** Orders `{ path }` records by path, comparing code units so the order is the same in every locale. */
function byPath(first, second) {
  if (first.path === second.path) {
    return 0
  }
  return first.path < second.path ? -1 : 1
}

/** Every regular file under `folder` as `{ path, size }`, with `path` starting with `prefix`; symbolic links are skipped. */
async function filesUnder(folder, prefix) {
  const entries = await readdir(folder, { withFileTypes: true })
  const nested = await Promise.all(
    entries.map(async (entry) => {
      const path = join(folder, entry.name)
      const relativePath = `${prefix}${entry.name}`
      if (entry.isDirectory()) {
        return filesUnder(path, `${relativePath}/`)
      }
      if (entry.isFile()) {
        const { size } = await lstat(path)
        return [{ path: relativePath, size }]
      }
      return []
    }),
  )
  return nested.flat()
}

/** Every regular file under `directory` as `{ path, size }`, sorted by path. */
async function regularFiles(directory) {
  return (await filesUnder(directory, '')).toSorted(byPath)
}

/**
 * Every regular file under `directory` (recursively, not following symbolic links) larger than `limit` bytes,
 * as `{ path, size }` with `path` relative to `directory` using `/`, sorted by path.
 */
export async function oversizedFiles(directory, limit = PAGES_FILE_LIMIT) {
  return (await regularFiles(directory)).filter((file) => file.size > limit)
}

/** Whether `path` is an existing directory. */
async function isDirectory(path) {
  try {
    return (await stat(path)).isDirectory()
  } catch (error) {
    if (error.code === 'ENOENT' || error.code === 'ENOTDIR') {
      return false
    }
    throw error
  }
}

/**
 * The problem with `{directory}/_headers`, if it has one. Like the size check, it looks only at regular files,
 * so a symbolic link or a folder named `_headers` counts as missing.
 */
async function headersProblems(directory, files) {
  if (!files.some((file) => file.path === '_headers')) {
    return [
      `${directory}/_headers is missing. It must set Cross-Origin-Opener-Policy and Cross-Origin-Embedder-Policy (docs/spec/delivery.md section 2.7).`,
    ]
  }
  if ((await readFile(join(directory, '_headers'), 'utf8')) !== EXPECTED_HEADERS) {
    return [
      `${directory}/_headers does not match the cross-origin isolation headers of docs/spec/delivery.md section 2.7.`,
    ]
  }
  return []
}

/** The attributes of a start tag by name; an attribute named twice keeps its first value, as in HTML. */
function attributesOf(text) {
  const attributes = new Map()
  for (const [, name, value = ''] of text.matchAll(ATTRIBUTE_PATTERN)) {
    if (!attributes.has(name)) {
      attributes.set(name, value)
    }
  }
  return attributes
}

/** The `src` of every script that has one and the `href` of every `modulepreload` link of a page, in order. */
function pageReferences(page) {
  return [...page.matchAll(TAG_PATTERN)].flatMap(([, scriptAttributes, linkAttributes]) => {
    if (scriptAttributes !== undefined) {
      const source = attributesOf(scriptAttributes).get('src')
      return source === undefined ? [] : [source]
    }
    if (linkAttributes !== undefined) {
      const attributes = attributesOf(linkAttributes)
      const target = attributes.get('href')
      return attributes.get('rel') === 'modulepreload' && target !== undefined ? [target] : []
    }
    return []
  })
}

/** The path of the regular file of the build that `reference` names, or undefined when it names none (2.6). */
function resolvedPath(reference, paths) {
  if (NOT_A_PATH_PATTERN.test(reference)) {
    return undefined
  }
  const path = reference.replace(/^\.?\//, '')
  return paths.has(path) ? path : undefined
}

/** The initial JavaScript of `directory` as `{ files, unresolved }`, given its regular files sorted by path. */
async function initialJavaScriptOf(directory, files) {
  const paths = new Set(files.map((file) => file.path))
  const initialPaths = new Set([...paths].filter((path) => CORE_WORKER_PATTERN.test(path)))
  const unresolved = new Set()
  if (paths.has('index.html')) {
    for (const reference of pageReferences(await readFile(join(directory, 'index.html'), 'utf8'))) {
      const path = resolvedPath(reference, paths)
      if (path === undefined) {
        unresolved.add(reference)
      } else {
        initialPaths.add(path)
      }
    }
  }
  const initialFiles = await Promise.all(
    files
      .filter((file) => initialPaths.has(file.path))
      .map(async ({ path }) => {
        const contents = await readFile(join(directory, path))
        return { path, size: contents.length, gzippedSize: gzipSync(contents, { level: 9 }).length }
      }),
  )
  return { files: initialFiles, unresolved: [...unresolved] }
}

/**
 * The initial JavaScript of `directory` (2.6) as `{ files, unresolved }`. `files` lists each initial script once as
 * `{ path, size, gzippedSize }`, sorted by path, with `gzippedSize` the length of `gzipSync(contents, { level: 9 })`.
 * `unresolved` lists each distinct reference of `index.html` that is not a file of the build, in order of first
 * appearance.
 */
export async function initialJavaScript(directory) {
  return initialJavaScriptOf(directory, await regularFiles(directory))
}

/** The sum of the gzipped sizes of the initial files. */
function gzippedTotal(initialFiles) {
  return initialFiles.reduce((total, file) => total + file.gzippedSize, 0)
}

/** The messages (section 4) for the initial JavaScript `initial` of `directory` against `limit`. */
function initialJavaScriptMessages(directory, initial, limit) {
  const unresolved = initial.unresolved.map(
    (reference) =>
      `${directory}/index.html loads ${reference}, which is not a file in ${directory}. Make it the path of a file of the build, with no scheme, query or fragment, and keep base in vite.config.ts as '/' or './'.`,
  )
  const total = gzippedTotal(initial.files)
  if (total <= limit) {
    return unresolved
  }
  const sizes = initial.files.map((file) => `${file.path} (${file.gzippedSize} bytes)`).join(', ')
  return [
    ...unresolved,
    `${directory} has ${total} bytes of initial JavaScript gzipped, over the limit of ${limit} bytes (docs/spec/ui.md section 5): ${sizes}. Move code that startup does not need into modules loaded with import() (docs/spec/delivery.md section 2.6).`,
  ]
}

/**
 * The initial JavaScript problems of `directory` as messages (section 4): one per unresolved reference, in order,
 * then the limit message when the gzipped sizes of its files sum to more than `limit` bytes.
 */
export async function initialJavaScriptProblems(directory, limit = INITIAL_JAVASCRIPT_LIMIT) {
  return initialJavaScriptMessages(directory, await initialJavaScript(directory), limit)
}

/**
 * The regular files of `directory`, the gzipped total of its initial JavaScript and every problem that keeps it from
 * deploying.
 */
async function inspect(directory) {
  if (!(await isDirectory(directory))) {
    return {
      files: [],
      initialTotal: 0,
      problems: [`${directory} does not exist. Run pnpm build first.`],
    }
  }
  const files = await regularFiles(directory)
  const initial = await initialJavaScriptOf(directory, files)
  const oversized = files
    .filter((file) => file.size > PAGES_FILE_LIMIT)
    .map(
      (file) =>
        `${directory}/${file.path} is ${file.size} bytes, over the Cloudflare Pages limit of ${PAGES_FILE_LIMIT} bytes (25 MiB). Split the core into lazily loaded modules or serve the file from R2 (docs/spec/delivery.md section 3.5).`,
    )
  return {
    files,
    initialTotal: gzippedTotal(initial.files),
    problems: [
      ...oversized,
      ...(await headersProblems(directory, files)),
      ...initialJavaScriptMessages(directory, initial, INITIAL_JAVASCRIPT_LIMIT),
    ],
  }
}

/**
 * Every problem that keeps `directory` from deploying, as messages (section 4): oversized files by path, then
 * `_headers`, then the initial JavaScript problems; only the missing-directory message when `directory` does not
 * exist; empty when it is ready.
 */
export async function checkDist(directory) {
  return (await inspect(directory)).problems
}

/** Runs the command with its arguments and returns its exit status. */
async function main(commandArguments) {
  if (commandArguments.length > 1) {
    process.stderr.write(`${USAGE}\n`)
    return 2
  }
  const directory = commandArguments[0] ?? 'dist'
  const { files, initialTotal, problems } = await inspect(directory)
  if (problems.length > 0) {
    process.stderr.write(problems.map((problem) => `${problem}\n`).join(''))
    return 1
  }
  // A ready directory holds at least the regular file `_headers`; the first of the largest files wins a tie.
  const largest = files.reduce((current, file) => (file.size > current.size ? file : current))
  process.stdout.write(
    `${directory} is ready for Cloudflare Pages: ${files.length} files, the largest is ${largest.path} (${largest.size} bytes).\n` +
      `Initial JavaScript: ${initialTotal} bytes gzipped, ${((initialTotal * 100) / INITIAL_JAVASCRIPT_LIMIT).toFixed(1)} percent of the limit of ${INITIAL_JAVASCRIPT_LIMIT} bytes.\n`,
  )
  return 0
}

// Runs as a command only when Node started this file, not when another module imports it.
const startedFile = process.argv[1]
if (startedFile !== undefined && (await realpath(startedFile)) === fileURLToPath(import.meta.url)) {
  process.exitCode = await main(process.argv.slice(2))
}
