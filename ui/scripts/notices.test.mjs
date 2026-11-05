// Acceptance tests 35 to 42, 84 and 96 of docs/spec/delivery.md section 6 for the third-party notices generator.
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
  wrapParagraph,
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

// The paragraphs of the OCCT statements of section 2.9 for 7.9.3, V7_9_3 and the Partsmith repository, each on one
// line as section 2.9 writes it.
const DESKTOP_STATEMENT = [
  'The Partsmith desktop app uses Open CASCADE Technology (OCCT) 7.9.3, https://dev.opencascade.org, copyright OPEN CASCADE SAS and the OCCT contributors. OCCT is free software under the GNU Lesser General Public License version 2.1 with the Open CASCADE exception version 1.0. It includes code from the other projects listed after it, under their own notices. All these texts are below.',
  'OCCT is linked statically into the app. It is the unmodified source of the tag V7_9_3 of https://github.com/Open-Cascade-SAS/OCCT, built as static libraries by the script scripts/build-occt.sh in the Partsmith source, which lists the toolkits and options used.',
  "You may modify OCCT and use your version with Partsmith. The source of each release is on its page under https://github.com/blai30/partsmith/releases: the Partsmith source is the release's tag, v followed by the app's version (for example v0.1.0), with every script, tool version and lock file the build uses, also as the release's asset partsmith-source.tar.gz, which scripts/partsmith-source-archive.sh makes from that tag; the OCCT source the build reads is the release's asset occt-7.9.3-source.tar.gz, which scripts/occt-source-archive.sh makes from the tag V7_9_3. To relink: put your OCCT source in <work>/src (from the asset: mkdir -p <work>/src && tar -xzf occt-7.9.3-source.tar.gz --strip-components=1 -C <work>/src), run OCCT_WORK_DIR=<work> scripts/build-occt.sh native <prefix>, and build the app with OCCT_ROOT=<prefix> as README.md describes (pnpm -C ui build, then cargo tauri build in desktop/src-tauri). The exact commands for each platform, including the environment Windows and macOS need, are the steps of the bundle job in .github/workflows/release.yml at that tag.",
  "The release also carries the source of every Rust crate and npm package the app is built from on Linux x86_64 with glibc, Windows x86_64 and macOS arm64, and of tauri-cli 2.12.2 and the crates of its own lock file, in the archives rust-crates-desktop.tar.gz and npm-packages-desktop.tar, which scripts/dependency-source-archive.mjs at that tag makes from Cargo.lock, ui/pnpm-lock.yaml and the Cargo.lock of tauri-cli. The second archive also holds the folder npm-sources-desktop, with the source of the code under MPL-2.0 in its native packages: lightningcss at the tag of each of its versions, with the crates its Node module compiles in, and the crates under MPL-2.0 that other native packages compile in. It also holds the license notices of the crates compiled into native packages whose tarballs do not carry them. Each archive is in the assets that end in .part-00, .part-01 and so on, which SHA256SUMS covers. Download them and SHA256SUMS, check them with sha256sum -c SHA256SUMS --ignore-missing, join the parts of each archive in order, for example cat rust-crates-desktop.tar.gz.part-* > rust-crates-desktop.tar.gz, and extract it. The archives take the place of crates.io and the npm registry, with the tools of README.md installed: set CARGO_HOME to the full path of the folder rust-crates-desktop that the first archive holds, install tauri-cli with cargo install tauri-cli --version 2.12.2 --locked, serve the folder npm-packages-desktop with python3 -m http.server --bind 127.0.0.1 8080 --directory npm-packages-desktop, and install the packages with pnpm -C ui install --frozen-lockfile --registry http://127.0.0.1:8080/ before the build. On Windows, cargo tauri build still downloads NSIS and Tauri's NSIS plugin for the installer.",
]

