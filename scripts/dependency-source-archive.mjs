// Writes the archives of the Rust crates and the npm packages that the desktop app's executables or the web build are
// compiled from, made from Cargo.lock and ui/pnpm-lock.yaml, as parts under Cloudflare Pages' file limit. The npm
// archives also hold the source of the code under MPL-2.0 in their native packages, and the desktop crate archive also
// holds tauri-cli and the crates of its own lock file. CI writes the web scope's into every web deployment. The
// release workflow attaches the desktop scope's to every release (docs/spec/delivery.md section 2.13, ADR 0034).
// Usage: node scripts/dependency-source-archive.mjs web|desktop <directory>
import { execFile } from 'node:child_process'
import { createHash } from 'node:crypto'
import { createReadStream } from 'node:fs'
import {
  copyFile,
  mkdir,
  mkdtemp,
  readFile,
  readdir,
  realpath,
  rename,
  rm,
  stat,
  writeFile,
} from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { basename, dirname, join, relative, sep } from 'node:path'
import process from 'node:process'
import { setTimeout as wait } from 'node:timers/promises'
import { fileURLToPath } from 'node:url'
import { promisify } from 'node:util'
import { gunzip } from 'node:zlib'

import {
  allowedLicenses,
  checkedRun,
  commandFailureMessage,
  compareNameAndVersion,
  licenseProblems,
  runCommand,
} from '../ui/scripts/check-licenses.mjs'
import {
  DESKTOP_TARGETS,
  WINDOWS_INSTALLER,
  cargoMetadata,
  excludedPaths,
  isExcluded,
  reachableIds,
  scopeCrates,
  workspacePackage,
} from '../ui/scripts/notices.mjs'

/**
 * The two scopes: the root crate and the triples whose crates each archive holds (2.9 input 1), and the platforms whose
 * npm packages it holds, as `{ os, cpu, libc }` with `libc` null where pnpm checks none.
 */
export const SCOPES = {
  web: {
    rootCrate: 'partsmith-wasm',
    triples: ['wasm32-unknown-emscripten'],
    platforms: [{ os: 'linux', cpu: 'x64', libc: 'glibc' }],
  },
  desktop: {
    rootCrate: 'partsmith-desktop',
    triples: DESKTOP_TARGETS.map((target) => target.triple),
    platforms: [
      { os: 'linux', cpu: 'x64', libc: 'glibc' },
      { os: 'win32', cpu: 'x64', libc: null },
      { os: 'darwin', cpu: 'arm64', libc: null },
    ],
  },
}

/** The size of every part but the last: 24 MiB. */
export const PART_SIZE = 25165824

/** The registry the command downloads the tarballs from. */
export const NPM_REGISTRY = 'https://registry.npmjs.org/'

/**
 * How long a download waits before its second and its third attempt, in milliseconds. Only a network error and an
 * answer of 408, 429 or 500 and above are tried again.
 */
export const DOWNLOAD_RETRY_DELAYS = [2000, 10000]

/** crates.io's download host, the `dl` of its index's `config.json`. Its storage serves a crate at `{name}/{name}-{version}.crate`. */
export const CRATE_REGISTRY = 'https://static.crates.io/crates/'

/**
 * The tauri-cli of 2.2, `WINDOWS_INSTALLER.tauriCli` of 2.9, and the SHA-256 of its `.crate`, which the crates.io index
 * records for that version. The desktop crate archive holds it and the crates of its own lock file.
 */
export const TAURI_CLI = {
  version: WINDOWS_INSTALLER.tauriCli,
  sha256: '7a08bf51957aad7b46276ba4242b2edeb80b6afae2b42b9547f9b9ae485db465',
}

/**
 * The npm packages of prebuilt native code, each at the version whose source was checked (Native packages, checked).
 * A package belongs to an entry when its name is `name` or starts with `name` followed by `-`, and its version is
 * `version`. `source` and `tag` name what it is built from. `commit` is the commit of `tag` when the archives hold the
 * tag's whole tree, with the crates of the lock file of `manifest`. `crates` are the crates under MPL-2.0 that it
 * compiles in, as `{ name, version, sha256 }` with the checksum that the lock file of its source records. `notices` is
 * there when its tarballs lack the notices of the crates it compiles in, which `ui/scripts/notices/native/` holds, as
 * `{ crate, commit }`: the crate of its source that builds it and the commit of `tag` that `ui/scripts/native-notices.mjs`
 * makes the file from.
 */
export const NATIVE_PACKAGES = [
  {
    name: '@oxfmt/binding',
    version: '0.72.0',
    source: 'https://github.com/oxc-project/oxc',
    tag: 'oxfmt_v0.72.0',
    crates: [],
    notices: { crate: 'oxfmt', commit: '2bd08ebe8f36fcf1954a675ffdeb4c6d0129f609' },
  },
  {
    name: '@oxlint/binding',
    version: '1.87.0',
    source: 'https://github.com/oxc-project/oxc',
    tag: 'oxlint_v1.87.0',
    crates: [],
    notices: { crate: 'oxlint', commit: '2bd08ebe8f36fcf1954a675ffdeb4c6d0129f609' },
  },
  {
    name: '@rolldown/binding',
    version: '1.2.13',
    source: 'https://github.com/rolldown/rolldown',
    tag: 'v1.2.13',
    crates: [],
    notices: {
      crate: 'rolldown_binding',
      commit: 'f7de35f45bcd4cb8e4b6d8d11e74fbfb20b03127',
    },
  },
  {
    name: '@tailwindcss/oxide',
    version: '4.3.3',
    source: 'https://github.com/tailwindlabs/tailwindcss',
    tag: 'v4.3.3',
    crates: [
      {
        name: 'bexpand',
        version: '1.2.0',
        sha256: '045d7d9db8390cf2c59f39f3bd138f1962ef616b096d1b9f5651c7acba19e5a7',
      },
    ],
    notices: { crate: 'tailwind-oxide', commit: 'c2b24dd15fed1c59dd521bd86082f520c9f5ad0d' },
  },
  {
    name: 'fsevents',
    version: '2.3.3',
    source: 'https://github.com/fsevents/fsevents',
    tag: 'v2.3.3',
    crates: [],
  },
  {
    name: 'lightningcss',
    version: '1.32.0',
    source: 'https://github.com/parcel-bundler/lightningcss',
    tag: 'v1.32.0',
    commit: '7f8a861bdee476fe90c89a8badeb3fd33a99c51a',
    manifest: 'node/Cargo.toml',
    crates: [],
  },
  {
    name: 'lightningcss',
    version: '1.33.0',
    source: 'https://github.com/parcel-bundler/lightningcss',
    tag: 'v1.33.0',
    commit: 'c6a0c3cebf3395635e61075d2c81a96a710d4910',
    manifest: 'node/Cargo.toml',
    crates: [],
  },
  {
    name: 'playwright-core',
    version: '1.64.0',
    source: 'https://github.com/microsoft/playwright',
    tag: 'v1.64.0',
    crates: [],
  },
]

