// Acceptance tests 55 and 58 of docs/spec/delivery.md section 6 for scripts/occt-source-archive.sh and the commit
// check of scripts/build-occt.sh (section 2.13).
import assert from 'node:assert/strict'
import { spawnSync } from 'node:child_process'
import {
  chmod,
  copyFile,
  mkdir,
  mkdtemp,
  readFile,
  readdir,
  rm,
  stat,
  writeFile,
} from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { basename, dirname, join } from 'node:path'
import { after, before, test } from 'node:test'
import { fileURLToPath, pathToFileURL } from 'node:url'
import { gunzipSync } from 'node:zlib'

import { git, isolatedGitEnvironment } from './isolated-git.mjs'

const archiveScriptPath = fileURLToPath(
  new URL('../../scripts/occt-source-archive.sh', import.meta.url),
)
const buildScriptPath = fileURLToPath(new URL('../../scripts/build-occt.sh', import.meta.url))
const versionPath = fileURLToPath(new URL('../../scripts/occt-version.sh', import.meta.url))

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

// The second commit's change, and the uncommitted change of the working tree.
const STANDARD_AFTER_TAG = '// Standard, after the tag\n'
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

// The git that the stub hands its commands to.
const realGit = spawnSync('bash', ['-c', 'command -v git'], { encoding: 'utf8' }).stdout.trim()

const versionText = await readFile(versionPath, 'utf8')

/** The value that scripts/occt-version.sh gives `name`. */
function pin(name) {
  const match = new RegExp(`^${name}=(\\S+)$`, 'm').exec(versionText)
  assert.ok(match, `scripts/occt-version.sh sets no ${name}`)
  return match[1]
}

const version = pin('OCCT_VERSION')
const tag = pin('OCCT_TAG')
const archiveName = `occt-${version}-source.tar.gz`

/** The no-tag message of section 4. */
function noTagMessage(src) {
  return `${src} holds no tag ${tag}. Point OCCT_WORK_DIR at a folder whose src is a clone that scripts/build-occt.sh made, or at one without src, where the script clones the tag (docs/spec/delivery.md section 2.13).`
}

/** The tag-commit message of section 4. */
function tagCommitMessage(src, commit, pinned) {
  return `The tag ${tag} in ${src} names the commit ${commit}, not ${pinned}, which scripts/occt-version.sh pins. Upstream may have moved the tag; nothing was built or archived from it (docs/spec/delivery.md section 2.13).`
}

/** Writes `files`, a map from paths relative to `directory` to texts, creating their folders. */
async function writeFiles(directory, files) {
  await Promise.all(
    Object.entries(files).map(async ([path, text]) => {
      await mkdir(dirname(join(directory, path)), { recursive: true })
      await writeFile(join(directory, path), text)
    }),
  )
}

/**
 * Makes a fixture repository at `directory`: a commit of `files` tagged `{tag}` (annotated), a second commit that
 * changes `src/Standard/Standard.cxx`, and an uncommitted change to `CMakeLists.txt`. Returns both commits.
 */
async function makeFixture(environment, directory, files) {
  await mkdir(directory, { recursive: true })
  git(environment, directory, 'init', '--initial-branch=main')
  await writeFiles(directory, files)
  git(environment, directory, 'add', '--all')
  git(environment, directory, 'commit', '--message', 'OCCT')
  git(environment, directory, 'tag', '--annotate', tag, '--message', `OCCT ${version}`)
  const tagged = git(environment, directory, 'rev-parse', 'HEAD')
  await writeFile(join(directory, 'src/Standard/Standard.cxx'), STANDARD_AFTER_TAG)
  git(environment, directory, 'commit', '--all', '--message', 'After the tag')
  const second = git(environment, directory, 'rev-parse', 'HEAD')
  await writeFile(join(directory, 'CMakeLists.txt'), CMAKE_UNCOMMITTED)
  return { tagged, second }
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
    `OCCT_VERSION=${version}\nOCCT_TAG=${tag}\nOCCT_COMMIT=${pinned}\n`,
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

/** Runs a copy of scripts/build-occt.sh with `native <work>/prefix` and `OCCT_JOBS=1`, as `runScript` does. */
function runBuild({ work, environment, ...options }) {
  return runScript(buildScriptPath, ['native', join(work, 'prefix')], {
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
  const stubTexts = { git: GIT_STUB, cmake: CMAKE_STUB }
  await Promise.all(
    Object.entries(stubTexts).map(async ([name, text]) => {
      await writeFile(join(folder, name), text)
      await chmod(join(folder, name), 0o755)
    }),
  )
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
  assert.match(pin('OCCT_COMMIT'), /^[0-9a-f]{40}$/)
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