const WEB_STATEMENT = [
  'The Partsmith web app uses Open CASCADE Technology (OCCT) 7.9.3, https://dev.opencascade.org, copyright OPEN CASCADE SAS and the OCCT contributors. OCCT is free software under the GNU Lesser General Public License version 2.1 with the Open CASCADE exception version 1.0. It includes code from the other projects listed after it, under their own notices. All these texts are below.',
  "OCCT is linked statically into the app's geometry core, core/partsmith_core.wasm. It is the unmodified source of the tag V7_9_3 of https://github.com/Open-Cascade-SAS/OCCT, built as static WebAssembly libraries by the script scripts/build-occt.sh in the Partsmith source, which lists the toolkits and options used.",
  "You may modify OCCT and use your version with Partsmith. Each deployment of this site names the commit of https://github.com/blai30/partsmith it was built from in /SOURCE.txt, with a link to that commit's source archive; that commit holds every script, tool version and lock file the build uses. Each deployment holds that commit's source too, at /partsmith-source.tar.gz, which scripts/partsmith-source-archive.sh at that commit makes. Production is built from the main branch, and a preview from the commit GitHub makes by merging its pull request into the base branch; for a preview, /SOURCE.txt also names the two commits that merge joins and its tree: if the merge commit is gone, check out the first, run git merge --no-ff with the second, and compare the tree that git log -1 --format=%T prints with that tree. Each deployment also holds the OCCT source the build reads, at /occt-7.9.3-source.tar.gz. The script scripts/occt-source-archive.sh at that commit makes that file from the tag V7_9_3. To relink: put your OCCT source in <work>/src (from the archive: mkdir -p <work>/src && tar -xzf occt-7.9.3-source.tar.gz --strip-components=1 -C <work>/src), run OCCT_WORK_DIR=<work> scripts/build-occt.sh wasm <prefix> with the Emscripten SDK of scripts/emsdk-version.sh active, and build the core with OCCT_ROOT_WASM32_UNKNOWN_EMSCRIPTEN=<prefix> and then the app as README.md describes (scripts/build-wasm.sh, then pnpm -C ui build). The exact commands are the steps of the core-wasm and web jobs in .github/workflows/ci.yml at that commit.",
  'Each deployment also holds the source of every Rust crate and npm package the app is built from on Linux x86_64 with glibc, where the web job builds it, in the archives rust-crates-web.tar.gz and npm-packages-web.tar, which scripts/dependency-source-archive.mjs at that commit makes from Cargo.lock and ui/pnpm-lock.yaml. The second archive also holds the folder npm-sources-web, with the source of the code under MPL-2.0 in its native packages: lightningcss at the tag of each of its versions, with the crates its Node module compiles in, and the crates under MPL-2.0 that other native packages compile in. It also holds the license notices of the crates compiled into native packages whose tarballs do not carry them. The parts of the two archives, which end in .part-00, .part-01 and so on, are in /dependencies/, and /dependencies/SHA256SUMS lists each part with its SHA-256. Download SHA256SUMS and each part it lists, check them with sha256sum -c SHA256SUMS, join the parts of each archive in order, for example cat npm-packages-web.tar.part-* > npm-packages-web.tar, and extract it. The archives take the place of crates.io and the npm registry, with the tools of README.md installed: set CARGO_HOME to the full path of the folder rust-crates-web that the first archive holds, serve the folder npm-packages-web with python3 -m http.server --bind 127.0.0.1 8080 --directory npm-packages-web, and install the packages with pnpm -C ui install --frozen-lockfile --registry http://127.0.0.1:8080/ before the build.',
]

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

