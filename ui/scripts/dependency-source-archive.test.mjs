// Acceptance tests 89 to 92 of docs/spec/delivery.md section 6 for scripts/dependency-source-archive.mjs, which writes
// the archives of the Rust crates and npm packages of every release and every web deployment (section 2.13).
import assert from 'node:assert/strict'
import { spawnSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import {
  access,
  chmod,
  cp,
  mkdir,
  mkdtemp,
  readFile,
  readdir,
  rm,
  writeFile,
} from 'node:fs/promises'
import { createServer } from 'node:http'
import { tmpdir } from 'node:os'
import { join, relative } from 'node:path'
import { test } from 'node:test'
import { fileURLToPath } from 'node:url'
import { gunzipSync } from 'node:zlib'

import {
  DOWNLOAD_RETRY_DELAYS,
  NPM_REGISTRY,
  PART_SIZE,
  SCOPES,
  archiveNames,
  cargoConfig,
  integrityMismatch,
  lockPackages,
  platformPackages,
  summaryLine,
  tarballPath,
  writeDependencySources,
  writeParts,
} from '../../scripts/dependency-source-archive.mjs'
import { DESKTOP_TARGETS, scopeCrates } from './notices.mjs'
import { writeFiles } from './occt-fixture.mjs'

const repositoryRoot = fileURLToPath(new URL('../..', import.meta.url))
const scriptPath = join(repositoryRoot, 'scripts', 'dependency-source-archive.mjs')

const usageLine = 'usage: node scripts/dependency-source-archive.mjs web|desktop <directory>'

const CRATES_IO = 'registry+https://github.com/rust-lang/crates.io-index'
const PARTSMITH_REPOSITORY = 'https://github.com/blai30/partsmith'
const WEB_TRIPLE = 'wasm32-unknown-emscripten'
const BUILD_MACHINE_TRIPLE = 'x86_64-unknown-linux-gnu'
const DESKTOP_TRIPLES = DESKTOP_TARGETS.map((target) => target.triple)

// The three platforms of section 2.13, as SCOPES names them.
const LINUX = { os: 'linux', cpu: 'x64', libc: 'glibc' }
const WINDOWS = { os: 'win32', cpu: 'x64', libc: null }
const MACOS = { os: 'darwin', cpu: 'arm64', libc: null }

// The waits of the tests before the second and the third attempt of a download, short ones in place of
// DOWNLOAD_RETRY_DELAYS.
const TEST_RETRY_DELAYS = [1, 1]

// The cargo commands of section 2.13 as the fake `run` records them, with the vendor folder as a placeholder.
const UNFILTERED_METADATA = 'cargo metadata --format-version 1 --locked --all-features'
const VENDOR_COMMAND = 'cargo vendor --locked --versioned-dirs <vendor folder>'

// The literal lock text of test 89: five entries of `packages`, one per case, and `snapshots` that repeats two keys.
const PLATFORM_LOCK = `lockfileVersion: '9.0'

settings:
  autoInstallPeers: true
  excludeLinksFromLockfile: false

importers:

  .:
    dependencies:
      b:
        specifier: ^2.0.0
        version: 2.0.0

packages:

  '@scope/a@1.0.0':
    resolution: {integrity: sha512-YWFh}
    engines: {node: '>=18'}
    cpu: [x64]
    os: [linux]

  b@2.0.0:
    resolution: {integrity: sha512-YmJi}
    peerDependencies:
      c: '*'

  c-musl@1.0.0:
    resolution: {integrity: sha512-Y2Nj}
    cpu: [x64]
    os: [linux]
    libc: [musl]

  d@1.0.0:
    resolution: {integrity: sha512-ZGRk}
    os: ['!win32']

  e@1.0.0:
    resolution: {tarball: https://example.com/e.tgz}

snapshots:

  '@scope/a@1.0.0': {}

  b@2.0.0: {}
`

// The entries that lockPackages reads from PLATFORM_LOCK, in its order.
const PLATFORM_ENTRIES = [
  {
    key: '@scope/a@1.0.0',
    name: '@scope/a',
    version: '1.0.0',
    integrity: 'sha512-YWFh',
    os: ['linux'],
    cpu: ['x64'],
    libc: null,
  },
  {
    key: 'b@2.0.0',
    name: 'b',
    version: '2.0.0',
    integrity: 'sha512-YmJi',
    os: null,
    cpu: null,
    libc: null,
  },
  {
    key: 'c-musl@1.0.0',
    name: 'c-musl',
    version: '1.0.0',
    integrity: 'sha512-Y2Nj',
    os: ['linux'],
    cpu: ['x64'],
    libc: ['musl'],
  },
  {
    key: 'd@1.0.0',
    name: 'd',
    version: '1.0.0',
    integrity: 'sha512-ZGRk',
    os: ['!win32'],
    cpu: null,
    libc: null,
  },
  {
    key: 'e@1.0.0',
    name: 'e',
    version: '1.0.0',
    integrity: null,
    os: null,
    cpu: null,
    libc: null,
  },
]

// The npm packages of the registry fixture of tests 90 to 92: three that the web scope holds, the second with a
// sha1 integrity, and one for Windows only, which only the desktop scope holds.
const NPM_PACKAGES = [
  { name: '@scope/one', version: '1.0.0', algorithm: 'sha512', platform: {} },
  { name: 'two', version: '2.0.0', algorithm: 'sha1', platform: {} },
  { name: 'three', version: '3.0.0', algorithm: 'sha512', platform: { cpu: 'x64', os: 'linux' } },
  {
    name: 'win-only',
    version: '1.0.0',
    algorithm: 'sha512',
    platform: { cpu: 'x64', os: 'win32' },
  },
]
const WEB_NPM_PACKAGES = NPM_PACKAGES.slice(0, 3)

// The crates of the vendor fixture (test 91). `kept` is a crate of every scope, `stubbed` a dev-dependency and so a
// stub, `unlisted` a crate the unfiltered metadata does not list, and the last two crates of one desktop graph each.
const KEPT_FILES = {
  'Cargo.toml': '[package]\nname = "kept"\nversion = "1.0.0"\n',
  'src/lib.rs': 'pub fn kept() {}\n',
}
const STUBBED_FILES = {
  'Cargo.toml': '[package]\nname = "stubbed"\nversion = "2.0.0"\n',
  'src/lib.rs': 'pub fn stubbed() {}\n',
  'src/bin/tool.rs': 'fn main() {}\n',
  'build.rs': 'fn main() {}\n',
  'README.md': '# stubbed\n',
}
const UNLISTED_FILES = {
  'Cargo.toml': '[package]\nname = "unlisted"\nversion = "3.0.0"\n',
  'src/lib.rs': 'pub fn unlisted() {}\n',
}
const WINDOWS_ONLY_FILES = {
  'Cargo.toml': '[package]\nname = "windows-only"\nversion = "1.0.0"\n',
  'src/lib.rs': 'pub fn windows_only() {}\n',
}
const MAC_ONLY_FILES = {
  'Cargo.toml': '[package]\nname = "mac-only"\nversion = "1.0.0"\n',
  'src/lib.rs': 'pub fn mac_only() {}\n',
}

// The two stubs as test 91 expects them: the manifest as vendored, the checksum with `files` empty and `package`
// kept, and an empty file at the path of each target the unfiltered metadata lists (none for `unlisted`).
const STUBBED_STUB = {
  '.cargo-checksum.json': '{"files":{},"package":"abc"}',
  'Cargo.toml': STUBBED_FILES['Cargo.toml'],
  'build.rs': '',
  'src/bin/tool.rs': '',
  'src/lib.rs': '',
}
const UNLISTED_STUB = {
  '.cargo-checksum.json': '{"files":{},"package":"def"}',
  'Cargo.toml': UNLISTED_FILES['Cargo.toml'],
}

/** Creates a temporary directory that is removed when the test ends. */
async function temporaryDirectory(context) {
  const directory = await mkdtemp(join(tmpdir(), 'partsmith-dependencies-'))
  context.after(() => rm(directory, { recursive: true, force: true }))
  return directory
}

/** Whether `path` exists. */
async function exists(path) {
  try {
    await access(path)
    return true
  } catch {
    return false
  }
}

/** The SHA-256 of `bytes` in hexadecimal, as sha256sum prints it. */
function sha256Of(bytes) {
  return createHash('sha256').update(bytes).digest('hex')
}

/** The integrity of `bytes` with `algorithm` (`sha512` or `sha1`), as a pnpm lock file writes it. */
function integrityOf(algorithm, bytes) {
  return `${algorithm}-${createHash(algorithm).update(bytes).digest('base64')}`
}

/** The bytes the registry fixture serves for `name` at `version`. */
function tarballBytes(name, version) {
  return Buffer.from(`the tarball of ${name} ${version}\n`)
}

/** The `.cargo-checksum.json` text of a vendored crate whose files are `files`, with `package` as its checksum. */
function checksumText(files, packageChecksum) {
  const hashes = Object.fromEntries(
    Object.entries(files).map(([path, contents]) => [path, sha256Of(contents)]),
  )
  return JSON.stringify({ files: hashes, package: packageChecksum })
}

/** The files of a vendored crate folder: `files` and a checksum file that lists them with `packageChecksum`. */
function vendoredCrate(files, packageChecksum) {
  return { ...files, '.cargo-checksum.json': checksumText(files, packageChecksum) }
}

/**
 * A lock text in the form of ui/pnpm-lock.yaml for `packages` of NPM_PACKAGES's shape, each with the integrity of its
 * tarball, or with a tarball resolution and no integrity when `integrity` is false.
 */
function npmLockText(packages) {
  const entries = packages.map(({ name, version, algorithm, platform, integrity = true }) => {
    const key = name.startsWith('@') ? `'${name}@${version}'` : `${name}@${version}`
    const resolution = integrity
      ? `{integrity: ${integrityOf(algorithm, tarballBytes(name, version))}}`
      : `{tarball: https://example.com/${name}.tgz}`
    const lines = [`  ${key}:`, `    resolution: ${resolution}`]
    if (platform.cpu !== undefined) {
      lines.push(`    cpu: [${platform.cpu}]`)
    }
    if (platform.os !== undefined) {
      lines.push(`    os: [${platform.os}]`)
    }
    return lines.join('\n')
  })
  const snapshots = packages.map(({ name, version }) => {
    const key = name.startsWith('@') ? `'${name}@${version}'` : `${name}@${version}`
    return `  ${key}: {}`
  })
  return `lockfileVersion: '9.0'\n\nsettings:\n  autoInstallPeers: true\n  excludeLinksFromLockfile: false\n\nimporters:\n\n  .: {}\n\npackages:\n\n${entries.join('\n\n')}\n\nsnapshots:\n\n${snapshots.join('\n\n')}\n`
}

/**
 * A package of `cargo metadata`: a crates.io crate whose folder is under `registry`, or a workspace crate in `folder`
 * when `workspace` is set, with `targets` as `[kind, path]` pairs relative to its folder.
 */
function cratePackage(name, version, { registry, folder, targets, workspace = false }) {
  const crateFolder = workspace ? folder : join(registry, `${name}-${version}`)
  return {
    name,
    version,
    id: workspace
      ? `path+file://${crateFolder}#${name}@${version}`
      : `${CRATES_IO}#${name}@${version}`,
    license: 'MIT',
    license_file: null,
    source: workspace ? null : CRATES_IO,
    manifest_path: join(crateFolder, 'Cargo.toml'),
    repository: workspace ? PARTSMITH_REPOSITORY : null,
    targets: targets.map(([kind, path]) => ({
      kind: [kind],
      crate_types: [kind === 'proc-macro' ? 'proc-macro' : kind === 'lib' ? 'lib' : 'bin'],
      name: kind === 'custom-build' ? 'build-script-build' : name.replaceAll('-', '_'),
      src_path: join(crateFolder, path),
    })),
  }
}

/** The parsed `cargo metadata` output of `packages` with `edges`, a list of `[package, [[dependency, kinds], ...]]`. */
function metadataOf(packages, edges) {
  const dependenciesOf = new Map(edges.map(([from, dependencies]) => [from.id, dependencies]))
  return {
    packages,
    resolve: {
      nodes: packages.map((listed) => ({
        id: listed.id,
        deps: (dependenciesOf.get(listed.id) ?? []).map(([to, kinds]) => ({
          name: to.name.replaceAll('-', '_'),
          pkg: to.id,
          dep_kinds: kinds.map((kind) => ({ kind, target: null })),
        })),
      })),
      root: null,
    },
  }
}

/** The command line of the metadata command of section 2.9 input 1 for `triple`. */
function filteredMetadata(triple) {
  return `${UNFILTERED_METADATA} --filter-platform ${triple}`
}

// How a command that `fakeRun` fails rejects: the exit status and standard error of a failed cargo.
const CARGO_FAILURE = { status: 101, stderr: 'error: failed to sync\n' }

/**
 * A fake `run` for the repository root `root`. It answers each metadata command of `answers` (keyed by its command
 * line), and `cargo vendor --locked --versioned-dirs <folder>` by copying `vendorFixture` into `<folder>`. It records
 * every call as `{folder}: {command line}`, with `folder` relative to `root` and the vendor folder as a placeholder,
 * and keeps the vendor folders it was given. The command line `failing` (with the placeholder) rejects with
 * CARGO_FAILURE, and any other command rejects as a failed command does.
 */
function fakeRun(root, answers, vendorFixture, failing) {
  const calls = []
  const vendorFolders = []
  async function run(command, commandArguments, directory) {
    const isVendor =
      command === 'cargo' &&
      commandArguments.length === 4 &&
      commandArguments.slice(0, 3).join(' ') === 'vendor --locked --versioned-dirs'
    const line = isVendor ? VENDOR_COMMAND : [command, ...commandArguments].join(' ')
    calls.push(`${relative(root, directory) || '.'}: ${line}`)
    if (isVendor) {
      vendorFolders.push(commandArguments[3])
    }
    if (line === failing) {
      throw Object.assign(new Error(`${command} exited with status 101`), CARGO_FAILURE)
    }
    if (isVendor) {
      await cp(vendorFixture, commandArguments[3], { recursive: true })
      return `[source.crates-io]\nreplace-with = "vendored-sources"\n`
    }
    const answer = answers.get(line)
    if (answer === undefined) {
      throw Object.assign(new Error(`unexpected command ${line}`), {
        status: 'unexpected',
        stderr: `the test does not expect ${line}`,
      })
    }
    return answer
  }
  return { run, calls, vendorFolders }
}

/**
 * Starts a registry on 127.0.0.1, port 0, that serves the files of `folder` at their paths and answers 404 for any
 * other path. `failures` maps a path to the answers it gets before the file, in order: an HTTP status, or `'drop'`,
 * which closes the connection with no answer. Resolves to its base URL and the paths it was asked for, in order; it
 * stops when the test ends.
 */
async function startRegistry(context, folder, failures = {}) {
  const requests = []
  const failuresLeft = new Map(
    Object.entries(failures).map(([path, answers]) => [path, [...answers]]),
  )
  const server = createServer(async (request, response) => {
    const path = decodeURIComponent(new URL(request.url, 'http://registry.invalid').pathname)
    requests.push(path)
    const failure = failuresLeft.get(path)?.shift()
    if (failure === 'drop') {
      request.socket.destroy()
      return
    }
    if (failure !== undefined) {
      response.writeHead(failure)
      response.end()
      return
    }
    try {
      const bytes = await readFile(join(folder, path))
      response.writeHead(200, { 'content-type': 'application/octet-stream' })
      response.end(bytes)
    } catch {
      response.writeHead(404)
      response.end()
    }
  })
  await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve))
  context.after(
    () =>
      new Promise((resolve) => {
        server.closeAllConnections()
        server.close(resolve)
      }),
  )
  return { url: `http://127.0.0.1:${server.address().port}/`, requests }
}