const USAGE = 'usage: node scripts/dependency-source-archive.mjs web|desktop <directory>'

const repositoryRoot = fileURLToPath(new URL('..', import.meta.url))

// The machine that builds the web core. Its build scripts and procedural macros compile for it.
const BUILD_MACHINE_TRIPLE = 'x86_64-unknown-linux-gnu'

/** The source that Cargo lock files write for a crate from crates.io, the one source the archives' config.toml replaces. */
export const CRATES_IO_SOURCE = 'registry+https://github.com/rust-lang/crates.io-index'

// The file in which cargo vendor records the SHA-256 of each file of a crate and of the crate's package.
const CHECKSUM_FILE = '.cargo-checksum.json'

// The committed notices of the crates compiled into native packages whose tarballs lack them.
const NATIVE_NOTICES_FOLDER = 'ui/scripts/notices/native'

// The options that give GNU tar the same bytes on every run: the order, the times, the owners and the modes.
const TAR_OPTIONS = [
  '--sort=name',
  '--mtime=@0',
  '--owner=0',
  '--group=0',
  '--numeric-owner',
  '--format=pax',
  '--pax-option=exthdr.name=%d/PaxHeaders/%f,delete=atime,delete=ctime',
  '--mode=u+rwX,go+rX,go-w',
]

// The options that keep a user's or the system's Git configuration out of the bytes of `git archive` (2.13, Bytes).
const GIT_ARCHIVE_OPTIONS = [
  '-c',
  'core.autocrlf=false',
  '-c',
  'tar.umask=0002',
  '-c',
  'core.attributesFile=/dev/null',
]

// The size of a tar block: a header, or a piece of a file's contents.
const TAR_BLOCK = 512

// The file names of prebuilt native code in an npm tarball.
const PREBUILT_FILE_PATTERN = /\.(?:node|wasm|so|dylib|dll|exe)$/i

// The first bytes of prebuilt native code whatever its name: ELF, Mach-O (32 and 64 bits in either byte order, and a
// universal file), PE (`MZ`), WebAssembly and an `ar` archive of objects, a static library.
const PREBUILT_MAGIC_NUMBERS = [
  '7f454c46',
  'feedface',
  'feedfacf',
  'cefaedfe',
  'cffaedfe',
  'cafebabe',
  'cafebabf',
  '4d5a',
  '0061736d',
  '213c617263683e0a',
].map((hex) => Buffer.from(hex, 'hex'))

// How many tarballs the command downloads at the same time.
const DOWNLOADS_AT_ONCE = 8

const execute = promisify(execFile)
const gunzipBytes = promisify(gunzip)

/** Whether a download is tried again after the HTTP status `status`: 408, 429, or a server error of 500 and above. */
function isRetriedStatus(status) {
  return status === 408 || status === 429 || status >= 500
}

/** The folder of the Rust crates' archive of `scope`, whose name the archive takes. */
function cratesFolder(scope) {
  return `rust-crates-${scope}`
}

/** The folder of the npm packages' archive of `scope`, whose name the archive takes. */
function packagesFolder(scope) {
  return `npm-packages-${scope}`
}

/** The folder of the npm archive of `scope` that holds the source of its native packages. */
function sourcesFolder(scope) {
  return `npm-sources-${scope}`
}

/** The `{stem}` of a native package's entry: its name without the leading `@` and with `/` written as `-`. */
function stemOf(name) {
  return name.replace(/^@/, '').replace('/', '-')
}

/** The vendor folder name of a crate: `{name}-{version}`. */
function crateFolder({ name, version }) {
  return `${name}-${version}`
}

/** The names of the two archives of `scope`, crates first: `[rust-crates-{scope}.tar.gz, npm-packages-{scope}.tar]`. */
export function archiveNames(scope) {
  return [`${cratesFolder(scope)}.tar.gz`, `${packagesFolder(scope)}.tar`]
}

/**
 * The exact text of `{folder}/config.toml` for the crates that build `subject` without the network: the text of 2.13
 * with `{folder}/vendor` as the directory and `subject` in its first line.
 */
export function vendorConfig(folder, subject) {
  return [
    `# Set CARGO_HOME to this folder to build ${subject} from these crates without the network (docs/spec/delivery.md section 2.13).`,
    '[source.crates-io]',
    'replace-with = "vendored-sources"',
    '',
    '[source.vendored-sources]',
    `directory = "${folder}/vendor"`,
    '',
    '[net]',
    'offline = true',
    '',
  ].join('\n')
}

/** The exact text of `rust-crates-{scope}/config.toml`: `vendorConfig('rust-crates-{scope}', 'Partsmith')`. */
export function cargoConfig(scope) {
  return vendorConfig(cratesFolder(scope), 'Partsmith')
}

/** The values of a YAML flow sequence such as `[x64]` or `['!win32']`, without quotes. */
function flowSequence(text) {
  return text
    .replace(/^\[|\]$/g, '')
    .split(',')
    .map((value) => value.trim().replace(/^'(.*)'$|^"(.*)"$/, '$1$2'))
    .filter((value) => value !== '')
}

/** The `integrity` of a YAML flow mapping such as `{integrity: sha512-...}`, or null when it has none. */
function integrityOf(resolution) {
  const integrity = resolution
    .replace(/^\{|\}$/g, '')
    .split(', ')
    .find((member) => member.startsWith('integrity: '))
  return integrity === undefined ? null : integrity.slice('integrity: '.length)
}

/**
 * Every entry of the `packages` section of a pnpm lock file's text (lockfileVersion 9.0), in the file's order, as
 * `{ key, name, version, integrity, os, cpu, libc }`, with null for a field the entry lacks.
 */