// The entry lines of the code the NSIS installer adds, in the desktop file only, after the WebView2 loader's line,
// with the versions of WINDOWS_INSTALLER (section 2.9 input 6), numbered as test 40's desktop file numbers them.
const INSTALLER_LINES = [
  'NSIS with its LZMA decompressor, Modern UI 2 and the plugins System, nsDialogs, StartMenu and NSISdl, in the Windows installer 3.11, Zlib AND CPL-1.0, https://sourceforge.net/projects/nsis/files/NSIS%203/3.11/, only Windows, texts [18] [19] [20]',
  "nsis_tauri_utils, Tauri's NSIS plugin, with the crates semver and windows-sys, in the Windows installer 0.5.3, MIT OR Apache-2.0, https://github.com/tauri-apps/nsis-tauri-utils/tree/nsis_tauri_utils-v0.5.3, only Windows, text from its repository, texts [21] [22]",
  'NSIS installer script of tauri-bundler, in the Windows installer 2.10.2, Apache-2.0 OR MIT, https://crates.io/crates/tauri-bundler/2.10.2, only Windows, texts [23] [24]',
]

// The committed copies under ui/scripts/notices/ that the installer's entries carry, in the order of their texts.
const INSTALLER_TEXTS = [
  'nsis-3.11/COPYING',
  'nsis-3.11/ModernUI2-License.txt',
  'nsis-3.11/NSISdl-License.txt',
  'nsis_tauri_utils-0.5.3/LICENSE_MIT',
  'nsis_tauri_utils-0.5.3/LICENSE_APACHE-2.0',
  'tauri-bundler-2.10.2/License-Apache.md.txt',
  'tauri-bundler-2.10.2/License-MIT.md.txt',
]

// The crates' source line as the files hold it, wrapped at 80 characters (section 2.9).
const CRATES_SOURCE_LINES = [
  "Each crate's source code is at its crates.io URL; for the crates under MPL-2.0",
  'that is where to obtain their Source Code Form.',
].join('\n')

// The commands of section 2.9 (Wrapping) that each file holds on one line, for OCCT 7.9.3.
const EXTRACT_COMMANDS = [
  'mkdir -p <work>/src &&',
  'tar -xzf occt-7.9.3-source.tar.gz --strip-components=1 -C <work>/src',
]

// The command of the fourth paragraph of both statements, which installs the packages from the served archive.
const OFFLINE_INSTALL = 'pnpm -C ui install --frozen-lockfile --registry http://127.0.0.1:8080/'

const DESKTOP_COMMANDS = [
  'pnpm -C ui notices',
  ...EXTRACT_COMMANDS,
  'OCCT_WORK_DIR=<work> scripts/build-occt.sh native <prefix>',
  'pnpm -C ui build',
  'cargo tauri build',
  'sha256sum -c SHA256SUMS --ignore-missing',
  'cat rust-crates-desktop.tar.gz.part-* > rust-crates-desktop.tar.gz',
  'cargo install tauri-cli --version 2.12.2 --locked',
  'python3 -m http.server --bind 127.0.0.1 8080 --directory npm-packages-desktop',
  OFFLINE_INSTALL,
]

const WEB_COMMANDS = [
  'pnpm -C ui notices',
  'git merge --no-ff',
  'git log -1 --format=%T',
  ...EXTRACT_COMMANDS,
  'OCCT_WORK_DIR=<work> scripts/build-occt.sh wasm <prefix>',
  'pnpm -C ui build',
  'sha256sum -c SHA256SUMS',
  'cat npm-packages-web.tar.part-* > npm-packages-web.tar',
  'python3 -m http.server --bind 127.0.0.1 8080 --directory npm-packages-web',
  OFFLINE_INSTALL,
]

// The committed notices files, each with the commands it holds (test 84).
const COMMITTED_FILES = [
  { file: 'THIRD-PARTY-NOTICES.txt', commands: DESKTOP_COMMANDS },
  { file: 'ui/public/THIRD-PARTY-NOTICES.txt', commands: WEB_COMMANDS },
]

const NO_BREAK_SPACE = '\u00A0'