/** The vendor fixture of test 91 (folder name to files): the web case's crates, and with `desktop` one per platform. */
function vendorCrates(desktop) {
  const crates = {
    'kept-1.0.0': vendoredCrate(KEPT_FILES, 'k1'),
    'stubbed-2.0.0': vendoredCrate(
      { 'Cargo.toml': STUBBED_FILES['Cargo.toml'], 'src/lib.rs': STUBBED_FILES['src/lib.rs'] },
      'abc',
    ),
    'unlisted-3.0.0': vendoredCrate(UNLISTED_FILES, 'def'),
  }
  // The checksum of `stubbed` lists two files; the others are on disk, as in a crate that ships more.
  Object.assign(crates['stubbed-2.0.0'], {
    'src/bin/tool.rs': STUBBED_FILES['src/bin/tool.rs'],
    'build.rs': STUBBED_FILES['build.rs'],
    'README.md': STUBBED_FILES['README.md'],
  })
  if (desktop) {
    crates['windows-only-1.0.0'] = vendoredCrate(WINDOWS_ONLY_FILES, 'w1')
    crates['mac-only-1.0.0'] = vendoredCrate(MAC_ONLY_FILES, 'm1')
  }
  return crates
}

/**
 * Makes the fixture of tests 90 to 92 in a new folder: a repository root whose ui/pnpm-lock.yaml records `packages`,
 * a registry folder that serves the tarballs of NPM_PACKAGES, the vendor fixture of `vendorCrates(desktop)` and the
 * metadata answers of every triple, consistent with it. With `buildMachineCrate`, `{ name, version, targets, kind }`,
 * the metadata for the build machine alone adds an edge of that kind from `kept` to that crate. Resolves to the
 * repository root, the output folder, the registry folder, the vendor fixture and the metadata answers.
 */
