// Acceptance tests 55, 58 and 99 of docs/spec/delivery.md section 6 for scripts/occt-source-archive.sh, the commit
// check of scripts/build-occt.sh (section 2.13), the arguments that script gives CMake on Windows and the Windows
// rustflags and rustdocflags of .cargo/config.toml (section 3.6, The C runtime on Windows).
import assert from 'node:assert/strict'
import { spawnSync } from 'node:child_process'
import { chmod, copyFile, mkdtemp, readFile, readdir, rm, stat, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { basename, dirname, join } from 'node:path'
import { after, before, test } from 'node:test'
import { fileURLToPath, pathToFileURL } from 'node:url'
import { gunzipSync } from 'node:zlib'

import { git, isolatedGitEnvironment } from './isolated-git.mjs'
import {
  makeTaggedRepository,
  occtVersionText,
  readOcctPins,
  sourceArchiveName,
  writeFiles,
} from './occt-fixture.mjs'

const archiveScriptPath = fileURLToPath(
  new URL('../../scripts/occt-source-archive.sh', import.meta.url),
)
const buildScriptPath = fileURLToPath(new URL('../../scripts/build-occt.sh', import.meta.url))
const cargoConfigurationPath = fileURLToPath(new URL('../../.cargo/config.toml', import.meta.url))

const usageLine = 'usage: scripts/occt-source-archive.sh <directory>'
const OCCT_URL = 'https://github.com/Open-Cascade-SAS/OCCT.git'

// The files of the fixture's tagged commit, each with a text of its own and LF line endings. `.gitattributes` names
// only *.bat, so no attribute covers the others, and the .cmake file has two lines.
const TAGGED_FILES = {
  '.clang-format': 'BasedOnStyle: Google\n',
  'CMakeLists.txt': 'project (OCCT)\n',
  'LICENSE_LGPL_21.txt': 'GNU LESSER GENERAL PUBLIC LICENSE\n',
  'OCCT_LGPL_EXCEPTION.txt': 'Open CASCADE Exception\n',
  'README.md': '# Open CASCADE Technology\n',
  'adm/cmake/occt_macros.cmake': 'macro (OCCT_CHECK)\nendmacro()\n',
  'src/TKernel/PACKAGES': 'Standard\n',
  'src/Standard/Standard.cxx': '// Standard, as tagged\n',
  'data/step/box.stp': 'ISO-10303-21;\n',
  'dox/index.md': '# Documentation\n',
  'samples/sample.cxx': '// A sample\n',
  'tests/bugs/begin.tcl': '# Test setup\n',
  'tools/tool.cxx': '// A tool\n',
  '.github/workflows/build.yml': 'name: Build\n',
  '.gitattributes': '*.bat eol=crlf\n',
}

// The uncommitted change of the working tree, after the second commit.
const CMAKE_UNCOMMITTED = 'project (Changed)\n'

// The files the archive holds, in the order git archive writes them (section 2.13).
const ARCHIVED_FILES = [
  '.clang-format',
  'CMakeLists.txt',
  'LICENSE_LGPL_21.txt',
  'OCCT_LGPL_EXCEPTION.txt',
  'README.md',
  'adm/cmake/occt_macros.cmake',
  'src/Standard/Standard.cxx',
  'src/TKernel/PACKAGES',
]

// A stand-in for git that records the arguments of each git clone as one line, separated by NUL, in $CLONE_CALLS and
// clones $FIXTURE_URL in place of OCCT's URL; it hands every command to the real git at $REAL_GIT.
const GIT_STUB = `#!/usr/bin/env bash
if [ "$1" = clone ]; then
  printf '%s\\0' "$@" >> "$CLONE_CALLS"
  printf '\\n' >> "$CLONE_CALLS"
  arguments=()
  for argument in "$@"; do
    if [ "$argument" = '${OCCT_URL}' ]; then
      arguments+=("$FIXTURE_URL")
    else
      arguments+=("$argument")
    fi
  done
  set -- "\${arguments[@]}"
fi
exec "$REAL_GIT" "$@"
`

// A stand-in for cmake that records its arguments as one line, separated by NUL, in $CMAKE_CALLS and exits 0.
const CMAKE_STUB = `#!/usr/bin/env bash
printf '%s\\0' "$@" >> "$CMAKE_CALLS"
printf '\\n' >> "$CMAKE_CALLS"
`

// A stand-in for emcmake that records its arguments as one line, separated by NUL, in $EMCMAKE_CALLS and exits 0.
const EMCMAKE_STUB = `#!/usr/bin/env bash
printf '%s\\0' "$@" >> "$EMCMAKE_CALLS"
printf '\\n' >> "$EMCMAKE_CALLS"
`

// A stand-in for uname that prints $UNAME_NAME, whatever its arguments.
const UNAME_STUB = `#!/usr/bin/env bash
printf '%s\\n' "$UNAME_NAME"
`

// The three arguments that scripts/build-occt.sh gives CMake on Windows, in their order (section 3.6).
const WINDOWS_ARGUMENTS = [
  '-DINSTALL_DIR_LAYOUT=Unix',
  '-DCMAKE_MSVC_RUNTIME_LIBRARY=MultiThreaded',
  '-DCMAKE_POLICY_DEFAULT_CMP0091=NEW',
]

// What uname -s prints in Git Bash, MSYS2 and Cygwin on Windows, and on Linux and macOS.
const WINDOWS_SYSTEMS = ['MINGW64_NT-10.0-26100', 'MSYS_NT-10.0-26100', 'CYGWIN_NT-10.0-26100']
const OTHER_SYSTEMS = ['Linux', 'Darwin']

// The Cargo table of the Windows target and the flags of the static runtime it gives rustc and rustdoc (section 3.6).
const WINDOWS_TARGET_TABLE = 'target.x86_64-pc-windows-msvc'
const STATIC_RUNTIME_FLAGS = ['-C', 'target-feature=+crt-static']

// The git that the stub hands its commands to.
const realGit = spawnSync('bash', ['-c', 'command -v git'], { encoding: 'utf8' }).stdout.trim()

const { version, tag, commit: pinnedCommit } = await readOcctPins()
const archiveName = sourceArchiveName(version)

/** The no-tag message of section 4. */
function noTagMessage(src) {
  return `${src} holds no tag ${tag}. Point OCCT_WORK_DIR at a folder whose src is a clone that scripts/build-occt.sh made, or at one without src, where the script clones the tag (docs/spec/delivery.md section 2.13).`
}

/** The tag-commit message of section 4. */
function tagCommitMessage(src, commit, pinned) {
  return `The tag ${tag} in ${src} names the commit ${commit}, not ${pinned}, which scripts/occt-version.sh pins. Upstream may have moved the tag; nothing was built or archived from it (docs/spec/delivery.md section 2.13).`
}

/**
 * Makes a fixture repository at `directory`: a commit of `files` tagged `{tag}` (annotated), a second commit that
 * changes `src/Standard/Standard.cxx`, and an uncommitted change to `CMakeLists.txt`. Returns both commits.
 */
async function makeFixture(environment, directory, files) {
  const commits = await makeTaggedRepository(environment, directory, files, { version, tag })
  await writeFile(join(directory, 'CMakeLists.txt'), CMAKE_UNCOMMITTED)
  return commits
}

// The fixture every test shares: its temporary root, the isolated Git environment, the folder whose src is the
// fixture repository, both commits, and the result and bytes of a first run into an empty folder.
let fixture

before(async () => {
  fixture = { root: await mkdtemp(join(tmpdir(), 'partsmith-occt-archive-')) }
  fixture.environment = await isolatedGitEnvironment(fixture.root)
  fixture.work = join(fixture.root, 'work')
  Object.assign(
    fixture,
    await makeFixture(fixture.environment, join(fixture.work, 'src'), TAGGED_FILES),
  )
  const out = await emptyFolder()
  const result = await runArchive([out], { pinned: fixture.tagged })
  fixture.first = { out, result, bytes: await readFile(join(out, archiveName)).catch(() => null) }
})

after(() => fixture && rm(fixture.root, { recursive: true, force: true }))

/** A new empty folder in the fixture's root. */
function emptyFolder() {
  return mkdtemp(join(fixture.root, 'out-'))
}

/**
 * A copy of the script at `scriptPath` in a new folder, next to an occt-version.sh that pins `{v}`, `{tag}` and
 * `pinned`.
 */
async function scriptCopy(scriptPath, pinned) {
  const folder = await mkdtemp(join(fixture.root, 'scripts-'))
  const copy = join(folder, basename(scriptPath))
  await copyFile(scriptPath, copy)
  await writeFile(
    join(folder, 'occt-version.sh'),
    occtVersionText({ version, tag, commit: pinned }),
  )
  return copy
}

/**
 * Runs a copy of the script at `scriptPath` with `bash`, `scriptArguments` and `OCCT_WORK_DIR` (the fixture's by
 * default) in the isolated environment plus `environment`.
 */
async function runScript(
  scriptPath,
  scriptArguments,
  { pinned, work = fixture.work, environment = {} },
) {
  return spawnSync('bash', [await scriptCopy(scriptPath, pinned), ...scriptArguments], {
    encoding: 'utf8',
    env: { ...fixture.environment, OCCT_WORK_DIR: work, ...environment },
  })
}

/** Runs a copy of scripts/occt-source-archive.sh as `runScript` does. */
function runArchive(scriptArguments, options) {
  return runScript(archiveScriptPath, scriptArguments, options)
}

/**
 * Runs a copy of scripts/build-occt.sh with `<target> <work>/prefix` (`native` unless `target` names another) and
 * `OCCT_JOBS=1`, as `runScript` does.
 */
function runBuild({ target = 'native', work, environment, ...options }) {
  return runScript(buildScriptPath, [target, join(work, 'prefix')], {
    ...options,
    work,
    environment: { OCCT_JOBS: '1', ...environment },
  })
}

/**
 * A new folder of stubs to put first on `PATH`: the git stub, which clones the fixture repository, and the cmake
 * stub. Returns the environment that sets them up and functions that read the argument lists of the git clones and
 * of the cmake calls they recorded.
 */
async function stubs() {
  const folder = await mkdtemp(join(fixture.root, 'stub-'))
  const cloneCalls = join(folder, 'clone-calls')
  const cmakeCalls = join(folder, 'cmake-calls')
  await writeStubs(folder, { git: GIT_STUB, cmake: CMAKE_STUB })
  await Promise.all([writeFile(cloneCalls, ''), writeFile(cmakeCalls, '')])
  return {
    environment: {
      PATH: `${folder}:${process.env.PATH}`,
      CLONE_CALLS: cloneCalls,
      CMAKE_CALLS: cmakeCalls,
      REAL_GIT: realGit,
      FIXTURE_URL: pathToFileURL(join(fixture.work, 'src')).href,
    },
    clones: () => recordedCalls(cloneCalls),
    cmakeCalls: () => recordedCalls(cmakeCalls),
  }
}

/** Writes each stub of `stubTexts`, a map from a command's name to its script, into `folder` as a file its owner can run. */
async function writeStubs(folder, stubTexts) {
  await Promise.all(
    Object.entries(stubTexts).map(async ([name, text]) => {
      await writeFile(join(folder, name), text)
      await chmod(join(folder, name), 0o755)
    }),
  )
}

/** The argument lists a stub recorded in `path`: one per line, each argument followed by NUL. */
async function recordedCalls(path) {
  return (await readFile(path, 'utf8'))
    .split('\n')
    .filter((call) => call !== '')
    .map((call) => call.split('\0').slice(0, -1))
}

/** The arguments of the clone both scripts make into `<work>/src` (section 2.13). */
function cloneArguments(work) {
  return ['clone', '--depth', '1', '--branch', tag, OCCT_URL, join(work, 'src')]
}

/** Whether `path` exists. */
function exists(path) {
  return stat(path).then(
    () => true,
    () => false,
  )
}

/** The bytes of the archive a successful run of the script writes into a new empty folder. */
async function archiveBytes(options) {
  const out = await emptyFolder()
  const result = await runArchive([out], options)
  assert.equal(result.status, 0, result.stderr)
  return readFile(join(out, archiveName))
}

/** Asserts that a run failed with `status` (any status but 0 when it is not given) and left `out` empty. */
async function assertFailed(result, out, status) {
  if (status === undefined) {
    assert.notEqual(result.status, 0)
  } else {
    assert.equal(result.status, status)
  }
  assert.deepEqual(await readdir(out), [])
}

/** The GIT_CONFIG_COUNT variables that set `settings`, a list of `[key, value]`. */
function configurationVariables(settings) {
  const variables = { GIT_CONFIG_COUNT: String(settings.length) }
  settings.forEach(([key, value], i) => {
    variables[`GIT_CONFIG_KEY_${i}`] = key
    variables[`GIT_CONFIG_VALUE_${i}`] = value
  })
  return variables
}

/**
 * The Git settings, as `[key, value]`, that would change the archive's bytes without the script's -c options: line
 * endings, file modes, and a global attributes file, written in the fixture's root, that leaves out the .cmake file.
 */
async function changingSettings() {
  const attributes = join(fixture.root, 'global-attributes')
  await writeFile(attributes, '*.cmake export-ignore\n')
  return [
    ['core.autocrlf', 'true'],
    ['tar.umask', '0077'],
    ['core.attributesFile', attributes],
  ]
}

test('55: scripts/occt-version.sh pins the commit of its tag as 40 lowercase hexadecimal digits', () => {
  assert.match(pinnedCommit, /^[0-9a-f]{40}$/)
})

test('55: the script writes the archive into an empty folder and prints what it wrote', async () => {
  const { out, result } = fixture.first

  assert.equal(result.status, 0, result.stderr)
  assert.equal(
    result.stdout,
    `Wrote ${out}/${archiveName} from ${tag}, commit ${fixture.tagged}.\n`,
  )
  assert.deepEqual(await readdir(out), [archiveName])
})

test('55: the archive holds the files the build reads, from the tagged commit', () => {
  const archive = join(fixture.first.out, archiveName)
  const listing = spawnSync('tar', ['-tzf', archive], { encoding: 'utf8' })
  const extract = (path) =>
    spawnSync('tar', ['-xzOf', archive, `occt-${version}/${path}`], { encoding: 'utf8' }).stdout

  assert.equal(listing.status, 0, listing.stderr)
  assert.deepEqual(
    listing.stdout.split('\n').filter((entry) => entry !== '' && !entry.endsWith('/')),
    ARCHIVED_FILES.map((path) => `occt-${version}/${path}`),
  )
  assert.equal(extract('CMakeLists.txt'), TAGGED_FILES['CMakeLists.txt'])
  assert.equal(extract('src/Standard/Standard.cxx'), TAGGED_FILES['src/Standard/Standard.cxx'])
})

test('55: the gzip header holds no file name and no time, and the tar names the tagged commit', () => {
  const { bytes } = fixture.first
  const commitId = spawnSync('git', ['get-tar-commit-id'], {
    input: gunzipSync(bytes),
    env: fixture.environment,
    encoding: 'utf8',
  })

  assert.equal(bytes[3], 0)
  assert.deepEqual([...bytes.subarray(4, 8)], [0, 0, 0, 0])
  assert.equal(commitId.stdout, `${fixture.tagged}\n`)
})

test('55: a second run writes the same bytes', async () => {
  assert.deepEqual(await archiveBytes({ pinned: fixture.tagged }), fixture.first.bytes)
})

test('55: without src the script clones the tag as build-occt.sh does and writes the same bytes', async () => {
  const work = await mkdtemp(join(fixture.root, 'work-'))
  const stub = await stubs()

  const bytes = await archiveBytes({ pinned: fixture.tagged, work, environment: stub.environment })

  assert.deepEqual(await stub.clones(), [cloneArguments(work)])
  assert.deepEqual(bytes, fixture.first.bytes)
})

test('55: each Git setting that changes a plain git archive leaves the bytes the same', async () => {
  const settings = await changingSettings()
  const plainArchive = (variables) =>
    spawnSync(
      'git',
      ['archive', '--format=tar', `--prefix=occt-${version}/`, tag, '--', ...ARCHIVED_FILES],
      { cwd: join(fixture.work, 'src'), env: { ...fixture.environment, ...variables } },
    ).stdout
  const plain = plainArchive({})
  // The fixture must exercise each setting: without the script's options, each one changes the archive.
  for (const setting of settings) {
    assert.notDeepEqual(plainArchive(configurationVariables([setting])), plain, setting[0])
  }

  const bytes = await archiveBytes({
    pinned: fixture.tagged,
    environment: configurationVariables(settings),
  })

  assert.deepEqual(bytes, fixture.first.bytes)
})

/** Asserts that a run with the arguments `extra` after an empty folder exits 2 with the usage line. */
async function assertUsage(extra) {
  const out = await emptyFolder()
  const result = await runArchive(extra === null ? [] : [out, ...extra], { pinned: fixture.tagged })

  await assertFailed(result, out, 2)
  assert.equal(result.stderr, `${usageLine}\n`)
}

test('55: no argument exits 2 with the usage line', () => assertUsage(null))

test('55: two arguments exit 2 with the usage line', () => assertUsage(['extra']))

test('55: a folder that is not a Git clone exits 1 with the no-tag message', async () => {
  const work = await mkdtemp(join(fixture.root, 'work-'))
  await writeFiles(join(work, 'src'), TAGGED_FILES)
  const out = await emptyFolder()

  const result = await runArchive([out], { pinned: fixture.tagged, work })

  await assertFailed(result, out, 1)
  assert.equal(result.stderr, `${noTagMessage(join(work, 'src'))}\n`)
})

test('55: a clone without the tag exits 1 with the no-tag message', async () => {
  const work = await mkdtemp(join(fixture.root, 'work-'))
  const src = join(work, 'src')
  git(fixture.environment, work, 'clone', '--quiet', join(fixture.work, 'src'), src)
  git(fixture.environment, src, 'tag', '--delete', tag)
  const out = await emptyFolder()

  const result = await runArchive([out], { pinned: fixture.tagged, work })

  await assertFailed(result, out, 1)
  assert.equal(result.stderr, `${noTagMessage(src)}\n`)
})

test('55: a tag that names another commit than OCCT_COMMIT exits 1 with the tag-commit message', async () => {
  const out = await emptyFolder()

  const result = await runArchive([out], { pinned: fixture.second })

  await assertFailed(result, out, 1)
  assert.equal(
    result.stderr,
    `${tagCommitMessage(join(fixture.work, 'src'), fixture.tagged, fixture.second)}\n`,
  )
  // A clone the script did not make stays.
  assert.ok(await exists(join(fixture.work, 'src', '.git')), 'the existing clone was removed')
})

test('55: a tag that names another commit in a clone the script made exits 1 and removes the clone', async () => {
  const work = await mkdtemp(join(fixture.root, 'work-'))
  const stub = await stubs()
  const out = await emptyFolder()

  const result = await runArchive([out], {
    pinned: fixture.second,
    work,
    environment: stub.environment,
  })

  await assertFailed(result, out, 1)
  assert.deepEqual(await stub.clones(), [cloneArguments(work)])
  // The message is the last line, below the lines of git clone.
  assert.ok(
    result.stderr.endsWith(
      `\n${tagCommitMessage(join(work, 'src'), fixture.tagged, fixture.second)}\n`,
    ),
    result.stderr,
  )
  assert.equal(await exists(join(work, 'src')), false, 'the refused clone is still there')
})

test('55: a tagged tree without README.md fails and leaves the folder empty', async () => {
  const work = await mkdtemp(join(fixture.root, 'work-'))
  const files = Object.fromEntries(
    Object.entries(TAGGED_FILES).filter(([path]) => path !== 'README.md'),
  )
  const { tagged } = await makeFixture(fixture.environment, join(work, 'src'), files)
  const out = await emptyFolder()

  const result = await runArchive([out], { pinned: tagged, work })

  await assertFailed(result, out)
})

test('55: a missing folder fails', async () => {
  const out = join(await emptyFolder(), 'missing')

  const result = await runArchive([out], { pinned: fixture.tagged })

  assert.notEqual(result.status, 0)
  assert.deepEqual(await readdir(dirname(out)), [])
})

test('58: build-occt.sh without src clones the tag, finds the pinned commit and builds the clone', async () => {
  const work = await mkdtemp(join(fixture.root, 'work-'))
  const stub = await stubs()

  const result = await runBuild({ pinned: fixture.tagged, work, environment: stub.environment })
  const cmakeCalls = await stub.cmakeCalls()

  assert.equal(result.status, 0, result.stderr)
  assert.deepEqual(await stub.clones(), [cloneArguments(work)])
  assert.equal(cmakeCalls.length, 3)
  assert.deepEqual(cmakeCalls[0].slice(0, 4), [
    '-S',
    join(work, 'src'),
    '-B',
    join(work, 'build-native'),
  ])
  assert.ok(await exists(join(work, 'src', '.git')), 'the clone is gone')
})

test('58: build-occt.sh exits 1 before building when its clone names another commit, and removes the clone', async () => {
  const work = await mkdtemp(join(fixture.root, 'work-'))
  const stub = await stubs()

  const result = await runBuild({ pinned: fixture.second, work, environment: stub.environment })

  assert.equal(result.status, 1)
  assert.deepEqual(await stub.clones(), [cloneArguments(work)])
  // The message is the last line, below the lines of git clone.
  assert.ok(
    result.stderr.endsWith(
      `\n${tagCommitMessage(join(work, 'src'), fixture.tagged, fixture.second)}\n`,
    ),
    result.stderr,
  )
  assert.deepEqual(await stub.cmakeCalls(), [])
  assert.equal(await exists(join(work, 'src')), false, 'the refused clone is still there')
})

test('58: build-occt.sh builds an existing src as it is, without cloning or checking a commit', async () => {
  const work = await mkdtemp(join(fixture.root, 'work-'))
  await writeFiles(join(work, 'src'), TAGGED_FILES)
  const stub = await stubs()

  const result = await runBuild({ pinned: fixture.second, work, environment: stub.environment })
  const cmakeCalls = await stub.cmakeCalls()

  assert.equal(result.status, 0, result.stderr)
  assert.deepEqual(await stub.clones(), [])
  assert.equal(cmakeCalls.length, 3)
  assert.deepEqual(cmakeCalls[0].slice(0, 2), ['-S', join(work, 'src')])
})

/** A new folder whose src holds the files of test 55's first commit, as a plain folder (nothing is cloned). */
async function plainWork() {
  const work = await mkdtemp(join(fixture.root, 'work-'))
  await writeFiles(join(work, 'src'), TAGGED_FILES)
  return work
}

/**
 * Runs a copy of scripts/build-occt.sh for `target` in `work`, with the cmake, emcmake and uname stubs first on
 * `PATH` and `uname` printing `system`. Resolves to the result and the argument lists of the cmake and emcmake calls.
 */
async function runForSystem(target, system, work) {
  const folder = await mkdtemp(join(fixture.root, 'stub-'))
  const cmakeCalls = join(folder, 'cmake-calls')
  const emcmakeCalls = join(folder, 'emcmake-calls')
  await writeStubs(folder, { cmake: CMAKE_STUB, emcmake: EMCMAKE_STUB, uname: UNAME_STUB })
  await Promise.all([writeFile(cmakeCalls, ''), writeFile(emcmakeCalls, '')])
  const result = await runBuild({
    target,
    pinned: fixture.tagged,
    work,
    environment: {
      PATH: `${folder}:${process.env.PATH}`,
      CMAKE_CALLS: cmakeCalls,
      EMCMAKE_CALLS: emcmakeCalls,
      UNAME_NAME: system,
    },
  })
  return {
    result,
    cmakeCalls: await recordedCalls(cmakeCalls),
    emcmakeCalls: await recordedCalls(emcmakeCalls),
  }
}

/** The arguments of the first cmake call of a native run of `runForSystem`, which must have exited 0. */
async function configureArguments(system, work) {
  const { result, cmakeCalls } = await runForSystem('native', system, work)
  assert.equal(result.status, 0, `${system}: ${result.stderr}`)
  assert.ok(cmakeCalls.length > 0, `${system}: cmake was never called`)
  return cmakeCalls[0]
}

/** Asserts that `callArguments` holds none of the three Windows arguments, naming `label` when one is there. */
function assertNoWindowsArgument(callArguments, label) {
  for (const argument of WINDOWS_ARGUMENTS) {
    assert.ok(
      !callArguments.includes(argument),
      `${label}: ${argument} is in ${callArguments.join(' ')}`,
    )
  }
}

/**
 * The tables of a Cargo configuration's text, as a map from each table's name ('' for the keys above the first
 * table) to its entries, each key mapped to its value as written, with the lines of a value that runs over several
 * lines joined by spaces. Blank lines and comments, which sit on lines of their own, are left out.
 */
function configurationTables(text) {
  const tables = new Map([['', {}]])
  let entries = tables.get('')
  let key
  for (const line of text.split('\n')) {
    const trimmed = line.trim()
    if (trimmed === '' || trimmed.startsWith('#')) {
      continue
    }
    const header = /^\[([^\]]+)\]$/.exec(trimmed)
    const entry = /^([\w.-]+)\s*=\s*(.*)$/.exec(trimmed)
    if (header !== null) {
      entries = {}
      tables.set(header[1], entries)
      key = undefined
    } else if (entry !== null) {
      key = entry[1]
      entries[key] = entry[2]
    } else {
      assert.ok(key !== undefined, `the configuration line ${trimmed} belongs to no key`)
      entries[key] += ` ${trimmed}`
    }
  }
  return tables
}

