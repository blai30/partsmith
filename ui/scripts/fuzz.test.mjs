// Acceptance tests 141 to 146 and 148 of docs/spec/delivery.md section 6 for the fuzz crate of section 2.15: its
// manifest, its targets, its toolchain pin, its lock file, its corpora and their seeds, and the license exception of
// deny.toml.
import assert from 'node:assert/strict'
import { spawnSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import { readFile, readdir } from 'node:fs/promises'
import { basename, join } from 'node:path'
import { test } from 'node:test'
import { fileURLToPath } from 'node:url'

import { allowedLicenses } from './check-licenses.mjs'

const repositoryRoot = fileURLToPath(new URL('../..', import.meta.url))

// The folders of the fuzz crate (section 2.15), as paths from the repository root.
const FUZZ_FOLDER = 'fuzz'
const TARGETS_FOLDER = 'fuzz/fuzz_targets'
const CORPUS_FOLDER = 'fuzz/corpus'

// The targets of wave 1, in the order of the table of targets (section 2.15, The targets).
const TARGETS = ['ptsm_load', 'protocol_frame', 'step_read']

// The calls of its reader that each target file holds as written (section 2.15, The targets, test 142).
const READER_CALLS = {
  ptsm_load: ['partsmith_model::load_bytes(data)'],
  protocol_frame: ['decode_request_frame(data)', 'decode_frame(data)'],
  step_read: ['partsmith_occt_sys::read_step(data, &partsmith_occt_sys::Interrupt::new())'],
}

// The tables of fuzz/Cargo.toml in order, each with exactly its keys and their values as written (section 2.15, The
// fuzz crate).
const MANIFEST_TABLES = [
  {
    header: '[package]',
    entries: {
      name: '"partsmith-fuzz"',
      version: '"0.0.0"',
      publish: 'false',
      edition: '"2024"',
      license: '"Apache-2.0"',
    },
  },
  { header: '[package.metadata]', entries: { 'cargo-fuzz': 'true' } },
  {
    header: '[dependencies]',
    entries: {
      'libfuzzer-sys': '"0.4"',
      'partsmith-model': '{ path = "../crates/partsmith-model" }',
      'partsmith-protocol': '{ path = "../crates/partsmith-protocol" }',
      'partsmith-occt-sys': '{ path = "../crates/partsmith-occt-sys" }',
    },
  },
  { header: '[lints.rust]', entries: { unsafe_code: '"forbid"', missing_docs: '"warn"' } },
  { header: '[lints.clippy]', entries: { all: '{ level = "warn", priority = -1 }' } },
  ...TARGETS.map((target) => ({
    header: '[[bin]]',
    entries: {
      name: `"${target}"`,
      path: `"fuzz_targets/${target}.rs"`,
      test: 'false',
      doc: 'false',
      bench: 'false',
    },
  })),
  { header: '[workspace]', entries: { members: '["."]' } },
]

// The lint tables of the root Cargo.toml that the fuzz crate's two lint tables copy (section 2.15, The fuzz crate).
const LINT_TABLES = {
  '[lints.rust]': '[workspace.lints.rust]',
  '[lints.clippy]': '[workspace.lints.clippy]',
}

// The root workspace's members as written, which do not match fuzz (section 2.15, The fuzz crate).
const ROOT_MEMBERS = '["crates/*", "desktop/src-tauri"]'

// The text of fuzz/.gitignore (section 2.15, The fuzz crate).
const FUZZ_IGNORE = '/target\n/artifacts\n/coverage\n'

// The Rust version that the root pins and its last nightly, which fuzz/rust-toolchain.toml pins (sections 2.2 and
// 2.15, ADR 0067 choice 2). The change that moves the root's version moves both.
const TOOLCHAIN_PAIR = { rust: '1.97.0', nightly: 'nightly-2026-05-22' }
const FUZZ_TOOLCHAIN = `[toolchain]\nchannel = "${TOOLCHAIN_PAIR.nightly}"\nprofile = "minimal"\n`

// The fuzz crate's own package and the three packages of fuzz/Cargo.lock that only libfuzzer-sys brings, which
// Cargo.lock does not hold (section 2.15, The fuzz crate).
const FUZZ_PACKAGE = { name: 'partsmith-fuzz', version: '0.0.0' }
const FUZZ_ONLY_PACKAGES = [
  { name: 'arbitrary', version: '1.5.0' },
  { name: 'jobserver', version: '0.1.35' },
  { name: 'libfuzzer-sys', version: '0.4.13' },
]

// The line of .gitattributes that keeps Git from changing the bytes of a corpus file (section 2.15, The fuzz crate).
const CORPUS_ATTRIBUTE = 'fuzz/corpus/** -text'

// The folders whose tracked .ptsm files seed ptsm_load (section 2.15, Corpora).
const PTSM_SEED_FOLDERS = ['crates/', 'ui/e2e/fixtures/']

// The four frames that seed protocol_frame (section 2.15, Corpora): examples A and B of protocol.md section 2.2 by
// their SHA-1s, and the two response frames that the test builds from their headers.
const EXAMPLE_FRAMES = [
  '7df290a4e6a72d320f5205fb4c4eb698bc38423a',
  'dc6ad0981e2862b7bc52a6f1e716a1eea6e37fd6',
]
const HEADER_FRAMES = [
  {
    name: 'b1c89b29052269037d5a18cfa7908ebb6c321a5b',
    header:
      '{"protocol_version":1,"messages":[{"kind":"response","id":1,"result":{"kind":"ok","type":"engine_info","protocol_version":1,"engine_version":"0.1.0","format_version":2,"kernel":"OCCT 7.9.3"}}],"buffers":[]}',
  },
  {
    name: '4206f764bf8e65f469c22b60e41cb67fe4fc6c03',
    header:
      '{"protocol_version":1,"messages":[{"kind":"response","id":5,"result":{"kind":"error","code":"malformed_request","message":"The request is not valid."}}],"buffers":[]}',
  },
]

// The magic of a frame and the alignment of its buffer section (protocol.md section 2.2).
const FRAME_MAGIC = 'PSM1'
const FRAME_ALIGNMENT = 8

// The two STEP files of examples/ that seed step_read, each with the SHA-1 of its copy (section 2.15, Corpora).
const IMPORT_STEP =
  'examples/import/assets/b8d947e1aae9d107df35bea403414dc9ae3df630b84426c70e92770948bdfdb4.step'
const EXAMPLE_STEPS = [
  { path: IMPORT_STEP, name: '52aea09b356d9bdbc4f61f977176b73b314c1b68' },
  {
    path: 'examples/sketch/assets/c4c0fd87f2ee8d1542836ba5287b3c20e5ca292fba1210975b920312eebc4692.step',
    name: 'e848e3184c904c1299b77c4694a845483b983364',
  },
]

// The input of the first finding and its recipe (section 3.8, Findings): lines 1 to 108 of the import example's STEP
// file, then one line, then lines 244 to 414, each ending in LF.
const FINDING_1 = {
  name: '10df39693bdfe46c07bc7d9505965cb76bb5a846',
  source: IMPORT_STEP,
  before: { first: 1, last: 108 },
  line: "#('2D SPACE',''",
  after: { first: 244, last: 414 },
}

// A TOML line that tomlTables reads: a table header, `[name]` or `[[name]]` with a bare or dotted name, and an entry,
// `key = value` with a bare key.
const TOML_HEADER = /^\[\[?[\w.-]+\]\]?$/
const TOML_ENTRY = /^([\w-]+) = (.*)$/

// The first line of the exception block of section 2.15 that deny.toml holds right after its allow list (test 148).
const EXCEPTION_FIRST_LINE = 'exceptions = ['

// The license that only libfuzzer-sys's exception allows, and the fourteen identifiers of the allow list (section
// 2.10), in order.
const FUZZ_LICENSE = 'NCSA'
const ALLOWED_LICENSES = [
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
]

/** The text of the repository file at `path`, a path from the repository root. */
function repositoryText(path) {
  return readFile(join(repositoryRoot, path), 'utf8')
}

/** The bytes of the repository file at `path`, a path from the repository root. */
function repositoryBytes(path) {
  return readFile(join(repositoryRoot, path))
}

/** The entries of the repository folder at `path` as `{ name, folder }`, sorted by name. */
async function folderEntries(path) {
  const entries = await readdir(join(repositoryRoot, path), { withFileTypes: true })
  return entries
    .map((entry) => ({ name: entry.name, folder: entry.isDirectory() }))
    .toSorted((first, second) => (first.name < second.name ? -1 : 1))
}

/** The names of the folders in the repository folder at `path`, sorted. */
async function folderNames(path) {
  return (await folderEntries(path)).filter((entry) => entry.folder).map((entry) => entry.name)
}

/** Each folder of fuzz/corpus/ as `{ folder, entries }`: its path and its entries as `folderEntries` reads them. */
async function corpusFolders() {
  const folders = (await folderNames(CORPUS_FOLDER)).map((target) => `${CORPUS_FOLDER}/${target}`)
  const entries = await Promise.all(folders.map(folderEntries))
  return folders.map((folder, i) => ({ folder, entries: entries[i] }))
}

/** The paths of the files of the corpus folders `folders`, as `corpusFolders` reads them, in order. */
function corpusPaths(folders) {
  return folders.flatMap(({ folder, entries }) =>
    entries.filter((entry) => !entry.folder).map((entry) => `${folder}/${entry.name}`),
  )
}

/** The path of the file of the fuzz target `target`. */
function targetPath(target) {
  return `${TARGETS_FOLDER}/${target}.rs`
}

/** The hexadecimal SHA-1 of `bytes`. */
function sha1Of(bytes) {
  return createHash('sha1').update(bytes).digest('hex')
}

/** Runs `git` with `gitArguments` in the repository and returns its standard output; fails the test when it fails. */
function repositoryGit(...gitArguments) {
  const result = spawnSync('git', gitArguments, { cwd: repositoryRoot, encoding: 'utf8' })
  assert.equal(result.status, 0, `git ${gitArguments.join(' ')} failed: ${result.stderr}`)
  return result.stdout
}

/**
 * The tables of a TOML text in order, as `{ header, entries }`: `header` is the line `[name]` or `[[name]]` as written,
 * or the empty string for the keys before the first table, and `entries` maps each key to its value as written. Every
 * other line is left out: comments, blank lines, the further lines of an array that spans lines, and any line that
 * `unreadLines` names.
 */
function tomlTables(text) {
  const tables = [{ header: '', entries: {} }]
  for (const line of text.split('\n')) {
    if (TOML_HEADER.test(line)) {
      tables.push({ header: line, entries: {} })
      continue
    }
    const entry = TOML_ENTRY.exec(line)
    if (entry !== null) {
      tables.at(-1).entries[entry[1]] = entry[2]
    }
  }
  return tables
}

/**
 * The lines of a TOML text that `tomlTables` cannot read: each line that is not blank, not a comment, not a table
 * header and not a `key = value` entry. A manifest that test 141 reads must have none. A dotted key, a quoted key, a
 * key without spaces around `=`, an indented key and a header with spaces or a comment are each such a line.
 */
function unreadLines(text) {
  return text
    .split('\n')
    .filter(
      (line) =>
        line.trim() !== '' &&
        !line.trim().startsWith('#') &&
        !TOML_HEADER.test(line) &&
        !TOML_ENTRY.test(line),
    )
}

/** The entries of the one table of `tables` with the header `header`; fails the test when there is not exactly one. */
function tableEntries(tables, header) {
  const found = tables.filter((table) => table.header === header)
  assert.equal(found.length, 1, `${found.length} tables ${header}`)
  return found[0].entries
}

/** A TOML basic string as written, `"text"`, without its quotes; undefined for undefined. */
function tomlString(value) {
  if (value === undefined) {
    return undefined
  }
  const quoted = /^"([^"\\]*)"$/.exec(value)
  assert.ok(quoted, `${value} is not a plain TOML string`)
  return quoted[1]
}