export function lockPackages(lockText) {
  const entries = []
  let inPackages = false
  for (const line of lockText.split(/\r?\n/)) {
    // A line that starts in the first column opens a section, such as `packages:` or `snapshots:`.
    if (/^\S/.test(line)) {
      inPackages = line.trimEnd() === 'packages:'
      continue
    }
    if (!inPackages) {
      continue
    }
    // An entry's key is indented by two spaces and its fields by four. Deeper lines belong to a field's value.
    const keyLine = /^ {2}(\S.*):$/.exec(line)
    if (keyLine !== null) {
      const key = keyLine[1].replace(/^'(.*)'$/, '$1')
      const at = key.lastIndexOf('@')
      entries.push({
        key,
        name: key.slice(0, at),
        version: key.slice(at + 1),
        integrity: null,
        os: null,
        cpu: null,
        libc: null,
      })
      continue
    }
    const fieldLine = /^ {4}(resolution|os|cpu|libc): (.*)$/.exec(line)
    if (fieldLine !== null && entries.length > 0) {
      const [, field, value] = fieldLine
      Object.assign(
        entries.at(-1),
        field === 'resolution'
          ? { integrity: integrityOf(value) }
          : { [field]: flowSequence(value) },
      )
    }
  }
  return entries
}

/**
 * Whether a lock entry's list of operating systems, CPUs or C libraries allows `value`: a list allows a value it
 * holds, and a list of values after a `!` every value but those, as pnpm reads them. No list allows every value.
 */
function allows(list, value) {
  if (list === null) {
    return true
  }
  return (
    list.includes(value) || list.every((listed) => listed.startsWith('!') && listed !== `!${value}`)
  )
}

/** The entries of `packages` whose `os`, `cpu` and `libc` lists allow one of `platforms`, in their order. */
export function platformPackages(packages, platforms) {
  return packages.filter((listed) =>
    platforms.some(
      (platform) =>
        allows(listed.os, platform.os) &&
        allows(listed.cpu, platform.cpu) &&
        (platform.libc === null || allows(listed.libc, platform.libc)),
    ),
  )
}

/** The path of a package's tarball under the registry: `{name}/-/{base}-{version}.tgz`. */
export function tarballPath(name, version) {
  return `${name}/-/${name.replace(/^@[^/]+\//, '')}-${version}.tgz`
}

/** null when `bytes` have the hash `integrity` names; otherwise the integrity of `bytes` with the same algorithm. */
export function integrityMismatch(bytes, integrity) {
  const algorithm = integrity.slice(0, integrity.indexOf('-'))
  const actual = `${algorithm}-${createHash(algorithm).update(bytes).digest('base64')}`
  return actual === integrity ? null : actual
}

/** Whether the lock entry `listed` has an `os`, `cpu` or `libc` list: it is built for some platforms only. */
function hasPlatformList(listed) {
  return listed.os !== null || listed.cpu !== null || listed.libc !== null
}

/** Whether the lock entry `listed` belongs to the table entry `entry` (Native packages, 2.13). */
function belongsTo(listed, entry) {
  return (
    (listed.name === entry.name || listed.name.startsWith(`${entry.name}-`)) &&
    listed.version === entry.version
  )
}

/** The native-package message of section 4 for the lock entry `listed`. */
function nativePackageMessage(listed) {
  return `npm package ${listed.name} ${listed.version} holds prebuilt native code, and NATIVE_PACKAGES of scripts/dependency-source-archive.mjs records no check of its source at that version. Check its source for code under MPL-2.0 or a copyleft license that it compiles in, and record the result there and in docs/spec/delivery.md section 2.13.`
}

/**
 * The entries of `nativePackages` that the native packages `native` belong to, each once, in the table's order. Throws
 * the native-package message for the first of `native`, in their order, that belongs to none.
 */
function recordedEntries(native, nativePackages) {
  const unrecorded = native.find(
    (listed) => !nativePackages.some((entry) => belongsTo(listed, entry)),
  )
  if (unrecorded !== undefined) {
    throw new Error(nativePackageMessage(unrecorded))
  }
  return nativePackages.filter((entry) => native.some((listed) => belongsTo(listed, entry)))
}

/**
 * The entries of `nativePackages` that the packages of `packages` with an `os`, `cpu` or `libc` list belong to, each
 * once, in the table's order. Throws an `Error` with the native-package message of section 4 for the first such
 * package, in the order of `packages`, that belongs to none.
 */
export function nativeEntries(packages, nativePackages) {
  return recordedEntries(packages.filter(hasPlatformList), nativePackages)
}

/** The text of the NUL-terminated field of `length` bytes at `offset` of `block`. */
function fieldText(block, offset, length) {
  const field = block.subarray(offset, offset + length)
  const end = field.indexOf(0)
  return field.subarray(0, end === -1 ? field.length : end).toString('utf8')
}

/** The `path` of a pax extended header's records, `{length} {key}={value}` each followed by LF, or null. */
function paxPath(records) {
  let offset = 0
  let path = null
  while (offset < records.length) {
    const space = records.indexOf(0x20, offset)
    const length = Number.parseInt(records.subarray(offset, space).toString('latin1'), 10)
    if (space === -1 || !(length > space - offset)) {
      break
    }
    const record = records.subarray(space + 1, offset + length - 1).toString('utf8')
    if (record.startsWith('path=')) {
      path = record.slice('path='.length)
    }
    offset += length
  }
  return path
}

/**
 * The entries of the tar `bytes` in their order, as `{ path, regular, contents }`. The path is the pax `path` or the
 * GNU long name that comes before an entry, or else its ustar name after its prefix.
 */
function tarEntries(bytes) {
  const entries = []
  let longPath = null
  let offset = 0
  while (offset + TAR_BLOCK <= bytes.length) {
    const header = bytes.subarray(offset, offset + TAR_BLOCK)
    // The archive ends with blocks of zeros.
    if (header.every((byte) => byte === 0)) {
      break
    }
    const size = Number.parseInt(fieldText(header, 124, 12).trim() || '0', 8)
    const type = String.fromCharCode(header[156])
    const contents = bytes.subarray(offset + TAR_BLOCK, offset + TAR_BLOCK + size)
    offset += TAR_BLOCK * (1 + Math.ceil(size / TAR_BLOCK))
    if (type === 'x') {
      longPath = paxPath(contents) ?? longPath
    } else if (type === 'L') {
      longPath = fieldText(contents, 0, contents.length)
    } else if (type !== 'g') {
      const isUstar = header.subarray(257, 263).toString('latin1') === 'ustar\0'
      const prefix = isUstar ? fieldText(header, 345, 155) : ''
      const name = fieldText(header, 0, 100)
      const path = longPath ?? (prefix === '' ? name : `${prefix}/${name}`)
      entries.push({ path, regular: ['0', '\0', '7'].includes(type), contents })
      longPath = null
    }
  }
  return entries
}