async function writeFixture(
  context,
  { packages = NPM_PACKAGES, desktop = false, buildMachineCrate } = {},
) {
  const workspace = await temporaryDirectory(context)
  const root = join(workspace, 'repository')
  const registry = join(workspace, 'cargo-registry')
  const npmFolder = join(workspace, 'npm-registry')
  const vendorFixture = join(workspace, 'vendor-fixture')
  const output = join(workspace, 'output')
  await mkdir(output)
  await writeFiles(root, { 'ui/pnpm-lock.yaml': npmLockText(packages) })
  await writeFiles(
    npmFolder,
    Object.fromEntries(
      NPM_PACKAGES.map(({ name, version }) => [
        tarballPath(name, version),
        tarballBytes(name, version),
      ]),
    ),
  )
  await Promise.all(
    Object.entries(vendorCrates(desktop)).map(([folder, files]) =>
      writeFiles(join(vendorFixture, folder), files),
    ),
  )

  const lib = ['lib', 'src/lib.rs']
  const webRoot = cratePackage('partsmith-wasm', '0.1.0', {
    folder: join(root, 'crates', 'partsmith-wasm'),
    targets: [lib],
    workspace: true,
  })
  const desktopRoot = cratePackage('partsmith-desktop', '0.1.0', {
    folder: join(root, 'desktop', 'src-tauri'),
    targets: [['bin', 'src/main.rs']],
    workspace: true,
  })
  const kept = cratePackage('kept', '1.0.0', { registry, targets: [lib] })
  const stubbed = cratePackage('stubbed', '2.0.0', {
    registry,
    targets: [lib, ['bin', 'src/bin/tool.rs'], ['custom-build', 'build.rs']],
  })
  const platformCrates = desktop
    ? {
        windows: [cratePackage('windows-only', '1.0.0', { registry, targets: [lib] })],
        macOS: [cratePackage('mac-only', '1.0.0', { registry, targets: [lib] })],
      }
    : { windows: [], macOS: [] }
  const hostOnly =
    buildMachineCrate === undefined
      ? []
      : [
          cratePackage(buildMachineCrate.name, buildMachineCrate.version, {
            registry,
            targets: buildMachineCrate.targets,
          }),
        ]

  /** The metadata text of one graph: the web root's edges, and the desktop root's with `extras`. */
  const graph = (extras, keptEdges = []) =>
    JSON.stringify(
      metadataOf(
        [webRoot, desktopRoot, kept, stubbed, ...extras, ...keptEdges.map(([to]) => to)],
        [
          [
            webRoot,
            [
              [kept, [null]],
              [stubbed, ['dev']],
            ],
          ],
          [desktopRoot, [[kept, [null]], ...extras.map((extra) => [extra, [null]])]],
          [kept, keptEdges.map(([to, kind]) => [to, [kind]])],
        ],
      ),
    )
  const answers = new Map([
    [filteredMetadata(WEB_TRIPLE), graph([])],
    [
      filteredMetadata(BUILD_MACHINE_TRIPLE),
      graph(
        [],
        hostOnly.map((to) => [to, buildMachineCrate.kind]),
      ),
    ],
    [filteredMetadata('x86_64-pc-windows-msvc'), graph(platformCrates.windows)],
    [filteredMetadata('aarch64-apple-darwin'), graph(platformCrates.macOS)],
    [UNFILTERED_METADATA, graph([...platformCrates.windows, ...platformCrates.macOS])],
  ])
  return { root, output, npmFolder, vendorFixture, answers }
}

/**
 * Runs writeDependencySources for `scope` on `fixture`, with its registry and a fake `run`. The registry gives the
 * answers of `failures` first (as `startRegistry` takes them), and a failed download waits `retryDelays`.
 */
async function writeSources(
  context,
  fixture,
  scope,
  { output = fixture.output, vendor, failures, retryDelays = TEST_RETRY_DELAYS } = {},
) {
  const registry = await startRegistry(context, fixture.npmFolder, failures)
  const cargo = fakeRun(fixture.root, fixture.answers, vendor ?? fixture.vendorFixture)
  const result = await writeDependencySources(fixture.root, scope, output, {
    registry: registry.url,
    fetch,
    run: cargo.run,
    retryDelays,
  })
  return { result, registry, cargo }
}

