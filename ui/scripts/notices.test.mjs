// Acceptance tests 35 to 42 of docs/spec/delivery.md section 6 for the third-party notices generator.
import assert from 'node:assert/strict'
import { spawnSync } from 'node:child_process'
import { cp, mkdir, mkdtemp, readFile, rm, symlink, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { dirname, join, relative } from 'node:path'
import { test } from 'node:test'
import { fileURLToPath } from 'node:url'

import { EMITTED_BUILD_PACKAGES } from './check-licenses.mjs'
import {
  DESKTOP_NOTICES_HEADING,
  DESKTOP_TARGETS,
  WEB_NOTICES_HEADING,
  artifactCrates,
  generateNotices,
  licenseFiles,
  normalizeText,
  prebuiltLibraries,
  renderNotices,
} from './notices.mjs'

const scriptPath = fileURLToPath(new URL('notices.mjs', import.meta.url))
const repositoryRoot = fileURLToPath(new URL('../..', import.meta.url))
const uiDirectory = join(repositoryRoot, 'ui')

const usageLine = 'usage: node scripts/notices.mjs write|check'

const REGISTRY = 'registry+https://github.com/rust-lang/crates.io-index'
const PARTSMITH_REPOSITORY = 'https://github.com/blai30/partsmith'
const WEB_TRIPLE = 'wasm32-unknown-emscripten'

// The repository files that generateNotices reads, copied into each temporary repository root (test 40).
const COPIED_INPUTS = [
  'scripts/occt-version.sh',
  'scripts/emsdk-version.sh',
  'rust-toolchain.toml',
  'LICENSE',
  'ui/public/fonts/Geist-OFL.txt',
  'ui/public/fonts/JetBrainsMono-OFL.txt',
  'ui/scripts/notices',
]

// The OCCT statements of section 2.9 for 7.9.3, V7_9_3 and the Partsmith repository.
const DESKTOP_STATEMENT = [
  'The Partsmith desktop app uses Open CASCADE Technology (OCCT) 7.9.3, https://dev.opencascade.org, copyright OPEN CASCADE SAS and the OCCT contributors. OCCT is free software under the GNU Lesser General Public License version 2.1 with the Open CASCADE exception version 1.0. It includes code from the other projects listed after it, under their own notices. All these texts are below.',
  'OCCT is linked statically into the app. It is the unmodified source of the tag V7_9_3 of https://github.com/Open-Cascade-SAS/OCCT, built as static libraries by the script scripts/build-occt.sh in the Partsmith source, which lists the toolkits and options used.',
  "You may modify OCCT and use your version with Partsmith. The source of each release is on its page under https://github.com/blai30/partsmith/releases: the Partsmith source is the release's tag, v followed by the app's version (for example v0.1.0), with every script, tool version and lock file the build uses, and the OCCT source the build reads is the release's asset occt-7.9.3-source.tar.gz, which scripts/occt-source-archive.sh makes from the tag V7_9_3. To relink: put your OCCT source in <work>/src (from the asset: mkdir -p <work>/src && tar -xzf occt-7.9.3-source.tar.gz --strip-components=1 -C <work>/src), run OCCT_WORK_DIR=<work> scripts/build-occt.sh native <prefix>, and build the app with OCCT_ROOT=<prefix> as README.md describes (pnpm -C ui build, then cargo tauri build in desktop/src-tauri). The exact commands for each platform, including the environment Windows and macOS need, are the steps of the bundle job in .github/workflows/release.yml at that tag.",
].join('\n\n')

const WEB_STATEMENT = [
  'The Partsmith web app uses Open CASCADE Technology (OCCT) 7.9.3, https://dev.opencascade.org, copyright OPEN CASCADE SAS and the OCCT contributors. OCCT is free software under the GNU Lesser General Public License version 2.1 with the Open CASCADE exception version 1.0. It includes code from the other projects listed after it, under their own notices. All these texts are below.',
  "OCCT is linked statically into the app's geometry core, core/partsmith_core.wasm. It is the unmodified source of the tag V7_9_3 of https://github.com/Open-Cascade-SAS/OCCT, built as static WebAssembly libraries by the script scripts/build-occt.sh in the Partsmith source, which lists the toolkits and options used.",
  "You may modify OCCT and use your version with Partsmith. Each deployment of this site names the commit of https://github.com/blai30/partsmith it was built from in /SOURCE.txt, with a link to that commit's source archive; that commit holds every script, tool version and lock file the build uses. Production is built from the main branch, and a preview from the commit GitHub makes by merging its pull request into the base branch; for a preview, /SOURCE.txt also names the two commits that merge joins and its tree: if the merge commit is gone, check out the first, run git merge --no-ff with the second, and compare the tree that git log -1 --format=%T prints with that tree. The OCCT source the build reads is occt-7.9.3-source.tar.gz, which scripts/occt-source-archive.sh at that commit makes from the tag V7_9_3; each Partsmith desktop release built with OCCT 7.9.3 carries that file as an asset under https://github.com/blai30/partsmith/releases. To relink: put your OCCT source in <work>/src (from the archive: mkdir -p <work>/src && tar -xzf occt-7.9.3-source.tar.gz --strip-components=1 -C <work>/src), run OCCT_WORK_DIR=<work> scripts/build-occt.sh wasm <prefix> with the Emscripten SDK of scripts/emsdk-version.sh active, and build the core with OCCT_ROOT_WASM32_UNKNOWN_EMSCRIPTEN=<prefix> and then the app as README.md describes (scripts/build-wasm.sh, then pnpm -C ui build). The exact commands are the steps of the core-wasm and web jobs in .github/workflows/ci.yml at that commit.",
].join('\n\n')

// The entry lines of OCCT and the four notices of bundled.json for 7.9.3, numbered as both files number them.
const OCCT_LINES = [
  'Open CASCADE Technology 7.9.3, LGPL-2.1-only WITH OCCT-exception-1.0, https://github.com/Open-Cascade-SAS/OCCT/tree/V7_9_3, texts [2] [3]',
  "flex, in OCCT's STEP reader (FlexLexer.h and the generated scanner), see texts, https://github.com/Open-Cascade-SAS/OCCT/blob/V7_9_3/src/FlexLexer/FlexLexer.h, texts [4]",
  "DELABELLA, in OCCT's mesher and in its copy in partsmith-occt-sys, MIT, https://github.com/Open-Cascade-SAS/OCCT/blob/V7_9_3/src/BRepMesh/delabella.cpp, texts [5]",
  "strtod of David M. Gay, in OCCT's number parsing, see texts, https://github.com/Open-Cascade-SAS/OCCT/blob/V7_9_3/src/Standard/Standard_Strtod.cxx, texts [6]",
  "ConvertUTF of Unicode, Inc., in OCCT's UTF iterator, see texts, https://github.com/Open-Cascade-SAS/OCCT/blob/V7_9_3/src/NCollection/NCollection_UtfIterator.lxx, texts [7]",
]

const FONT_LINES = [
  'Geist 1.800, OFL-1.1, https://github.com/vercel/geist-font, texts [8]',
  'JetBrains Mono 2.304, OFL-1.1, https://github.com/JetBrains/JetBrainsMono, texts [9]',
]

const CRATES_SOURCE_LINE =
  "Each crate's source code is at its crates.io URL; for the crates under MPL-2.0 that is where to obtain their Source Code Form."

/** Creates a temporary directory that is removed when the test ends. */
async function temporaryDirectory(context) {
  const directory = await mkdtemp(join(tmpdir(), 'partsmith-notices-'))
  context.after(() => rm(directory, { recursive: true, force: true }))
  return directory
}

/** Writes a file, creating its parent folders. */
async function writeFileAt(path, contents) {
  await mkdir(dirname(path), { recursive: true })
  await writeFile(path, contents)
}

/** Whether `path` exists. */
async function exists(path) {
  try {
    await readFile(path)
    return true
  } catch {
    return false
  }
}

/** A package of `cargo metadata`: a crates.io crate, or a workspace crate when `workspace` is set. */
function cratePackage(name, version, options) {
  const {
    folder,
    license = 'MIT',
    licenseFile = null,
    repository = null,
    workspace = false,
  } = options
  return {
    name,
    version,
    id: workspace ? `path+file://${folder}#${name}@${version}` : `${REGISTRY}#${name}@${version}`,
    license,
    license_file: licenseFile,
    source: workspace ? null : REGISTRY,
    manifest_path: join(folder, 'Cargo.toml'),
    repository,
  }
}

/** The parsed `cargo metadata` output for `edges`, a list of `[package, [[dependency, kinds], ...]]`. */
function metadataOf(edges) {
  const packages = new Map()
  for (const [from, dependencies] of edges) {
    packages.set(from.id, from)
    for (const [to] of dependencies) {
      packages.set(to.id, to)
    }
  }
  const dependenciesOf = new Map(edges.map(([from, dependencies]) => [from.id, dependencies]))
  return {
    packages: [...packages.values()],
    resolve: {
      nodes: [...packages.values()].map((listed) => ({
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

/** The `cargo metadata` output, as text, of `rootPackage` with a normal dependency on each of `crates`. */
function dependencyGraph(rootPackage, crates) {
  return JSON.stringify(metadataOf([[rootPackage, crates.map((listed) => [listed, [null]])]]))
}

/** A crate as `artifactCrates` returns it. */
function artifactCrate(listed, repository) {
  return {
    name: listed.name,
    version: listed.version,
    license: listed.license,
    licenseFile: listed.license_file,
    folder: dirname(listed.manifest_path),
    repository,
  }
}

/** The command line of a `cargo metadata` call for `triple`, as the fake `run` keys it. */
function metadataCommand(triple) {
  return `.: cargo metadata --format-version 1 --locked --all-features --filter-platform ${triple}`
}

const productionListingCommand = 'ui: pnpm licenses list --prod --json'
const fullListingCommand = 'ui: pnpm licenses list --json'

/**
 * A fake `run` for the repository root `root` that answers each command of `answers` (keyed as `{folder}: {command
 * line}` with `folder` relative to `root`) and records every call. An unexpected command, or an answer that is an
 * `Error`, rejects.
 */
function fakeRun(root, answers) {
  const calls = []
  async function run(command, commandArguments, directory) {
    const key = `${relative(root, directory) || '.'}: ${[command, ...commandArguments].join(' ')}`
    calls.push(key)
    const answer = answers.get(key)
    if (answer === undefined) {
      throw Object.assign(new Error(`unexpected command ${key}`), {
        status: 'unexpected',
        stderr: `the test does not expect ${key}`,
      })
    }
    if (answer instanceof Error) {
      throw answer
    }
    return answer
  }
  return { run, calls }
}

/** The standard output of `pnpm licenses list --json` for `packages`, as `{ name, version, license, path }`. */
function pnpmListing(packages) {
  const groups = {}
  for (const listed of packages) {
    groups[listed.license] ??= []
    groups[listed.license].push({
      name: listed.name,
      versions: [listed.version],
      paths: [listed.path],
      license: listed.license,
    })
  }
  return JSON.stringify(groups, null, 2)
}

/** The version of `webview2-com-sys` that the repository's Cargo.lock locks. */
async function lockedWebView2Version() {
  const lock = await readFile(join(repositoryRoot, 'Cargo.lock'), 'utf8')
  return /\[\[package\]\]\nname = "webview2-com-sys"\nversion = "([^"]+)"/.exec(lock)[1]
}

/** The `channel` of the repository's rust-toolchain.toml. */
async function rustChannel() {
  const toolchain = await readFile(join(repositoryRoot, 'rust-toolchain.toml'), 'utf8')
  return /^channel = "([^"]+)"/m.exec(toolchain)[1]
}

/**
 * Writes the repository root of test 40 under a temporary directory: copies of the inputs, a crate `alpha` with a
 * license file in every graph, `webview2-com-sys` with its static loader in the Windows graph only, one production
 * npm package and the packages of EMITTED_BUILD_PACKAGES, each with its own license text. Returns the root, the
 * answers of the fake `run` and the parts the failing-input tests change.
 */
async function writeRepository(context) {
  const workspace = await temporaryDirectory(context)
  const root = join(workspace, 'repository')
  await Promise.all(
    COPIED_INPUTS.map((input) =>
      cp(join(repositoryRoot, input), join(root, input), { recursive: true }),
    ),
  )
  const registry = join(workspace, 'registry')
  const alphaFolder = join(registry, 'alpha-1.0.0')
  await writeFileAt(join(alphaFolder, 'LICENSE'), Buffer.from([0x61, 0xff, 0x62, 0x0a]))
  const webView2Version = await lockedWebView2Version()
  const webView2Folder = join(registry, `webview2-com-sys-${webView2Version}`)
  await writeFileAt(join(webView2Folder, 'x64', 'WebView2LoaderStatic.lib'), 'loader')

  const desktopRoot = cratePackage('partsmith-desktop', '0.1.0', {
    folder: join(root, 'desktop', 'src-tauri'),
    license: 'Apache-2.0',
    repository: PARTSMITH_REPOSITORY,
    workspace: true,
  })
  const webRoot = cratePackage('partsmith-wasm', '0.1.0', {
    folder: join(root, 'crates', 'partsmith-wasm'),
    license: 'Apache-2.0',
    repository: PARTSMITH_REPOSITORY,
    workspace: true,
  })
  const alpha = cratePackage('alpha', '1.0.0', { folder: alphaFolder })
  const webView2 = cratePackage('webview2-com-sys', webView2Version, { folder: webView2Folder })

  const npmPackages = [
    ['react', '19.3.0'],
    ['rolldown', '1.2.13'],
    ['tailwindcss', '4.3.3'],
    ['vite', '8.3.4'],
  ].map(([name, version]) => ({
    name,
    version,
    license: 'MIT',
    path: join(workspace, 'npm', name),
  }))
  await Promise.all(
    npmPackages.map((listed) =>
      writeFileAt(join(listed.path, 'LICENSE'), `MIT text of ${listed.name}\n`),
    ),
  )

  /**
   * The answers of the fake `run`: graphs whose root depends on `alpha` and `extraCrates` (and, on Windows, on
   * `windowsCrate`), the production listing and `fullPackages` as the full listing.
   */
  function answers({ extraCrates = [], windowsCrate = webView2, fullPackages = npmPackages } = {}) {
    const common = [alpha, ...extraCrates]
    return new Map([
      [metadataCommand('x86_64-unknown-linux-gnu'), dependencyGraph(desktopRoot, common)],
      [
        metadataCommand('x86_64-pc-windows-msvc'),
        dependencyGraph(desktopRoot, [...common, windowsCrate]),
      ],
      [metadataCommand('aarch64-apple-darwin'), dependencyGraph(desktopRoot, common)],
      [metadataCommand(WEB_TRIPLE), dependencyGraph(webRoot, common)],
      [productionListingCommand, pnpmListing(npmPackages.slice(0, 1))],
      [fullListingCommand, pnpmListing(fullPackages)],
    ])
  }

  return { root, registry, webView2Version, webView2, npmPackages, answers }
}

/** The texts of `notices` by number, from the `== License texts ==` section. */
function numberedTexts(notices) {
  const marker = '\n== License texts ==\n\n'
  const section = notices.slice(notices.indexOf(marker) + marker.length)
  const texts = new Map()
  for (const block of section.split(/\n\n(?=\[\d+\]\n\n)/)) {
    const match = /^\[(\d+)\]\n\n([\s\S]*?)\n?$/.exec(block)
    texts.set(Number(match[1]), `${match[2]}\n`)
  }
  return texts
}

/** The texts that the entry line starting with `prefix` refers to, in order. */
function entryTexts(notices, prefix) {
  const line = notices.split('\n').find((candidate) => candidate.startsWith(prefix))
  assert.ok(line !== undefined, `no line starts with ${prefix}`)
  const texts = numberedTexts(notices)
  return [...line.matchAll(/\[(\d+)\]/g)].map(([, number]) => texts.get(Number(number)))
}

/** An entry of renderNotices with `options.platforms` (default null) and `options.textSource` (default shipped). */
function entry(name, version, license, url, texts, options = {}) {
  return {
    name,
    version,
    license,
    url,
    platforms: options.platforms ?? null,
    textSource: options.textSource ?? 'shipped',
    texts,
  }
}

/** The entry line of an npm package under MIT with one text, numbered `number`. */
function packageLine(name, version, number) {
  return `${name} ${version}, MIT, https://www.npmjs.com/package/${name}/v/${version}, texts [${number}]`
}

/** Asserts that `generateNotices` rejects with `message` and that the repository holds no notices file after. */
async function assertRejects(root, run, message) {
  await assert.rejects(generateNotices(root, { run }), { message })
  assert.equal(await exists(join(root, 'THIRD-PARTY-NOTICES.txt')), false)
  assert.equal(await exists(join(root, 'ui', 'public', 'THIRD-PARTY-NOTICES.txt')), false)
}

/** The section 4 message for a committed copy that is missing. */
function missingCopyMessage(path) {
  return `ui/scripts/notices/${path} is missing. Make it as docs/spec/delivery.md section 2.9 describes, from the version that the pins name.`
}

/** Runs `node scripts/notices.mjs` with `commandArguments` in ui/. */
function runCommand(commandArguments) {
  const result = spawnSync(process.execPath, [scriptPath, ...commandArguments], {
    cwd: uiDirectory,
    encoding: 'utf8',
  })
  return { status: result.status, stdout: result.stdout, stderr: result.stderr }
}

test('35: the headings are the first lines of the two notices files', async () => {
  const desktop = await readFile(join(repositoryRoot, 'THIRD-PARTY-NOTICES.txt'), 'utf8')
  const web = await readFile(join(repositoryRoot, 'ui/public/THIRD-PARTY-NOTICES.txt'), 'utf8')

  assert.equal(DESKTOP_NOTICES_HEADING, 'Third-party notices for the Partsmith desktop app')
  assert.equal(WEB_NOTICES_HEADING, 'Third-party notices for the Partsmith web app')
  assert.equal(desktop.slice(0, desktop.indexOf('\n')), DESKTOP_NOTICES_HEADING)
  assert.equal(web.slice(0, web.indexOf('\n')), WEB_NOTICES_HEADING)
})

test('35: DESKTOP_TARGETS lists the three desktop triples with their platforms, in order', () => {
  assert.deepEqual(DESKTOP_TARGETS, [
    { triple: 'x86_64-unknown-linux-gnu', platform: 'Linux' },
    { triple: 'x86_64-pc-windows-msvc', platform: 'Windows' },
    { triple: 'aarch64-apple-darwin', platform: 'macOS' },
  ])
})

test('36: normalizeText removes the byte order mark, carriage returns, form feeds, trailing spaces and blank edge lines', () => {
  assert.equal(
    normalizeText('\uFEFF\r\n  \nMIT License \t\r\n\fCopyright\rline\n\n\n'),
    'MIT License\nCopyright\nline\n',
  )
})

test('36: normalizeText of blank lines is the empty string', () => {
  assert.equal(normalizeText(' \n\t\n'), '')
})

test('36: normalizeText keeps every other character', () => {
  assert.equal(normalizeText('a \u2014 b'), 'a \u2014 b\n')
})

test('37: artifactCrates follows normal and build dependencies through workspace crates, sorted by name and version', () => {
  const repository = 'https://github.com/example/app'
  const crate = (name, version, options = {}) =>
    cratePackage(name, version, { folder: `/registry/${name}-${version}`, ...options })
  const app = cratePackage('app', '0.1.0', { folder: '/work/app', repository, workspace: true })
  const lib = cratePackage('lib', '0.1.0', { folder: '/work/lib', workspace: true })
  const a = crate('a', '1.0.0', { license: 'MIT OR Apache-2.0' })
  const b = crate('b', '1.0.0', { license: null, licenseFile: 'LICENSE.txt' })
  const c = crate('c', '1.0.0')
  const d = crate('d', '2.0.0', { license: 'Zlib' })
  const newerE = crate('e', '0.10.0')
  const olderE = crate('e', '0.9.1', { repository: 'https://github.com/example/e' })
  const f = crate('f', '1.0.0')
  const g = crate('g', '1.0.0')
  const metadata = metadataOf([
    [
      app,
      [
        [a, [null]],
        [b, ['build']],
        [c, ['dev']],
        [g, ['dev', null]],
      ],
    ],
    [
      a,
      [
        [d, [null]],
        [lib, [null]],
      ],
    ],
    [
      lib,
      [
        [newerE, [null]],
        [olderE, [null]],
      ],
    ],
    [c, [[f, [null]]]],
  ])

  assert.deepEqual(
    artifactCrates(metadata, 'app'),
    [a, b, d, olderE, newerE, g].map((listed) => artifactCrate(listed, repository)),
  )
})

test('38: licenseFiles reads the license files of a folder in their groups, without symbolic links', async (context) => {
  const workspace = await temporaryDirectory(context)
  const folder = join(workspace, 'crate')
  const regularFiles = [
    'LICENSE-MIT',
    'license',
    'COPYING.txt',
    'NOTICE',
    'UNLICENSE',
    'Licence.md',
    'THIRD-PARTY-LICENSE',
    'ThirdPartyNotices.txt',
    'README.md',
    'LICENSES/MIT.txt',
    'LICENSES/sub/x.txt',
    'docs/THIRD_PARTY.md',
    'src/LICENSE',
    'src/unicode/LICENSE-UNICODE',
    'src/license.rs',
    'vendor/lib/COPYING',
    'legal/TERMS.txt',
    'dist/icons/copyright.mjs',
    'dist/icons/copyright.mjs.map',
    'dist/icons/copyright.d.ts',
  ]
  await Promise.all(regularFiles.map((path) => writeFileAt(join(folder, path), `${path}\n`)))
  await mkdir(join(folder, 'COPYRIGHT'))
  await writeFileAt(join(workspace, 'outside', 'LICENSE-APACHE'), 'outside\n')
  await writeFileAt(join(workspace, 'linked', 'LICENSE'), 'linked\n')
  await symlink(join(workspace, 'outside', 'LICENSE-APACHE'), join(folder, 'LICENSE-APACHE'))
  await symlink(join(workspace, 'linked'), join(folder, 'linked'), 'dir')

  assert.deepEqual(await licenseFiles(folder, 'legal/TERMS.txt', ['vendor/']), [
    'legal/TERMS.txt',
    'COPYING.txt',
    'LICENSE-MIT',
    'Licence.md',
    'NOTICE',
    'THIRD-PARTY-LICENSE',
    'ThirdPartyNotices.txt',
    'UNLICENSE',
    'license',
    'LICENSES/MIT.txt',
    'docs/THIRD_PARTY.md',
    'src/LICENSE',
    'src/unicode/LICENSE-UNICODE',
  ])
  assert.deepEqual(await licenseFiles(folder, 'LICENSE-MIT', ['vendor/']), [
    'LICENSE-MIT',
    'COPYING.txt',
    'Licence.md',
    'NOTICE',
    'THIRD-PARTY-LICENSE',
    'ThirdPartyNotices.txt',
    'UNLICENSE',
    'license',
    'LICENSES/MIT.txt',
    'docs/THIRD_PARTY.md',
    'src/LICENSE',
    'src/unicode/LICENSE-UNICODE',
  ])
  assert.deepEqual(await licenseFiles(folder, 'legal/TERMS.txt', []), [
    'legal/TERMS.txt',
    'COPYING.txt',
    'LICENSE-MIT',
    'Licence.md',
    'NOTICE',
    'THIRD-PARTY-LICENSE',
    'ThirdPartyNotices.txt',
    'UNLICENSE',
    'license',
    'LICENSES/MIT.txt',
    'docs/THIRD_PARTY.md',
    'src/LICENSE',
    'src/unicode/LICENSE-UNICODE',
    'vendor/lib/COPYING',
  ])
  assert.deepEqual(await licenseFiles(folder, 'legal/TERMS.txt', ['src/LICENSE']), [
    'legal/TERMS.txt',
    'COPYING.txt',
    'LICENSE-MIT',
    'Licence.md',
    'NOTICE',
    'THIRD-PARTY-LICENSE',
    'ThirdPartyNotices.txt',
    'UNLICENSE',
    'license',
    'LICENSES/MIT.txt',
    'docs/THIRD_PARTY.md',
    'src/unicode/LICENSE-UNICODE',
    'vendor/lib/COPYING',
  ])
})

test('38: prebuiltLibraries lists the libraries of a folder outside folders named tests, in byte order', async (context) => {
  const folder = await temporaryDirectory(context)
  const paths = [
    'x64/a.lib',
    'lib/b.a',
    'c.dll',
    'd.so',
    'e.dylib',
    'src/tests/lib/f.a',
    'tests/g.a',
    'h.lib.txt',
  ]
  await Promise.all(paths.map((path) => writeFileAt(join(folder, path), 'library\n')))

  assert.deepEqual(await prebuiltLibraries(folder), [
    'c.dll',
    'd.so',
    'e.dylib',
    'lib/b.a',
    'x64/a.lib',
  ])
})

test('39: renderNotices lays out the entries and numbers each distinct text once', () => {
  const allPlatforms = ['Linux', 'Windows', 'macOS']
  const notices = {
    heading: 'Third-party notices for the Partsmith desktop app',
    scope: 'The Partsmith desktop app for Linux, Windows and macOS',
    partsmith: 'Apache text\n',
    occt: {
      statement: 'Statement.',
      entries: [
        entry(
          'Open CASCADE Technology',
          '7.9.3',
          'LGPL-2.1-only WITH OCCT-exception-1.0',
          'https://github.com/Open-Cascade-SAS/OCCT/tree/V7_9_3',
          ['LGPL\n', 'Exception\n'],
        ),
        entry("x, in OCCT's y", null, null, 'https://example.com/y', ['Bundled\n']),
      ],
    },
    fonts: [
      entry('Geist', '1.800', 'OFL-1.1', 'https://github.com/vercel/geist-font', ['OFL\n'], {
        platforms: allPlatforms,
      }),
    ],
    crates: [
      entry('b', '1.0.0', 'MIT', 'https://crates.io/crates/b/1.0.0', ['MIT b\n'], {
        platforms: allPlatforms,
      }),
      entry(
        'a',
        '1.0.0',
        'MIT OR Apache-2.0',
        'https://crates.io/crates/a/1.0.0',
        ['Apache text\n', 'MIT a\n'],
        { platforms: ['Windows'] },
      ),
      entry('c', '2.0.0', 'BSD-3-Clause', 'https://crates.io/crates/c/2.0.0', ['BSD c\n'], {
        platforms: ['Linux', 'macOS'],
        textSource: 'repository',
      }),
      entry('d', '0.1.0', null, 'https://crates.io/crates/d/0.1.0', ['Custom\n'], {
        platforms: allPlatforms,
      }),
      entry(
        'e',
        '0.2.0',
        'Zlib OR Apache-2.0 OR MIT',
        'https://crates.io/crates/e/0.2.0',
        ['Apache text\n'],
        { platforms: ['macOS'], textSource: 'standard' },
      ),
    ],
    packages: [
      entry('p', '0.1.0', 'MIT', 'https://www.npmjs.com/package/p/v/0.1.0', ['MIT b\n'], {
        platforms: allPlatforms,
      }),
    ],
    libraries: [
      entry(
        'LLVM libc++, libc++abi, libunwind and compiler-rt, in the Emscripten runtime',
        null,
        'Apache-2.0 WITH LLVM-exception AND (NCSA OR MIT)',
        'https://llvm.org',
        ['LLVM\n'],
        { platforms: allPlatforms },
      ),
    ],
  }

  const expected = [
    'Third-party notices for the Partsmith desktop app',
    '',
    'Generated by pnpm -C ui notices from the lock files (docs/spec/delivery.md section 2.9). Do not edit it by hand.',
    '',
    'Partsmith is licensed under the Apache License 2.0, text [1]. Models you make with Partsmith belong to you. The Partsmith desktop app for Linux, Windows and macOS also contains the software listed below, each part under its own license; the license texts follow the list.',
    '',
    '== Open CASCADE Technology ==',
    '',
    'Statement.',
    '',
    'Open CASCADE Technology 7.9.3, LGPL-2.1-only WITH OCCT-exception-1.0, https://github.com/Open-Cascade-SAS/OCCT/tree/V7_9_3, texts [2] [3]',
    "x, in OCCT's y, see texts, https://example.com/y, texts [4]",
    '',
    '== Fonts ==',
    '',
    'Geist 1.800, OFL-1.1, https://github.com/vercel/geist-font, texts [5]',
    '',
    '== Rust crates ==',
    '',
    CRATES_SOURCE_LINE,
    '',
    'b 1.0.0, MIT, https://crates.io/crates/b/1.0.0, texts [6]',
    'a 1.0.0, MIT OR Apache-2.0, https://crates.io/crates/a/1.0.0, only Windows, texts [1] [7]',
    'c 2.0.0, BSD-3-Clause, https://crates.io/crates/c/2.0.0, only Linux and macOS, text from its repository, texts [8]',
    'd 0.1.0, see texts, https://crates.io/crates/d/0.1.0, texts [9]',
    'e 0.2.0, Zlib OR Apache-2.0 OR MIT, https://crates.io/crates/e/0.2.0, only macOS, standard text, texts [1]',
    '',
    '== npm packages ==',
    '',
    'p 0.1.0, MIT, https://www.npmjs.com/package/p/v/0.1.0, texts [6]',
    '',
    '== Other libraries ==',
    '',
    'LLVM libc++, libc++abi, libunwind and compiler-rt, in the Emscripten runtime, Apache-2.0 WITH LLVM-exception AND (NCSA OR MIT), https://llvm.org, texts [10]',
    '',
    '== License texts ==',
    '',
    '[1]',
    '',
    'Apache text',
    '',
    '[2]',
    '',
    'LGPL',
    '',
    '[3]',
    '',
    'Exception',
    '',
    '[4]',
    '',
    'Bundled',
    '',
    '[5]',
    '',
    'OFL',
    '',
    '[6]',
    '',
    'MIT b',
    '',
    '[7]',
    '',
    'MIT a',
    '',
    '[8]',
    '',
    'BSD c',
    '',
    '[9]',
    '',
    'Custom',
    '',
    '[10]',
    '',
    'LLVM',
    '',
  ].join('\n')

  assert.equal(renderNotices(notices), expected)
})

test('40: generateNotices builds both files from the inputs, with no OCCT install', async (context) => {
  const savedOcctRoot = process.env.OCCT_ROOT
  delete process.env.OCCT_ROOT
  context.after(() => {
    if (savedOcctRoot !== undefined) {
      process.env.OCCT_ROOT = savedOcctRoot
    }
  })
  const { root, webView2Version, answers } = await writeRepository(context)
  const { run, calls } = fakeRun(root, answers())
  const channel = await rustChannel()

  const { desktop, web } = await generateNotices(root, { run })

  assert.deepEqual(calls.toSorted(), [
    metadataCommand('aarch64-apple-darwin'),
    metadataCommand(WEB_TRIPLE),
    metadataCommand('x86_64-pc-windows-msvc'),
    metadataCommand('x86_64-unknown-linux-gnu'),
    fullListingCommand,
    productionListingCommand,
  ])
  assert.ok(desktop.startsWith(`${DESKTOP_NOTICES_HEADING}\n\n`))
  assert.ok(web.startsWith(`${WEB_NOTICES_HEADING}\n\n`))
  for (const [notices, statement] of [
    [desktop, DESKTOP_STATEMENT],
    [web, WEB_STATEMENT],
  ]) {
    const occtAndFonts = [
      '== Open CASCADE Technology ==',
      statement,
      OCCT_LINES.join('\n'),
      '== Fonts ==',
      FONT_LINES.join('\n'),
      '== Rust crates ==',
    ].join('\n\n')
    assert.ok(notices.includes(`\n\n${occtAndFonts}\n\n`))
  }
  assert.ok(
    desktop.includes(
      [
        '== Rust crates ==',
        '',
        CRATES_SOURCE_LINE,
        '',
        'alpha 1.0.0, MIT, https://crates.io/crates/alpha/1.0.0, texts [10]',
        `webview2-com-sys ${webView2Version}, MIT, https://crates.io/crates/webview2-com-sys/${webView2Version}, only Windows, text from its repository, texts [11]`,
        '',
        '== npm packages ==',
        '',
        packageLine('react', '19.3.0', 12),
        packageLine('rolldown', '1.2.13', 13),
        packageLine('tailwindcss', '4.3.3', 14),
        packageLine('vite', '8.3.4', 15),
        '',
        '== Other libraries ==',
        '',
        `Rust standard library ${channel}, (MIT OR Apache-2.0) AND Unicode-3.0, https://www.rust-lang.org, texts [1] [16]`,
        `Microsoft Edge WebView2 SDK loader, in webview2-com-sys ${webView2Version}, BSD-3-Clause, https://www.nuget.org/packages/Microsoft.Web.WebView2, only Windows, texts [17]`,
        '',
        '== License texts ==',
        '',
      ].join('\n'),
    ),
  )
  assert.ok(
    web.includes(
      [
        '== Rust crates ==',
        '',
        CRATES_SOURCE_LINE,
        '',
        'alpha 1.0.0, MIT, https://crates.io/crates/alpha/1.0.0, texts [10]',
        '',
        '== npm packages ==',
        '',
        packageLine('react', '19.3.0', 11),
        packageLine('rolldown', '1.2.13', 12),
        packageLine('tailwindcss', '4.3.3', 13),
        packageLine('vite', '8.3.4', 14),
        '',
        '== Other libraries ==',
        '',
        `Rust standard library ${channel}, (MIT OR Apache-2.0) AND Unicode-3.0, https://www.rust-lang.org, texts [1] [15]`,
        'Emscripten runtime 4.0.15, MIT OR NCSA, https://emscripten.org, texts [16]',
        'musl libc, in the Emscripten runtime, MIT, https://musl.libc.org, texts [17]',
        'LLVM libc++, libc++abi, libunwind and compiler-rt, in the Emscripten runtime, Apache-2.0 WITH LLVM-exception AND (NCSA OR MIT), https://llvm.org, texts [18] [19] [20] [21]',
        '',
        '== License texts ==',
        '',
      ].join('\n'),
    ),
  )
  assert.equal(desktop.includes('Emscripten'), false)
  assert.equal(web.includes('WebView2'), false)

  const copies = join(root, 'ui', 'scripts', 'notices')
  const normalizedCopy = async (path) => normalizeText(await readFile(join(copies, path), 'utf8'))
  const desktopTexts = numberedTexts(desktop)
  assert.equal(desktopTexts.size, 17)
  assert.equal(desktopTexts.get(1), normalizeText(await readFile(join(root, 'LICENSE'), 'utf8')))
  assert.equal(desktopTexts.get(2), await normalizedCopy('occt-7.9.3/LICENSE_LGPL_21.txt'))
  assert.equal(desktopTexts.get(3), await normalizedCopy('occt-7.9.3/OCCT_LGPL_EXCEPTION.txt'))
  assert.equal(desktopTexts.get(5), await normalizedCopy('occt-7.9.3/delabella.txt'))
  assert.equal(desktopTexts.get(10), 'a\uFFFDb\n')
  assert.equal(desktopTexts.get(11), await normalizedCopy('crates/webview2-com-sys/LICENSE'))
  assert.equal(desktopTexts.get(12), 'MIT text of react\n')
  assert.equal(desktopTexts.get(16), await normalizedCopy(`rust-${channel}/Unicode-3.0.txt`))
  assert.equal(
    desktopTexts.get(17),
    await normalizedCopy(`webview2-com-sys-${webView2Version}/LICENSE.txt`),
  )
  const webTexts = numberedTexts(web)
  assert.equal(webTexts.size, 21)
  assert.equal(webTexts.get(16), await normalizedCopy('emscripten-4.0.15/LICENSE'))
  assert.equal(webTexts.get(17), await normalizedCopy('emscripten-4.0.15/musl-COPYRIGHT'))
  assert.deepEqual(
    [18, 19, 20, 21].map((number) => webTexts.get(number)),
    await Promise.all(
      ['libcxx', 'libcxxabi', 'libunwind', 'compiler-rt'].map((library) =>
        normalizedCopy(`emscripten-4.0.15/${library}-LICENSE.TXT`),
      ),
    ),
  )
  assert.ok(web.endsWith(webTexts.get(21)))
})

test('40: generateNotices gives the same texts twice', async (context) => {
  const { root, answers } = await writeRepository(context)

  const first = await generateNotices(root, fakeRun(root, answers()))
  const second = await generateNotices(root, fakeRun(root, answers()))

  assert.deepEqual(second, first)
})

test('41: a crate under MIT without license files and without a copy from its repository fails', async (context) => {
  const { root, registry, answers } = await writeRepository(context)
  const folder = join(registry, 'bare-1.0.0')
  await mkdir(folder, { recursive: true })
  const bare = cratePackage('bare', '1.0.0', { folder })

  await assertRejects(
    root,
    fakeRun(root, answers({ extraCrates: [bare] })).run,
    'bare 1.0.0 ships no license text, and ui/scripts/notices/crates/bare/ is missing. Commit a copy of the license file of its repository there (docs/spec/delivery.md section 2.9).',
  )
})

test('41: a crate whose standard text is missing fails', async (context) => {
  const { root, registry, answers } = await writeRepository(context)
  const folder = join(registry, 'mozilla-1.0.0')
  await mkdir(folder, { recursive: true })
  const mozilla = cratePackage('mozilla', '1.0.0', { folder, license: 'BSD-3-Clause OR MPL-2.0' })
  await rm(join(root, 'ui', 'scripts', 'notices', 'licenses', 'MPL-2.0.txt'))

  await assertRejects(
    root,
    fakeRun(root, answers({ extraCrates: [mozilla] })).run,
    'mozilla 1.0.0 ships no license text, and ui/scripts/notices/licenses/MPL-2.0.txt is missing. Add the SPDX License List text of MPL-2.0 there.',
  )
})

test('41: an OCCT version without its folder fails, and so does a folder without a notice', async (context) => {
  const { root, answers } = await writeRepository(context)
  const pins = join(root, 'scripts', 'occt-version.sh')
  const pinned = await readFile(pins, 'utf8')
  await writeFile(pins, pinned.replace(/^OCCT_VERSION=.*$/m, 'OCCT_VERSION=9.9.9'))

  await assertRejects(
    root,
    fakeRun(root, answers()).run,
    missingCopyMessage('occt-9.9.9/LICENSE_LGPL_21.txt'),
  )

  await writeFile(pins, pinned)
  await rm(join(root, 'ui', 'scripts', 'notices', 'occt-7.9.3', 'delabella.txt'))

  await assertRejects(
    root,
    fakeRun(root, answers()).run,
    missingCopyMessage('occt-7.9.3/delabella.txt'),
  )
})

test('41: an Emscripten version without its texts fails', async (context) => {
  const { root, answers } = await writeRepository(context)
  await writeFile(join(root, 'scripts', 'emsdk-version.sh'), 'EMSDK_VERSION=9.9.9\n')

  await assertRejects(
    root,
    fakeRun(root, answers()).run,
    missingCopyMessage('emscripten-9.9.9/LICENSE'),
  )
})

test('41: a Rust channel without its Unicode text fails', async (context) => {
  const { root, answers } = await writeRepository(context)
  await writeFile(join(root, 'rust-toolchain.toml'), '[toolchain]\nchannel = "9.9.9"\n')

  await assertRejects(
    root,
    fakeRun(root, answers()).run,
    missingCopyMessage('rust-9.9.9/Unicode-3.0.txt'),
  )
})

test('41: a webview2-com-sys version without the license of its loader fails', async (context) => {
  const { root, registry, answers } = await writeRepository(context)
  const folder = join(registry, 'webview2-com-sys-9.9.9')
  await writeFileAt(join(folder, 'x64', 'WebView2LoaderStatic.lib'), 'loader')
  const webView2 = cratePackage('webview2-com-sys', '9.9.9', { folder })

  await assertRejects(
    root,
    fakeRun(root, answers({ windowsCrate: webView2 })).run,
    missingCopyMessage('webview2-com-sys-9.9.9/LICENSE.txt'),
  )
})

test('41: a crate that ships a prebuilt library and is not in PREBUILT_LIBRARY_CRATES fails', async (context) => {
  const { root, registry, answers } = await writeRepository(context)
  const folder = join(registry, 'z-1.0.0')
  await writeFileAt(join(folder, 'LICENSE'), 'MIT text of z\n')
  await writeFileAt(join(folder, 'lib', 'z.a'), 'library')
  const z = cratePackage('z', '1.0.0', { folder })

  await assertRejects(
    root,
    fakeRun(root, answers({ extraCrates: [z] })).run,
    'z 1.0.0 ships the prebuilt library lib/z.a. Find its license, give it an entry if it needs one, and add the crate to PREBUILT_LIBRARY_CRATES (docs/spec/delivery.md section 2.9).',
  )
})

test('41: a full pnpm listing without rolldown fails', async (context) => {
  const { root, npmPackages, answers } = await writeRepository(context)
  const fullPackages = npmPackages.filter((listed) => listed.name !== 'rolldown')
  assert.ok(EMITTED_BUILD_PACKAGES.includes('rolldown'))

  await assertRejects(
    root,
    fakeRun(root, answers({ fullPackages })).run,
    'npm package rolldown of EMITTED_BUILD_PACKAGES is not in pnpm licenses list --json. Remove it from the list if the build no longer uses it (docs/spec/delivery.md section 2.10).',
  )
})

test('41: a cargo metadata that fails gives the command, its exit status and its standard error', async (context) => {
  const { root, answers } = await writeRepository(context)
  const failing = answers()
  failing.set(
    metadataCommand('x86_64-unknown-linux-gnu'),
    Object.assign(new Error('cargo exited with status 101'), { status: 101, stderr: 'error: x\n' }),
  )

  await assertRejects(
    root,
    fakeRun(root, failing).run,
    'cargo metadata --format-version 1 --locked --all-features --filter-platform x86_64-unknown-linux-gnu failed with exit status 101: error: x',
  )
})

test('41: a crate without license files takes the standard text of Apache-2.0 or MPL-2.0 that satisfies its expression', async (context) => {
  const { root, registry, answers } = await writeRepository(context)
  const bare = async (name, license) => {
    const folder = join(registry, `${name}-1.0.0`)
    await mkdir(folder, { recursive: true })
    return cratePackage(name, '1.0.0', { folder, license })
  }
  const extraCrates = [
    await bare('dual', 'MIT OR Apache-2.0'),
    await bare('trio', 'Zlib OR Apache-2.0 OR MIT'),
    await bare('mozilla', 'MPL-2.0'),
  ]

  const { desktop, web } = await generateNotices(root, fakeRun(root, answers({ extraCrates })))

  const standard = async (identifier) =>
    normalizeText(
      await readFile(
        join(root, 'ui', 'scripts', 'notices', 'licenses', `${identifier}.txt`),
        'utf8',
      ),
    )
  const apache = await standard('Apache-2.0')
  const mozilla = await standard('MPL-2.0')
  for (const notices of [desktop, web]) {
    assert.ok(
      notices.includes(
        '\ndual 1.0.0, MIT OR Apache-2.0, https://crates.io/crates/dual/1.0.0, standard text, texts [',
      ),
    )
    assert.ok(
      notices.includes(
        '\nmozilla 1.0.0, MPL-2.0, https://crates.io/crates/mozilla/1.0.0, standard text, texts [',
      ),
    )
    assert.deepEqual(entryTexts(notices, 'dual 1.0.0, '), [apache])
    assert.deepEqual(entryTexts(notices, 'trio 1.0.0, '), [apache])
    assert.deepEqual(entryTexts(notices, 'mozilla 1.0.0, '), [mozilla])
  }
})

test('41: a crate without license files that no standard text satisfies takes the files of its repository copy', async (context) => {
  const { root, registry, answers } = await writeRepository(context)
  const copies = join(root, 'ui', 'scripts', 'notices', 'crates')
  const bare = async (name, license) => {
    const folder = join(registry, `${name}-1.0.0`)
    await mkdir(folder, { recursive: true })
    await writeFileAt(join(copies, name, 'LICENSE-MIT'), `MIT text of ${name}\n`)
    await writeFileAt(join(copies, name, 'LICENSE-APACHE'), `Apache text of ${name}\n`)
    return cratePackage(name, '1.0.0', { folder, license })
  }
  const extraCrates = [
    await bare('both', 'MIT AND Apache-2.0'),
    await bare('llvm', 'Apache-2.0 WITH LLVM-exception'),
  ]

  const { desktop } = await generateNotices(root, fakeRun(root, answers({ extraCrates })))

  assert.ok(
    desktop.includes(
      '\nboth 1.0.0, MIT AND Apache-2.0, https://crates.io/crates/both/1.0.0, text from its repository, texts [',
    ),
  )
  assert.ok(
    desktop.includes(
      '\nllvm 1.0.0, Apache-2.0 WITH LLVM-exception, https://crates.io/crates/llvm/1.0.0, text from its repository, texts [',
    ),
  )
  assert.deepEqual(entryTexts(desktop, 'both 1.0.0, '), [
    'Apache text of both\n',
    'MIT text of both\n',
  ])
  assert.deepEqual(entryTexts(desktop, 'llvm 1.0.0, '), [
    'Apache text of llvm\n',
    'MIT text of llvm\n',
  ])
})

test('42: the command with no argument, a wrong argument or two arguments exits 2 with the usage line', () => {
  for (const commandArguments of [[], ['wrong'], ['write', 'check']]) {
    assert.deepEqual(runCommand(commandArguments), {
      status: 2,
      stdout: '',
      stderr: `${usageLine}\n`,
    })
  }
})
