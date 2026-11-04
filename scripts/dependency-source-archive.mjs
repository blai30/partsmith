// Writes the archives of the Rust crates and the npm packages that the desktop app's executables or the web build are
// compiled from, made from Cargo.lock and ui/pnpm-lock.yaml, as parts under Cloudflare Pages' file limit. CI writes the
// web scope's into every web deployment. The release workflow attaches the desktop scope's to every release
// (docs/spec/delivery.md section 2.13, ADR 0034).
// Usage: node scripts/dependency-source-archive.mjs web|desktop <directory>
import { execFile } from 'node:child_process'
import { createHash } from 'node:crypto'
import { createReadStream } from 'node:fs'
import { mkdir, mkdtemp, readFile, readdir, realpath, rm, stat, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { basename, dirname, join, relative } from 'node:path'
import process from 'node:process'
import { setTimeout as wait } from 'node:timers/promises'
import { fileURLToPath } from 'node:url'
import { promisify } from 'node:util'

import {
  checkedRun,
  commandFailureMessage,
  compareNameAndVersion,
  runCommand,
} from '../ui/scripts/check-licenses.mjs'
import {
  DESKTOP_TARGETS,
  cargoMetadata,
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

const USAGE = 'usage: node scripts/dependency-source-archive.mjs web|desktop <directory>'

const repositoryRoot = fileURLToPath(new URL('..', import.meta.url))

// The machine that builds the web core. Its build scripts and procedural macros compile for it.
const BUILD_MACHINE_TRIPLE = 'x86_64-unknown-linux-gnu'

// The file in which cargo vendor records the SHA-256 of each file of a crate and of the crate's package.
const CHECKSUM_FILE = '.cargo-checksum.json'

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

// How many tarballs the command downloads at the same time.
const DOWNLOADS_AT_ONCE = 8

const execute = promisify(execFile)

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

/** The names of the two archives of `scope`, crates first: `[rust-crates-{scope}.tar.gz, npm-packages-{scope}.tar]`. */
export function archiveNames(scope) {
  return [`${cratesFolder(scope)}.tar.gz`, `${packagesFolder(scope)}.tar`]
}

/** The exact text of `rust-crates-{scope}/config.toml`. */
export function cargoConfig(scope) {
  return [
    '# Set CARGO_HOME to this folder to build Partsmith from these crates without the network (docs/spec/delivery.md section 2.13).',
    '[source.crates-io]',
    'replace-with = "vendored-sources"',
    '',
    '[source.vendored-sources]',
    `directory = "${cratesFolder(scope)}/vendor"`,
    '',
    '[net]',
    'offline = true',
    '',
  ].join('\n')
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
 * Runs `tar` or `gzip` in `directory` with `LC_ALL=C` and resolves to its standard output. A failure rejects with the
 * message of section 4.
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
 * The npm packages of `scope` from the lock file of `root`. Rejects before any download when one of them has no
 * integrity.
 */
async function scopePackages(root, scope) {
  const lockText = await readFile(join(root, 'ui', 'pnpm-lock.yaml'), 'utf8')
  const packages = platformPackages(lockPackages(lockText), SCOPES[scope].platforms)
  const unchecked = packages.find((listed) => listed.integrity === null)
  if (unchecked !== undefined) {
    throw new Error(
      `npm package ${unchecked.key} has no integrity in ui/pnpm-lock.yaml, so its tarball cannot be checked (docs/spec/delivery.md section 2.13).`,
    )
  }
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
 * Writes `folder` (`rust-crates-{scope}`) with `config.toml` and `vendor/`, every crate of Cargo.lock vendored, those
 * of `crates` whole and every other one as a stub.
 */
async function vendorCrates(root, run, scope, folder, crates) {
  const vendor = join(folder, 'vendor')
  await checkedRun(run, 'cargo', ['vendor', '--locked', '--versioned-dirs', vendor], root)
  const metadata = await cargoMetadata(root, run)
  const registryPackages = new Map(
    metadata.packages
      .filter((listed) => listed.source !== null)
      .map((listed) => [`${listed.name}-${listed.version}`, listed]),
  )
  const kept = new Set(crates.map((crate) => `${crate.name}-${crate.version}`))
  const stubbed = (await readdir(vendor)).filter((name) => !kept.has(name))
  await Promise.all(
    stubbed.map((name) => stubCrate(join(vendor, name), registryPackages.get(name))),
  )
  await writeFile(join(folder, 'config.toml'), cargoConfig(scope))
}

/**
 * One request for `url` with `fetch`. Resolves to `{ bytes }` for a successful answer, and otherwise to
 * `{ failure, retry }`: the status or the error's message, and whether the download is tried again for it.
 */
async function requestTarball(url, fetch) {
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

/** `requestTarball` of `url`, tried again after each of `retryDelays` while it fails in a way that is retried. */
async function requestWithRetries(url, fetch, retryDelays) {
  const answer = await requestTarball(url, fetch)
  if (answer.bytes !== undefined || !answer.retry || retryDelays.length === 0) {
    return answer
  }
  await wait(retryDelays[0])
  return requestWithRetries(url, fetch, retryDelays.slice(1))
}

/**
 * The bytes of the tarball of `listed` from `registry`, tried again after each of `retryDelays` as
 * `requestWithRetries` does, and checked against its integrity.
 */
async function downloadTarball(listed, { registry, fetch, retryDelays }) {
  const url = `${registry}${tarballPath(listed.name, listed.version)}`
  const failure = (reason) => new Error(`npm package ${listed.name} ${listed.version}: ${reason}`)
  const answer = await requestWithRetries(url, fetch, retryDelays)
  if (answer.bytes === undefined) {
    throw failure(`${url} answered ${answer.failure}.`)
  }
  const actual = integrityMismatch(answer.bytes, listed.integrity)
  if (actual !== null) {
    throw failure(
      `${url} sent a tarball whose integrity is ${actual}, not ${listed.integrity}, which ui/pnpm-lock.yaml records. Nothing was written.`,
    )
  }
  return answer.bytes
}

/** Downloads the tarball of `listed` and writes it into `folder` at its `tarballPath`. */
async function saveTarball(listed, folder, downloadOptions) {
  const bytes = await downloadTarball(listed, downloadOptions)
  const path = join(folder, tarballPath(listed.name, listed.version))
  await mkdir(dirname(path), { recursive: true })
  await writeFile(path, bytes)
}

/**
 * Downloads the tarball of each of `packages` into `folder` at its `tarballPath`, a few at a time, with
 * `downloadOptions`, `{ registry, fetch, retryDelays }`. The first failure stops the downloads that have not started
 * and rejects once the others have settled.
 */
async function downloadPackages(packages, folder, downloadOptions) {
  let next = 0
  let failure
  const download = async () => {
    while (failure === undefined && next < packages.length) {
      try {
        // oxlint-disable-next-line no-await-in-loop -- each of the DOWNLOADS_AT_ONCE loops saves one tarball at a time
        await saveTarball(packages[next++], folder, downloadOptions)
      } catch (error) {
        failure ??= error
      }
    }
  }
  await Promise.all(Array.from({ length: DOWNLOADS_AT_ONCE }, download))
  if (failure !== undefined) {
    throw failure
  }
}

/** Packs the folder `name` of `work` into `work/{name}.tar` with GNU tar, the same bytes on every run. */
async function packFolder(work, name) {
  await runDirectly('tar', [...TAR_OPTIONS, '--create', `--file=${name}.tar`, name], work)
}

/** The size and SHA-256 of the file at `path`. */
async function sizeAndSha256(path) {
  const hash = createHash('sha256')
  for await (const chunk of createReadStream(path)) {
    hash.update(chunk)
  }
  return { bytes: (await stat(path)).size, sha256: hash.digest('hex') }
}

/**
 * Writes the parts of both archives of `scope` for the repository at `root` into `directory`. It downloads each tarball
 * from `registry` with `fetch` and sends every `cargo` command through `run`. A download that fails with a network
 * error or a retried status waits each of `retryDelays` in turn and tries again. Resolves to one
 * `{ archive, bytes, sha256, lastPart }` per archive, crates first, where `bytes` is the archive's size, `sha256` its
 * SHA-256 in hexadecimal and `lastPart` the name of its last part. Rejects with the message of section 4 and leaves
 * `directory` as it was on any failure.
 */
export async function writeDependencySources(
  root,
  scope,
  directory,
  { registry, fetch, run, retryDelays = DOWNLOAD_RETRY_DELAYS },
) {
  await requireDirectory(directory)
  await requireGnuTar()
  const packages = await scopePackages(root, scope)
  const { rootCrate, triples } = SCOPES[scope]
  const crates = await scopeCrates(root, run, triples, rootCrate)
  if (scope === 'web') {
    await requireBuildMachineCrates(root, run, rootCrate, crates)
  }

  const work = await mkdtemp(join(tmpdir(), 'partsmith-dependencies-'))
  try {
    await vendorCrates(root, run, scope, join(work, cratesFolder(scope)), crates)
    await downloadPackages(packages, join(work, packagesFolder(scope)), {
      registry,
      fetch,
      retryDelays,
    })
    await packFolder(work, cratesFolder(scope))
    await runDirectly('gzip', ['-9', '-n', `${cratesFolder(scope)}.tar`], work)
    await packFolder(work, packagesFolder(scope))

    const archives = archiveNames(scope)
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