/**
 * Runs writeDependencySources for `web` on `fixture` into its output folder, downloading from `registry` (as
 * `startRegistry` resolves it), with the fake `run` of the fixture and TEST_RETRY_DELAYS unless `run` or `retryDelays`
 * is given.
 */
function writeWebSources(
  fixture,
  registry,
  {
    run = fakeRun(fixture.root, fixture.answers, fixture.vendorFixture).run,
    retryDelays = TEST_RETRY_DELAYS,
  } = {},
) {
  return writeDependencySources(fixture.root, 'web', fixture.output, {
    registry: registry.url,
    fetch,
    run,
    retryDelays,
  })
}

/** The parts of `archive` in `directory`, in name order. */
async function partNames(directory, archive) {
  return (await readdir(directory)).filter((name) => name.startsWith(`${archive}.part-`)).toSorted()
}

/** The bytes of `archive` joined from its parts in `directory`. */
async function joinedArchive(directory, archive) {
  const parts = await partNames(directory, archive)
  return Buffer.concat(await Promise.all(parts.map((part) => readFile(join(directory, part)))))
}

/** Extracts the tar `bytes` into a new folder and resolves to it. */
async function extractTar(context, bytes) {
  const folder = await temporaryDirectory(context)
  const result = spawnSync('tar', ['-xf', '-', '-C', folder], { input: bytes })
  assert.equal(result.status, 0, String(result.stderr))
  return folder
}

/** The paths of the regular files below `folder`, relative to it with `/`, in byte order. */
async function filesBelow(folder) {
  const entries = await readdir(folder, { recursive: true, withFileTypes: true })
  return entries
    .filter((listed) => listed.isFile())
    .map((listed) => relative(folder, join(listed.parentPath, listed.name)))
    .toSorted()
}

/** The files below `folder` as an object of their paths and texts. */
async function folderTexts(folder) {
  const paths = await filesBelow(folder)
  const texts = await Promise.all(paths.map((path) => readFile(join(folder, path), 'utf8')))
  return Object.fromEntries(paths.map((path, i) => [path, texts[i]]))
}

/** The extracted crates' archive that writeDependencySources wrote for `scope` into `output`. */
async function extractedCrates(context, output, scope) {
  const [crates] = archiveNames(scope)
  return extractTar(context, gunzipSync(await joinedArchive(output, crates)))
}

/** Compares two paths as `--sort=name` orders them: folder by folder, each name in byte order. */
function compareTreeOrder(left, right) {
  const leftNames = left.replace(/\/$/, '').split('/')
  const rightNames = right.replace(/\/$/, '').split('/')
  for (let i = 0; i < Math.min(leftNames.length, rightNames.length); i++) {
    const order = Buffer.compare(Buffer.from(leftNames[i]), Buffer.from(rightNames[i]))
    if (order !== 0) {
      return order
    }
  }
  return leftNames.length - rightNames.length
}

/** Runs the command `node scripts/dependency-source-archive.mjs` with `commandArguments` in `directory`. */
function runCommand(commandArguments, directory, environment = process.env) {
  const result = spawnSync(process.execPath, [scriptPath, ...commandArguments], {
    cwd: directory,
    env: environment,
    encoding: 'utf8',
  })
  return { status: result.status, stdout: result.stdout, stderr: result.stderr }
}

/**
 * A folder of stand-ins for the command tests of test 92: `cargo`, which records its arguments in $CARGO_CALLS and
 * fails with `error: no network` on standard error, and, with `bsdTar`, a `tar` whose --version names bsdtar. Resolves
 * to the environment that puts them first on PATH and the path of the calls file.
 */
async function stubEnvironment(context, { bsdTar = false } = {}) {
  const directory = await temporaryDirectory(context)
  const stubs = join(directory, 'stubs')
  const calls = join(directory, 'cargo-calls')
  await mkdir(stubs)
  await writeFile(calls, '')
  await writeFile(
    join(stubs, 'cargo'),
    '#!/usr/bin/env bash\nprintf \'%s\\n\' "$*" >> "$CARGO_CALLS"\necho \'error: no network\' >&2\nexit 1\n',
  )
  await chmod(join(stubs, 'cargo'), 0o755)
  if (bsdTar) {
    await writeFile(
      join(stubs, 'tar'),
      '#!/usr/bin/env bash\nif [ "$1" = --version ]; then\n  echo \'bsdtar 3.7.4 - libarchive 3.7.4 zlib/1.3\'\n  exit 0\nfi\nexit 1\n',
    )
    await chmod(join(stubs, 'tar'), 0o755)
  }
  return {
    environment: { ...process.env, PATH: `${stubs}:${process.env.PATH}`, CARGO_CALLS: calls },
    calls,
  }
}

/** The integrity message of section 4 for `listed`, whose tarball at `url` has the integrity `actual`. */
function integrityMessage(listed, url, actual, expected) {
  return `npm package ${listed.name} ${listed.version}: ${url} sent a tarball whose integrity is ${actual}, not ${expected}, which ui/pnpm-lock.yaml records. Nothing was written.`
}

/** How many requests `registry` answered for the tarball of `listed`. */
function requestCount(registry, listed) {
  const path = `/${tarballPath(listed.name, listed.version)}`
  return registry.requests.filter((requested) => requested === path).length
}

/** The download message of section 4 for `listed`, whose tarball at `url` got the answer or error `answer`. */
function downloadMessage(listed, url, answer) {
  return `npm package ${listed.name} ${listed.version}: ${url} answered ${answer}.`
}

/** The build-machine message of section 4 for the crate `name` at `version`. */
function buildMachineMessage(name, version) {
  return `Rust crate ${name} ${version} is built for the machine that builds the web core, and the web crate set of docs/spec/delivery.md section 2.9 input 1 does not hold it, so rust-crates-web.tar.gz would lack it. Nothing was written.`
}

test('89: lockPackages reads each entry of the packages section in order, with null for a field the entry lacks', () => {
  assert.deepEqual(lockPackages(PLATFORM_LOCK), PLATFORM_ENTRIES)
})

/** The keys of the entries of PLATFORM_ENTRIES that platformPackages keeps for `platforms`. */
function platformKeys(platforms) {
  return platformPackages(PLATFORM_ENTRIES, platforms).map((listed) => listed.key)
}

test('89: platformPackages keeps the entries whose os, cpu and libc lists allow one of the platforms', () => {
  assert.deepEqual(platformKeys([LINUX]), ['@scope/a@1.0.0', 'b@2.0.0', 'd@1.0.0', 'e@1.0.0'])
  assert.deepEqual(platformKeys([WINDOWS]), ['b@2.0.0', 'e@1.0.0'])
  assert.deepEqual(platformKeys([MACOS]), ['b@2.0.0', 'd@1.0.0', 'e@1.0.0'])
  assert.deepEqual(platformKeys([LINUX, WINDOWS, MACOS]), [
    '@scope/a@1.0.0',
    'b@2.0.0',
    'd@1.0.0',
    'e@1.0.0',
  ])
})

test('89: a libc list allows only the values it holds, and a platform without libc passes it', () => {
  const musl = PLATFORM_ENTRIES[2]
  const anyLinux = { ...musl, key: 'any-linux@1.0.0', os: null, cpu: null }

  assert.deepEqual(platformPackages([musl], [{ os: 'linux', cpu: 'x64', libc: 'musl' }]), [musl])
  assert.deepEqual(platformPackages([anyLinux], [LINUX]), [])
  assert.deepEqual(platformPackages([anyLinux], [WINDOWS]), [anyLinux])
})