/** The strings of a TOML array of basic strings written as `value`, a trailing comma allowed. */
function stringArray(value) {
  return JSON.parse(value.replace(/,\s*\]$/, ']'))
}

test('99: the configuration reader takes the keys above the first table, comments and arrays over several lines', () => {
  const tables = configurationTables(
    'jobs = 2\n# A comment\n[target.a]\nrustflags = [\n  "-C",\n  "x=1",\n]\n\nrunner = "node"\n[b]\nkey = "value"\n',
  )

  assert.deepEqual(
    [...tables],
    [
      ['', { jobs: '2' }],
      ['target.a', { rustflags: '[ "-C", "x=1", ]', runner: '"node"' }],
      ['b', { key: '"value"' }],
    ],
  )
  assert.deepEqual(stringArray(tables.get('target.a').rustflags), ['-C', 'x=1'])
})

for (const system of WINDOWS_SYSTEMS) {
  test(`99: with uname printing ${system}, build-occt.sh gives CMake the Linux arguments, then the three Windows arguments`, async () => {
    const work = await plainWork()

    const linux = await configureArguments('Linux', work)
    const windows = await configureArguments(system, work)

    assert.deepEqual(windows, [...linux, ...WINDOWS_ARGUMENTS])
  })
}

for (const system of OTHER_SYSTEMS) {
  test(`99: with uname printing ${system}, build-occt.sh gives CMake none of the Windows arguments`, async () => {
    const callArguments = await configureArguments(system, await plainWork())

    assertNoWindowsArgument(callArguments, system)
  })
}