/** The `license` string of the `package.json` file `manifest`, or `Unknown` when it has none or is not valid JSON. */
function manifestLicense(manifest) {
  try {
    const { license } = JSON.parse(manifest.contents.toString('utf8'))
    return typeof license === 'string' ? license : 'Unknown'
  } catch {
    return 'Unknown'
  }
}

/** Whether the regular file `entry` of a tarball holds prebuilt native code: by its name or by its first bytes. */
function isPrebuilt(entry) {
  return (
    PREBUILT_FILE_PATTERN.test(entry.path) ||
    PREBUILT_MAGIC_NUMBERS.some((magic) => entry.contents.subarray(0, magic.length).equals(magic))
  )
}

/**
 * What the license check and the native-package check read from an npm tarball's `bytes`: the `license` of its
 * `package.json` one folder deep (the first in the tarball's order), `Unknown` without one, and whether it holds a
 * prebuilt file.
 */
async function tarballFacts(bytes) {
  const files = tarEntries(await gunzipBytes(bytes)).filter((entry) => entry.regular)
  const manifest = files.find((entry) => /^[^/]+\/package\.json$/.test(entry.path))
  return {
    license: manifest === undefined ? 'Unknown' : manifestLicense(manifest),
    prebuilt: files.some(isPrebuilt),
  }
}

/**
 * The license of the npm tarball at `file`: the `license` string of its `package.json`, or `Unknown` (2.13). It reads
 * the tarball in Node (`zlib`'s gunzip, then the tar headers: the ustar name with its prefix, or the pax `path` or GNU
 * long name before it) and runs no command.
 */
export async function tarballLicense(file) {
  return (await tarballFacts(await readFile(file))).license
}

/**
 * Every `[[package]]` of a Cargo lock file's text, in the file's order, as `{ name, version, source }`, with `source`
 * null for a package that has none (a member of the lock file's workspace).
 */
export function lockCrates(lockText) {
  const crates = []
  let current = null
  for (const line of lockText.split(/\r?\n/)) {
    if (line.trim() === '[[package]]') {
      current = { name: null, version: null, source: null }
      crates.push(current)
      continue
    }
    // Any other table, such as `[metadata]`, ends the package before it.
    if (line.startsWith('[')) {
      current = null
      continue
    }
    const field = /^(name|version|source) = "(.*)"$/.exec(line)
    if (current !== null && field !== null) {
      current[field[1]] = field[2]
    }
  }
  return crates
}

/**
 * Throws the tool-lock source message of section 4 for the first of `crates`, packages of the Cargo lock file of
 * `subject`, that comes from a source other than crates.io.
 */
function requireCratesIo(crates, subject) {
  const foreign = crates.find((crate) => crate.source !== null && crate.source !== CRATES_IO_SOURCE)
  if (foreign !== undefined) {
    throw new Error(
      `Rust crate ${foreign.name} ${foreign.version} in the Cargo.lock of ${subject} comes from ${foreign.source}, not from crates.io. The config.toml of the dependency source archives replaces only crates.io, and an offline build would not find it. Nothing was written (docs/spec/delivery.md section 2.13).`,
    )
  }
}

/** Resolves to the values of `promises` once all have settled, or rejects with the first failure in their order. */
async function inOrder(promises) {
  const results = await Promise.allSettled(promises)
  const failure = results.find((result) => result.status === 'rejected')
  if (failure !== undefined) {
    throw failure.reason
  }
  return results.map((result) => result.value)
}

/**
 * The values of `writes`, promises that each resolve to the name or names they wrote into `directory`. When one
 * rejects, the names the others wrote are removed once all have settled. The first failure in their order is the one
 * that rejects. `directory` is then left as it was.
 */
async function allOrNothing(directory, writes) {
  const results = await Promise.allSettled(writes)
  const failure = results.find((result) => result.status === 'rejected')
  if (failure === undefined) {
    return results.map((result) => result.value)
  }
  const written = results
    .filter((result) => result.status === 'fulfilled')
    .flatMap((result) => result.value)
  await Promise.all(written.map((name) => rm(join(directory, name), { force: true })))
  throw failure.reason
}

/** Writes the bytes of `file` from `start` up to `end` to the new file `path`, which it removes when writing fails. */
async function writePart(file, path, start, end) {
  const contents = end > start ? createReadStream(file, { start, end: end - 1 }) : ''
  try {
    await writeFile(path, contents, { flag: 'wx' })
  } catch (error) {
    // A file that was there before is not this part's to remove.
    if (error.code !== 'EEXIST') {
      await rm(path, { force: true })
    }
    throw error
  }
}

/**
 * Writes `file` into `directory` as parts of `partSize` bytes named `{base name of file}.part-{NN}`, the last one
 * shorter, and resolves to their names in order.
 */
export async function writeParts(file, directory, partSize = PART_SIZE) {
  const { size } = await stat(file)
  const count = Math.max(1, Math.ceil(size / partSize))
  const names = Array.from(
    { length: count },
    (_, i) => `${basename(file)}.part-${String(i).padStart(2, '0')}`,
  )
  return allOrNothing(
    directory,
    names.map(async (name, i) => {
      await writePart(file, join(directory, name), i * partSize, Math.min(size, (i + 1) * partSize))
      return name
    }),
  )
}

/** The line the command prints for one result of `writeDependencySources` written into `directory` (Output, 2.13). */
export function summaryLine({ archive, bytes, sha256, lastPart }, directory) {
  return `Wrote ${archive} to ${directory}: ${bytes} bytes, SHA-256 ${sha256}, last part ${lastPart}.`
}

/**
 * Runs `git`, `tar` or `gzip` in `directory` with `LC_ALL=C` and resolves to its standard output. A failure rejects
 * with the message of section 4.
 */
async function runDirectly(command, commandArguments, directory) {
  try {
    const { stdout } = await execute(command, commandArguments, {
      cwd: directory,
      env: { ...process.env, LC_ALL: 'C' },
    })
    return stdout
  } catch (error) {
    const failure = { status: error.code ?? error.signal, stderr: error.stderr || error.message }
    throw new Error(commandFailureMessage(command, commandArguments, failure), { cause: error })
  }
}