test('89: SCOPES names the root crate, the triples and the platforms of each scope', () => {
  assert.deepEqual(SCOPES, {
    web: { rootCrate: 'partsmith-wasm', triples: [WEB_TRIPLE], platforms: [LINUX] },
    desktop: {
      rootCrate: 'partsmith-desktop',
      triples: DESKTOP_TRIPLES,
      platforms: [LINUX, WINDOWS, MACOS],
    },
  })
})

test("89: tarballPath is the path of the tarball's URL under the registry", () => {
  assert.equal(tarballPath('@scope/a', '1.0.0'), '@scope/a/-/a-1.0.0.tgz')
  assert.equal(tarballPath('b', '2.0.0'), 'b/-/b-2.0.0.tgz')
  assert.equal(NPM_REGISTRY, 'https://registry.npmjs.org/')
})

test("89: every entry of the repository's ui/pnpm-lock.yaml has an integrity", async () => {
  const entries = lockPackages(await readFile(join(repositoryRoot, 'ui', 'pnpm-lock.yaml'), 'utf8'))

  assert.ok(entries.length > 0, 'ui/pnpm-lock.yaml has no packages')
  for (const listed of entries) {
    assert.match(listed.integrity ?? '', /^(?:sha512|sha1)-[A-Za-z0-9+/]+=*$/, listed.key)
    assert.equal(listed.key, `${listed.name}@${listed.version}`)
  }
})

test('90: integrityMismatch is null for the bytes an integrity names, in sha512 and in sha1', () => {
  const bytes = Buffer.from('some bytes\n')

  assert.equal(integrityMismatch(bytes, integrityOf('sha512', bytes)), null)
  assert.equal(integrityMismatch(bytes, integrityOf('sha1', bytes)), null)
})

test('90: integrityMismatch of other bytes is their integrity with the same algorithm', () => {
  const bytes = Buffer.from('some bytes\n')
  const other = Buffer.from('other bytes\n')

  assert.equal(integrityMismatch(other, integrityOf('sha512', bytes)), integrityOf('sha512', other))
  assert.equal(integrityMismatch(other, integrityOf('sha1', bytes)), integrityOf('sha1', other))
})

test('90: the npm archive of the web scope holds the three tarballs at their paths, each asked for once', async (context) => {
  const fixture = await writeFixture(context)

  const { registry } = await writeSources(context, fixture, 'web')

  const [, npmArchive] = archiveNames('web')
  const extracted = await extractTar(context, await joinedArchive(fixture.output, npmArchive))
  const expectedPaths = WEB_NPM_PACKAGES.map(
    ({ name, version }) => `npm-packages-web/${tarballPath(name, version)}`,
  )
  assert.deepEqual(await filesBelow(extracted), expectedPaths.toSorted())
  const extractedTarballs = await Promise.all(
    WEB_NPM_PACKAGES.map(({ name, version }) =>
      readFile(join(extracted, 'npm-packages-web', tarballPath(name, version))),
    ),
  )
  assert.deepEqual(
    extractedTarballs,
    WEB_NPM_PACKAGES.map(({ name, version }) => tarballBytes(name, version)),
  )
  assert.deepEqual(
    registry.requests.toSorted(),
    WEB_NPM_PACKAGES.map(({ name, version }) => `/${tarballPath(name, version)}`).toSorted(),
  )
})

test('90: a tarball whose bytes changed on the server fails with the integrity message and writes nothing', async (context) => {
  const fixture = await writeFixture(context)
  const changed = WEB_NPM_PACKAGES[2]
  const changedBytes = Buffer.from('changed bytes\n')
  await writeFile(join(fixture.npmFolder, tarballPath(changed.name, changed.version)), changedBytes)
  const registry = await startRegistry(context, fixture.npmFolder)
  const url = `${registry.url}${tarballPath(changed.name, changed.version)}`

  await assert.rejects(writeWebSources(fixture, registry), {
    message: integrityMessage(
      changed,
      url,
      integrityOf(changed.algorithm, changedBytes),
      integrityOf(changed.algorithm, tarballBytes(changed.name, changed.version)),
    ),
  })
  assert.deepEqual(await readdir(fixture.output), [])
  assert.equal(requestCount(registry, changed), 1)
})

test('90: a tarball the server answers with 404 fails with the download message and writes nothing', async (context) => {
  const fixture = await writeFixture(context)
  const missing = WEB_NPM_PACKAGES[1]
  await rm(join(fixture.npmFolder, tarballPath(missing.name, missing.version)))
  const registry = await startRegistry(context, fixture.npmFolder)
  const url = `${registry.url}${tarballPath(missing.name, missing.version)}`

  await assert.rejects(writeWebSources(fixture, registry), {
    message: downloadMessage(missing, url, 404),
  })
  assert.deepEqual(await readdir(fixture.output), [])
  assert.equal(requestCount(registry, missing), 1)
})

test('90: DOWNLOAD_RETRY_DELAYS waits 2 s before the second attempt of a download and 10 s before the third', () => {
  assert.deepEqual(DOWNLOAD_RETRY_DELAYS, [2000, 10000])
})

test('90: a download that fails with a dropped connection, 408, 429 or 5xx is tried again, up to three times', async (context) => {
  const fixture = await writeFixture(context)
  const [one, two, three] = WEB_NPM_PACKAGES
  const failures = {
    [`/${tarballPath(one.name, one.version)}`]: [429, 408],
    [`/${tarballPath(two.name, two.version)}`]: ['drop'],
    [`/${tarballPath(three.name, three.version)}`]: [503, 500],
  }

  const { registry } = await writeSources(context, fixture, 'web', { failures })

  const [, npmArchive] = archiveNames('web')
  const extracted = await extractTar(context, await joinedArchive(fixture.output, npmArchive))
  const extractedTarballs = await Promise.all(
    WEB_NPM_PACKAGES.map(({ name, version }) =>
      readFile(join(extracted, 'npm-packages-web', tarballPath(name, version))),
    ),
  )
  assert.deepEqual(
    extractedTarballs,
    WEB_NPM_PACKAGES.map(({ name, version }) => tarballBytes(name, version)),
  )
  assert.deepEqual(
    WEB_NPM_PACKAGES.map((listed) => requestCount(registry, listed)),
    [3, 2, 3],
  )
})

/**
 * Asserts that writeDependencySources for `web` fails with the download message `answer` and writes nothing when the
 * registry gives the third tarball the three `answers`, after three requests and the two waits of 30 ms between them.
 */
async function assertDownloadGivesUp(context, answers, answer) {
  const fixture = await writeFixture(context)
  const failing = WEB_NPM_PACKAGES[2]
  const path = tarballPath(failing.name, failing.version)
  const registry = await startRegistry(context, fixture.npmFolder, { [`/${path}`]: answers })
  const started = performance.now()

  await assert.rejects(writeWebSources(fixture, registry, { retryDelays: [30, 30] }), {
    message: downloadMessage(failing, `${registry.url}${path}`, answer),
  })
  // A timer may fire a little early, never 5 ms early.
  assert.ok(performance.now() - started >= 55, 'the download did not wait between its attempts')
  assert.equal(requestCount(registry, failing), 3)
  assert.deepEqual(await readdir(fixture.output), [])
}

test('90: a download that gets a server error three times fails with the last status and writes nothing', async (context) => {
  await assertDownloadGivesUp(context, [503, 502, 504], 504)
})