/** The `[[package]]` tables of a Cargo.lock text, as `{ name, version, source, checksum }`, `source` and `checksum` undefined when absent. */
function lockPackages(text) {
  return tomlTables(text)
    .filter((table) => table.header === '[[package]]')
    .map(({ entries }) => ({
      name: tomlString(entries.name),
      version: tomlString(entries.version),
      source: tomlString(entries.source),
      checksum: tomlString(entries.checksum),
    }))
}

/** A package as a failure message names it: its name and version. */
function packageLabel(lockPackage) {
  return `${lockPackage.name} ${lockPackage.version}`
}

/**
 * The packages of `fuzzLock` that test 144 does not find in `rootLock`, by name and version, sorted: a package with a
 * source that `rootLock` lacks with the same name, version, source and checksum, and a package without a source that is
 * neither partsmith-fuzz 0.0.0 nor a package of `rootLock` without a source and with the same version.
 */
function packagesNotInRootLock(fuzzLock, rootLock) {
  const rootPackages = new Set(rootLock.map((rootPackage) => JSON.stringify(rootPackage)))
  return fuzzLock
    .filter(
      (fuzzPackage) =>
        !rootPackages.has(JSON.stringify(fuzzPackage)) &&
        !(
          fuzzPackage.source === undefined &&
          fuzzPackage.name === FUZZ_PACKAGE.name &&
          fuzzPackage.version === FUZZ_PACKAGE.version
        ),
    )
    .map(packageLabel)
    .toSorted()
}