/** Rejects with the message of section 4 unless `directory` is a folder. */
async function requireDirectory(directory) {
  const missing = `${directory} does not exist. Make it before writing the dependency source archives into it (docs/spec/delivery.md section 2.13).`
  try {
    if ((await stat(directory)).isDirectory()) {
      return
    }
  } catch (error) {
    if (!['ENOENT', 'ENOTDIR'].includes(error.code)) {
      throw error
    }
  }
  throw new Error(missing)
}

/** Rejects with the message of section 4 unless the first `tar` on PATH is GNU tar. */
async function requireGnuTar() {
  const version = await runDirectly('tar', ['--version'], process.cwd())
  if (!/\bGNU tar\b/.test(version.split('\n')[0])) {
    throw new Error(
      'tar --version does not name GNU tar, which the dependency source archives need for the same bytes on every run. Put GNU tar first on PATH (docs/spec/delivery.md section 2.13).',
    )
  }
}

/**
 * Rejects with the temporary-folder message of section 4 when Node's temporary folder is the repository at `root` or
 * inside it, where Cargo would take the extracted tauri-cli for a member of Partsmith's workspace.
 */
async function requireTemporaryFolderOutside(root) {
  const [folder, repository] = await Promise.all([realpath(tmpdir()), realpath(root)])
  if (folder === repository || folder.startsWith(`${repository}${sep}`)) {
    throw new Error(
      `The temporary folder ${folder} is inside ${repository}, where Cargo would take tauri-cli for a member of Partsmith's workspace. Set TMPDIR to a folder outside the repository (docs/spec/delivery.md section 2.13).`,
    )
  }
}

/**
 * The npm packages of `scope` from the lock file of `root`. Rejects before any download when one of them has no
 * integrity, or when one with an `os`, `cpu` or `libc` list belongs to no entry of `nativePackages`.
 */
async function scopePackages(root, scope, nativePackages) {
  const lockText = await readFile(join(root, 'ui', 'pnpm-lock.yaml'), 'utf8')
  const packages = platformPackages(lockPackages(lockText), SCOPES[scope].platforms)
  const unchecked = packages.find((listed) => listed.integrity === null)
  if (unchecked !== undefined) {
    throw new Error(
      `npm package ${unchecked.key} has no integrity in ui/pnpm-lock.yaml, so its tarball cannot be checked (docs/spec/delivery.md section 2.13).`,
    )
  }
  nativeEntries(packages, nativePackages)
  return packages
}

/**
 * Rejects with the build-machine message of section 4 when a registry crate that the web core's build compiles for
 * the machine that builds it, a procedural macro, a build dependency or a crate these reach, is not in `crates`.
 */
async function requireBuildMachineCrates(root, run, rootCrate, crates) {
  const metadata = await cargoMetadata(root, run, BUILD_MACHINE_TRIPLE)
  const packagesById = new Map(metadata.packages.map((listed) => [listed.id, listed]))
  const nodesById = new Map(metadata.resolve.nodes.map((node) => [node.id, node]))
  const built = [...reachableIds(metadata, [workspacePackage(metadata, rootCrate).id])]
  const procMacros = built.filter((id) =>
    packagesById.get(id).targets.some((target) => target.kind.includes('proc-macro')),
  )
  const buildDependencies = built.flatMap((id) =>
    (nodesById.get(id)?.deps ?? [])
      .filter((dependency) => dependency.dep_kinds.some(({ kind }) => kind === 'build'))
      .map((dependency) => dependency.pkg),
  )
  const inScope = new Set(crates.map((crate) => `${crate.name} ${crate.version}`))
  const [missing] = [...reachableIds(metadata, [...procMacros, ...buildDependencies])]
    .map((id) => packagesById.get(id))
    .filter((listed) => listed.source !== null && !inScope.has(`${listed.name} ${listed.version}`))
    .toSorted(compareNameAndVersion)
  if (missing !== undefined) {
    throw new Error(
      `Rust crate ${missing.name} ${missing.version} is built for the machine that builds the web core, and the web crate set of docs/spec/delivery.md section 2.9 input 1 does not hold it, so ${archiveNames('web')[0]} would lack it. Nothing was written.`,
    )
  }
}

/**
 * One request for `url` with `fetch`. Resolves to `{ bytes }` for a successful answer, and otherwise to
 * `{ failure, retry }`: the status or the error's message, and whether the download is tried again for it.
 */
async function request(url, fetch) {
  let response
  try {
    response = await fetch(url)
    if (response.ok) {
      return { bytes: Buffer.from(await response.arrayBuffer()) }
    }
  } catch (error) {
    // No answer, or the connection broke while the body came.
    return { failure: error.message, retry: true }
  }
  await response.body?.cancel()
  return { failure: response.status, retry: isRetriedStatus(response.status) }
}

/**
 * The bytes at `url`, requested with `fetch` and tried again after each of `retryDelays` while a request fails in a
 * way that is retried. Rejects with the download message of section 4 for `subject` after the last attempt.
 */
async function download(url, subject, { fetch, retryDelays }) {
  const answer = await request(url, fetch)
  if (answer.bytes !== undefined) {
    return answer.bytes
  }
  if (!answer.retry || retryDelays.length === 0) {
    throw new Error(`${subject}: ${url} answered ${answer.failure}.`)
  }
  await wait(retryDelays[0])
  return download(url, subject, { fetch, retryDelays: retryDelays.slice(1) })
}

/**
 * The bytes of the `.crate` of `crate`, `{ name, version, sha256 }`, from `crateRegistry`, tried again as `download`
 * does and checked against its SHA-256.
 */
async function downloadCrate(crate, { crateRegistry, ...downloads }) {
  const subject = `Rust crate ${crate.name} ${crate.version}`
  const url = `${crateRegistry}${crate.name}/${crateFolder(crate)}.crate`
  const bytes = await download(url, subject, downloads)
  const actual = createHash('sha256').update(bytes).digest('hex')
  if (actual !== crate.sha256) {
    throw new Error(
      `${subject}: ${url} sent a file whose SHA-256 is ${actual}, not ${crate.sha256}, which scripts/dependency-source-archive.mjs records. Nothing was written.`,
    )
  }
  return bytes
}

/**
 * Downloads, checks and extracts the `.crate` of `tauriCli` into `work` as `tauri-cli-{version}/`, and resolves to
 * that folder, the crate's SHA-256 and the crates of its own lock file that come from crates.io. Rejects when a crate
 * of that lock file comes from another source.
 */