test("90: a download whose connection drops three times fails with the error's message and writes nothing", async (context) => {
  await assertDownloadGivesUp(context, ['drop', 'drop', 'drop'], 'fetch failed')
})

test('90: a lock entry without integrity fails with its message before any request and writes nothing', async (context) => {
  const packages = [...WEB_NPM_PACKAGES.slice(0, 2), { ...WEB_NPM_PACKAGES[2], integrity: false }]
  const fixture = await writeFixture(context, { packages })
  const registry = await startRegistry(context, fixture.npmFolder)

  await assert.rejects(writeWebSources(fixture, registry), {
    message:
      'npm package three@3.0.0 has no integrity in ui/pnpm-lock.yaml, so its tarball cannot be checked (docs/spec/delivery.md section 2.13).',
  })
  assert.deepEqual(registry.requests, [])
  assert.deepEqual(await readdir(fixture.output), [])
})

test('91: the crates of the web scope stay whole and the other crates become stubs Cargo can resolve', async (context) => {
  const fixture = await writeFixture(context)

  await writeSources(context, fixture, 'web')

  const vendor = join(
    await extractedCrates(context, fixture.output, 'web'),
    'rust-crates-web',
    'vendor',
  )
  assert.deepEqual(await readdir(vendor).then((names) => names.toSorted()), [
    'kept-1.0.0',
    'stubbed-2.0.0',
    'unlisted-3.0.0',
  ])
  assert.deepEqual(await folderTexts(join(vendor, 'kept-1.0.0')), {
    ...KEPT_FILES,
    '.cargo-checksum.json': checksumText(KEPT_FILES, 'k1'),
  })
  assert.deepEqual(await folderTexts(join(vendor, 'stubbed-2.0.0')), STUBBED_STUB)
  assert.deepEqual(await folderTexts(join(vendor, 'unlisted-3.0.0')), UNLISTED_STUB)
})

test('91: the crates archive holds config.toml with the text of section 2.13 for its scope', async (context) => {
  const fixture = await writeFixture(context)

  await writeSources(context, fixture, 'web')

  const extracted = await extractedCrates(context, fixture.output, 'web')
  assert.deepEqual(await readdir(join(extracted)), ['rust-crates-web'])
  assert.deepEqual((await readdir(join(extracted, 'rust-crates-web'))).toSorted(), [
    'config.toml',
    'vendor',
  ])
  assert.equal(
    await readFile(join(extracted, 'rust-crates-web', 'config.toml'), 'utf8'),
    cargoConfig('web'),
  )
})

/** The text of config.toml of section 2.13 for `scope`, written out. */
function configText(scope) {
  return [
    '# Set CARGO_HOME to this folder to build Partsmith from these crates without the network (docs/spec/delivery.md section 2.13).',
    '[source.crates-io]',
    'replace-with = "vendored-sources"',
    '',
    '[source.vendored-sources]',
    `directory = "rust-crates-${scope}/vendor"`,
    '',
    '[net]',
    'offline = true',
    '',
  ].join('\n')
}

test('91: cargoConfig is the config.toml of section 2.13 with the folder of its scope', () => {
  assert.equal(cargoConfig('web'), configText('web'))
  assert.equal(cargoConfig('desktop'), configText('desktop'))
})

test('91: the web scope sends the vendor command and the three metadata commands through run, in the repository root', async (context) => {
  const fixture = await writeFixture(context)

  const { cargo } = await writeSources(context, fixture, 'web')

  assert.deepEqual(cargo.calls.toSorted(), [
    `.: ${UNFILTERED_METADATA}`,
    `.: ${filteredMetadata(WEB_TRIPLE)}`,
    `.: ${filteredMetadata(BUILD_MACHINE_TRIPLE)}`,
    `.: ${VENDOR_COMMAND}`,
  ])
})

test('91: the vendor folder is a temporary folder outside the output folder, gone when the archives are written', async (context) => {
  const fixture = await writeFixture(context)

  const { cargo } = await writeSources(context, fixture, 'web')

  assert.equal(cargo.vendorFolders.length, 1)
  const [vendorFolder] = cargo.vendorFolders
  assert.ok(
    relative(fixture.output, vendorFolder).startsWith('..'),
    `the vendor folder ${vendorFolder} is inside the output folder`,
  )
  assert.equal(await exists(vendorFolder), false)
})

/** Asserts that writeDependencySources for `web` on `fixture` rejects with `message` and writes nothing. */
async function assertWebRejected(context, fixture, message) {
  const registry = await startRegistry(context, fixture.npmFolder)

  await assert.rejects(writeWebSources(fixture, registry), { message })
  assert.deepEqual(await readdir(fixture.output), [])
}

test('91: a crate behind a build edge on the build machine that the web set lacks fails with the build-machine message', async (context) => {
  const fixture = await writeFixture(context, {
    buildMachineCrate: {
      name: 'hosted',
      version: '4.0.0',
      targets: [['lib', 'src/lib.rs']],
      kind: 'build',
    },
  })

  await assertWebRejected(context, fixture, buildMachineMessage('hosted', '4.0.0'))
})

test('91: a procedural macro crate on the build machine that the web set lacks fails with the build-machine message', async (context) => {
  const fixture = await writeFixture(context, {
    buildMachineCrate: {
      name: 'macro-crate',
      version: '5.0.0',
      targets: [['proc-macro', 'src/lib.rs']],
      kind: null,
    },
  })

  await assertWebRejected(context, fixture, buildMachineMessage('macro-crate', '5.0.0'))
})

test('91: a crate of a normal edge on the build machine alone, with no build edge or procedural macro, is no build-machine crate', async (context) => {
  const fixture = await writeFixture(context, {
    buildMachineCrate: {
      name: 'linux-only',
      version: '6.0.0',
      targets: [['lib', 'src/lib.rs']],
      kind: null,
    },
  })

  const { result } = await writeSources(context, fixture, 'web')

  assert.deepEqual(
    result.map(({ archive }) => archive),
    archiveNames('web'),
  )
})

test('91: the desktop scope keeps whole a crate that one triple alone reaches, and checks no build machine', async (context) => {
  const fixture = await writeFixture(context, { desktop: true })

  const { cargo } = await writeSources(context, fixture, 'desktop')

  const vendor = join(
    await extractedCrates(context, fixture.output, 'desktop'),
    'rust-crates-desktop',
    'vendor',
  )
  assert.deepEqual(await folderTexts(join(vendor, 'windows-only-1.0.0')), {
    ...WINDOWS_ONLY_FILES,
    '.cargo-checksum.json': checksumText(WINDOWS_ONLY_FILES, 'w1'),
  })
  assert.deepEqual(await folderTexts(join(vendor, 'mac-only-1.0.0')), {
    ...MAC_ONLY_FILES,
    '.cargo-checksum.json': checksumText(MAC_ONLY_FILES, 'm1'),
  })
  assert.deepEqual(await folderTexts(join(vendor, 'kept-1.0.0')), {
    ...KEPT_FILES,
    '.cargo-checksum.json': checksumText(KEPT_FILES, 'k1'),
  })
  assert.deepEqual(await folderTexts(join(vendor, 'stubbed-2.0.0')), STUBBED_STUB)
  assert.deepEqual(await folderTexts(join(vendor, 'unlisted-3.0.0')), UNLISTED_STUB)
  assert.deepEqual(
    cargo.calls.toSorted(),
    [
      `.: ${UNFILTERED_METADATA}`,
      ...DESKTOP_TRIPLES.map((triple) => `.: ${filteredMetadata(triple)}`),
      `.: ${VENDOR_COMMAND}`,
    ].toSorted(),
  )
})

