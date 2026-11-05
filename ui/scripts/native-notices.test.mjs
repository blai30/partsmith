// Acceptance test 116 of docs/spec/delivery.md section 6 for ui/scripts/native-notices.mjs, which writes the notices
// of the crates compiled into the native npm packages whose tarballs lack them (section 2.13).
import assert from 'node:assert/strict'
import { spawnSync } from 'node:child_process'
import { mkdir, mkdtemp, readFile, readdir, realpath, rm } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import process from 'node:process'
import { test } from 'node:test'
import { fileURLToPath, pathToFileURL } from 'node:url'

import { git, isolatedGitEnvironment } from './isolated-git.mjs'
import { nativeNotices, repositoryFolder, writeNativeNotices } from './native-notices.mjs'
import { writeFiles } from './occt-fixture.mjs'

const scriptPath = fileURLToPath(new URL('native-notices.mjs', import.meta.url))

const CRATES_IO = 'registry+https://github.com/rust-lang/crates.io-index'

// The waits of the tests before the second and the third attempt of a clone.
const TEST_RETRY_DELAYS = [1, 1]

// The files of the fixture's source at its tag: a workspace whose binding crate ships no license text, a helper
// crate that ships one, a crate the binding does not reach, and two license files at the root.
const SOURCE_FILES = {
  'Cargo.toml': '[workspace]\nmembers = ["crates/*"]\n',
  COPYRIGHT: 'native-ui copyright\n',
  LICENSE: 'native-ui license\n',
  'README.md': 'native-ui\n',
  'crates/binding/Cargo.toml': '[package]\nname = "native_ui_binding"\nversion = "0.0.0"\n',
  'crates/helper/Cargo.toml': '[package]\nname = "helper"\nversion = "0.1.0"\n',
  'crates/helper/LICENSE': 'helper license\n',
  'crates/other/Cargo.toml': '[package]\nname = "other"\nversion = "0.1.0"\n',
}

// The crates of the fixture's registry, each folder `{name}-{version}` with its files beside its Cargo.toml.
const REGISTRY_FILES = {
  'shipped-0.9.0/LICENSE-APACHE': 'Apache shipped\n',
  'shipped-0.9.0/LICENSE-MIT': 'MIT shipped\n',
  'shipped-0.10.0/LICENSE-APACHE': 'Apache shipped\n',
  'shipped-0.10.0/LICENSE-MIT': 'MIT shipped\n',
  'slash-0.2.0/src/lib.rs': '// no license file\n',
  'bare-2.0.0/src/lib.rs': '// no license file\n',
  'dev-only-1.0.0/LICENSE': 'dev only\n',
}

// The files of the fixture's repository root: the standard Apache-2.0 text and the copies of bare's repository.
const ROOT_FILES = {
  'ui/scripts/notices/licenses/Apache-2.0.txt': 'Apache License text\n',
  'ui/scripts/notices/repositories/github.com/owner/bare/LICENSE': 'Copyright bare\n',
  'ui/scripts/notices/repositories/github.com/owner/bare/NOTICE': 'bare notice\n',
}

/** A new folder that the test removes when it ends. */
async function temporaryDirectory(context) {
  const directory = await mkdtemp(join(tmpdir(), 'partsmith-native-notices-test-'))
  context.after(() => rm(directory, { recursive: true, force: true }))
  return directory
}

/**
 * Makes the fixture: a Git repository of SOURCE_FILES with the tag `v2.0.0`, a registry folder and a repository root.
 * Resolves to their paths, the repository's `file:` URL and the tag's commit.
 */
async function writeFixture(context) {
  const folder = await temporaryDirectory(context)
  const environment = await isolatedGitEnvironment(folder)
  const repository = join(folder, 'native-ui')
  await mkdir(repository)
  git(environment, repository, 'init', '--initial-branch=main')
  await writeFiles(repository, SOURCE_FILES)
  git(environment, repository, 'add', '--all')
  git(environment, repository, 'commit', '--message', 'native-ui 2.0.0')
  git(environment, repository, 'tag', '--annotate', 'v2.0.0', '--message', 'native-ui 2.0.0')
  const commit = git(environment, repository, 'rev-parse', 'HEAD')
  const registry = join(folder, 'registry')
  const root = join(folder, 'root')
  await writeFiles(registry, REGISTRY_FILES)
  await writeFiles(root, ROOT_FILES)
  return { repository, environment, source: pathToFileURL(repository).href, commit, registry, root }
}