async function extractTauriCli(work, tauriCli, downloads) {
  const crate = { name: 'tauri-cli', ...tauriCli }
  const file = `${crateFolder(crate)}.crate`
  await writeFile(join(work, file), await downloadCrate(crate, downloads))
  await runDirectly('tar', ['--extract', '--gzip', `--file=${file}`], work)
  await rm(join(work, file))
  const folder = join(work, crateFolder(crate))
  const locked = lockCrates(await readFile(join(folder, 'Cargo.lock'), 'utf8'))
  requireCratesIo(locked, `tauri-cli ${tauriCli.version}`)
  return {
    folder,
    sha256: tauriCli.sha256,
    crates: locked.filter((listed) => listed.source !== null),
  }
}

/**
 * Turns the vendored crate in `folder` into a stub that Cargo can resolve: its manifest, its checksum file with no
 * files, and an empty file at the path of each target of `listed`, its package in `cargo metadata` (none when absent).
 */
async function stubCrate(folder, listed) {
  const manifest = await readFile(join(folder, 'Cargo.toml'))
  const checksum = JSON.parse(await readFile(join(folder, CHECKSUM_FILE), 'utf8'))
  const targets = (listed?.targets ?? []).map((target) =>
    relative(dirname(listed.manifest_path), target.src_path),
  )
  await rm(folder, { recursive: true })
  const files = [
    ['Cargo.toml', manifest],
    [CHECKSUM_FILE, JSON.stringify({ files: {}, package: checksum.package })],
    ...targets.map((path) => [path, '']),
  ]
  await Promise.all(
    files.map(async ([path, contents]) => {
      await mkdir(dirname(join(folder, path)), { recursive: true })
      await writeFile(join(folder, path), contents)
    }),
  )
}

/**
 * Removes `path` (a file, or a folder ending in `/`, as EXCLUDED_LICENSE_FILES writes it) from the vendored crate in
 * `folder` and from `files` of its checksum file, which keeps `package`.
 */
async function leaveOut(folder, path) {
  await rm(join(folder, path), { recursive: true, force: true })
  const checksum = JSON.parse(await readFile(join(folder, CHECKSUM_FILE), 'utf8'))
  const files = Object.entries(checksum.files).filter(([listed]) => !isExcluded(listed, [path]))
  await writeFile(
    join(folder, CHECKSUM_FILE),
    JSON.stringify({ ...checksum, files: Object.fromEntries(files) }),
  )
}

/** The paths of the regular files below `folder`, relative to it with `/`, in byte order. */
async function regularFiles(folder) {
  const entries = await readdir(folder, { recursive: true, withFileTypes: true })
  return entries
    .filter((entry) => entry.isFile())
    .map((entry) => relative(folder, join(entry.parentPath, entry.name)).split(sep).join('/'))
    .toSorted((first, second) => Buffer.compare(Buffer.from(first), Buffer.from(second)))
}

/** The SHA-256 of the file at `path` in hexadecimal. */
async function sha256Of(path) {
  const hash = createHash('sha256')
  for await (const chunk of createReadStream(path)) {
    hash.update(chunk)
  }
  return hash.digest('hex')
}

/**
 * Moves the extracted crate `tool` into `vendor` and writes its checksum file as `cargo vendor` writes one: the
 * SHA-256 of every regular file, by path in byte order, and the SHA-256 of its `.crate` as `package`.
 */
async function vendorTool(tool, vendor) {
  const folder = join(vendor, basename(tool.folder))
  await rename(tool.folder, folder)
  const paths = await regularFiles(folder)
  const hashes = await Promise.all(paths.map((path) => sha256Of(join(folder, path))))
  const files = Object.fromEntries(paths.map((path, i) => [path, hashes[i]]))
  await writeFile(join(folder, CHECKSUM_FILE), JSON.stringify({ files, package: tool.sha256 }))
}

/**
 * Writes `rust-crates-{scope}` into `work` with `config.toml` and `vendor/`: every crate of Cargo.lock vendored, and
 * for `desktop` every crate of the lock file of `tool`, the extracted tauri-cli (null for `web`). Those of `crates`
 * and of the tool's lock file stay whole, without the files EXCLUDED_LICENSE_FILES names, and every other one becomes
 * a stub. The tool's own folder moves into `vendor/` last.
 */
async function vendorCrates(root, run, scope, work, crates, tool) {
  const folder = join(work, cratesFolder(scope))
  const vendor = join(folder, 'vendor')
  const sync = tool === null ? [] : ['--sync', join(tool.folder, 'Cargo.toml')]
  await checkedRun(run, 'cargo', ['vendor', '--locked', '--versioned-dirs', ...sync, vendor], root)
  const metadata = await cargoMetadata(root, run)
  const registryPackages = new Map(
    metadata.packages
      .filter((listed) => listed.source !== null)
      .map((listed) => [crateFolder(listed), listed]),
  )
  const whole = new Map(
    [...crates, ...(tool?.crates ?? [])].map((crate) => [crateFolder(crate), crate]),
  )
  const stubbed = (await readdir(vendor)).filter((name) => !whole.has(name))
  await Promise.all(
    stubbed.map((name) => stubCrate(join(vendor, name), registryPackages.get(name))),
  )
  const excluded = [...whole].flatMap(([name, crate]) =>
    excludedPaths(crate.name).map((path) => leaveOut(join(vendor, name), path)),
  )
  await Promise.all(excluded)
  if (tool !== null) {
    await vendorTool(tool, vendor)
  }
  await writeFile(join(folder, 'config.toml'), cargoConfig(scope))
}

/**
 * Downloads the tarball of `listed` from `registry`, tried again as `download` does, checks it against its integrity,
 * writes it into `folder` at its `tarballPath` and resolves to what `tarballFacts` reads from it.
 */
async function saveTarball(listed, folder, { registry, ...downloads }) {
  const url = `${registry}${tarballPath(listed.name, listed.version)}`
  const bytes = await download(url, `npm package ${listed.name} ${listed.version}`, downloads)
  const actual = integrityMismatch(bytes, listed.integrity)
  if (actual !== null) {
    throw new Error(
      `npm package ${listed.name} ${listed.version}: ${url} sent a tarball whose integrity is ${actual}, not ${listed.integrity}, which ui/pnpm-lock.yaml records. Nothing was written.`,
    )
  }
  const path = join(folder, tarballPath(listed.name, listed.version))
  await mkdir(dirname(path), { recursive: true })
  await writeFile(path, bytes)
  return tarballFacts(bytes)
}

