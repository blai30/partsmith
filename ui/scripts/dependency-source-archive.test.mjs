// Acceptance tests 89 to 92, 108 to 113 and 115 of docs/spec/delivery.md section 6 for
// scripts/dependency-source-archive.mjs, which writes the archives of the Rust crates and npm packages of every release
// and every web deployment (section 2.13).
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
  realpath,
  rm,
  writeFile,
} from 'node:fs/promises'
import { createServer } from 'node:http'
import { tmpdir } from 'node:os'
import { dirname, join, relative } from 'node:path'
import { test } from 'node:test'
import { fileURLToPath, pathToFileURL } from 'node:url'
import { gunzipSync, gzipSync } from 'node:zlib'

import {
  CRATE_REGISTRY,
  DOWNLOAD_RETRY_DELAYS,
  NATIVE_PACKAGES,
  NPM_REGISTRY,
  PART_SIZE,
  SCOPES,
  TAURI_CLI,
  archiveNames,
  cargoConfig,
  integrityMismatch,
  lockCrates,
  lockPackages,
  nativeEntries,
  platformPackages,
  summaryLine,
  tarballLicense,
  tarballPath,
  vendorConfig,
  writeDependencySources,
  writeParts,
} from '../../scripts/dependency-source-archive.mjs'
import { allowedLicenses, compareNameAndVersion } from './check-licenses.mjs'
import { git, isolatedGitEnvironment } from './isolated-git.mjs'
import {
  DESKTOP_TARGETS,
  EXCLUDED_LICENSE_FILES,
  WINDOWS_INSTALLER,
  renderCrateNotices,
  scopeCrates,
} from './notices.mjs'
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

// The cargo commands of section 2.13 as the fake `run` records them, with the vendor folder and the temporary folder
// as placeholders.
const UNFILTERED_METADATA = 'cargo metadata --format-version 1 --locked --all-features'
const VENDOR_COMMAND = 'cargo vendor --locked --versioned-dirs <vendor folder>'

// The version of the fixture's tauri-cli, which the fixture's crate registry serves (test 112).
const TAURI_FIXTURE_VERSION = '9.9.9'

// The vendor command of the desktop scope, which also vendors the lock file of the fixture's tauri-cli.
const DESKTOP_VENDOR_COMMAND = `cargo vendor --locked --versioned-dirs --sync <temporary folder>/tauri-cli-${TAURI_FIXTURE_VERSION}/Cargo.toml <vendor folder>`

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

// The table of native packages that the fixture's calls pass (section 6, after test 92): the fixture's packages with
// an os or cpu list, with nothing to carry.
const FIXTURE_NATIVE_PACKAGES = [
  {
    name: 'three',
    version: '3.0.0',
    source: 'https://example.invalid/three',
    tag: 'v3.0.0',
    crates: [],
  },
  {
    name: 'win-only',
    version: '1.0.0',
    source: 'https://example.invalid/win-only',
    tag: 'v1.0.0',
    crates: [],
  },
]