/** The table entry of the fixture's native package, whose tag names `commit`. */
function nativeEntry(fixture, commit = fixture.commit) {
  return {
    name: '@scope/native-ui',
    version: '2.0.0',
    source: fixture.source,
    tag: 'v2.0.0',
    crates: [],
    notices: { crate: 'native_ui_binding', commit },
  }
}

/** A package of `cargo metadata` with the folder `folder`. */
function metadataPackage(name, version, folder, fields = {}) {
  return {
    id: `${name} ${version}`,
    name,
    version,
    source: CRATES_IO,
    license: null,
    license_file: null,
    repository: null,
    manifest_path: join(folder, 'Cargo.toml'),
    ...fields,
  }
}

/** A dependency edge of `cargo metadata` to `pkg` of the kind `kind` (null for a normal one). */
function edge(pkg, kind = null) {
  return { pkg, dep_kinds: [{ kind, target: null }] }
}

/**
 * The parsed `cargo metadata` output of the fixture's source at `clone` with its registry: the binding reaches the
 * helper and shipped 0.10.0 through normal edges, slash through a build edge and dev-only through a dev edge only;
 * the helper reaches shipped 0.9.0 and bare. `other` and `unreached` are reached by nothing. `extra` adds packages,
 * each reached from the binding.
 */
function fixtureMetadata(fixture, clone, extra = []) {
  const registryCrate = (name, version, fields) =>
    metadataPackage(name, version, join(fixture.registry, `${name}-${version}`), fields)
  const workspaceCrate = (name, folder, fields) =>
    metadataPackage(name, fields.version, join(clone, 'crates', folder), {
      source: null,
      ...fields,
    })
  const packages = [
    workspaceCrate('native_ui_binding', 'binding', { version: '0.0.0' }),
    workspaceCrate('helper', 'helper', { version: '0.1.0', license: 'MIT' }),
    workspaceCrate('other', 'other', { version: '0.1.0' }),
    registryCrate('shipped', '0.9.0', { license: 'MIT OR Apache-2.0' }),
    registryCrate('shipped', '0.10.0', { license: 'MIT OR Apache-2.0' }),
    registryCrate('slash', '0.2.0', { license: 'MIT/Apache-2.0' }),
    registryCrate('bare', '2.0.0', {
      license: 'MIT',
      repository: 'https://github.com/owner/bare.git',
    }),
    registryCrate('dev-only', '1.0.0', { license: 'MIT' }),
    registryCrate('unreached', '1.0.0', { license: 'GPL-3.0-only' }),
    ...extra,
  ]
  const deps = {
    'native_ui_binding 0.0.0': [
      edge('helper 0.1.0'),
      edge('shipped 0.10.0'),
      edge('slash 0.2.0', 'build'),
      edge('dev-only 1.0.0', 'dev'),
      ...extra.map((listed) => edge(listed.id)),
    ],
    'helper 0.1.0': [edge('shipped 0.9.0'), edge('bare 2.0.0')],
  }
  return {
    packages,
    resolve: { nodes: packages.map(({ id }) => ({ id, deps: deps[id] ?? [] })) },
  }
}

/**
 * A fake `run` that answers `cargo metadata` with the fixture's metadata for the clone its `--manifest-path` names,
 * and records each call as `{ command, directory }`.
 */
function fakeRun(fixture, extra = []) {
  const calls = []
  const run = async (command, commandArguments, directory) => {
    calls.push({ command: [command, ...commandArguments].join(' '), directory })
    const manifest = commandArguments[commandArguments.indexOf('--manifest-path') + 1]
    return JSON.stringify(fixtureMetadata(fixture, join(manifest, '..'), extra))
  }
  return { run, calls }
}