/**
 * Downloads the tarball of each of `packages` into `folder` at its `tarballPath`, a few at a time, with
 * `downloads`, `{ registry, fetch, retryDelays }`, and resolves to what `tarballFacts` reads from each, in the order
 * of `packages`. The first failure stops the downloads that have not started and rejects once the others have
 * settled.
 */
async function downloadPackages(packages, folder, downloads) {
  const facts = []
  let next = 0
  let failure
  const downloadNext = async () => {
    while (failure === undefined && next < packages.length) {
      const i = next++
      try {
        // oxlint-disable-next-line no-await-in-loop -- each of the DOWNLOADS_AT_ONCE loops saves one tarball at a time
        facts[i] = await saveTarball(packages[i], folder, downloads)
      } catch (error) {
        failure ??= error
      }
    }
  }
  await Promise.all(Array.from({ length: DOWNLOADS_AT_ONCE }, downloadNext))
  if (failure !== undefined) {
    throw failure
  }
  return facts
}

/**
 * Rejects with one line per package of `packages` whose license, as `facts` read it from its tarball, the allow list
 * of `root`'s deny.toml does not allow (2.10), in the order of `packages`.
 */
async function requireAllowedLicenses(root, packages, facts) {
  const allowed = allowedLicenses(await readFile(join(root, 'deny.toml'), 'utf8'))
  const licensed = packages.map((listed, i) => ({ ...listed, license: facts[i].license }))
  const problems = licenseProblems(licensed, allowed)
  if (problems.length > 0) {
    throw new Error(problems.join('\n'))
  }
}

/** Packs the folders `folders` of `directory` into the tar `file` with GNU tar, the same bytes on every run. */
async function packFolders(directory, file, folders) {
  await runDirectly('tar', [...TAR_OPTIONS, '--create', `--file=${file}`, ...folders], directory)
}

/**
 * Clones the tag of `entry` into `clone` with a shallow `git clone`, tried again after each of `retryDelays` with the
 * partial clone removed first. Rejects with the command message of the last attempt.
 */
async function cloneTag(entry, clone, retryDelays) {
  await rm(clone, { recursive: true, force: true })
  const cloneArguments = ['clone', '--quiet', '--depth', '1', '--branch', entry.tag]
  try {
    await runDirectly('git', [...cloneArguments, entry.source, clone], dirname(clone))
  } catch (error) {
    if (retryDelays.length === 0) {
      throw error
    }
    await wait(retryDelays[0])
    await cloneTag(entry, clone, retryDelays.slice(1))
  }
}

/**
 * Clones the tag of the table entry `entry` into the new folder `clone`, as `cloneTag` does, and checks that the tag
 * names `commit`, the commit the entry pins. Rejects with the tag-commit message of section 4 when it names another.
 */
export async function cloneCheckedTag(entry, commit, clone, retryDelays) {
  await mkdir(dirname(clone), { recursive: true })
  await cloneTag(entry, clone, retryDelays)
  const tagged = (
    await runDirectly('git', ['-C', clone, 'rev-parse', `${entry.tag}^{commit}`], clone)
  ).trim()
  if (tagged !== commit) {
    throw new Error(
      `The tag ${entry.tag} of ${entry.source} names the commit ${tagged}, not ${commit}, which NATIVE_PACKAGES of scripts/dependency-source-archive.mjs pins for ${entry.name} ${entry.version}. Upstream may have moved the tag; nothing was written (docs/spec/delivery.md section 2.13).`,
    )
  }
}

/**
 * Clones the tag of `entry` into `{work}/clones/{stem}-{version}`, checks that it names the commit the entry pins,
 * and writes `{stem}-{version}-source.tar.gz` of its tree into `sources`. Resolves to the clone's folder.
 */
async function archiveTag(work, sources, entry, retryDelays) {
  const name = `${stemOf(entry.name)}-${entry.version}`
  const clone = join(work, 'clones', name)
  await cloneCheckedTag(entry, entry.commit, clone, retryDelays)
  const tar = join(sources, `${name}-source.tar`)
  const archiveArguments = ['archive', '--format=tar', `--prefix=${name}/`, `--output=${tar}`]
  await runDirectly(
    'git',
    ['-C', clone, ...GIT_ARCHIVE_OPTIONS, ...archiveArguments, entry.tag],
    work,
  )
  await runDirectly('gzip', ['-9', '-n', basename(tar)], sources)
  return clone
}

/**
 * Writes the source of the entries of `group`, the entries with a `commit` and one name, into `sources`: the tree of
 * each tag, and `{stem}-crates.tar.gz` with `config.toml` and every crate of their lock files, vendored with one
 * `cargo vendor` sent through `run` in `root`.
 */
async function archiveWholeSource(root, run, work, sources, group, retryDelays) {
  const clones = await inOrder(group.map((entry) => archiveTag(work, sources, entry, retryDelays)))
  const locks = await Promise.all(
    clones.map((clone) => readFile(join(clone, 'Cargo.lock'), 'utf8')),
  )
  for (const [i, entry] of group.entries()) {
    requireCratesIo(lockCrates(locks[i]), `${entry.name} ${entry.version}`)
  }
  const [first, ...others] = group.map((entry, i) => join(clones[i], entry.manifest))
  const folder = `${stemOf(group[0].name)}-crates`
  const vendor = join(work, folder, 'vendor')
  const manifests = [
    '--manifest-path',
    first,
    ...others.flatMap((manifest) => ['--sync', manifest]),
  ]
  await checkedRun(
    run,
    'cargo',
    ['vendor', '--locked', '--versioned-dirs', ...manifests, vendor],
    root,
  )
  await writeFile(join(work, folder, 'config.toml'), vendorConfig(folder, group[0].name))
  await packFolders(work, `${basename(sources)}/${folder}.tar`, [folder])
  await runDirectly('gzip', ['-9', '-n', `${folder}.tar`], sources)
  await Promise.all(
    [join(work, folder), ...clones].map((path) => rm(path, { recursive: true, force: true })),
  )
}

/** Downloads and checks the `.crate` of `crate` and writes it into `sources` as `{name}-{version}.crate`. */
async function saveCrate(crate, sources, downloads) {
  await writeFile(
    join(sources, `${crateFolder(crate)}.crate`),
    await downloadCrate(crate, downloads),
  )
}