/**
 * The lines of the job `fuzz` of a nightly.yml text below its line `  fuzz:`, up to the next line indented by fewer
 * than four spaces that is no comment; fails the test when there is no such job.
 */
function fuzzJobLines(workflow) {
  const lines = workflow.split('\n')
  const start = lines.indexOf('  fuzz:')
  assert.notEqual(start, -1, 'nightly.yml has no job fuzz')
  const end = lines.findIndex((line, i) => i > start && /^ {0,3}[^\s#]/.test(line))
  return lines.slice(start + 1, end === -1 ? undefined : end)
}

/** The `target`s of the matrix rows of the job `fuzz` of a nightly.yml text, in order. */
function matrixTargets(workflow) {
  return fuzzJobLines(workflow)
    .map((line) => /^ +- target: (\S+)$/.exec(line)?.[1])
    .filter((target) => target !== undefined)
}

/** The names of the corpus folder of `target` mapped to the bytes of each file. */
async function corpusFiles(target) {
  const folder = `${CORPUS_FOLDER}/${target}`
  const names = (await folderEntries(folder)).map((entry) => entry.name)
  const files = await Promise.all(names.map((name) => repositoryBytes(`${folder}/${name}`)))
  return new Map(names.map((name, i) => [name, files[i]]))
}

/** Whether the corpus `corpus`, as `corpusFiles` reads it, holds a file named `name` with the bytes `bytes`. */
function holds(corpus, name, bytes) {
  return corpus.get(name)?.equals(bytes) === true
}

/**
 * The frame of the header `header` with no buffer, as protocol.md section 2.2 lays it out: the magic, the header's
 * length as a little-endian u32, the header, and zero bytes up to a multiple of 8.
 */
function headerFrame(header) {
  const headerBytes = Buffer.from(header, 'utf8')
  const length = Buffer.alloc(4)
  length.writeUInt32LE(headerBytes.length)
  const padding = Buffer.alloc(
    (FRAME_ALIGNMENT - (headerBytes.length % FRAME_ALIGNMENT)) % FRAME_ALIGNMENT,
  )
  return Buffer.concat([Buffer.from(FRAME_MAGIC, 'latin1'), length, headerBytes, padding])
}

/** The input of the first finding, built from the lines of `source` by the recipe of section 3.8. */
function findingInput(source) {
  const lines = source.toString('latin1').split('\n')
  const kept = [
    ...lines.slice(FINDING_1.before.first - 1, FINDING_1.before.last),
    FINDING_1.line,
    ...lines.slice(FINDING_1.after.first - 1, FINDING_1.after.last),
  ]
  return Buffer.from(kept.map((line) => `${line}\n`).join(''), 'latin1')
}

/** The lines of the fenced code block of a Markdown `text` whose first line is `firstLine`; undefined when there is none. */
function fencedBlock(text, firstLine) {
  const lines = text.split('\n')
  const start = lines.findIndex((line, i) => line === firstLine && lines[i - 1]?.startsWith('```'))
  return start === -1 ? undefined : lines.slice(start, lines.indexOf('```', start))
}

/** The `count` lines of deny.toml right after the closing line of the allow list of its `[licenses]` table. */
function linesAfterAllow(denyToml, count) {
  const lines = denyToml.split('\n')
  const licenses = lines.indexOf('[licenses]')
  assert.notEqual(licenses, -1, 'deny.toml has no [licenses] table')
  const allow = lines.findIndex((line, i) => i > licenses && line.startsWith('allow = ['))
  assert.notEqual(allow, -1, 'the [licenses] table of deny.toml has no allow list')
  const end = lines.indexOf(']', allow)
  assert.notEqual(end, -1, 'the allow list of deny.toml has no closing line ]')
  return lines.slice(end + 1, end + 1 + count)
}

test('141: fuzz/Cargo.toml holds the tables of section 2.15 in order, each with exactly its keys and values', async () => {
  const manifest = await repositoryText(`${FUZZ_FOLDER}/Cargo.toml`)
  const tables = tomlTables(manifest)

  assert.deepEqual(unreadLines(manifest), [], 'fuzz/Cargo.toml has lines that test 141 cannot read')
  assert.deepEqual(tables[0].entries, {}, 'fuzz/Cargo.toml has keys before its first table')
  assert.deepEqual(tables.slice(1), MANIFEST_TABLES)
})

test('141: unreadLines names each line that is no blank line, comment, table header or key = value entry', () => {
  const text = [
    '# A comment.',
    '[package]',
    'name = "partsmith-fuzz"',
    '',
    '  # An indented comment.',
    'build="build.rs"',
    '  autobins = false',
    '"unused" = "allow"',
    'profile.release.debug = true',
    '[ profile.release ]',
    '[profile.dev] # keep symbols',
    '[[bin]]',
    'test = false',
    '',
  ].join('\n')

  assert.deepEqual(unreadLines(text), [
    'build="build.rs"',
    '  autobins = false',
    '"unused" = "allow"',
    'profile.release.debug = true',
    '[ profile.release ]',
    '[profile.dev] # keep symbols',
  ])
})

test("141: the fuzz crate's lint tables copy the root's workspace lints", async () => {
  const [fuzzTables, rootTables] = await Promise.all(
    [`${FUZZ_FOLDER}/Cargo.toml`, 'Cargo.toml'].map(async (path) =>
      tomlTables(await repositoryText(path)),
    ),
  )

  for (const [fuzzHeader, rootHeader] of Object.entries(LINT_TABLES)) {
    assert.deepEqual(
      tableEntries(fuzzTables, fuzzHeader),
      tableEntries(rootTables, rootHeader),
      `${fuzzHeader} of fuzz/Cargo.toml is not ${rootHeader} of Cargo.toml`,
    )
  }
})

test("141: the root workspace's members are still crates/* and desktop/src-tauri, with no exclude", async () => {
  const workspace = tableEntries(tomlTables(await repositoryText('Cargo.toml')), '[workspace]')

  assert.equal(workspace.members, ROOT_MEMBERS)
  assert.ok(
    !Object.hasOwn(workspace, 'exclude'),
    `the root workspace excludes ${workspace.exclude}`,
  )
})

test('141: fuzz/.gitignore is exactly /target, /artifacts and /coverage, each ending in LF', async () => {
  assert.equal(await repositoryText(`${FUZZ_FOLDER}/.gitignore`), FUZZ_IGNORE)
})

test('142: the bins, the target files, the corpus folders and the nightly matrix name the same targets', async () => {
  const [manifest, files, folders, workflow] = await Promise.all([
    repositoryText(`${FUZZ_FOLDER}/Cargo.toml`),
    folderEntries(TARGETS_FOLDER),
    folderNames(CORPUS_FOLDER),
    repositoryText('.github/workflows/nightly.yml'),
  ])
  const expected = TARGETS.toSorted()
  const sets = {
    '[[bin]] names of fuzz/Cargo.toml': tomlTables(manifest)
      .filter((table) => table.header === '[[bin]]')
      .map((table) => tomlString(table.entries.name)),
    'target files of fuzz/fuzz_targets': files
      .filter((entry) => !entry.folder && entry.name.endsWith('.rs'))
      .map((entry) => entry.name.slice(0, -'.rs'.length)),
    'folders of fuzz/corpus': folders,
    "targets of nightly.yml's fuzz matrix": matrixTargets(workflow),
  }

  for (const [what, targets] of Object.entries(sets)) {
    assert.deepEqual(targets.toSorted(), expected, what)
  }
})

test('142: each target file has the lines of section 2.15 and no unsafe', async () => {
  const texts = await Promise.all(TARGETS.map((target) => repositoryText(targetPath(target))))

  for (const [i, target] of TARGETS.entries()) {
    const path = targetPath(target)
    const lines = texts[i].split('\n')

    assert.ok(lines[0].startsWith('//!'), `${path} does not start with a //! line`)
    assert.ok(lines.includes('#![no_main]'), `${path} has no line #![no_main]`)
    assert.ok(
      lines.includes('use libfuzzer_sys::fuzz_target;'),
      `${path} has no line use libfuzzer_sys::fuzz_target;`,
    )
    assert.equal(
      lines.filter((line) => line.startsWith('fuzz_target!(|data: &[u8]|')).length,
      1,
      `${path} has not exactly one line that starts with fuzz_target!(|data: &[u8]|`,
    )
    assert.deepEqual(
      lines.filter((line) => line.includes('unsafe')),
      [],
      `${path} has lines that hold unsafe`,
    )
  }
})

test('142: each target file calls its reader as section 2.15 names it', async () => {
  const texts = await Promise.all(TARGETS.map((target) => repositoryText(targetPath(target))))

  for (const [i, target] of TARGETS.entries()) {
    for (const call of READER_CALLS[target]) {
      assert.ok(texts[i].includes(call), `${targetPath(target)} does not hold ${call}`)
    }
  }
})

test("142: matrixTargets reads the targets of the fuzz job's rows and no other job's", () => {
  const workflow = [
    'jobs:',
    '  properties:',
    '    strategy:',
    '      matrix:',
    '        include:',
    '          - target: elsewhere',
    '  fuzz:',
    '    strategy:',
    '      matrix:',
    '        include:',
    '          # A comment between the rows.',
    '          - target: first',
    '            detect_leaks: 1',
    '          - target: second',
    '            detect_leaks: 0',
    '    steps:',
    '      - run: echo "- target: not a row"',
    '  later:',
    '    strategy:',
    '      matrix:',
    '        include:',
    '          - target: after',
    '',
  ].join('\n')

  assert.deepEqual(matrixTargets(workflow), ['first', 'second'])
})

test('143: fuzz/rust-toolchain.toml is exactly the three lines of section 2.15, each ending in LF', async () => {
  assert.equal(await repositoryText(`${FUZZ_FOLDER}/rust-toolchain.toml`), FUZZ_TOOLCHAIN)
})

test('143: the root pins the Rust version whose last nightly the fuzz crate pins', async () => {
  const [root, fuzz] = await Promise.all(
    ['rust-toolchain.toml', `${FUZZ_FOLDER}/rust-toolchain.toml`].map(async (path) =>
      tomlString(tableEntries(tomlTables(await repositoryText(path)), '[toolchain]').channel),
    ),
  )

  assert.equal(
    root,
    TOOLCHAIN_PAIR.rust,
    'the root pins another Rust version: move the pair and the nightly pin',
  )
  assert.equal(fuzz, TOOLCHAIN_PAIR.nightly, 'the fuzz crate pins another nightly than the pair')
})

test('144: every package of fuzz/Cargo.lock is in Cargo.lock but the three that only libfuzzer-sys brings', async () => {
  const [fuzzLock, rootLock] = await Promise.all(
    [`${FUZZ_FOLDER}/Cargo.lock`, 'Cargo.lock'].map(async (path) =>
      lockPackages(await repositoryText(path)),
    ),
  )

  assert.deepEqual(
    packagesNotInRootLock(fuzzLock, rootLock),
    FUZZ_ONLY_PACKAGES.map(packageLabel),
    'the packages of fuzz/Cargo.lock that Cargo.lock lacks: make fuzz/Cargo.lock again from Cargo.lock (docs/spec/delivery.md section 2.15)',
  )
  assert.ok(
    fuzzLock.some(
      (fuzzPackage) =>
        fuzzPackage.name === FUZZ_PACKAGE.name &&
        fuzzPackage.version === FUZZ_PACKAGE.version &&
        fuzzPackage.source === undefined,
    ),
    'fuzz/Cargo.lock does not hold partsmith-fuzz 0.0.0',
  )
  for (const only of FUZZ_ONLY_PACKAGES) {
    assert.ok(
      !rootLock.some((rootPackage) => packageLabel(rootPackage) === packageLabel(only)),
      `Cargo.lock holds ${packageLabel(only)}`,
    )
  }
})

test('144: fuzz/Cargo.lock has the version line of Cargo.lock', async () => {
  const [fuzzVersion, rootVersion] = await Promise.all(
    [`${FUZZ_FOLDER}/Cargo.lock`, 'Cargo.lock'].map(
      async (path) => tomlTables(await repositoryText(path))[0].entries.version,
    ),
  )

  assert.ok(rootVersion, 'Cargo.lock has no version line')
  assert.equal(fuzzVersion, rootVersion)
})

test('144: packagesNotInRootLock names each package that differs, and keeps the fuzz crate and path packages', () => {
  const registry = 'registry+https://github.com/rust-lang/crates.io-index'
  const rootLock = [
    { name: 'cc', version: '1.2.0', source: registry, checksum: 'aa' },
    { name: 'libc', version: '0.2.190', source: registry, checksum: 'bb' },
    { name: 'partsmith-model', version: '0.1.0', source: undefined, checksum: undefined },
  ]
  const fuzzLock = [
    { name: 'cc', version: '1.2.0', source: registry, checksum: 'aa' },
    { name: 'libc', version: '0.2.190', source: registry, checksum: 'cc' },
    { name: 'jobserver', version: '0.1.35', source: registry, checksum: 'dd' },
    { name: 'partsmith-model', version: '0.1.0', source: undefined, checksum: undefined },
    { name: 'partsmith-protocol', version: '0.2.0', source: undefined, checksum: undefined },
    { name: 'partsmith-fuzz', version: '0.0.0', source: undefined, checksum: undefined },
  ]

  assert.deepEqual(packagesNotInRootLock(fuzzLock, rootLock), [
    'jobserver 0.1.35',
    'libc 0.2.190',
    'partsmith-protocol 0.2.0',
  ])
})

test('144: lockPackages reads each package of a lock text and leaves out its dependency lines', () => {
  const text = [
    '# This file is automatically @generated by Cargo.',
    'version = 4',
    '',
    '[[package]]',
    'name = "cc"',
    'version = "1.2.0"',
    'source = "registry+https://github.com/rust-lang/crates.io-index"',
    'checksum = "aa"',
    'dependencies = [',
    ' "jobserver",',
    ']',
    '',
    '[[package]]',
    'name = "partsmith-fuzz"',
    'version = "0.0.0"',
    '',
  ].join('\n')

  assert.equal(tomlTables(text)[0].entries.version, '4')
  assert.deepEqual(lockPackages(text), [
    {
      name: 'cc',
      version: '1.2.0',
      source: 'registry+https://github.com/rust-lang/crates.io-index',
      checksum: 'aa',
    },
    { name: 'partsmith-fuzz', version: '0.0.0', source: undefined, checksum: undefined },
  ])
})

test('145: each corpus folder holds files and no folder, each file named by the SHA-1 of its bytes', async () => {
  const folders = await corpusFolders()
  const paths = corpusPaths(folders)
  const sha1s = (await Promise.all(paths.map(repositoryBytes))).map(sha1Of)

  for (const { folder, entries } of folders) {
    assert.ok(entries.length > 0, `${folder} holds no file`)
    assert.deepEqual(
      entries.filter((entry) => entry.folder).map((entry) => entry.name),
      [],
      `${folder} holds folders`,
    )
  }
  // A SHA-1 is 40 lowercase hexadecimal digits. The check also covers the form of each name.
  assert.deepEqual(
    paths.flatMap((path, i) =>
      basename(path) === sha1s[i] ? [] : [`${path} (SHA-1 ${sha1s[i]})`],
    ),
    [],
    'corpus files whose name is not the SHA-1 of their bytes',
  )
})

test('145: .gitattributes holds the line of section 2.15, and Git treats no corpus file as text', async () => {
  const attributes = (await repositoryText('.gitattributes')).split('\n')
  const files = corpusPaths(await corpusFolders())
  const output = repositoryGit('check-attr', 'text', '--', ...files)

  assert.ok(attributes.includes(CORPUS_ATTRIBUTE), `.gitattributes has no line ${CORPUS_ATTRIBUTE}`)
  assert.deepEqual(
    output.split('\n').filter((line) => line !== ''),
    files.map((file) => `${file}: text: unset`),
  )
})

test('146: the ptsm_load corpus holds the bytes of each .ptsm file that Git tracks under crates/ and ui/e2e/fixtures/', async () => {
  const tracked = repositoryGit('ls-files', '-z', '--', ...PTSM_SEED_FOLDERS)
    .split('\0')
    .filter((path) => path.endsWith('.ptsm'))
  const [corpus, files] = await Promise.all([
    corpusFiles('ptsm_load'),
    Promise.all(tracked.map(repositoryBytes)),
  ])
  const missing = tracked.flatMap((path, i) => {
    const sha1 = sha1Of(files[i])
    return holds(corpus, sha1, files[i]) ? [] : [`${path} (SHA-1 ${sha1})`]
  })

  assert.ok(tracked.length > 0, 'Git tracks no .ptsm file under crates/ and ui/e2e/fixtures/')
  assert.deepEqual(
    missing,
    [],
    'fuzz/corpus/ptsm_load/ lacks a copy of these files under their SHA-1 (docs/spec/delivery.md section 2.15, Corpora)',
  )
})

test('146: the protocol_frame corpus holds examples A and B and the two response frames built from their headers', async () => {
  const corpus = await corpusFiles('protocol_frame')

  for (const name of EXAMPLE_FRAMES) {
    assert.ok(corpus.has(name), `fuzz/corpus/protocol_frame/ has no file ${name}`)
  }
  for (const { name, header } of HEADER_FRAMES) {
    assert.ok(
      holds(corpus, name, headerFrame(header)),
      `fuzz/corpus/protocol_frame/${name} is not the frame of ${header}`,
    )
  }
})

test("146: the step_read corpus holds the two STEP files of examples/ and the first finding's input", async () => {
  const [corpus, examples] = await Promise.all([
    corpusFiles('step_read'),
    Promise.all(EXAMPLE_STEPS.map(({ path }) => repositoryBytes(path))),
  ])

  for (const [i, { path, name }] of EXAMPLE_STEPS.entries()) {
    assert.ok(
      holds(corpus, name, examples[i]),
      `fuzz/corpus/step_read/${name} is not a copy of ${path}`,
    )
  }
  assert.ok(
    holds(corpus, FINDING_1.name, findingInput(await repositoryBytes(FINDING_1.source))),
    `fuzz/corpus/step_read/${FINDING_1.name} is not the input that the recipe of section 3.8 builds`,
  )
})

test('146: headerFrame lays out a frame as protocol.md section 2.2, which gives example A', async () => {
  const frame = headerFrame(
    '{"protocol_version":1,"messages":[{"kind":"request","id":1,"command":{"type":"engine_info"}}],"buffers":[]}',
  )

  assert.equal(frame.length, 120)
  assert.equal(sha1Of(frame), EXAMPLE_FRAMES[0])
})

test('146: findingInput keeps the lines of the recipe, each ending in LF', () => {
  const source = Buffer.from(
    Array.from({ length: 414 }, (_, i) => `line ${i + 1}\n`).join(''),
    'latin1',
  )
  const lines = findingInput(source).toString('latin1').split('\n')

  assert.equal(lines.length, 108 + 1 + 171 + 1)
  assert.equal(lines[107], 'line 108')
  assert.equal(lines[108], FINDING_1.line)
  assert.equal(lines[109], 'line 244')
  assert.equal(lines.at(-2), 'line 414')
  assert.equal(lines.at(-1), '')
})

test('148: right after its allow list, deny.toml holds the exception of section 2.15, line for line', async () => {
  const [denyToml, spec] = await Promise.all([
    repositoryText('deny.toml'),
    repositoryText('docs/spec/delivery.md'),
  ])
  const block = fencedBlock(spec, EXCEPTION_FIRST_LINE)

  assert.ok(block, `section 2.15 has no TOML block that starts with ${EXCEPTION_FIRST_LINE}`)
  assert.equal(block.length, 4)
  assert.deepEqual(linesAfterAllow(denyToml, block.length), block)
})

test('148: the allow list is still the fourteen identifiers of section 2.10, without NCSA', async () => {
  const allowed = allowedLicenses(await repositoryText('deny.toml'))

  assert.deepEqual(allowed, ALLOWED_LICENSES)
  assert.ok(!allowed.includes(FUZZ_LICENSE), `the allow list of deny.toml holds ${FUZZ_LICENSE}`)
})

test('148: linesAfterAllow takes the lines after the closing line of the allow list of [licenses]', () => {
  const denyToml = [
    '[graph]',
    'allow = [',
    '  "Other",',
    ']',
    'not = "this"',
    '[licenses]',
    'allow = [',
    '  "MIT",',
    ']',
    'first = 1',
    'second = 2',
    '',
  ].join('\n')

  assert.deepEqual(linesAfterAllow(denyToml, 2), ['first = 1', 'second = 2'])
})