test("91: the desktop scope's npm archive also holds the packages of Windows and macOS", async (context) => {
  const fixture = await writeFixture(context, { desktop: true })

  await writeSources(context, fixture, 'desktop')

  const [, npmArchive] = archiveNames('desktop')
  const extracted = await extractTar(context, await joinedArchive(fixture.output, npmArchive))
  assert.deepEqual(
    await filesBelow(extracted),
    NPM_PACKAGES.map(
      ({ name, version }) => `npm-packages-desktop/${tarballPath(name, version)}`,
    ).toSorted(),
  )
})

test('91: scopeCrates gives each crate of the triples once, sorted by name, with one metadata command per triple', async (context) => {
  const fixture = await writeFixture(context, { desktop: true })
  const cargo = fakeRun(fixture.root, fixture.answers, fixture.vendorFixture)

  const crates = await scopeCrates(fixture.root, cargo.run, DESKTOP_TRIPLES, 'partsmith-desktop')

  assert.deepEqual(
    crates.map((listed) => `${listed.name} ${listed.version}`),
    ['kept 1.0.0', 'mac-only 1.0.0', 'windows-only 1.0.0'],
  )
  assert.deepEqual(
    cargo.calls,
    DESKTOP_TRIPLES.map((triple) => `.: ${filteredMetadata(triple)}`),
  )
})

test('92: PART_SIZE is 24 MiB', () => {
  assert.equal(PART_SIZE, 25165824)
})

test('92: writeParts cuts a file into parts of the part size, the last one shorter, and names them in order', async (context) => {
  const directory = await temporaryDirectory(context)
  const file = join(directory, 'name.bin')
  const bytes = Buffer.from('abcdefghijklmnopqrstuvwxy')
  await writeFile(file, bytes)
  const output = join(directory, 'parts')
  await mkdir(output)

  const names = await writeParts(file, output, 10)

  assert.deepEqual(names, ['name.bin.part-00', 'name.bin.part-01', 'name.bin.part-02'])
  const parts = await Promise.all(names.map((name) => readFile(join(output, name))))
  assert.deepEqual(
    parts.map((part) => part.length),
    [10, 10, 5],
  )
  assert.deepEqual(Buffer.concat(parts), bytes)
  assert.deepEqual((await readdir(output)).toSorted(), names)
})

test('92: writeParts writes no empty part after a full one, and one empty part for an empty file', async (context) => {
  const directory = await temporaryDirectory(context)
  await writeFile(join(directory, 'twenty.bin'), Buffer.alloc(20, 1))
  await writeFile(join(directory, 'empty.bin'), Buffer.alloc(0))
  const output = join(directory, 'parts')
  await mkdir(output)

  assert.deepEqual(await writeParts(join(directory, 'twenty.bin'), output, 10), [
    'twenty.bin.part-00',
    'twenty.bin.part-01',
  ])
  assert.deepEqual(await writeParts(join(directory, 'empty.bin'), output, 10), [
    'empty.bin.part-00',
  ])
  assert.equal((await readFile(join(output, 'empty.bin.part-00'))).length, 0)
})

test('92: writeParts writes a file smaller than PART_SIZE as one part by default', async (context) => {
  const directory = await temporaryDirectory(context)
  await writeFile(join(directory, 'small.bin'), Buffer.from('small\n'))
  const output = join(directory, 'parts')
  await mkdir(output)

  assert.deepEqual(await writeParts(join(directory, 'small.bin'), output), ['small.bin.part-00'])
})

test('92: archiveNames gives the crates archive and then the packages archive of a scope', () => {
  assert.deepEqual(archiveNames('web'), ['rust-crates-web.tar.gz', 'npm-packages-web.tar'])
  assert.deepEqual(archiveNames('desktop'), [
    'rust-crates-desktop.tar.gz',
    'npm-packages-desktop.tar',
  ])
})

test('92: two runs write the same parts, byte for byte', async (context) => {
  const fixture = await writeFixture(context)
  const second = join(await temporaryDirectory(context), 'second')
  await mkdir(second)

  await writeSources(context, fixture, 'web')
  await writeSources(context, fixture, 'web', { output: second })

  const names = (await readdir(fixture.output)).toSorted()
  assert.deepEqual((await readdir(second)).toSorted(), names)
  const [first, again] = await Promise.all(
    [fixture.output, second].map((folder) =>
      Promise.all(names.map((name) => readFile(join(folder, name)))),
    ),
  )
  assert.deepEqual(again, first)
})

test('92: the crates archive is a gzip file with no file name and a modification time of 0', async (context) => {
  const fixture = await writeFixture(context)

  await writeSources(context, fixture, 'web')

  const [crates] = archiveNames('web')
  const archive = await joinedArchive(fixture.output, crates)
  assert.deepEqual([archive[0], archive[1]], [0x1f, 0x8b])
  // FLG bit 3 (FNAME) is clear and MTIME is 0.
  assert.equal(archive[3] & 0x08, 0)
  assert.equal(archive.readUInt32LE(4), 0)
})

test('92: each archive lists its entries in the order of --sort=name, owned by 0 and dated 1970-01-01 00:00', async (context) => {
  const fixture = await writeFixture(context)

  await writeSources(context, fixture, 'web')

  const [crates, npmArchive] = archiveNames('web')
  const tars = [
    gunzipSync(await joinedArchive(fixture.output, crates)),
    await joinedArchive(fixture.output, npmArchive),
  ]
  for (const tar of tars) {
    const listing = spawnSync('tar', ['-tvf', '-'], {
      input: tar,
      encoding: 'utf8',
      env: { ...process.env, TZ: 'UTC', LC_ALL: 'C' },
    })
    assert.equal(listing.status, 0, listing.stderr)
    const entries = listing.stdout
      .trimEnd()
      .split('\n')
      .map((line) => /^(\S+) (\S+) +(\d+) (\S+) (\S+) (.*)$/.exec(line))
    const names = entries.map((entry) => entry[6])
    assert.deepEqual(names, names.toSorted(compareTreeOrder))
    for (const entry of entries) {
      assert.equal(entry[2], '0/0', entry[0])
      assert.equal(`${entry[4]} ${entry[5]}`, '1970-01-01 00:00', entry[6])
    }
  }
})

test('92: the result gives the size, SHA-256 and last part of each archive, and summaryLine the line the command prints', async (context) => {
  const fixture = await writeFixture(context)

  const { result } = await writeSources(context, fixture, 'web')

  const expected = await Promise.all(
    archiveNames('web').map(async (archive) => {
      const joined = await joinedArchive(fixture.output, archive)
      const parts = await partNames(fixture.output, archive)
      return { archive, bytes: joined.length, sha256: sha256Of(joined), lastPart: parts.at(-1) }
    }),
  )
  assert.deepEqual(result, expected)
  assert.deepEqual(
    expected.map((archive) => summaryLine(archive, 'dist/dependencies')),
    expected.map(
      ({ archive, bytes, sha256 }) =>
        `Wrote ${archive} to dist/dependencies: ${bytes} bytes, SHA-256 ${sha256}, last part ${archive}.part-00.`,
    ),
  )
  assert.deepEqual(
    (await readdir(fixture.output)).toSorted(),
    expected.flatMap(({ archive }) => [`${archive}.part-00`]).toSorted(),
  )
})