// NATIVE_PACKAGES as section 2.13 writes it (test 109).
const EXPECTED_NATIVE_PACKAGES = [
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
    notices: { crate: 'rolldown_binding', commit: 'f7de35f45bcd4cb8e4b6d8d11e74fbfb20b03127' },
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

// A crate of the lock file of the fixture's tauri-cli that Cargo.lock lacks (test 112).
const TOOL_ONLY_FILES = {
  'Cargo.toml': '[package]\nname = "tool-only"\nversion = "1.0.0"\n',
  'src/lib.rs': 'pub fn tool_only() {}\n',
}

// libdbus-sys with the vendored libdbus that EXCLUDED_LICENSE_FILES names (test 113).
const LIBDBUS_FILES = {
  'Cargo.toml': '[package]\nname = "libdbus-sys"\nversion = "0.2.7"\n',
  'build.rs': 'fn main() {}\n',
  'src/lib.rs': 'pub fn dbus() {}\n',
  'vendor/dbus/COPYING': 'Academic Free License 2.1 or GNU General Public License 2\n',
  'vendor/dbus/dbus/dbus-bus.c': 'int dbus_bus(void) { return 0; }\n',
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

// The crates that the fake `run` vendors for the lock files of a native package's source (test 110).
const NATIVE_VENDOR_FILES = {
  'Cargo.toml': '[package]\nname = "cssparser"\nversion = "0.33.0"\n',
  'src/lib.rs': 'pub fn parse() {}\n',
}

// The packages of the lock file of the fixture's tauri-cli, besides its root (test 112).
const STUBBED_LOCKED = { name: 'stubbed', version: '2.0.0', source: CRATES_IO, checksum: 'abc' }
const TOOL_ONLY_LOCKED = { name: 'tool-only', version: '1.0.0', source: CRATES_IO, checksum: 't1' }
const GIT_LOCKED = {
  name: 'git-dep',
  version: '0.1.0',
  source: 'git+https://github.com/example/git-dep?rev=abc#abc',
}

// The size of a tar block: a header, or a piece of a file's contents.
const TAR_BLOCK = 512

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

/** `value` as the octal digits of a tar header field of `length` bytes, ended by a NUL. */
function octalField(value, length) {
  return `${value.toString(8).padStart(length - 1, '0')}\0`
}

/**
 * The 512-byte ustar header of the entry `path` (at most 100 bytes) of `size` bytes, owned by 0 and dated 0: a folder
 * when `path` ends in `/`, otherwise a regular file.
 */
function tarHeader(path, size) {
  assert.ok(Buffer.byteLength(path) <= 100, `${path} needs a ustar prefix`)
  const folder = path.endsWith('/')
  const header = Buffer.alloc(TAR_BLOCK)
  header.write(path, 0)
  header.write(octalField(folder ? 0o755 : 0o644, 8), 100)
  header.write(octalField(0, 8), 108)
  header.write(octalField(0, 8), 116)
  header.write(octalField(size, 12), 124)
  header.write(octalField(0, 12), 136)
  header.write(' '.repeat(8), 148)
  header.write(folder ? '5' : '0', 156)
  header.write('ustar\u000000', 257, 'latin1')
  const checksum = header.reduce((total, byte) => total + byte, 0)
  header.write(`${checksum.toString(8).padStart(6, '0')}\0 `, 148)
  return header
}

/**
 * A gzip file of a ustar tar that holds `files` (paths to contents, a path ending in `/` for a folder) in their order,
 * as `npm pack` and `cargo package` write them.
 */
function gzipTar(files) {
  const blocks = Object.entries(files).flatMap(([path, contents]) => {
    const bytes = Buffer.from(contents)
    const padding = Buffer.alloc((TAR_BLOCK - (bytes.length % TAR_BLOCK)) % TAR_BLOCK)
    return [tarHeader(path, bytes.length), bytes, padding]
  })
  return gzipSync(Buffer.concat([...blocks, Buffer.alloc(2 * TAR_BLOCK)]))
}

/** `files` with `prefix` before each path. */
function prefixed(prefix, files) {
  return Object.fromEntries(
    Object.entries(files).map(([path, contents]) => [`${prefix}${path}`, contents]),
  )
}

/** The text of a package.json with `fields`. */
function packageJson(fields) {
  return `${JSON.stringify(fields, null, 2)}\n`
}

/** The files of the npm tarball of `name` at `version` under `license`, in the folder `package/` as npm packs it. */
function packageFiles(name, version, license = 'MIT') {
  return { 'package/package.json': packageJson({ name, version, license }) }
}

/** The bytes the registry fixture serves for `name` at `version`: an npm tarball under MIT. */
function tarballBytes(name, version) {
  return gzipTar(packageFiles(name, version))
}

/** The tarball of a package of NPM_PACKAGES's shape: its `bytes`, its `files` packed, or `tarballBytes`. */
function packageBytes({ name, version, files, bytes }) {
  if (bytes !== undefined) {
    return bytes
  }
  return files === undefined ? tarballBytes(name, version) : gzipTar(files)
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
 * tarball (`packageBytes`), or with a tarball resolution and no integrity when `integrity` is false.
 */
function npmLockText(packages) {
  const entries = packages.map((listed) => {
    const { name, version, algorithm, platform, integrity = true } = listed
    const key = name.startsWith('@') ? `'${name}@${version}'` : `${name}@${version}`
    const resolution = integrity
      ? `{integrity: ${integrityOf(algorithm, packageBytes(listed))}}`
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
 * The text of a Cargo lock file of `packages`, `{ name, version, source, checksum, dependencies }` with the fields a
 * package lacks left out, in their order, as Cargo writes version 4.
 */
function cargoLockText(packages) {
  const entries = packages.map(({ name, version, source, checksum, dependencies = [] }) => {
    const lines = ['[[package]]', `name = "${name}"`, `version = "${version}"`]
    if (source !== undefined) {
      lines.push(`source = "${source}"`)
    }
    if (checksum !== undefined) {
      lines.push(`checksum = "${checksum}"`)
    }
    if (dependencies.length > 0) {
      lines.push('dependencies = [', ...dependencies.map((dependency) => ` "${dependency}",`), ']')
    }
    return lines.join('\n')
  })
  return [
    '# This file is automatically @generated by Cargo.',
    '# It is not intended for manual editing.',
    'version = 4',
    '',
    entries.join('\n\n'),
    '',
  ].join('\n')
}

/** Orders two packages of a Cargo lock file by name, as Cargo sorts them. */
function byName(first, second) {
  return Buffer.compare(Buffer.from(first.name), Buffer.from(second.name))
}

/**
 * The files of the fixture's tauri-cli crate, relative to its folder: its Cargo.lock lists the root `tauri-cli`
 * without a source, depending on `locked`, and then `locked`, sorted by name as Cargo sorts them.
 */
function tauriCrateFiles(locked) {
  const root = {
    name: 'tauri-cli',
    version: TAURI_FIXTURE_VERSION,
    dependencies: locked.map((listed) => listed.name),
  }
  return {
    'Cargo.toml': `[package]\nname = "tauri-cli"\nversion = "${TAURI_FIXTURE_VERSION}"\n`,
    'Cargo.lock': cargoLockText([root, ...locked].toSorted(byName)),
    'src/main.rs': 'fn main() {}\n',
    'templates/app/.gitignore': 'target/\n',
  }
}

/** The `.crate` file of the fixture's tauri-cli: a gzip file of a tar whose folder is `tauri-cli-9.9.9/`. */
function tauriCrate(locked) {
  return gzipTar(prefixed(`tauri-cli-${TAURI_FIXTURE_VERSION}/`, tauriCrateFiles(locked)))
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
 * line). It answers `cargo vendor --locked --versioned-dirs ... <folder>` by copying `vendorFixture` into `<folder>`,
 * or `nativeVendorFixture` when the command has a `--manifest-path`, once every manifest that a `--manifest-path` or a
 * `--sync` names exists. It records every call as `{folder}: {command line}`, with `folder` relative to `root`, the
 * vendor folder as `<vendor folder>` and the temporary folder that holds the Rust crates' vendor folder as
 * `<temporary folder>`, and keeps the vendor folders it was given. The command line `failing` rejects with
 * CARGO_FAILURE, and any other command rejects as a failed command does.
 */
function fakeRun(root, answers, { vendorFixture, nativeVendorFixture, failing }) {
  const calls = []
  const vendorFolders = []
  let temporaryFolder
  const withPlaceholder = (argument) =>
    temporaryFolder !== undefined && argument.startsWith(`${temporaryFolder}/`)
      ? `<temporary folder>/${relative(temporaryFolder, argument)}`
      : argument
  async function run(command, commandArguments, directory) {
    const isVendor =
      command === 'cargo' &&
      commandArguments.slice(0, 3).join(' ') === 'vendor --locked --versioned-dirs'
    const isNative = isVendor && commandArguments.includes('--manifest-path')
    if (isVendor) {
      vendorFolders.push(commandArguments.at(-1))
      // The Rust crates' vendor folder is <temporary folder>/rust-crates-{scope}/vendor.
      if (!isNative) {
        temporaryFolder = dirname(dirname(commandArguments.at(-1)))
      }
    }
    const line = isVendor
      ? ['cargo', ...commandArguments.slice(0, -1).map(withPlaceholder), '<vendor folder>'].join(
          ' ',
        )
      : [command, ...commandArguments].join(' ')
    calls.push(`${relative(root, directory) || '.'}: ${line}`)
    if (line === failing) {
      throw Object.assign(new Error(`${command} exited with status 101`), CARGO_FAILURE)
    }
    if (isVendor) {
      const manifests = commandArguments.filter((_, i) =>
        ['--manifest-path', '--sync'].includes(commandArguments[i - 1]),
      )
      const found = await Promise.all(manifests.map(exists))
      const missing = manifests.find((_, i) => !found[i])
      if (missing !== undefined) {
        throw Object.assign(new Error(`no manifest ${missing}`), {
          status: 101,
          stderr: `error: manifest path \`${missing}\` does not exist`,
        })
      }
      await cp(isNative ? nativeVendorFixture : vendorFixture, commandArguments.at(-1), {
        recursive: true,
      })
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

/** The fake `run` of `fixture` (as `writeFixture` resolves it), failing the command line `failing`. */
function fixtureRun(fixture, failing) {
  return fakeRun(fixture.root, fixture.answers, {
    vendorFixture: fixture.vendorFixture,
    nativeVendorFixture: fixture.nativeVendorFixture,
    failing,
  })
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

/**
 * The vendor fixture of test 91 (folder name to files): the web case's crates, with `desktop` one per platform, with
 * `toolOnly` the crate of the tauri-cli lock file of test 112, and with `libdbus` the libdbus-sys of test 113.
 */
function vendorCrates({ desktop, toolOnly, libdbus }) {
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
  if (toolOnly) {
    crates['tool-only-1.0.0'] = vendoredCrate(TOOL_ONLY_FILES, 't1')
  }
  if (libdbus) {
    crates['libdbus-sys-0.2.7'] = vendoredCrate(LIBDBUS_FILES, 'ld')
  }
  return crates
}

/**
 * Makes the fixture of tests 90 to 92 and 108 to 115 in a new folder: a repository root whose ui/pnpm-lock.yaml
 * records `packages` and that holds a copy of the repository's deny.toml and `rootFiles`; an npm registry folder that
 * serves the tarballs of NPM_PACKAGES and `packages`; a crate registry folder that serves the fixture's tauri-cli,
 * whose lock file lists `tauriLocked`, and `crateFiles`; the vendor fixtures; and the metadata answers of every
 * triple, consistent with them. With `buildMachineCrate`, `{ name, version, targets, kind }`, the metadata for the
 * build machine alone adds an edge of that kind from `kept` to that crate. With `libdbus`, the Linux graph of the
 * desktop root reaches libdbus-sys 0.2.7, whose targets the unfiltered metadata lists. Resolves to the repository
 * root, the output folder, the registry folders, the vendor fixtures, the metadata answers and the fixture's
 * `tauriCli`.
 */
async function writeFixture(
  context,
  {
    packages = NPM_PACKAGES,
    desktop = false,
    buildMachineCrate,
    toolOnly = false,
    libdbus = false,
    tauriLocked = [],
    crateFiles = {},
    rootFiles = {},
  } = {},
) {
  const workspace = await temporaryDirectory(context)
  const root = join(workspace, 'repository')
  const registry = join(workspace, 'cargo-registry')
  const npmFolder = join(workspace, 'npm-registry')
  const crateFolder = join(workspace, 'crate-registry')
  const vendorFixture = join(workspace, 'vendor-fixture')
  const nativeVendorFixture = join(workspace, 'native-vendor-fixture')
  const output = join(workspace, 'output')
  await mkdir(output)
  await writeFiles(root, {
    'ui/pnpm-lock.yaml': npmLockText(packages),
    'deny.toml': await readFile(join(repositoryRoot, 'deny.toml')),
    ...rootFiles,
  })
  const tarballs = new Map(
    [...NPM_PACKAGES, ...packages].map((listed) => [
      tarballPath(listed.name, listed.version),
      packageBytes(listed),
    ]),
  )
  await writeFiles(npmFolder, Object.fromEntries(tarballs))
  const tauriBytes = tauriCrate(tauriLocked)
  await writeFiles(crateFolder, {
    [`tauri-cli/tauri-cli-${TAURI_FIXTURE_VERSION}.crate`]: tauriBytes,
    ...crateFiles,
  })
  await Promise.all(
    Object.entries(vendorCrates({ desktop, toolOnly, libdbus })).map(([folder, files]) =>
      writeFiles(join(vendorFixture, folder), files),
    ),
  )
  await writeFiles(
    join(nativeVendorFixture, 'cssparser-0.33.0'),
    vendoredCrate(NATIVE_VENDOR_FILES, 'c33'),
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
  const linuxCrates = libdbus
    ? [
        cratePackage('libdbus-sys', '0.2.7', {
          registry,
          targets: [lib, ['custom-build', 'build.rs']],
        }),
      ]
    : []
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
        linuxCrates,
        hostOnly.map((to) => [to, buildMachineCrate.kind]),
      ),
    ],
    [filteredMetadata('x86_64-pc-windows-msvc'), graph(platformCrates.windows)],
    [filteredMetadata('aarch64-apple-darwin'), graph(platformCrates.macOS)],
    [
      UNFILTERED_METADATA,
      graph([...linuxCrates, ...platformCrates.windows, ...platformCrates.macOS]),
    ],
  ])
  return {
    root,
    output,
    npmFolder,
    crateFolder,
    vendorFixture,
    nativeVendorFixture,
    answers,
    tauriCli: { version: TAURI_FIXTURE_VERSION, sha256: sha256Of(tauriBytes) },
  }
}

/**
 * Starts the npm registry and the crate registry of `fixture`, which give the answers of `failures` and
 * `crateFailures` first (as `startRegistry` takes them), and the fixture's fake `run`, which fails the command line
 * `failing`. Resolves to them and to `write(scope, options)`, which runs writeDependencySources for `scope` on the
 * fixture with them into `output` (the fixture's output folder by default), with `retryDelays` (TEST_RETRY_DELAYS),
 * `nativePackages` (FIXTURE_NATIVE_PACKAGES) and `tauriCli` (the fixture's) unless the options give others.
 */
async function startSources(context, fixture, { failures, crateFailures, failing } = {}) {
  const registry = await startRegistry(context, fixture.npmFolder, failures)
  const crates = await startRegistry(context, fixture.crateFolder, crateFailures)
  const cargo = fixtureRun(fixture, failing)
  const write = (
    scope,
    {
      output = fixture.output,
      retryDelays = TEST_RETRY_DELAYS,
      nativePackages = FIXTURE_NATIVE_PACKAGES,
      tauriCli = fixture.tauriCli,
    } = {},
  ) =>
    writeDependencySources(fixture.root, scope, output, {
      registry: registry.url,
      crateRegistry: crates.url,
      fetch,
      run: cargo.run,
      retryDelays,
      nativePackages,
      tauriCli,
    })
  return { registry, crates, cargo, write }
}

/** Runs writeDependencySources for `scope` on `fixture` as `startSources` does, and resolves to its result as well. */
async function writeSources(context, fixture, scope, options = {}) {
  const sources = await startSources(context, fixture, options)
  const result = await sources.write(scope, options)
  return { result, ...sources }
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

/** The paths of the entries of the tar `bytes`, as `tar -t` lists them, in the tar's order. */
function tarEntries(bytes) {
  const listing = spawnSync('tar', ['-tf', '-'], { input: bytes, encoding: 'utf8' })
  assert.equal(listing.status, 0, listing.stderr)
  return listing.stdout.trimEnd().split('\n')
}

/** The first folder of each entry of the tar `bytes`, each once, in the tar's order. */
function topFolders(bytes) {
  return [...new Set(tarEntries(bytes).map((path) => path.split('/')[0]))]
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

/** The `vendor/` folder of the crates' archive that writeDependencySources wrote for `scope` into `output`. */
async function extractedVendor(context, output, scope) {
  return join(await extractedCrates(context, output, scope), `rust-crates-${scope}`, 'vendor')
}

/** The extracted packages' archive that writeDependencySources wrote for `scope` into `output`. */
async function extractedPackages(context, output, scope) {
  const [, packages] = archiveNames(scope)
  return extractTar(context, await joinedArchive(output, packages))
}

/** Asserts that `bytes` are a gzip file whose header holds no file name and a modification time of 0. */
function assertBareGzip(bytes, name) {
  assert.deepEqual([bytes[0], bytes[1]], [0x1f, 0x8b], `${name} is not a gzip file`)
  // FLG bit 3 (FNAME) is clear and MTIME is 0.
  assert.equal(bytes[3] & 0x08, 0, `${name} names a file in its gzip header`)
  assert.equal(bytes.readUInt32LE(4), 0, `${name} has a modification time in its gzip header`)
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

/**
 * Puts a stand-in for `command` first on PATH until the test ends. It answers `--version` with `version` and fails any
 * other call with `{command}: stand-in failure` on standard error; with no `version` it fails every call.
 */
async function failingTool(context, command, version) {
  const stubs = await temporaryDirectory(context)
  const versionLines =
    version === undefined
      ? ''
      : `if [ "$1" = --version ]; then\n  echo '${version}'\n  exit 0\nfi\n`
  await writeFile(
    join(stubs, command),
    `#!/usr/bin/env bash\n${versionLines}echo '${command}: stand-in failure' >&2\nexit 1\n`,
  )
  await chmod(join(stubs, command), 0o755)
  const path = process.env.PATH
  process.env.PATH = `${stubs}:${path}`
  context.after(() => {
    process.env.PATH = path
  })
}

/** Sets TMPDIR to `folder` and returns a function that sets it back, which also runs when the test ends. */
function useTemporaryFolder(context, folder) {
  const saved = process.env.TMPDIR
  process.env.TMPDIR = folder
  const restore = () => {
    if (saved === undefined) {
      delete process.env.TMPDIR
    } else {
      process.env.TMPDIR = saved
    }
  }
  context.after(restore)
  return restore
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

/** The message of section 2.10 for the npm package `name` at `version` under `license`. */
function notAllowedMessage(name, version, license) {
  return `npm package ${name} ${version} is licensed ${license}, which deny.toml does not allow. Replace the package, or allow the license in deny.toml with an ADR (docs/spec/delivery.md section 2.10).`
}

/** The native-package message of section 4 for the npm package `name` at `version`. */
function nativePackageMessage(name, version) {
  return `npm package ${name} ${version} holds prebuilt native code, and NATIVE_PACKAGES of scripts/dependency-source-archive.mjs records no check of its source at that version. Check its source for code under MPL-2.0 or a copyleft license that it compiles in, and record the result there and in docs/spec/delivery.md section 2.13.`
}

/** The tag-commit message of section 4 for the table entry `entry`, whose tag names `commit`. */
function tagCommitMessage(entry, commit) {
  return `The tag ${entry.tag} of ${entry.source} names the commit ${commit}, not ${entry.commit}, which NATIVE_PACKAGES of scripts/dependency-source-archive.mjs pins for ${entry.name} ${entry.version}. Upstream may have moved the tag; nothing was written (docs/spec/delivery.md section 2.13).`
}

/** The crate download message of section 4 for the crate `name` at `version`, whose `url` got the answer `answer`. */
function crateDownloadMessage(name, version, url, answer) {
  return `Rust crate ${name} ${version}: ${url} answered ${answer}.`
}

/** The crate checksum message of section 4 for the crate `name` at `version`. */
function crateChecksumMessage(name, version, url, actual, expected) {
  return `Rust crate ${name} ${version}: ${url} sent a file whose SHA-256 is ${actual}, not ${expected}, which scripts/dependency-source-archive.mjs records. Nothing was written.`
}

/** The tool-lock source message of section 4 for the package `listed` of the Cargo.lock of `subject`. */
function toolLockMessage(listed, subject) {
  return `Rust crate ${listed.name} ${listed.version} in the Cargo.lock of ${subject} comes from ${listed.source}, not from crates.io. The config.toml of the dependency source archives replaces only crates.io, and an offline build would not find it. Nothing was written (docs/spec/delivery.md section 2.13).`
}

/** The temporary-folder message of section 4 for the temporary folder `folder` inside the repository root `root`. */
function temporaryFolderMessage(folder, root) {
  return `The temporary folder ${folder} is inside ${root}, where Cargo would take tauri-cli for a member of Partsmith's workspace. Set TMPDIR to a folder outside the repository (docs/spec/delivery.md section 2.13).`
}

/** The native-notices message of section 4 for the table entry `entry`. */
function nativeNoticesMessage(entry) {
  return `ui/scripts/notices/native/${stemOf(entry.name)}-${entry.version}.txt is missing. Write it from the source of ${entry.name} ${entry.version} with node scripts/native-notices.mjs in ui/ (docs/spec/delivery.md section 2.13).`
}

/** The `{stem}` of section 2.13: an entry's name without its leading `@` and with `/` written as `-`. */
function stemOf(name) {
  return name.replace(/^@/, '').replace('/', '-')
}

/** The text of config.toml of section 2.13 for the crates in `folder` that build `subject`, written out. */
function configText(folder, subject) {
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

// The calls of the web scope through `run` (test 91), sorted.
const WEB_CALLS = [
  `.: ${UNFILTERED_METADATA}`,
  `.: ${filteredMetadata(WEB_TRIPLE)}`,
  `.: ${filteredMetadata(BUILD_MACHINE_TRIPLE)}`,
  `.: ${VENDOR_COMMAND}`,
]

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

  const extracted = await extractedPackages(context, fixture.output, 'web')
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
  const sources = await startSources(context, fixture)
  const url = `${sources.registry.url}${tarballPath(changed.name, changed.version)}`

  await assert.rejects(sources.write('web'), {
    message: integrityMessage(
      changed,
      url,
      integrityOf(changed.algorithm, changedBytes),
      integrityOf(changed.algorithm, tarballBytes(changed.name, changed.version)),
    ),
  })
  assert.deepEqual(await readdir(fixture.output), [])
  assert.equal(requestCount(sources.registry, changed), 1)
})

test('90: a tarball the server answers with 404 fails with the download message and writes nothing', async (context) => {
  const fixture = await writeFixture(context)
  const missing = WEB_NPM_PACKAGES[1]
  await rm(join(fixture.npmFolder, tarballPath(missing.name, missing.version)))
  const sources = await startSources(context, fixture)
  const url = `${sources.registry.url}${tarballPath(missing.name, missing.version)}`

  await assert.rejects(sources.write('web'), {
    message: downloadMessage(missing, url, 404),
  })
  assert.deepEqual(await readdir(fixture.output), [])
  assert.equal(requestCount(sources.registry, missing), 1)
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

  const extracted = await extractedPackages(context, fixture.output, 'web')
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
  const sources = await startSources(context, fixture, { failures: { [`/${path}`]: answers } })
  const started = performance.now()

  await assert.rejects(sources.write('web', { retryDelays: [30, 30] }), {
    message: downloadMessage(failing, `${sources.registry.url}${path}`, answer),
  })
  // A timer may fire a little early, never 5 ms early.
  assert.ok(performance.now() - started >= 55, 'the download did not wait between its attempts')
  assert.equal(requestCount(sources.registry, failing), 3)
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
  const sources = await startSources(context, fixture)

  await assert.rejects(sources.write('web'), {
    message:
      'npm package three@3.0.0 has no integrity in ui/pnpm-lock.yaml, so its tarball cannot be checked (docs/spec/delivery.md section 2.13).',
  })
  assert.deepEqual(sources.registry.requests, [])
  assert.deepEqual(await readdir(fixture.output), [])
})

test('91: the crates of the web scope stay whole and the other crates become stubs Cargo can resolve', async (context) => {
  const fixture = await writeFixture(context)

  await writeSources(context, fixture, 'web')

  const vendor = await extractedVendor(context, fixture.output, 'web')
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

test('91: cargoConfig is the config.toml of section 2.13 with the folder of its scope', () => {
  assert.equal(cargoConfig('web'), configText('rust-crates-web', 'Partsmith'))
  assert.equal(cargoConfig('desktop'), configText('rust-crates-desktop', 'Partsmith'))
})

test('91: the web scope sends the vendor command and the three metadata commands through run, in the repository root', async (context) => {
  const fixture = await writeFixture(context)

  const { cargo } = await writeSources(context, fixture, 'web')

  assert.deepEqual(cargo.calls.toSorted(), WEB_CALLS)
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
  const sources = await startSources(context, fixture)

  await assert.rejects(sources.write('web'), { message })
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

  const vendor = await extractedVendor(context, fixture.output, 'desktop')
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
      `.: ${DESKTOP_VENDOR_COMMAND}`,
    ].toSorted(),
  )
})

test("91: the desktop scope's npm archive also holds the packages of Windows and macOS", async (context) => {
  const fixture = await writeFixture(context, { desktop: true })

  await writeSources(context, fixture, 'desktop')

  const extracted = await extractedPackages(context, fixture.output, 'desktop')
  assert.deepEqual(
    await filesBelow(extracted),
    NPM_PACKAGES.map(
      ({ name, version }) => `npm-packages-desktop/${tarballPath(name, version)}`,
    ).toSorted(),
  )
})

test('91: scopeCrates gives each crate of the triples once, sorted by name, with one metadata command per triple', async (context) => {
  const fixture = await writeFixture(context, { desktop: true })
  const cargo = fixtureRun(fixture)

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

/** Asserts that two runs of writeDependencySources for `scope` on `fixture` write the same parts, byte for byte. */
async function assertSameParts(context, fixture, scope, options = {}) {
  const second = join(await temporaryDirectory(context), 'second')
  await mkdir(second)

  await writeSources(context, fixture, scope, options)
  await writeSources(context, fixture, scope, { ...options, output: second })

  const names = (await readdir(fixture.output)).toSorted()
  assert.deepEqual((await readdir(second)).toSorted(), names)
  const [first, again] = await Promise.all(
    [fixture.output, second].map((folder) =>
      Promise.all(names.map((name) => readFile(join(folder, name)))),
    ),
  )
  assert.deepEqual(again, first)
}

test('92: two runs write the same parts, byte for byte', async (context) => {
  await assertSameParts(context, await writeFixture(context), 'web')
})

test('92: the crates archive is a gzip file with no file name and a modification time of 0', async (context) => {
  const fixture = await writeFixture(context)

  await writeSources(context, fixture, 'web')

  const [crates] = archiveNames('web')
  assertBareGzip(await joinedArchive(fixture.output, crates), crates)
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
  const sources = await startSources(context, fixture, { failing: VENDOR_COMMAND })

  await assert.rejects(sources.write('web'), (error) => {
    const [vendorFolder] = sources.cargo.vendorFolders
    assert.equal(
      error.message,
      `cargo vendor --locked --versioned-dirs ${vendorFolder} failed with exit status 101: error: failed to sync`,
    )
    return true
  })
  assert.deepEqual(await readdir(fixture.output), [])
  // The vendor folder is <temporary folder>/rust-crates-web/vendor.
  assert.equal(await exists(join(sources.cargo.vendorFolders[0], '..', '..')), false)
})

/**
 * Asserts that writeDependencySources for `web` fails with the command message of `line` when the stand-in of
 * `failingTool` replaces `command`, writes nothing and removes its temporary folder.
 */
async function assertToolFailure(context, command, version, line) {
  const fixture = await writeFixture(context)
  await failingTool(context, command, version)
  const sources = await startSources(context, fixture)

  await assert.rejects(sources.write('web'), {
    message: `${line} failed with exit status 1: ${command}: stand-in failure`,
  })
  assert.deepEqual(await readdir(fixture.output), [])
  // The vendor folder is <temporary folder>/rust-crates-web/vendor.
  assert.equal(await exists(join(sources.cargo.vendorFolders[0], '..', '..')), false)
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

/** An npm package of NPM_PACKAGES's shape with no list, `name` at `version`, whose tarball holds `files`. */
function packageWith(name, files, version = '1.0.0') {
  return { name, version, algorithm: 'sha512', platform: {}, files }
}

/** A package of `packageWith` whose tarball's `package/package.json` holds `license`. */
function licensed(name, license, version = '1.0.0') {
  return packageWith(name, packageFiles(name, version, license), version)
}

/** Writes the npm tarball of `files` (`gzipTar`) into a new folder and resolves to its path. */
async function tarballFile(context, files) {
  const file = join(await temporaryDirectory(context), 'package.tgz')
  await writeFile(file, gzipTar(files))
  return file
}

/** The license `tarballLicense` reads from the npm tarball of `files`. */
async function licenseOf(context, files) {
  return tarballLicense(await tarballFile(context, files))
}

// The five packages of test 108, in the lock file's order, each with the license of its tarball's package.json; the
// last packs it in the folder `types-x/`.
const LICENSE_PACKAGES = [
  licensed('zero-pkg', 'MIT-0'),
  licensed('gpl-pkg', 'GPL-3.0-only'),
  licensed('dual-pkg', '(MIT OR GPL-3.0-only)'),
  licensed('object-pkg', { type: 'MIT' }),
  packageWith('types-x', {
    'types-x/package.json': packageJson({
      name: 'types-x',
      version: '1.0.0',
      license: 'Python-2.0',
    }),
  }),
]

test("108: allowedLicenses of the repository's deny.toml is the list of 2.10, MIT-0, Python-2.0 and BlueOak-1.0.0 last", async () => {
  assert.deepEqual(allowedLicenses(await readFile(join(repositoryRoot, 'deny.toml'), 'utf8')), [
    'MIT',
    'Apache-2.0',
    'Apache-2.0 WITH LLVM-exception',
    'BSD-2-Clause',
    'BSD-3-Clause',
    'ISC',
    'Zlib',
    'Unicode-3.0',
    'CC0-1.0',
    'MPL-2.0',
    'BSL-1.0',
    'MIT-0',
    'Python-2.0',
    'BlueOak-1.0.0',
  ])
})

test("108: tarballLicense is the license string of the package.json in the tarball's one folder", async (context) => {
  assert.equal(
    await licenseOf(context, { 'package/package.json': packageJson({ license: 'ISC' }) }),
    'ISC',
  )
  assert.equal(
    await licenseOf(context, { 'aria-query/package.json': packageJson({ license: 'MIT' }) }),
    'MIT',
  )
})

test('108: tarballLicense reads the package.json one folder deep, not one deeper', async (context) => {
  const files = {
    'package/lib/package.json': packageJson({ license: 'GPL-3.0-only' }),
    'package/package.json': packageJson({ license: 'ISC' }),
  }

  assert.equal(await licenseOf(context, files), 'ISC')
})

test("108: tarballLicense takes the first package.json one folder deep, in the tarball's order", async (context) => {
  const mit = ['package/package.json', packageJson({ license: 'MIT' })]
  const gpl = ['other/package.json', packageJson({ license: 'GPL-3.0-only' })]

  assert.equal(await licenseOf(context, Object.fromEntries([mit, gpl])), 'MIT')
  assert.equal(await licenseOf(context, Object.fromEntries([gpl, mit])), 'GPL-3.0-only')
})

test('108: tarballLicense is Unknown without a package.json one folder deep or without a license string in it', async (context) => {
  const cases = {
    'no package.json': { 'package/README.md': '# readme\n' },
    'a package.json outside any folder': { 'package.json': packageJson({ license: 'MIT' }) },
    'a package.json that is not valid JSON': { 'package/package.json': '{ "license": "MIT", ' },
    'a license object': { 'package/package.json': packageJson({ license: { type: 'MIT' } }) },
    'only a licenses list': {
      'package/package.json': packageJson({ licenses: [{ type: 'MIT' }] }),
    },
  }

  const licenses = await Promise.all(Object.values(cases).map((files) => licenseOf(context, files)))

  assert.deepEqual(
    Object.fromEntries(Object.keys(cases).map((name, i) => [name, licenses[i]])),
    Object.fromEntries(Object.keys(cases).map((name) => [name, 'Unknown'])),
  )
})

test('108: tarballLicense reads a package.json whose path is longer than 100 bytes as ustar, pax and GNU tar write it', async (context) => {
  // A folder of 99 bytes: ustar splits the path into a prefix and a name, pax writes a path record and GNU tar a long
  // name.
  const folder = 'f'.repeat(99)
  const files = { [`${folder}/package.json`]: packageJson({ license: 'ISC' }) }
  const formats = ['ustar', 'pax', 'gnu']

  const licenses = await Promise.all(
    formats.map(async (format) => {
      const file = join(await temporaryDirectory(context), 'package.tgz')
      await writeFile(file, await gnuTarball(context, files, format, folder))
      return tarballLicense(file)
    }),
  )

  assert.deepEqual(licenses, ['ISC', 'ISC', 'ISC'])
})

test('108: tarballLicense reads the tarball in Node and runs no command', async (context) => {
  const file = await tarballFile(context, {
    'package/package.json': packageJson({ license: 'ISC' }),
  })
  await failingTool(context, 'tar')
  await failingTool(context, 'gzip')

  assert.equal(await tarballLicense(file), 'ISC')
})

test("108: packages under licenses outside the list fail the archive with one line each, in the lock file's order", async (context) => {
  const fixture = await writeFixture(context, { packages: LICENSE_PACKAGES })
  const sources = await startSources(context, fixture)

  await assert.rejects(sources.write('web'), {
    message: [
      notAllowedMessage('gpl-pkg', '1.0.0', 'GPL-3.0-only'),
      notAllowedMessage('object-pkg', '1.0.0', 'Unknown'),
    ].join('\n'),
  })
  assert.deepEqual(await readdir(fixture.output), [])
  assert.deepEqual(
    LICENSE_PACKAGES.map((listed) => requestCount(sources.registry, listed)),
    [1, 1, 1, 1, 1],
  )
  assert.equal(sources.registry.requests.length, LICENSE_PACKAGES.length)
  assert.deepEqual(sources.cargo.calls.toSorted(), WEB_CALLS)
})

test('108: without the two packages outside the list, the npm archive holds the other three', async (context) => {
  const allowed = [LICENSE_PACKAGES[0], LICENSE_PACKAGES[2], LICENSE_PACKAGES[4]]
  const fixture = await writeFixture(context, { packages: allowed })

  await writeSources(context, fixture, 'web')

  const extracted = await extractedPackages(context, fixture.output, 'web')
  assert.deepEqual(
    await filesBelow(extracted),
    allowed.map(({ name, version }) => `npm-packages-web/${tarballPath(name, version)}`).toSorted(),
  )
})

test('108: the license comes from the tarball of the locked version, not from another version of the name', async (context) => {
  const fixture = await writeFixture(context, {
    packages: [
      licensed('same-name', 'GPL-3.0-only', '1.0.0'),
      licensed('same-name', 'MIT', '2.0.0'),
    ],
  })

  await assertWebRejected(context, fixture, notAllowedMessage('same-name', '1.0.0', 'GPL-3.0-only'))
})

/** Whether the lock entry `listed` belongs to the table entry `entry` (section 2.13, Native packages). */
function belongsTo(listed, entry) {
  return (
    (listed.name === entry.name || listed.name.startsWith(`${entry.name}-`)) &&
    listed.version === entry.version
  )
}

/** Whether the lock entry `listed` has an os, cpu or libc list. */
function hasPlatformList(listed) {
  return listed.os !== null || listed.cpu !== null || listed.libc !== null
}

/** The packages of `scope` from the repository's ui/pnpm-lock.yaml. */
async function repositoryScopePackages(scope) {
  const entries = lockPackages(await readFile(join(repositoryRoot, 'ui', 'pnpm-lock.yaml'), 'utf8'))
  return platformPackages(entries, SCOPES[scope].platforms)
}

/** A lock entry of `name` at `version` with the `os`, `cpu` and `libc` lists of `lists`, null where it lacks one. */
function lockEntry(name, version, lists = {}) {
  return {
    key: `${name}@${version}`,
    name,
    version,
    integrity: 'sha512-AAAA',
    os: lists.os ?? null,
    cpu: lists.cpu ?? null,
    libc: lists.libc ?? null,
  }
}

/** A table entry of `name` at `version` with nothing to carry. */
function plainEntry(name, version) {
  return {
    name,
    version,
    source: `https://example.invalid/${stemOf(name)}`,
    tag: `v${version}`,
    crates: [],
  }
}

// The table of the nativeEntries cases of test 109.
const ENTRY_TABLE = [
  plainEntry('@scope/binding', '1.0.0'),
  plainEntry('solo', '2.0.0'),
  plainEntry('unused', '1.0.0'),
]

test('109: NATIVE_PACKAGES is the table of section 2.13, in its order', () => {
  assert.deepEqual(NATIVE_PACKAGES, EXPECTED_NATIVE_PACKAGES)
})

test('109: every package of either scope with an os, cpu or libc list belongs to an entry of NATIVE_PACKAGES', async () => {
  const scopes = await Promise.all(['web', 'desktop'].map(repositoryScopePackages))

  for (const packages of scopes) {
    for (const listed of packages.filter(hasPlatformList)) {
      assert.ok(
        NATIVE_PACKAGES.some((entry) => belongsTo(listed, entry)),
        `${listed.key} belongs to no entry of NATIVE_PACKAGES`,
      )
    }
    assert.doesNotThrow(() => nativeEntries(packages, NATIVE_PACKAGES))
  }
})

test('109: each entry of NATIVE_PACKAGES has a package of the desktop scope that belongs to it', async () => {
  const packages = await repositoryScopePackages('desktop')

  for (const entry of NATIVE_PACKAGES) {
    assert.ok(
      packages.some((listed) => belongsTo(listed, entry)),
      `no package of the desktop scope belongs to ${entry.name} ${entry.version}`,
    )
  }
})

test("109: nativeEntries gives the entries that the packages with a list belong to, each once, in the table's order", () => {
  const packages = [
    lockEntry('@scope/binding-linux-x64-gnu', '1.0.0', { cpu: ['x64'] }),
    lockEntry('@scope/binding', '1.0.0'),
    lockEntry('solo', '2.0.0', { os: ['linux'] }),
    lockEntry('solo-linux-x64', '2.0.0', { cpu: ['x64'] }),
  ]

  assert.deepEqual(nativeEntries(packages, ENTRY_TABLE), ENTRY_TABLE.slice(0, 2))
  assert.deepEqual(nativeEntries(packages.toReversed(), ENTRY_TABLE), ENTRY_TABLE.slice(0, 2))
})

test('109: nativeEntries gives no entry for packages without a list', () => {
  assert.deepEqual(nativeEntries([lockEntry('@scope/binding', '1.0.0')], ENTRY_TABLE), [])
  assert.deepEqual(nativeEntries([], ENTRY_TABLE), [])
})

test('109: nativeEntries throws the native-package message for a package with a list whose version or name no entry has', () => {
  const unknownPackages = [
    lockEntry('@scope/binding-linux-x64-gnu', '1.0.1', { cpu: ['x64'] }),
    lockEntry('@scope/bindingx', '1.0.0', { cpu: ['x64'] }),
  ]

  for (const unknown of unknownPackages) {
    assert.throws(
      () => nativeEntries([lockEntry('solo', '2.0.0', { os: ['linux'] }), unknown], ENTRY_TABLE),
      { message: nativePackageMessage(unknown.name, unknown.version) },
    )
  }
})

test('109: nativeEntries throws for the first package, in their order, that belongs to no entry', () => {
  const first = lockEntry('first-native', '1.0.0', { os: ['linux'] })
  const second = lockEntry('second-native', '1.0.0', { cpu: ['x64'] })

  assert.throws(() => nativeEntries([first, second], ENTRY_TABLE), {
    message: nativePackageMessage('first-native', '1.0.0'),
  })
  assert.throws(() => nativeEntries([second, first], ENTRY_TABLE), {
    message: nativePackageMessage('second-native', '1.0.0'),
  })
})

test('109: a package with a list that no entry records stops the archive before any command or request', async (context) => {
  const fixture = await writeFixture(context)
  const sources = await startSources(context, fixture)

  await assert.rejects(
    sources.write('web', {
      nativePackages: FIXTURE_NATIVE_PACKAGES.filter((entry) => entry.name !== 'three'),
    }),
    { message: nativePackageMessage('three', '3.0.0') },
  )
  assert.deepEqual(sources.cargo.calls, [])
  assert.deepEqual(sources.registry.requests, [])
  assert.deepEqual(sources.crates.requests, [])
  assert.deepEqual(await readdir(fixture.output), [])
})

/**
 * Asserts that writeDependencySources for `web` rejects with the native-package message for `listed`, a package
 * with no list whose tarball holds a prebuilt file, once the tarballs are downloaded, and writes nothing.
 */
async function assertPrebuiltRejected(context, listed) {
  const fixture = await writeFixture(context, { packages: [...WEB_NPM_PACKAGES, listed] })
  const sources = await startSources(context, fixture)

  await assert.rejects(sources.write('web'), {
    message: nativePackageMessage(listed.name, listed.version),
  })
  assert.equal(requestCount(sources.registry, listed), 1)
  assert.deepEqual(sources.cargo.calls.toSorted(), WEB_CALLS)
  assert.deepEqual(await readdir(fixture.output), [])
}

test('109: a tarball that holds a .node file makes its package native, checked before its license', async (context) => {
  const files = {
    ...packageFiles('addon-pkg', '1.0.0', 'GPL-3.0-only'),
    'package/build/addon.node': 'a prebuilt addon\n',
  }

  await assertPrebuiltRejected(context, packageWith('addon-pkg', files))
})

test('109: a tarball that holds a .wasm file makes its package native', async (context) => {
  const files = {
    ...packageFiles('codec-pkg', '1.0.0'),
    'package/lib/codec.wasm': 'a prebuilt codec\n',
  }

  await assertPrebuiltRejected(context, packageWith('codec-pkg', files))
})

/**
 * The npm tarball of `files` that GNU tar writes with `--format={format}`, the folder `folder` and its files in name
 * order, compressed with Node's gzip.
 */
async function gnuTarball(context, files, format, folder = 'package') {
  const directory = await temporaryDirectory(context)
  await writeFiles(directory, files)
  const result = spawnSync(
    'tar',
    [
      `--format=${format}`,
      '--sort=name',
      '--owner=0',
      '--group=0',
      '--numeric-owner',
      '--create',
      '--file=-',
      folder,
    ],
    { cwd: directory, env: { ...process.env, LC_ALL: 'C' }, maxBuffer: 1 << 26 },
  )
  assert.equal(result.status, 0, String(result.stderr))
  return gzipSync(result.stdout)
}

test('109: a .node file whose path is longer than 100 bytes is found under a ustar prefix, a pax path and a GNU long name', async (context) => {
  const cases = [
    ['ustar', `package/${'deep/'.repeat(20)}addon.node`],
    ['pax', `package/lib/${'a'.repeat(120)}.node`],
    ['gnu', `package/lib/${'g'.repeat(120)}.node`],
  ]

  await Promise.all(
    cases.map(async ([format, path]) => {
      assert.ok(Buffer.byteLength(path) > 100, path)
      const name = `long-${format}`
      const files = { ...packageFiles(name, '1.0.0'), [path]: 'addon\n' }
      const bytes = await gnuTarball(context, files, format)
      await assertPrebuiltRejected(context, { ...packageWith(name, {}), bytes })
    }),
  )
})

// The first bytes of the prebuilt files that a name does not show (section 2.13, Native packages), each followed by
// bytes of no meaning.
const PREBUILT_MAGIC_CASES = {
  ELF: '7f454c46',
  'Mach-O 32-bit': 'feedface',
  'Mach-O 64-bit': 'feedfacf',
  'Mach-O 32-bit, little-endian': 'cefaedfe',
  'Mach-O 64-bit, little-endian': 'cffaedfe',
  'Mach-O universal': 'cafebabe',
  'Mach-O universal, 64-bit': 'cafebabf',
  PE: '4d5a',
  WebAssembly: '0061736d',
  'ar archive': '213c617263683e0a',
}

test('109: a file with no telling name whose first bytes are those of prebuilt code makes its package native', async (context) => {
  await Promise.all(
    Object.entries(PREBUILT_MAGIC_CASES).map(async ([kind, hex], i) => {
      const name = `magic-${i}`
      const files = {
        ...packageFiles(name, '1.0.0'),
        'package/bin/tool': Buffer.concat([Buffer.from(hex, 'hex'), Buffer.from('rest')]),
      }
      await assert.doesNotReject(assertPrebuiltRejected(context, packageWith(name, files)), kind)
    }),
  )
})

test('109: a script, a short file and magic bytes after the start are no prebuilt files', async (context) => {
  const listed = packageWith('plain-pkg', {
    ...packageFiles('plain-pkg', '1.0.0'),
    'package/bin/run': '#!/usr/bin/env node\nconsole.log(1)\n',
    'package/short': 'M',
    'package/empty': '',
    'package/later.txt': Buffer.concat([Buffer.from('text '), Buffer.from('7f454c46', 'hex')]),
  })
  const fixture = await writeFixture(context, { packages: [...WEB_NPM_PACKAGES, listed] })

  await writeSources(context, fixture, 'web')

  const extracted = await extractedPackages(context, fixture.output, 'web')
  assert.ok(
    await exists(join(extracted, 'npm-packages-web', tarballPath(listed.name, listed.version))),
  )
})

test('109: with an entry for it, a package whose tarball holds a .node file passes', async (context) => {
  const listed = packageWith('addon-pkg', {
    ...packageFiles('addon-pkg', '1.0.0'),
    'package/build/addon.node': 'a prebuilt addon\n',
  })
  const fixture = await writeFixture(context, { packages: [...WEB_NPM_PACKAGES, listed] })

  const { result } = await writeSources(context, fixture, 'web', {
    nativePackages: [...FIXTURE_NATIVE_PACKAGES, plainEntry('addon-pkg', '1.0.0')],
  })

  assert.deepEqual(
    result.map(({ archive }) => archive),
    archiveNames('web'),
  )
})

test('109: a folder named like a prebuilt file and a file whose name only holds .node are no prebuilt files', async (context) => {
  const listed = packageWith('notes-pkg', {
    ...packageFiles('notes-pkg', '1.0.0'),
    'package/x.node/': '',
    'package/x.node/index.js': 'export {}\n',
    'package/notes.node.txt': 'notes\n',
  })
  const fixture = await writeFixture(context, { packages: [...WEB_NPM_PACKAGES, listed] })

  await writeSources(context, fixture, 'web')

  const extracted = await extractedPackages(context, fixture.output, 'web')
  assert.ok(
    await exists(join(extracted, 'npm-packages-web', tarballPath(listed.name, listed.version))),
  )
})

// The versions of the native-css fixture repository of test 110, each a tag.
const NATIVE_CSS_VERSIONS = ['1.0.0', '1.1.0']

/** The files of the native-css fixture repository at v1.0.0, whose Cargo.lock also lists `locked` (test 110). */
function nativeCssFiles(locked = []) {
  const root = { name: 'native_css_node', version: '1.0.0' }
  return {
    'node/Cargo.toml': '[package]\nname = "native_css_node"\nversion = "1.0.0"\n',
    'Cargo.lock': cargoLockText([
      { name: 'cssparser', version: '0.33.0', source: CRATES_IO, checksum: 'c33' },
      ...locked,
      root,
    ]),
    'README.md': 'native-css 1.0.0\n',
  }
}

/**
 * Makes the Git repository of test 110 in a new folder: a commit of `nativeCssFiles(locked)` tagged `v1.0.0`, and a
 * second commit, which changes README.md, tagged `v1.1.0`. Resolves to its folder, its Git environment, its `file:`
 * URL and the commit of each version.
 */
async function makeNativeRepository(context, locked = []) {
  const folder = await temporaryDirectory(context)
  const environment = await isolatedGitEnvironment(folder)
  const repository = join(folder, 'native-css')
  await mkdir(repository)
  git(environment, repository, 'init', '--initial-branch=main')
  await writeFiles(repository, nativeCssFiles(locked))
  const commits = {}
  for (const version of NATIVE_CSS_VERSIONS) {
    // oxlint-disable-next-line no-await-in-loop -- each version is a commit after the one before
    await writeFile(join(repository, 'README.md'), `native-css ${version}\n`)
    git(environment, repository, 'add', '--all')
    git(environment, repository, 'commit', '--message', `native-css ${version}`)
    git(
      environment,
      repository,
      'tag',
      '--annotate',
      `v${version}`,
      '--message',
      `native-css ${version}`,
    )
    commits[version] = git(environment, repository, 'rev-parse', 'HEAD')
  }
  return { repository, environment, source: pathToFileURL(repository).href, commits }
}

/** The table entries of test 110 for `versions` of the native-css repository `native`. */
function nativeCssTable(native, versions = NATIVE_CSS_VERSIONS) {
  return versions.map((version) => ({
    name: 'native-css',
    version,
    source: native.source,
    tag: `v${version}`,
    commit: native.commits[version],
    manifest: 'node/Cargo.toml',
    crates: [],
  }))
}

/** The packages of the lock text of test 110: native-css-linux-x64-gnu at each of `versions`, with lists. */
function nativeCssPackages(versions = NATIVE_CSS_VERSIONS) {
  return versions.map((version) => ({
    name: 'native-css-linux-x64-gnu',
    version,
    algorithm: 'sha512',
    platform: { cpu: 'x64', os: 'linux' },
  }))
}

/** The source archive of `version` of `native` as section 2.13 makes it: `git archive` of its tag and `gzip -9 -n`. */
function expectedSourceArchive(native, version) {
  const tar = spawnSync(
    'git',
    [
      '-C',
      native.repository,
      '-c',
      'core.autocrlf=false',
      '-c',
      'tar.umask=0002',
      '-c',
      'core.attributesFile=/dev/null',
      'archive',
      '--format=tar',
      `--prefix=native-css-${version}/`,
      `v${version}`,
    ],
    { env: native.environment, maxBuffer: 1 << 26 },
  )
  assert.equal(tar.status, 0, String(tar.stderr))
  const gzip = spawnSync('gzip', ['-9', '-n'], { input: tar.stdout, maxBuffer: 1 << 26 })
  assert.equal(gzip.status, 0, String(gzip.stderr))
  return gzip.stdout
}

/**
 * Runs writeDependencySources for `web` with the native-css table of `native` for `tableVersions` and a lock text
 * that holds the packages of `lockVersions`, and resolves to the fixture and the sources it used.
 */
async function writeNativeCss(
  context,
  native,
  { tableVersions = NATIVE_CSS_VERSIONS, lockVersions = NATIVE_CSS_VERSIONS } = {},
) {
  const fixture = await writeFixture(context, {
    packages: [...WEB_NPM_PACKAGES, ...nativeCssPackages(lockVersions)],
  })
  const sources = await writeSources(context, fixture, 'web', {
    nativePackages: [...FIXTURE_NATIVE_PACKAGES, ...nativeCssTable(native, tableVersions)],
  })
  return { fixture, ...sources }
}

test('110: the npm archive holds npm-sources-web after npm-packages-web, with each version and one crates archive', async (context) => {
  const native = await makeNativeRepository(context)

  const { fixture } = await writeNativeCss(context, native)

  const [, npmArchive] = archiveNames('web')
  const tar = await joinedArchive(fixture.output, npmArchive)
  assert.deepEqual(topFolders(tar), ['npm-packages-web', 'npm-sources-web'])
  const extracted = await extractTar(context, tar)
  assert.deepEqual((await readdir(join(extracted, 'npm-sources-web'))).toSorted(), [
    'native-css-1.0.0-source.tar.gz',
    'native-css-1.1.0-source.tar.gz',
    'native-css-crates.tar.gz',
  ])
})

test('110: each source archive is git archive of its tag with gzip -9 -n, under its own folder, naming the tag commit', async (context) => {
  const native = await makeNativeRepository(context)

  const { fixture } = await writeNativeCss(context, native)

  const sources = join(await extractedPackages(context, fixture.output, 'web'), 'npm-sources-web')
  const archives = await Promise.all(
    NATIVE_CSS_VERSIONS.map((version) =>
      readFile(join(sources, `native-css-${version}-source.tar.gz`)),
    ),
  )
  for (const [i, version] of NATIVE_CSS_VERSIONS.entries()) {
    const archive = archives[i]
    assertBareGzip(archive, `native-css-${version}-source.tar.gz`)
    assert.deepEqual(archive, expectedSourceArchive(native, version))
    const files = tarEntries(gunzipSync(archive)).filter((path) => !path.endsWith('/'))
    assert.deepEqual(files.toSorted(), [
      `native-css-${version}/Cargo.lock`,
      `native-css-${version}/README.md`,
      `native-css-${version}/node/Cargo.toml`,
    ])
    const commitId = spawnSync('git', ['get-tar-commit-id'], {
      input: gunzipSync(archive),
      encoding: 'utf8',
    })
    assert.equal(commitId.stdout.trim(), native.commits[version])
  }
})

test('110: the crates archive holds config.toml for the native package and the vendored crates of its lock files', async (context) => {
  const native = await makeNativeRepository(context)

  const { fixture, cargo } = await writeNativeCss(context, native)

  const sources = join(await extractedPackages(context, fixture.output, 'web'), 'npm-sources-web')
  const crates = await extractTar(
    context,
    gunzipSync(await readFile(join(sources, 'native-css-crates.tar.gz'))),
  )
  assert.deepEqual(await readdir(crates), ['native-css-crates'])
  assert.equal(
    await readFile(join(crates, 'native-css-crates', 'config.toml'), 'utf8'),
    configText('native-css-crates', 'native-css'),
  )
  assert.equal(
    vendorConfig('native-css-crates', 'native-css'),
    configText('native-css-crates', 'native-css'),
  )
  assert.deepEqual(
    await folderTexts(join(crates, 'native-css-crates', 'vendor', 'cssparser-0.33.0')),
    vendoredCrate(NATIVE_VENDOR_FILES, 'c33'),
  )
  assert.deepEqual(
    cargo.calls.filter((line) => line.includes('--manifest-path')),
    [
      '.: cargo vendor --locked --versioned-dirs --manifest-path <temporary folder>/clones/native-css-1.0.0/node/Cargo.toml --sync <temporary folder>/clones/native-css-1.1.0/node/Cargo.toml <vendor folder>',
    ],
  )
  const nativeVendor = cargo.vendorFolders.find((folder) =>
    folder.endsWith('/native-css-crates/vendor'),
  )
  assert.notEqual(nativeVendor, undefined, 'no vendor folder native-css-crates/vendor')
})

test('110: with one version in the lock file, the vendor command has no --sync and the folder no other version', async (context) => {
  const native = await makeNativeRepository(context)

  const { fixture, cargo } = await writeNativeCss(context, native, { lockVersions: ['1.0.0'] })

  const sources = join(await extractedPackages(context, fixture.output, 'web'), 'npm-sources-web')
  assert.deepEqual((await readdir(sources)).toSorted(), [
    'native-css-1.0.0-source.tar.gz',
    'native-css-crates.tar.gz',
  ])
  assert.deepEqual(
    cargo.calls.filter((line) => line.includes('--manifest-path')),
    [
      '.: cargo vendor --locked --versioned-dirs --manifest-path <temporary folder>/clones/native-css-1.0.0/node/Cargo.toml <vendor folder>',
    ],
  )
})

test("110: vendorConfig of Partsmith's crates folder is cargoConfig", () => {
  assert.equal(vendorConfig('rust-crates-web', 'Partsmith'), cargoConfig('web'))
  assert.equal(vendorConfig('rust-crates-desktop', 'Partsmith'), cargoConfig('desktop'))
})

test('110: two runs with the source of a native package write the same parts', async (context) => {
  const native = await makeNativeRepository(context)
  const fixture = await writeFixture(context, {
    packages: [...WEB_NPM_PACKAGES, ...nativeCssPackages()],
  })

  await assertSameParts(context, fixture, 'web', {
    nativePackages: [...FIXTURE_NATIVE_PACKAGES, ...nativeCssTable(native)],
  })
})

test('110: a tag that names another commit than the table pins fails with the tag-commit message', async (context) => {
  const native = await makeNativeRepository(context)
  const fixture = await writeFixture(context, {
    packages: [...WEB_NPM_PACKAGES, ...nativeCssPackages()],
  })
  const [first, second] = nativeCssTable(native)
  const moved = { ...second, commit: first.commit }
  const sources = await startSources(context, fixture)

  await assert.rejects(
    sources.write('web', { nativePackages: [...FIXTURE_NATIVE_PACKAGES, first, moved] }),
    { message: tagCommitMessage(moved, native.commits['1.1.0']) },
  )
  assert.deepEqual(await readdir(fixture.output), [])
})

test('110: with no package of an entry that carries something, the npm archive has no npm-sources-web', async (context) => {
  const native = await makeNativeRepository(context)

  const { fixture } = await writeNativeCss(context, native, { lockVersions: [] })

  const [, npmArchive] = archiveNames('web')
  assert.deepEqual(topFolders(await joinedArchive(fixture.output, npmArchive)), [
    'npm-packages-web',
  ])
})

test('110: a package of the source lock file from a Git source fails before its crates are vendored', async (context) => {
  const native = await makeNativeRepository(context, [GIT_LOCKED])
  const fixture = await writeFixture(context, {
    packages: [...WEB_NPM_PACKAGES, ...nativeCssPackages(['1.0.0'])],
  })
  const sources = await startSources(context, fixture)

  await assert.rejects(
    sources.write('web', {
      nativePackages: [...FIXTURE_NATIVE_PACKAGES, ...nativeCssTable(native, ['1.0.0'])],
    }),
    { message: toolLockMessage(GIT_LOCKED, 'native-css 1.0.0') },
  )
  assert.deepEqual(
    sources.cargo.calls.filter((line) => line.includes('--manifest-path')),
    [],
  )
  assert.deepEqual(await readdir(fixture.output), [])
})

test('110: a clone that fails three times fails with the command message of the last attempt, after the waits', async (context) => {
  const missing = pathToFileURL(join(await temporaryDirectory(context), 'missing')).href
  const entry = {
    ...plainEntry('native-css', '1.0.0'),
    source: missing,
    commit: 'a'.repeat(40),
    manifest: 'node/Cargo.toml',
  }
  const fixture = await writeFixture(context, {
    packages: [...WEB_NPM_PACKAGES, ...nativeCssPackages(['1.0.0'])],
  })
  const sources = await startSources(context, fixture)
  const started = performance.now()

  await assert.rejects(
    sources.write('web', {
      nativePackages: [...FIXTURE_NATIVE_PACKAGES, entry],
      retryDelays: [30, 30],
    }),
    (error) => {
      const escaped = missing.replaceAll(/[.*+?^${}()|[\]\\]/g, '\\$&')
      assert.match(
        error.message,
        new RegExp(
          `^git clone --quiet --depth 1 --branch v1\\.0\\.0 ${escaped} \\S+/clones/native-css-1\\.0\\.0 failed with exit status 128: `,
        ),
      )
      return true
    },
  )
  // A timer may fire a little early, never 5 ms early.
  assert.ok(performance.now() - started >= 55, 'the clone did not wait between its attempts')
  assert.deepEqual(await readdir(fixture.output), [])
})

// The MPL-2.0 crate of test 111, as the crate registry serves it.
const MPL_CRATE_BYTES = gzipTar({
  'mpl-crate-0.1.0/Cargo.toml': '[package]\nname = "mpl-crate"\nversion = "0.1.0"\n',
  'mpl-crate-0.1.0/LICENSE': 'Mozilla Public License Version 2.0\n',
})
const MPL_CRATE_PATH = 'mpl-crate/mpl-crate-0.1.0.crate'

// The two table entries of test 111, which both name the MPL-2.0 crate.
const MPL_TABLE = [
  {
    ...plainEntry('native-ui', '2.0.0'),
    crates: [{ name: 'mpl-crate', version: '0.1.0', sha256: sha256Of(MPL_CRATE_BYTES) }],
  },
  {
    ...plainEntry('native-ux', '1.0.0'),
    crates: [{ name: 'mpl-crate', version: '0.1.0', sha256: sha256Of(MPL_CRATE_BYTES) }],
  },
]

// The packages of the lock text of test 111, one of each entry of MPL_TABLE.
const MPL_PACKAGES = [
  {
    name: 'native-ui-linux-x64-gnu',
    version: '2.0.0',
    algorithm: 'sha512',
    platform: { cpu: 'x64', os: 'linux' },
  },
  {
    name: 'native-ux-linux-x64-gnu',
    version: '1.0.0',
    algorithm: 'sha512',
    platform: { cpu: 'x64', os: 'linux' },
  },
]

/** The fixture of test 111, whose crate registry serves `crateBytes` for the MPL-2.0 crate, or nothing when null. */
function mplFixture(context, crateBytes = MPL_CRATE_BYTES) {
  return writeFixture(context, {
    packages: [...WEB_NPM_PACKAGES, ...MPL_PACKAGES],
    crateFiles: crateBytes === null ? {} : { [MPL_CRATE_PATH]: crateBytes },
  })
}

/** The options of test 111's calls: FIXTURE_NATIVE_PACKAGES and MPL_TABLE. */
const MPL_OPTIONS = { nativePackages: [...FIXTURE_NATIVE_PACKAGES, ...MPL_TABLE] }

test('111: a crate under MPL-2.0 that two entries name is in npm-sources-web once, as served', async (context) => {
  const fixture = await mplFixture(context)

  const { crates } = await writeSources(context, fixture, 'web', MPL_OPTIONS)

  const sources = join(await extractedPackages(context, fixture.output, 'web'), 'npm-sources-web')
  assert.deepEqual(await readdir(sources), ['mpl-crate-0.1.0.crate'])
  assert.deepEqual(await readFile(join(sources, 'mpl-crate-0.1.0.crate')), MPL_CRATE_BYTES)
  assert.deepEqual(crates.requests, [`/${MPL_CRATE_PATH}`])
})

test('111: a crate whose bytes differ fails with the crate checksum message and writes nothing', async (context) => {
  const other = Buffer.from('other bytes\n')
  const fixture = await mplFixture(context, other)
  const sources = await startSources(context, fixture)

  await assert.rejects(sources.write('web', MPL_OPTIONS), {
    message: crateChecksumMessage(
      'mpl-crate',
      '0.1.0',
      `${sources.crates.url}${MPL_CRATE_PATH}`,
      sha256Of(other),
      sha256Of(MPL_CRATE_BYTES),
    ),
  })
  assert.deepEqual(await readdir(fixture.output), [])
})

test('111: a crate the server answers with 404 fails with the crate download message and writes nothing', async (context) => {
  const fixture = await mplFixture(context, null)
  const sources = await startSources(context, fixture)

  await assert.rejects(sources.write('web', MPL_OPTIONS), {
    message: crateDownloadMessage(
      'mpl-crate',
      '0.1.0',
      `${sources.crates.url}${MPL_CRATE_PATH}`,
      404,
    ),
  })
  assert.deepEqual(await readdir(fixture.output), [])
})

test('111: a crate download answered 429 is tried again', async (context) => {
  const fixture = await mplFixture(context)

  const { crates } = await writeSources(context, fixture, 'web', {
    ...MPL_OPTIONS,
    crateFailures: { [`/${MPL_CRATE_PATH}`]: [429] },
  })

  const sources = join(await extractedPackages(context, fixture.output, 'web'), 'npm-sources-web')
  assert.deepEqual(await readFile(join(sources, 'mpl-crate-0.1.0.crate')), MPL_CRATE_BYTES)
  assert.deepEqual(crates.requests, [`/${MPL_CRATE_PATH}`, `/${MPL_CRATE_PATH}`])
})

test('112: TAURI_CLI is the tauri-cli of WINDOWS_INSTALLER with the SHA-256 of the crates.io index', () => {
  assert.deepEqual(TAURI_CLI, {
    version: '2.12.2',
    sha256: '7a08bf51957aad7b46276ba4242b2edeb80b6afae2b42b9547f9b9ae485db465',
  })
  assert.equal(TAURI_CLI.version, WINDOWS_INSTALLER.tauriCli)
  assert.equal(CRATE_REGISTRY, 'https://static.crates.io/crates/')
})

test('112: lockCrates gives name, version and source of every package of a Cargo lock file, in its order', () => {
  const lockText = cargoLockText([
    { name: 'tauri-cli', version: '9.9.9', dependencies: ['git-dep', 'stubbed'] },
    { ...GIT_LOCKED, dependencies: ['stubbed'] },
    { ...STUBBED_LOCKED, dependencies: ['kept'] },
  ])

  assert.deepEqual(lockCrates(lockText), [
    { name: 'tauri-cli', version: '9.9.9', source: null },
    { name: 'git-dep', version: '0.1.0', source: GIT_LOCKED.source },
    { name: 'stubbed', version: '2.0.0', source: CRATES_IO },
  ])
})

test('112: lockCrates of a lock file without packages gives none', () => {
  assert.deepEqual(lockCrates(cargoLockText([])), [])
})

/** The fixture of test 112: desktop, with tool-only in the vendor fixture and tauri-cli's lock file listing `locked`. */
function tauriFixture(context, locked = [STUBBED_LOCKED, TOOL_ONLY_LOCKED]) {
  return writeFixture(context, { desktop: true, toolOnly: true, tauriLocked: locked })
}

test('112: the desktop crate archive holds tauri-cli with its files unchanged and a checksum file of them', async (context) => {
  const fixture = await tauriFixture(context)

  await writeSources(context, fixture, 'desktop')

  const vendor = await extractedVendor(context, fixture.output, 'desktop')
  const files = tauriCrateFiles([STUBBED_LOCKED, TOOL_ONLY_LOCKED])
  const paths = Object.keys(files).toSorted((a, b) =>
    Buffer.compare(Buffer.from(a), Buffer.from(b)),
  )
  const checksum = JSON.stringify({
    files: Object.fromEntries(paths.map((path) => [path, sha256Of(files[path])])),
    package: fixture.tauriCli.sha256,
  })
  assert.deepEqual(await folderTexts(join(vendor, `tauri-cli-${TAURI_FIXTURE_VERSION}`)), {
    ...files,
    '.cargo-checksum.json': checksum,
  })
})

test("112: the crates of tauri-cli's lock file stay whole, and a crate of neither lock file stays a stub", async (context) => {
  const fixture = await tauriFixture(context)

  const { cargo, crates } = await writeSources(context, fixture, 'desktop')

  const vendor = await extractedVendor(context, fixture.output, 'desktop')
  assert.deepEqual(await folderTexts(join(vendor, 'stubbed-2.0.0')), {
    ...STUBBED_FILES,
    '.cargo-checksum.json': checksumText(
      { 'Cargo.toml': STUBBED_FILES['Cargo.toml'], 'src/lib.rs': STUBBED_FILES['src/lib.rs'] },
      'abc',
    ),
  })
  assert.deepEqual(await folderTexts(join(vendor, 'tool-only-1.0.0')), {
    ...TOOL_ONLY_FILES,
    '.cargo-checksum.json': checksumText(TOOL_ONLY_FILES, 't1'),
  })
  assert.deepEqual(await folderTexts(join(vendor, 'unlisted-3.0.0')), UNLISTED_STUB)
  assert.deepEqual(
    cargo.calls.filter((line) => line.includes('cargo vendor')),
    [`.: ${DESKTOP_VENDOR_COMMAND}`],
  )
  assert.deepEqual(crates.requests, [`/tauri-cli/tauri-cli-${TAURI_FIXTURE_VERSION}.crate`])
})

test('112: the web scope vendors without --sync and downloads no tauri-cli', async (context) => {
  const fixture = await tauriFixture(context)

  const { cargo, crates } = await writeSources(context, fixture, 'web')

  assert.deepEqual(
    cargo.calls.filter((line) => line.includes('cargo vendor')),
    [`.: ${VENDOR_COMMAND}`],
  )
  assert.deepEqual(crates.requests, [])
})

test('112: two desktop runs write the same parts, byte for byte', async (context) => {
  await assertSameParts(context, await tauriFixture(context), 'desktop')
})

test('112: a tauri-cli crate whose SHA-256 differs fails with the crate checksum message before any cargo vendor', async (context) => {
  const fixture = await tauriFixture(context)
  const sources = await startSources(context, fixture)
  const pinned = { version: TAURI_FIXTURE_VERSION, sha256: 'b'.repeat(64) }

  await assert.rejects(sources.write('desktop', { tauriCli: pinned }), {
    message: crateChecksumMessage(
      'tauri-cli',
      TAURI_FIXTURE_VERSION,
      `${sources.crates.url}tauri-cli/tauri-cli-${TAURI_FIXTURE_VERSION}.crate`,
      fixture.tauriCli.sha256,
      pinned.sha256,
    ),
  })
  assert.deepEqual(
    sources.cargo.calls.filter((line) => line.includes('cargo vendor')),
    [],
  )
  assert.deepEqual(await readdir(fixture.output), [])
})

test("112: a package of tauri-cli's lock file from a Git source fails before any cargo vendor", async (context) => {
  const fixture = await tauriFixture(context, [GIT_LOCKED, STUBBED_LOCKED])
  const sources = await startSources(context, fixture)

  await assert.rejects(sources.write('desktop'), {
    message: toolLockMessage(GIT_LOCKED, `tauri-cli ${TAURI_FIXTURE_VERSION}`),
  })
  assert.deepEqual(
    sources.cargo.calls.filter((line) => line.includes('cargo vendor')),
    [],
  )
  assert.deepEqual(await readdir(fixture.output), [])
})

test('112: a temporary folder inside the repository stops the desktop scope before any command or request', async (context) => {
  const fixture = await tauriFixture(context)
  const inside = join(fixture.root, 'target', 'tmp')
  await mkdir(inside, { recursive: true })
  const sources = await startSources(context, fixture)
  const message = temporaryFolderMessage(await realpath(inside), await realpath(fixture.root))
  const restore = useTemporaryFolder(context, inside)

  await assert.rejects(sources.write('desktop'), { message })
  restore()

  assert.deepEqual(sources.cargo.calls, [])
  assert.deepEqual(sources.registry.requests, [])
  assert.deepEqual(sources.crates.requests, [])
  assert.deepEqual(await readdir(fixture.output), [])
})

test('112: a temporary folder inside the repository does not stop the web scope', async (context) => {
  const fixture = await tauriFixture(context)
  const inside = join(fixture.root, 'target', 'tmp')
  await mkdir(inside, { recursive: true })
  const sources = await startSources(context, fixture)
  const restore = useTemporaryFolder(context, inside)

  const result = await sources.write('web')
  restore()

  assert.deepEqual(
    result.map(({ archive }) => archive),
    archiveNames('web'),
  )
})

test('113: the desktop crate archive leaves vendor/dbus/ out of libdbus-sys and out of its checksum file', async (context) => {
  const fixture = await writeFixture(context, { desktop: true, libdbus: true })

  await writeSources(context, fixture, 'desktop')

  const vendor = await extractedVendor(context, fixture.output, 'desktop')
  const kept = {
    'Cargo.toml': LIBDBUS_FILES['Cargo.toml'],
    'build.rs': LIBDBUS_FILES['build.rs'],
    'src/lib.rs': LIBDBUS_FILES['src/lib.rs'],
  }
  assert.deepEqual(await folderTexts(join(vendor, 'libdbus-sys-0.2.7')), {
    ...kept,
    '.cargo-checksum.json': `{"files":{"Cargo.toml":"${sha256Of(kept['Cargo.toml'])}","build.rs":"${sha256Of(kept['build.rs'])}","src/lib.rs":"${sha256Of(kept['src/lib.rs'])}"},"package":"ld"}`,
  })
})

test('113: in the web crate archive libdbus-sys is a stub with its targets', async (context) => {
  const fixture = await writeFixture(context, { desktop: true, libdbus: true })

  await writeSources(context, fixture, 'web')

  const vendor = await extractedVendor(context, fixture.output, 'web')
  assert.deepEqual(await folderTexts(join(vendor, 'libdbus-sys-0.2.7')), {
    '.cargo-checksum.json': '{"files":{},"package":"ld"}',
    'Cargo.toml': LIBDBUS_FILES['Cargo.toml'],
    'build.rs': '',
    'src/lib.rs': '',
  })
})

test('113: EXCLUDED_LICENSE_FILES is still the one entry of libdbus-sys and vendor/dbus/', () => {
  assert.deepEqual(
    EXCLUDED_LICENSE_FILES.map(({ crate, path }) => ({ crate, path })),
    [{ crate: 'libdbus-sys', path: 'vendor/dbus/' }],
  )
})

// The table entry of test 115, whose tarballs lack the notices of the crates it compiles in.
const NOTICES_ENTRY = {
  ...plainEntry('native-ui', '2.0.0'),
  notices: { crate: 'native_ui', commit: '0'.repeat(40) },
}
const NOTICES_TEXT = 'cssparser 0.33.0, MPL-2.0\n\nMozilla Public License Version 2.0\n'

/**
 * Runs writeDependencySources for `web` with the table entry `entry` and a lock text that holds its
 * `-linux-x64-gnu` package, with the repository root holding `rootFiles`. Resolves to the fixture and the sources.
 */
async function writeNotices(context, entry, rootFiles) {
  const fixture = await writeFixture(context, {
    packages: [
      ...WEB_NPM_PACKAGES,
      {
        name: `${entry.name}-linux-x64-gnu`,
        version: entry.version,
        algorithm: 'sha512',
        platform: { cpu: 'x64', os: 'linux' },
      },
    ],
    rootFiles,
  })
  const sources = await startSources(context, fixture)
  return { fixture, sources, options: { nativePackages: [...FIXTURE_NATIVE_PACKAGES, entry] } }
}

test('115: an entry with notices gives a copy of its committed notices file in npm-sources-web', async (context) => {
  const { fixture, sources, options } = await writeNotices(context, NOTICES_ENTRY, {
    'ui/scripts/notices/native/native-ui-2.0.0.txt': NOTICES_TEXT,
  })

  await sources.write('web', options)

  const folder = join(await extractedPackages(context, fixture.output, 'web'), 'npm-sources-web')
  assert.deepEqual(await readdir(folder), ['native-ui-2.0.0-notices.txt'])
  assert.equal(await readFile(join(folder, 'native-ui-2.0.0-notices.txt'), 'utf8'), NOTICES_TEXT)
})

test('115: the notices file of a scoped entry is named after its stem', async (context) => {
  const entry = { ...NOTICES_ENTRY, name: '@scope/native-ui' }
  const { fixture, sources, options } = await writeNotices(context, entry, {
    'ui/scripts/notices/native/scope-native-ui-2.0.0.txt': NOTICES_TEXT,
  })

  await sources.write('web', options)

  const folder = join(await extractedPackages(context, fixture.output, 'web'), 'npm-sources-web')
  assert.deepEqual(await readdir(folder), ['scope-native-ui-2.0.0-notices.txt'])
})

test('115: a missing notices file fails with the native-notices message and writes nothing', async (context) => {
  const { fixture, sources, options } = await writeNotices(context, NOTICES_ENTRY, {})

  await assert.rejects(sources.write('web', options), {
    message: nativeNoticesMessage(NOTICES_ENTRY),
  })
  assert.deepEqual(await readdir(fixture.output), [])
})

test('115: an entry without notices gives no notices file', async (context) => {
  const { fixture, sources, options } = await writeNotices(
    context,
    plainEntry('native-ui', '2.0.0'),
    {
      'ui/scripts/notices/native/native-ui-2.0.0.txt': NOTICES_TEXT,
    },
  )

  await sources.write('web', options)

  const [, npmArchive] = archiveNames('web')
  assert.deepEqual(topFolders(await joinedArchive(fixture.output, npmArchive)), [
    'npm-packages-web',
  ])
})

test('115: every entry of NATIVE_PACKAGES with notices has its notices file, and the folder holds no other file', async () => {
  const folder = join(repositoryRoot, 'ui', 'scripts', 'notices', 'native')
  const expected = NATIVE_PACKAGES.filter((entry) => entry.notices).map(
    (entry) => `${stemOf(entry.name)}-${entry.version}.txt`,
  )

  assert.deepEqual((await readdir(folder)).toSorted(), expected.toSorted())
  const texts = await Promise.all(expected.map((name) => readFile(join(folder, name))))
  for (const [i, name] of expected.entries()) {
    const text = texts[i].toString('utf8')
    assert.ok(text.length > 0, `${name} is empty`)
    assert.deepEqual(Buffer.from(text, 'utf8'), texts[i], `${name} is not UTF-8`)
    assert.equal(text.includes('\r'), false, `${name} has a CR`)
    assert.match(text.split('\n')[0], /^\S+ \S+, \S.*$/, `${name} does not start with a crate line`)
  }
})

// An entry line of a native package's notices file (section 2.13): name, version, license (`see texts` for none),
// URL, where its texts come from when not shipped, and the numbers of its texts.
const NOTICES_LINE =
  /^(\S+) (\S+), (.+), (https:\/\/[^ ,]+)(?:, (standard text|text from its repository))?, texts ((?:\[\d+\])(?: \[\d+\])*)$/

// The `textSource` of an entry for the words its line ends with before its texts; `shipped` when it has none.
const TEXT_SOURCES = { 'standard text': 'standard', 'text from its repository': 'repository' }

/**
 * The entries of a native package's notices file `text`, as renderCrateNotices takes them, read back from its entry
 * lines and its numbered texts. Throws when a line or a text block does not have the layout of section 2.13.
 */
function parsedNotices(text) {
  const listEnd = text.indexOf('\n\n')
  const textBlocks = text.slice(listEnd + 2, -1).split(/\n\n(?=\[\d+\]\n\n)/)
  const texts = new Map(
    textBlocks.map((block) => {
      const header = /^\[(\d+)\]\n\n/.exec(block)
      assert.ok(header !== null, `a text block starts with ${block.slice(0, 20)}`)
      return [header[1], `${block.slice(header[0].length)}\n`]
    }),
  )
  assert.equal(texts.size, textBlocks.length, 'a text number is defined twice')
  return text
    .slice(0, listEnd)
    .split('\n')
    .map((line) => {
      const match = NOTICES_LINE.exec(line)
      assert.ok(match !== null, `not an entry line: ${line}`)
      const [, name, version, license, url, from, numbers] = match
      return {
        name,
        version,
        license: license === 'see texts' ? null : license,
        url,
        platforms: null,
        textSource: TEXT_SOURCES[from] ?? 'shipped',
        texts: numbers.match(/\d+/g).map((number) => texts.get(number)),
      }
    })
}

test('115: each notices file lists its crates sorted, each text once, numbered in first use, none unreferenced', async () => {
  const folder = join(repositoryRoot, 'ui', 'scripts', 'notices', 'native')
  const files = NATIVE_PACKAGES.filter((entry) => entry.notices !== undefined).map(
    (entry) => `${stemOf(entry.name)}-${entry.version}.txt`,
  )

  for (const name of files) {
    // oxlint-disable-next-line no-await-in-loop -- one file of about 500 KB at a time
    const text = await readFile(join(folder, name), 'utf8')
    const entries = parsedNotices(text)
    assert.deepEqual(
      entries.map(({ name: crate, version }) => `${crate} ${version}`),
      entries
        .toSorted(compareNameAndVersion)
        .map(({ name: crate, version }) => `${crate} ${version}`),
      `${name} is not sorted by name and version`,
    )
    assert.ok(
      entries.every((entry) => entry.texts.every((listed) => listed !== undefined)),
      `${name} names a text it does not hold`,
    )
    // Rendering the entries again numbers the texts in first use and drops a text no line names.
    assert.equal(renderCrateNotices(entries), text, `${name} does not have the layout of 2.13`)
  }
})

test('115: the layout check reads back what renderCrateNotices writes and refuses a text no line names', () => {
  const entries = [
    {
      name: 'a',
      version: '1.0.0',
      license: 'MIT',
      url: 'https://crates.io/crates/a/1.0.0',
      platforms: null,
      textSource: 'shipped',
      texts: ['first\n', 'second\n'],
    },
    {
      name: 'b',
      version: '0.1.0',
      license: null,
      url: 'https://example.com/tree/v1',
      platforms: null,
      textSource: 'repository',
      texts: ['second\n'],
    },
    {
      name: 'c',
      version: '2.0.0',
      license: 'MIT/Apache-2.0',
      url: 'https://crates.io/crates/c/2.0.0',
      platforms: null,
      textSource: 'standard',
      texts: ['third\n'],
    },
  ]
  const text = renderCrateNotices(entries)

  assert.equal(
    text,
    'a 1.0.0, MIT, https://crates.io/crates/a/1.0.0, texts [1] [2]\nb 0.1.0, see texts, https://example.com/tree/v1, text from its repository, texts [2]\nc 2.0.0, MIT/Apache-2.0, https://crates.io/crates/c/2.0.0, standard text, texts [3]\n\n[1]\n\nfirst\n\n[2]\n\nsecond\n\n[3]\n\nthird\n',
  )
  assert.deepEqual(parsedNotices(text), entries)
  const unreferenced = `${text}\n[4]\n\nfourth\n`
  assert.notEqual(renderCrateNotices(parsedNotices(unreferenced)), unreferenced)
})
