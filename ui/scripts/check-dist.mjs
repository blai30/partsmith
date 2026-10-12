// Checks that a web build can deploy to Cloudflare Pages (docs/spec/delivery.md section 2.6).
// Usage: node scripts/check-dist.mjs [directory]
import { lstat, readFile, readdir, realpath, stat } from 'node:fs/promises'
import { join } from 'node:path'
import process from 'node:process'
import { fileURLToPath } from 'node:url'

/** Cloudflare Pages' per-file limit: 25 MiB. */
export const PAGES_FILE_LIMIT = 26_214_400

/** The exact text of `_headers` (2.7). */
export const EXPECTED_HEADERS =
  '/*\n  Cross-Origin-Opener-Policy: same-origin\n  Cross-Origin-Embedder-Policy: require-corp\n'

const USAGE = 'usage: node scripts/check-dist.mjs [directory]'

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

/** The regular files of `directory` and every problem that keeps it from deploying to Pages. */
async function inspect(directory) {
  if (!(await isDirectory(directory))) {
    return { files: [], problems: [`${directory} does not exist. Run pnpm build first.`] }
  }
  const files = await regularFiles(directory)
  const oversized = files
    .filter((file) => file.size > PAGES_FILE_LIMIT)
    .map(
      (file) =>
        `${directory}/${file.path} is ${file.size} bytes, over the Cloudflare Pages limit of ${PAGES_FILE_LIMIT} bytes (25 MiB). Split the core into lazily loaded modules or serve the file from R2 (docs/spec/delivery.md section 3.5).`,
    )
  return { files, problems: [...oversized, ...(await headersProblems(directory, files))] }
}

/** Every problem that keeps `directory` from deploying to Pages, as messages (section 4); empty when it is ready. */
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
  const { files, problems } = await inspect(directory)
  if (problems.length > 0) {
    process.stderr.write(problems.map((problem) => `${problem}\n`).join(''))
    return 1
  }
  // A ready directory holds at least the regular file `_headers`; the first of the largest files wins a tie.
  const largest = files.reduce((current, file) => (file.size > current.size ? file : current))
  process.stdout.write(
    `${directory} is ready for Cloudflare Pages: ${files.length} files, the largest is ${largest.path} (${largest.size} bytes).\n`,
  )
  return 0
}

// Runs as a command only when Node started this file, not when another module imports it.
const startedFile = process.argv[1]
if (startedFile !== undefined && (await realpath(startedFile)) === fileURLToPath(import.meta.url)) {
  process.exitCode = await main(process.argv.slice(2))
}