/** The notices file that the fixture gives, as section 2.13 lays it out. */
function expectedNotices(fixture) {
  const tree = `${fixture.source}/tree/v2.0.0`
  return [
    'bare 2.0.0, MIT, https://crates.io/crates/bare/2.0.0, text from its repository, texts [1] [2]',
    `helper 0.1.0, MIT, ${tree}, texts [3]`,
    `native_ui_binding 0.0.0, see texts, ${tree}, text from its repository, texts [4] [5]`,
    'shipped 0.9.0, MIT OR Apache-2.0, https://crates.io/crates/shipped/0.9.0, texts [6] [7]',
    'shipped 0.10.0, MIT OR Apache-2.0, https://crates.io/crates/shipped/0.10.0, texts [6] [7]',
    'slash 0.2.0, MIT/Apache-2.0, https://crates.io/crates/slash/0.2.0, standard text, texts [8]',
    '',
    '[1]',
    '',
    'Copyright bare',
    '',
    '[2]',
    '',
    'bare notice',
    '',
    '[3]',
    '',
    'helper license',
    '',
    '[4]',
    '',
    'native-ui copyright',
    '',
    '[5]',
    '',
    'native-ui license',
    '',
    '[6]',
    '',
    'Apache shipped',
    '',
    '[7]',
    '',
    'MIT shipped',
    '',
    '[8]',
    '',
    'Apache License text',
    '',
  ].join('\n')
}

/** The path of the fixture's notices file, which the entry's stem names. */
function noticesPath(fixture) {
  return join(fixture.root, 'ui', 'scripts', 'notices', 'native', 'scope-native-ui-2.0.0.txt')
}

/** Writes the notices of the fixture's entry with the fake run of `extra` and resolves to the run's calls. */
async function writeFixtureNotices(fixture, { entry = nativeEntry(fixture), extra = [] } = {}) {
  const { run, calls } = fakeRun(fixture, extra)
  const file = await writeNativeNotices(fixture.root, entry, {
    run,
    retryDelays: TEST_RETRY_DELAYS,
  })
  return { file, calls }
}

test('116: the notices file lists every crate the binding reaches, sorted, with its texts numbered in first use', async (context) => {
  const fixture = await writeFixture(context)

  const { file } = await writeFixtureNotices(fixture)

  assert.equal(file, 'ui/scripts/notices/native/scope-native-ui-2.0.0.txt')
  assert.equal(await readFile(noticesPath(fixture), 'utf8'), expectedNotices(fixture))
})

test('116: cargo metadata of the clone runs once in the repository root, and the clone is gone afterwards', async (context) => {
  const fixture = await writeFixture(context)

  const { calls } = await writeFixtureNotices(fixture)

  assert.equal(calls.length, 1)
  assert.equal(calls[0].directory, fixture.root)
  const command = 'cargo metadata --format-version 1 --locked --all-features --manifest-path '
  assert.ok(calls[0].command.startsWith(command), calls[0].command)
  const clone = join(calls[0].command.slice(command.length), '..')
  await assert.rejects(readdir(clone), { code: 'ENOENT' })
})

test('116: two runs write the same bytes', async (context) => {
  const fixture = await writeFixture(context)

  await writeFixtureNotices(fixture)
  const first = await readFile(noticesPath(fixture))
  await writeFixtureNotices(fixture)

  assert.deepEqual(await readFile(noticesPath(fixture)), first)
})

test('116: nativeNotices reads the clone it is given and runs no git command', async (context) => {
  const fixture = await writeFixture(context)
  const clone = join(await temporaryDirectory(context), 'source')
  await writeFiles(clone, SOURCE_FILES)
  const { run } = fakeRun(fixture)

  const text = await nativeNotices(fixture.root, run, clone, nativeEntry(fixture))

  assert.equal(text, expectedNotices(fixture))
})

test('116: a tag that names another commit than the entry pins fails with the tag-commit message and writes nothing', async (context) => {
  const fixture = await writeFixture(context)
  const pinned = 'f'.repeat(40)

  await assert.rejects(writeFixtureNotices(fixture, { entry: nativeEntry(fixture, pinned) }), {
    message: `The tag v2.0.0 of ${fixture.source} names the commit ${fixture.commit}, not ${pinned}, which NATIVE_PACKAGES of scripts/dependency-source-archive.mjs pins for @scope/native-ui 2.0.0. Upstream may have moved the tag; nothing was written (docs/spec/delivery.md section 2.13).`,
  })
  await assert.rejects(readFile(noticesPath(fixture)), { code: 'ENOENT' })
})