// The literal paragraphs of test 84, each with the lines that wrapParagraph gives it.
const WRAPPING_CASES = {
  short: ['Statement.', ['Statement.']],
  fullLine: [`${'a'.repeat(39)} ${'b'.repeat(40)} c`, [`${'a'.repeat(39)} ${'b'.repeat(40)}`, 'c']],
  oneOver: [`${'a'.repeat(40)} ${'b'.repeat(40)}`, ['a'.repeat(40), 'b'.repeat(40)]],
  longWord: [`short ${'u'.repeat(90)} end`, ['short', 'u'.repeat(90), 'end']],
  noBreak: [
    `${'a'.repeat(70)} run${NO_BREAK_SPACE}it${NO_BREAK_SPACE}now end`,
    ['a'.repeat(70), 'run it now end'],
  ],
}

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

/** `count` times the word `word`, with single spaces between. */
function words(count) {
  return Array.from({ length: count }, () => 'word').join(' ')
}

/** `paragraph` with the words of each of `commands` joined by no-break spaces, as the statements mark them. */
function markCommands(paragraph, commands) {
  return commands.reduce(
    (marked, command) => marked.replaceAll(command, command.replaceAll(' ', NO_BREAK_SPACE)),
    paragraph,
  )
}

/** Each of COMMITTED_FILES with its text in the repository, as `{ file, commands, text }`. */
async function committedNotices() {
  const texts = await Promise.all(
    COMMITTED_FILES.map(({ file }) => readFile(join(repositoryRoot, file), 'utf8')),
  )
  return COMMITTED_FILES.map(({ file, commands }, i) => ({ file, commands, text: texts[i] }))
}