/** The committed notices file of a table entry with `notices`, relative to the repository root. */
export function nativeNoticesFile(entry) {
  return `${NATIVE_NOTICES_FOLDER}/${stemOf(entry.name)}-${entry.version}.txt`
}

/**
 * Copies the committed notices file of `entry` from `root` into `sources` as `{stem}-{version}-notices.txt`, or
 * rejects with the native-notices message of section 4 when it is missing.
 */
async function copyNotices(root, sources, entry) {
  const committed = nativeNoticesFile(entry)
  const copy = `${stemOf(entry.name)}-${entry.version}-notices.txt`
  try {
    await copyFile(join(root, committed), join(sources, copy))
  } catch (error) {
    if (error.code === 'ENOENT') {
      throw new Error(
        `${committed} is missing. Write it from the source of ${entry.name} ${entry.version} with node scripts/native-notices.mjs in ui/ (docs/spec/delivery.md section 2.13).`,
        { cause: error },
      )
    }
    throw error
  }
}

/** Whether a table entry asks the archives for something: a whole source, crates or notices. */
function asksForSource(entry) {
  return entry.commit !== undefined || entry.crates.length > 0 || entry.notices !== undefined
}

/**
 * Writes `npm-sources-{scope}` into `work` with what `entries` ask for (Source of the native packages, 2.13), with
 * `downloads`, `{ crateRegistry, fetch, retryDelays }`. Resolves to the folders the npm archive holds after
 * `npm-packages-{scope}`: that one, or none when no entry asks for anything.
 */
async function writeNativeSources(root, run, scope, work, entries, downloads) {
  if (!entries.some(asksForSource)) {
    return []
  }
  const sources = join(work, sourcesFolder(scope))
  await mkdir(sources)
  const wholeSources = Map.groupBy(
    entries.filter((entry) => entry.commit !== undefined),
    (entry) => entry.name,
  )
  const crates = new Map(
    entries.flatMap((entry) => entry.crates).map((crate) => [crateFolder(crate), crate]),
  )
  await inOrder([
    ...[...wholeSources.values()].map((group) =>
      archiveWholeSource(root, run, work, sources, group, downloads.retryDelays),
    ),
    ...[...crates.values()].map((crate) => saveCrate(crate, sources, downloads)),
    ...entries
      .filter((entry) => entry.notices !== undefined)
      .map((entry) => copyNotices(root, sources, entry)),
  ])
  return [sourcesFolder(scope)]
}

/** The size and SHA-256 of the file at `path`. */
async function sizeAndSha256(path) {
  return { bytes: (await stat(path)).size, sha256: await sha256Of(path) }
}

/**
 * Writes the parts of both archives of `scope` for the repository at `root` into `directory`, downloading each tarball
 * from `registry` and each crate from `crateRegistry` with `fetch` and sending every `cargo` command through `run`. A
 * download that fails with a network error or a retried status, and a failed `git clone`, wait each of `retryDelays`
 * in turn and try again.
 * `nativePackages` and `tauriCli` take the place of NATIVE_PACKAGES and TAURI_CLI. Resolves to one
 * `{ archive, bytes, sha256, lastPart }` per archive, crates first, where `bytes` is the archive's size, `sha256` its
 * SHA-256 in hexadecimal and `lastPart` the name of its last part. Rejects with the message of section 4 and leaves
 * `directory` as it was on any failure.
 */
export async function writeDependencySources(
  root,
  scope,
  directory,
  {
    registry,
    crateRegistry,
    fetch,
    run,
    retryDelays = DOWNLOAD_RETRY_DELAYS,
    nativePackages = NATIVE_PACKAGES,
    tauriCli = TAURI_CLI,
  },
) {
  await requireDirectory(directory)
  await requireGnuTar()
  if (scope === 'desktop') {
    await requireTemporaryFolderOutside(root)
  }
  const packages = await scopePackages(root, scope, nativePackages)
  const { rootCrate, triples } = SCOPES[scope]
  const crates = await scopeCrates(root, run, triples, rootCrate)
  if (scope === 'web') {
    await requireBuildMachineCrates(root, run, rootCrate, crates)
  }

  const downloads = { registry, crateRegistry, fetch, retryDelays }
  const work = await mkdtemp(join(tmpdir(), 'partsmith-dependencies-'))
  try {
    const tool = scope === 'desktop' ? await extractTauriCli(work, tauriCli, downloads) : null
    await vendorCrates(root, run, scope, work, crates, tool)
    const facts = await downloadPackages(packages, join(work, packagesFolder(scope)), downloads)
    const native = packages.filter((listed, i) => hasPlatformList(listed) || facts[i].prebuilt)
    const entries = recordedEntries(native, nativePackages)
    await requireAllowedLicenses(root, packages, facts)
    const sources = await writeNativeSources(root, run, scope, work, entries, downloads)
    const [cratesArchive, packagesArchive] = archiveNames(scope)
    await packFolders(work, `${cratesFolder(scope)}.tar`, [cratesFolder(scope)])
    await runDirectly('gzip', ['-9', '-n', `${cratesFolder(scope)}.tar`], work)
    await packFolders(work, packagesArchive, [packagesFolder(scope), ...sources])

    const archives = [cratesArchive, packagesArchive]
    const measures = await Promise.all(
      archives.map((archive) => sizeAndSha256(join(work, archive))),
    )
    const parts = await allOrNothing(
      directory,
      archives.map((archive) => writeParts(join(work, archive), directory)),
    )
    return archives.map((archive, i) => {
      const { bytes, sha256 } = measures[i]
      return { archive, bytes, sha256, lastPart: parts[i].at(-1) }
    })
  } finally {
    await rm(work, { recursive: true, force: true })
  }
}

/** Runs the command with its arguments and returns its exit status. */
async function main(commandArguments) {
  const [scope, directory] = commandArguments
  if (commandArguments.length !== 2 || !Object.hasOwn(SCOPES, scope)) {
    process.stderr.write(`${USAGE}\n`)
    return 2
  }
  try {
    const results = await writeDependencySources(repositoryRoot, scope, directory, {
      registry: NPM_REGISTRY,
      crateRegistry: CRATE_REGISTRY,
      fetch,
      run: runCommand,
    })
    process.stdout.write(results.map((result) => `${summaryLine(result, directory)}\n`).join(''))
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