test('116: a crate without texts whose repository has no copies fails with the missing-copy message', async (context) => {
  const fixture = await writeFixture(context)
  const repository = 'https://github.com/owner/missing'
  const missing = metadataPackage('missing', '1.0.0', join(fixture.registry, 'bare-2.0.0'), {
    license: 'ISC',
    repository,
  })

  await assert.rejects(writeFixtureNotices(fixture, { extra: [missing] }), {
    message: `Rust crate missing 1.0.0 ships no license text, and ui/scripts/notices/repositories/github.com/owner/missing/ is missing. Commit a copy of the license files at the root of ${repository} there (docs/spec/delivery.md section 2.13).`,
  })
  await assert.rejects(readFile(noticesPath(fixture)), { code: 'ENOENT' })
})

test('116: a crate without texts that names no repository fails with its message', async (context) => {
  const fixture = await writeFixture(context)
  const nameless = metadataPackage('nameless', '1.0.0', join(fixture.registry, 'bare-2.0.0'), {
    license: 'ISC',
  })

  await assert.rejects(writeFixtureNotices(fixture, { extra: [nameless] }), {
    message:
      'Rust crate nameless 1.0.0 ships no license text, takes no standard text and names no repository with an owner and a name (docs/spec/delivery.md section 2.13).',
  })
})

test('116: a crate of the workspace without texts fails when the root of the tag holds no license file', async (context) => {
  const fixture = await writeFixture(context)
  const clone = join(await temporaryDirectory(context), 'source')
  const { COPYRIGHT, LICENSE, ...unlicensed } = SOURCE_FILES
  assert.ok(COPYRIGHT && LICENSE)
  await writeFiles(clone, unlicensed)
  const { run } = fakeRun(fixture)

  await assert.rejects(nativeNotices(fixture.root, run, clone, nativeEntry(fixture)), {
    message: `Rust crate native_ui_binding 0.0.0 of ${fixture.source} ships no license text, and the root of the tag v2.0.0 holds none (docs/spec/delivery.md section 2.13).`,
  })
})

test('116: a crate from a Git source fails with its message before any text is read', async (context) => {
  const fixture = await writeFixture(context)
  const source = 'git+https://github.com/owner/git-dep?rev=abc#abc'
  const fromGit = metadataPackage('git-dep', '0.1.0', join(fixture.registry, 'missing-folder'), {
    source,
  })

  await assert.rejects(writeFixtureNotices(fixture, { extra: [fromGit] }), {
    message: `Rust crate git-dep 0.1.0 of the source of @scope/native-ui 2.0.0 comes from ${source}, neither crates.io nor the source's own workspace. The notices name a crate on crates.io or in the tag's tree (docs/spec/delivery.md section 2.13).`,
  })
})

test('116: repositoryFolder is the host, the owner and the name of a repository URL', () => {
  const cases = [
    ['https://github.com/oxc-project/oxc', 'github.com/oxc-project/oxc'],
    ['https://github.com/oxc-project/fast-glob.git', 'github.com/oxc-project/fast-glob'],
    [
      'https://github.com/Canop/lazy-regex/tree/main/src/proc_macros',
      'github.com/Canop/lazy-regex',
    ],
    ['https://example.com/', null],
    ['https://example.com/owner', null],
    ['not a URL', null],
    [null, null],
  ]

  assert.deepEqual(
    cases.map(([url]) => repositoryFolder(url)),
    cases.map(([, folder]) => folder),
  )
})

test('116: the command exits 2 with the usage line for any argument', async () => {
  const result = spawnSync(process.execPath, [await realpath(scriptPath), 'write'], {
    encoding: 'utf8',
  })

  assert.equal(result.status, 2)
  assert.equal(result.stderr, 'usage: node scripts/native-notices.mjs\n')
  assert.equal(result.stdout, '')
})