/** Asserts that wrapParagraph gives the lines of the case. */
function assertWraps([paragraph, lines]) {
  assert.equal(wrapParagraph(paragraph), lines.join('\n'))
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
    'Generated by pnpm -C ui notices from the lock files (docs/spec/delivery.md',
    'section 2.9). Do not edit it by hand.',
    '',
    'Partsmith is licensed under the Apache License 2.0, text [1]. Models you make',
    'with Partsmith belong to you. The Partsmith desktop app for Linux, Windows and',
    'macOS also contains the software listed below, each part under its own license;',
    'the license texts follow the list.',
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
    CRATES_SOURCE_LINES,
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
  for (const [notices, statement, commands] of [
    [desktop, DESKTOP_STATEMENT, DESKTOP_COMMANDS],
    [web, WEB_STATEMENT, WEB_COMMANDS],
  ]) {
    const occtAndFonts = [
      '== Open CASCADE Technology ==',
      statement.map((paragraph) => wrapParagraph(markCommands(paragraph, commands))).join('\n\n'),
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
        CRATES_SOURCE_LINES,
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
        ...INSTALLER_LINES,
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
        CRATES_SOURCE_LINES,
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
  assert.equal(web.includes('in the Windows installer'), false)

  const copies = join(root, 'ui', 'scripts', 'notices')
  const normalizedCopy = async (path) => normalizeText(await readFile(join(copies, path), 'utf8'))
  const desktopTexts = numberedTexts(desktop)
  assert.equal(desktopTexts.size, 24)
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
  assert.deepEqual(
    [18, 19, 20, 21, 22, 23, 24].map((number) => desktopTexts.get(number)),
    await Promise.all(INSTALLER_TEXTS.map(normalizedCopy)),
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

test('84: wrapParagraph keeps a paragraph of 80 characters or fewer as it is', () => {
  assertWraps(WRAPPING_CASES.short)
})

test('84: wrapParagraph fills each line with words up to 80 characters', () => {
  assertWraps(WRAPPING_CASES.fullLine)
  assertWraps(WRAPPING_CASES.oneOver)
})

test('84: wrapParagraph gives a word longer than 80 characters a line of its own', () => {
  assertWraps(WRAPPING_CASES.longWord)
})

test('84: wrapParagraph keeps the words that no-break spaces join on one line, with spaces between', () => {
  assertWraps(WRAPPING_CASES.noBreak)
})

test('84: joining the lines of wrapParagraph with single spaces gives the paragraph back', () => {
  for (const [paragraph] of Object.values(WRAPPING_CASES)) {
    assert.equal(
      wrapParagraph(paragraph).split('\n').join(' '),
      paragraph.replaceAll(NO_BREAK_SPACE, ' '),
    )
  }
})

test('84: renderNotices wraps the OCCT statement and keeps entry lines and license texts whole', () => {
  const name = 'n'.repeat(100)
  const longText = 'x'.repeat(200)
  const notices = renderNotices({
    heading: WEB_NOTICES_HEADING,
    scope: 'The Partsmith web app',
    partsmith: `${longText}\n`,
    occt: {
      statement: `${words(30)}\n\n${words(30)}`,
      entries: [entry(name, null, 'MIT', 'https://example.com/n', ['Bundled\n'])],
    },
    fonts: [],
    crates: [],
    packages: [],
    libraries: [],
  })

  const statement = [words(16), words(14), '', words(16), words(14)].join('\n')
  const entryLine = `${name}, MIT, https://example.com/n, texts [2]`
  assert.ok(notices.includes(`\n== Open CASCADE Technology ==\n\n${statement}\n\n${entryLine}\n`))
  assert.ok(notices.includes(`\n[1]\n\n${longText}\n`))
})

test('84: every line above the license texts of both notices files that is longer than 80 characters is an entry line', async () => {
  for (const { file, text } of await committedNotices()) {
    const listed = text.slice(0, text.indexOf('\n== License texts ==\n'))
    const longLines = listed.split('\n').filter((line) => line.length > 80)
    assert.deepEqual(
      longLines.filter((line) => !/, texts(?: \[\d+\])+$/.test(line)),
      [],
      `${file} has long lines that are not entry lines`,
    )
  }
})

test('84: both notices files hold each command of their paragraphs on one line', async () => {
  for (const { file, commands, text } of await committedNotices()) {
    const lines = text.split('\n')
    for (const command of commands) {
      assert.ok(
        lines.some((line) => line.includes(command)),
        `${file} holds ${command} on one line`,
      )
    }
  }
})

// The versions of the code the NSIS installer adds (section 2.9 input 6, test 96).
const EXPECTED_WINDOWS_INSTALLER = {
  tauriCli: '2.12.2',
  tauriBundler: '2.10.2',
  nsis: '3.11',
  nsisTauriUtils: '0.5.3',
}

// The workflow files that install tauri-cli, each by version and with a cache key that names it (section 2.2).
const TAURI_CLI_WORKFLOWS = ['.github/workflows/ci.yml', '.github/workflows/release.yml']

/** The lines of the committed desktop notices that start with `prefix`. */
function linesStartingWith(notices, prefix) {
  return notices.split('\n').filter((line) => line.startsWith(prefix))
}

test('96: WINDOWS_INSTALLER holds the versions of section 2.9', async () => {
  const { WINDOWS_INSTALLER } = await import('./notices.mjs')

  assert.deepEqual(WINDOWS_INSTALLER, EXPECTED_WINDOWS_INSTALLER)
})

test('96: every tauri-cli install and cache key of the workflows names the tauri-cli of WINDOWS_INSTALLER', async () => {
  const { WINDOWS_INSTALLER } = await import('./notices.mjs')
  const texts = await Promise.all(
    TAURI_CLI_WORKFLOWS.map((file) => readFile(join(repositoryRoot, file), 'utf8')),
  )

  for (const [i, text] of texts.entries()) {
    const installs = [...text.matchAll(/cargo install tauri-cli --version (\S+) --locked/g)]
    const keys = [...text.matchAll(/\btauri-cli-(\d[^-\s]*)-/g)]
    assert.ok(installs.length > 0, `${TAURI_CLI_WORKFLOWS[i]} installs no tauri-cli`)
    assert.ok(keys.length > 0, `${TAURI_CLI_WORKFLOWS[i]} has no tauri-cli cache key`)
    for (const [, version] of [...installs, ...keys]) {
      assert.equal(version, WINDOWS_INSTALLER?.tauriCli, TAURI_CLI_WORKFLOWS[i])
    }
  }
})

test('96: the copies of the installer texts exist as UTF-8 with LF line endings and hold their notices', async () => {
  const copies = join(repositoryRoot, 'ui', 'scripts', 'notices')
  const decoder = new TextDecoder('utf-8', { fatal: true })
  const texts = Object.fromEntries(
    await Promise.all(
      INSTALLER_TEXTS.map(async (path) => [
        path,
        decoder.decode(await readFile(join(copies, path))),
      ]),
    ),
  )

  for (const [path, text] of Object.entries(texts)) {
    assert.equal(text.includes('\r'), false, `${path} holds a CR`)
  }
  assert.ok(
    texts['nsis-3.11/ModernUI2-License.txt'].startsWith('Copyright © 2002-2025 Joost Verburg'),
  )
  assert.ok(
    texts['nsis-3.11/COPYING']
      .split('\n')
      .some((line) => line.trim() === 'SPECIAL EXCEPTION FOR LZMA COMPRESSION MODULE'),
  )
  assert.ok(
    texts['tauri-bundler-2.10.2/License-Apache.md.txt'].startsWith(
      'Copyright 2017 Cargo-Bundle developers',
    ),
  )
  const mit = texts['tauri-bundler-2.10.2/License-MIT.md.txt']
  assert.ok(mit.includes('Copyright 2017 Cargo-Bundle developers'))
  assert.ok(mit.includes('Copyright (c) 2017 - Present Tauri Apps Contributors'))
})

test("96: the committed desktop notices list the installer's three entries after the WebView2 loader, for Windows only", async () => {
  const desktop = await readFile(join(repositoryRoot, 'THIRD-PARTY-NOTICES.txt'), 'utf8')
  const lines = desktop.split('\n')
  const loader = lines.findIndex((line) =>
    line.startsWith('Microsoft Edge WebView2 SDK loader, in webview2-com-sys '),
  )
  assert.notEqual(loader, -1, 'the desktop notices have no WebView2 loader line')
  const [nsis, plugin, script] = lines.slice(loader + 1, loader + 4)

  assert.ok(nsis.startsWith('NSIS with its LZMA decompressor, Modern UI 2 and the plugins'), nsis)
  assert.ok(plugin.startsWith("nsis_tauri_utils, Tauri's NSIS plugin"), plugin)
  assert.ok(script.startsWith('NSIS installer script of tauri-bundler'), script)
  for (const line of [nsis, plugin, script]) {
    assert.ok(line.includes(', only Windows,'), line)
  }
  assert.match(nsis, /, texts \[\d+\] \[\d+\] \[\d+\]$/)
  assert.match(plugin, /, text from its repository, texts \[\d+\] \[\d+\]$/)
  assert.match(script, /, only Windows, texts \[\d+\] \[\d+\]$/)
  assert.equal(script.includes(', standard text'), false)
  assert.equal(script.includes(', text from its repository'), false)
})

test('96: the desktop notices list semver and windows-sys, whose code the plugin holds, with their texts', async () => {
  const desktop = await readFile(join(repositoryRoot, 'THIRD-PARTY-NOTICES.txt'), 'utf8')

  for (const name of ['semver', 'windows-sys']) {
    const entries = linesStartingWith(desktop, `${name} `)
    assert.ok(entries.length > 0, `the desktop notices list no ${name}`)
    for (const line of entries) {
      assert.match(
        line,
        new RegExp(`^${name} \\S+, .*https://crates\\.io/crates/${name}/.*, texts( \\[\\d+\\])+$`),
      )
    }
  }
})

test('96: the web notices name none of the installer entries', async () => {
  const web = await readFile(
    join(repositoryRoot, 'ui', 'public', 'THIRD-PARTY-NOTICES.txt'),
    'utf8',
  )

  assert.equal(web.includes('in the Windows installer'), false)
})