test('99: for WebAssembly, build-occt.sh calls emcmake once, with none of the Windows arguments', async () => {
  const { result, emcmakeCalls } = await runForSystem('wasm', 'Linux', await plainWork())

  assert.equal(result.status, 0, result.stderr)
  assert.equal(emcmakeCalls.length, 1)
  assertNoWindowsArgument(emcmakeCalls[0], 'emcmake')
})

test('99: .cargo/config.toml gives x86_64-pc-windows-msvc the rustflags and rustdocflags of the static runtime and no other entry', async () => {
  const tables = configurationTables(await readFile(cargoConfigurationPath, 'utf8'))
  const windows = tables.get(WINDOWS_TARGET_TABLE)

  assert.ok(windows, `.cargo/config.toml has no table [${WINDOWS_TARGET_TABLE}]`)
  assert.deepEqual(Object.keys(windows), ['rustflags', 'rustdocflags'])
  assert.deepEqual(stringArray(windows.rustflags), STATIC_RUNTIME_FLAGS)
  assert.deepEqual(stringArray(windows.rustdocflags), STATIC_RUNTIME_FLAGS)
})

test('99: no other table of .cargo/config.toml holds crt-static', async () => {
  const tables = configurationTables(await readFile(cargoConfigurationPath, 'utf8'))

  for (const [name, entries] of tables) {
    if (name !== WINDOWS_TARGET_TABLE) {
      for (const [key, value] of Object.entries(entries)) {
        assert.ok(!value.includes('crt-static'), `[${name}] ${key} holds crt-static`)
      }
    }
  }
})
