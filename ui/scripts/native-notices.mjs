// Writes the notices of the crates compiled into the native npm packages whose tarballs lack them: one file in
// ui/scripts/notices/native/ for each entry of NATIVE_PACKAGES with `notices`, made from its source at the pinned
// commit (docs/spec/delivery.md section 2.13).
// Usage: node scripts/native-notices.mjs
import { mkdir, mkdtemp, realpath, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { dirname, join } from 'node:path'
import process from 'node:process'
import { fileURLToPath } from 'node:url'

import {
  CRATES_IO_SOURCE,
  DOWNLOAD_RETRY_DELAYS,
  NATIVE_PACKAGES,
  cloneCheckedTag,
  nativeNoticesFile,
} from '../../scripts/dependency-source-archive.mjs'
import { compareNameAndVersion, runCommand } from './check-licenses.mjs'
import {
  cargoMetadata,
  copiedTexts,
  excludedPaths,
  folderTexts,
  reachableIds,
  readInOrder,
  renderCrateNotices,
  rootLicenseTexts,
  standardLicense,
  standardText,
  workspacePackage,
} from './notices.mjs'

const USAGE = 'usage: node scripts/native-notices.mjs'

const repositoryRoot = fileURLToPath(new URL('../..', import.meta.url))

// The committed copies of the license files at the root of a crate's repository, for a crate that ships none.
const REPOSITORY_COPIES_FOLDER = 'ui/scripts/notices/repositories'

/**
 * The folder under ui/scripts/notices/repositories/ of the repository at `url`: `{host}/{owner}/{name}`, from the
 * host and the first two segments of its path, without a final `.git`. Null when `url` is null or has no such path.
 */
export function repositoryFolder(url) {
  if (url === null || !URL.canParse(url)) {
    return null
  }
  const { host, pathname } = new URL(url)
  const [owner, name] = pathname.split('/').filter((segment) => segment !== '')
  return name === undefined ? null : `${host}/${owner}/${name.replace(/\.git$/, '')}`
}

/** `{name} {version}` of a package of `cargo metadata`. */
function keyOf(listed) {
  return `${listed.name} ${listed.version}`
}

/**
 * The texts of `listed`, a crate of the source of `entry` at `clone` that ships no license text and takes no standard
 * one: the license files at the root of the clone for a crate of the source's workspace, and otherwise the committed
 * copies of its repository's under `root`.
 */
async function repositoryTexts(root, clone, entry, listed) {
  if (listed.source === null) {
    const texts = await rootLicenseTexts(clone)
    if (texts.length === 0) {
      throw new Error(
        `Rust crate ${keyOf(listed)} of ${entry.source} ships no license text, and the root of the tag ${entry.tag} holds none (docs/spec/delivery.md section 2.13).`,
      )
    }
    return texts
  }
  const folder = repositoryFolder(listed.repository)
  if (folder === null) {
    throw new Error(
      `Rust crate ${keyOf(listed)} ships no license text, takes no standard text and names no repository with an owner and a name (docs/spec/delivery.md section 2.13).`,
    )
  }
  const texts = await copiedTexts(join(root, REPOSITORY_COPIES_FOLDER, folder))
  if (texts.length === 0) {
    throw new Error(
      `Rust crate ${keyOf(listed)} ships no license text, and ${REPOSITORY_COPIES_FOLDER}/${folder}/ is missing. Commit a copy of the license files at the root of ${listed.repository} there (docs/spec/delivery.md section 2.13).`,
    )
  }
  return texts
}

/**
 * The `{ textSource, texts }` of `listed`, a crate of the source of `entry` at `clone`, found as 2.9 input 3 finds
 * them, with `/` in its license read as `OR` for a standard text, and the texts of its repository otherwise.
 */
async function crateTexts(root, clone, entry, listed) {
  const folder = dirname(listed.manifest_path)
  const shipped = await folderTexts(folder, listed.license_file, excludedPaths(listed.name))
  if (shipped.length > 0) {
    return { textSource: 'shipped', texts: shipped }
  }
  const identifier = standardLicense(listed.license?.replaceAll('/', ' OR ') ?? null)
  if (identifier !== undefined) {
    return { textSource: 'standard', texts: [await standardText(root, identifier, listed)] }
  }
  return { textSource: 'repository', texts: await repositoryTexts(root, clone, entry, listed) }
}

/** The URL of `listed` in the notices of `entry`: the tag's tree for its source's own crates, crates.io's otherwise. */
function crateUrl(entry, listed) {
  if (listed.source === null) {
    return `${entry.source}/tree/${entry.tag}`
  }
  if (listed.source !== CRATES_IO_SOURCE) {
    throw new Error(
      `Rust crate ${keyOf(listed)} of the source of ${entry.name} ${entry.version} comes from ${listed.source}, neither crates.io nor the source's own workspace. The notices name a crate on crates.io or in the tag's tree (docs/spec/delivery.md section 2.13).`,
    )
  }
  return `https://crates.io/crates/${listed.name}/${listed.version}`
}

/**
 * The text of the notices file of the table entry `entry` from `clone`, its source at the tag (2.13): every crate that
 * `entry.notices.crate` reaches through normal and build edges, on every platform, in the `cargo metadata` output of
 * the source's workspace, sent through `run` in `root`. Its own crates are included.
 */
export async function nativeNotices(root, run, clone, entry) {
  const metadata = await cargoMetadata(root, run, null, join(clone, 'Cargo.toml'))
  const binding = workspacePackage(metadata, entry.notices.crate)
  const packagesById = new Map(metadata.packages.map((listed) => [listed.id, listed]))
  const crates = [...reachableIds(metadata, [binding.id])]
    .map((id) => packagesById.get(id))
    .toSorted(compareNameAndVersion)
  const urls = crates.map((listed) => crateUrl(entry, listed))
  const texts = await readInOrder(crates, (listed) => crateTexts(root, clone, entry, listed))
  return renderCrateNotices(
    crates.map((listed, i) => ({
      name: listed.name,
      version: listed.version,
      license: listed.license,
      url: urls[i],
      platforms: null,
      textSource: texts[i].textSource,
      texts: texts[i].texts,
    })),
  )
}

/**
 * Writes the notices file of the table entry `entry` into the repository at `root` (`nativeNoticesFile`), from a
 * shallow clone of its tag in a temporary folder, which must name `entry.notices.commit`, and resolves to the file's
 * path relative to `root`. Every `cargo` command goes through `run`. A failed clone waits each of `retryDelays` in
 * turn and is tried again. On a failure the file stays as it was.
 */
export async function writeNativeNotices(
  root,
  entry,
  { run, retryDelays = DOWNLOAD_RETRY_DELAYS },
) {
  const work = await mkdtemp(join(tmpdir(), 'partsmith-native-notices-'))
  try {
    const clone = join(work, 'source')
    await cloneCheckedTag(entry, entry.notices.commit, clone, retryDelays)
    const text = await nativeNotices(root, run, clone, entry)
    const file = nativeNoticesFile(entry)
    await mkdir(dirname(join(root, file)), { recursive: true })
    await writeFile(join(root, file), text)
    return file
  } finally {
    await rm(work, { recursive: true, force: true })
  }
}

/** Runs the command with its arguments and returns its exit status. */
async function main(commandArguments) {
  if (commandArguments.length > 0) {
    process.stderr.write(`${USAGE}\n`)
    return 2
  }
  try {
    for (const entry of NATIVE_PACKAGES.filter((listed) => listed.notices !== undefined)) {
      // oxlint-disable-next-line no-await-in-loop -- one clone at a time keeps the disk to one source
      const file = await writeNativeNotices(repositoryRoot, entry, { run: runCommand })
      process.stdout.write(`Wrote ${file} from ${entry.source} at ${entry.tag}.\n`)
    }
    return 0
  } catch (error) {
    process.stderr.write(`${error.message}\n`)
    return 1
  }
}

// Runs as a command only when Node started this file, not when another module imports it.
const startedFile = process.argv[1]
if (startedFile !== undefined && (await realpath(startedFile)) === fileURLToPath(import.meta.url)) {
  process.exitCode = await main(process.argv.slice(2))
}