test('92: summaryLine names the last part of an archive of several parts', () => {
  assert.equal(
    summaryLine(
      {
        archive: 'npm-packages-web.tar',
        bytes: 83374080,
        sha256: 'a'.repeat(64),
        lastPart: 'npm-packages-web.tar.part-03',
      },
      'dist/dependencies',
    ),
    `Wrote npm-packages-web.tar to dist/dependencies: 83374080 bytes, SHA-256 ${'a'.repeat(64)}, last part npm-packages-web.tar.part-03.`,
  )
})

test('92: the command exits 2 with the usage line for arguments other than a scope and a directory', async (context) => {
  const directory = await temporaryDirectory(context)
  await mkdir(join(directory, 'out'))
  const { environment, calls } = await stubEnvironment(context)

  for (const commandArguments of [[], ['web'], ['wrong', 'out'], ['web', 'out', 'extra']]) {
    const result = runCommand(commandArguments, directory, environment)

    assert.equal(result.status, 2, commandArguments.join(' '))
    assert.equal(result.stderr, `${usageLine}\n`, commandArguments.join(' '))
    assert.equal(result.stdout, '', commandArguments.join(' '))
  }
  assert.deepEqual(await readdir(join(directory, 'out')), [])
  assert.equal(await readFile(calls, 'utf8'), '')
})

test('92: the command exits 1 with its message for a directory that does not exist, before any cargo command', async (context) => {
  const directory = await temporaryDirectory(context)
  const missing = join(directory, 'missing')
  const { environment, calls } = await stubEnvironment(context)

  const result = runCommand(['web', missing], directory, environment)

  assert.equal(result.status, 1)
  assert.equal(
    result.stderr,
    `${missing} does not exist. Make it before writing the dependency source archives into it (docs/spec/delivery.md section 2.13).\n`,
  )
  assert.equal(await exists(missing), false)
  assert.equal(await readFile(calls, 'utf8'), '')
})

test('92: the command exits 1 with the GNU tar message when tar is not GNU tar, before any cargo command', async (context) => {
  const directory = await temporaryDirectory(context)
  const output = join(directory, 'out')
  await mkdir(output)
  const { environment, calls } = await stubEnvironment(context, { bsdTar: true })

  const result = runCommand(['web', output], directory, environment)

  assert.equal(result.status, 1)
  assert.equal(
    result.stderr,
    'tar --version does not name GNU tar, which the dependency source archives need for the same bytes on every run. Put GNU tar first on PATH (docs/spec/delivery.md section 2.13).\n',
  )
  assert.deepEqual(await readdir(output), [])
  assert.equal(await readFile(calls, 'utf8'), '')
})

test('92: the command exits 1 with the message of a failed cargo command and writes nothing', async (context) => {
  const directory = await temporaryDirectory(context)
  const output = join(directory, 'out')
  await mkdir(output)
  const { environment, calls } = await stubEnvironment(context)
  const metadata = `metadata --format-version 1 --locked --all-features --filter-platform ${WEB_TRIPLE}`

  const result = runCommand(['web', output], directory, environment)

  assert.equal(result.status, 1)
  assert.equal(result.stderr, `cargo ${metadata} failed with exit status 1: error: no network\n`)
  assert.equal(result.stdout, '')
  assert.deepEqual(await readdir(output), [])
  assert.equal(await readFile(calls, 'utf8'), `${metadata}\n`)
})

test('92: a failed cargo vendor rejects with its command message, writes nothing and removes the temporary folder', async (context) => {
  const fixture = await writeFixture(context)
  const registry = await startRegistry(context, fixture.npmFolder)
  const cargo = fakeRun(fixture.root, fixture.answers, fixture.vendorFixture, VENDOR_COMMAND)

  await assert.rejects(writeWebSources(fixture, registry, { run: cargo.run }), (error) => {
    const [vendorFolder] = cargo.vendorFolders
    assert.equal(
      error.message,
      `cargo vendor --locked --versioned-dirs ${vendorFolder} failed with exit status 101: error: failed to sync`,
    )
    return true
  })
  assert.deepEqual(await readdir(fixture.output), [])
  // The vendor folder is <temporary folder>/rust-crates-web/vendor.
  assert.equal(await exists(join(cargo.vendorFolders[0], '..', '..')), false)
})

/**
 * Puts a stand-in for `command` first on PATH until the test ends: it answers `--version` with `version` and fails
 * any other call with `{command}: stand-in failure` on standard error.
 */
async function failingTool(context, command, version) {
  const stubs = await temporaryDirectory(context)
  await writeFile(
    join(stubs, command),
    `#!/usr/bin/env bash\nif [ "$1" = --version ]; then\n  echo '${version}'\n  exit 0\nfi\necho '${command}: stand-in failure' >&2\nexit 1\n`,
  )
  await chmod(join(stubs, command), 0o755)
  const path = process.env.PATH
  process.env.PATH = `${stubs}:${path}`
  context.after(() => {
    process.env.PATH = path
  })
}

/**
 * Asserts that writeDependencySources for `web` fails with the command message of `line` when the stand-in of
 * `failingTool` replaces `command`, writes nothing and removes its temporary folder.
 */
async function assertToolFailure(context, command, version, line) {
  const fixture = await writeFixture(context)
  await failingTool(context, command, version)
  const registry = await startRegistry(context, fixture.npmFolder)
  const cargo = fakeRun(fixture.root, fixture.answers, fixture.vendorFixture)

  await assert.rejects(writeWebSources(fixture, registry, { run: cargo.run }), {
    message: `${line} failed with exit status 1: ${command}: stand-in failure`,
  })
  assert.deepEqual(await readdir(fixture.output), [])
  // The vendor folder is <temporary folder>/rust-crates-web/vendor.
  assert.equal(await exists(join(cargo.vendorFolders[0], '..', '..')), false)
}

test('92: a failed tar rejects with its command message, writes nothing and removes the temporary folder', async (context) => {
  await assertToolFailure(
    context,
    'tar',
    'tar (GNU tar) 1.35',
    'tar --sort=name --mtime=@0 --owner=0 --group=0 --numeric-owner --format=pax --pax-option=exthdr.name=%d/PaxHeaders/%f,delete=atime,delete=ctime --mode=u+rwX,go+rX,go-w --create --file=rust-crates-web.tar rust-crates-web',
  )
})

test('92: a failed gzip rejects with its command message, writes nothing and removes the temporary folder', async (context) => {
  await assertToolFailure(context, 'gzip', 'gzip 1.12', 'gzip -9 -n rust-crates-web.tar')
})

test('92: a part already in the output folder rejects, stays as it was, and the parts written before it are removed', async (context) => {
  const fixture = await writeFixture(context)
  const [, npmArchive] = archiveNames('web')
  const existing = `${npmArchive}.part-00`
  const existingBytes = Buffer.from('a part from an earlier run\n')
  await writeFile(join(fixture.output, existing), existingBytes)

  await assert.rejects(writeSources(context, fixture, 'web'), { code: 'EEXIST' })

  assert.deepEqual(await readdir(fixture.output), [existing])
  assert.deepEqual(await readFile(join(fixture.output, existing)), existingBytes)
})
