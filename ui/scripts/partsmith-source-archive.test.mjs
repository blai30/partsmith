// Acceptance test 117 of docs/spec/delivery.md section 6 for scripts/partsmith-source-archive.sh, which writes
// Partsmith's own source archive into every deployment, every release and every source artifact (section 2.13,
// Partsmith's source archive).
import assert from 'node:assert/strict'
import { spawnSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import {
  chmod,
  copyFile,
  mkdir,
  mkdtemp,
  readFile,
  readdir,
  realpath,
  rm,
  symlink,
  writeFile,
} from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { dirname, join } from 'node:path'
import { after, before, test } from 'node:test'
import { fileURLToPath, pathToFileURL } from 'node:url'
import { gunzipSync } from 'node:zlib'

import { git, isolatedGitEnvironment } from './isolated-git.mjs'
import { writeFiles } from './occt-fixture.mjs'
import {
  archiveFolder,
  PARTSMITH_ARCHIVE,
  PARTSMITH_SCRIPT,
  writePartsmithArchive,
} from './partsmith-fixture.mjs'

const scriptPath = fileURLToPath(new URL(`../../${PARTSMITH_SCRIPT}`, import.meta.url))

const usageLine = `usage: ${PARTSMITH_SCRIPT} <directory>`

// The files of the fixture's first commit, each a short text with LF line endings. The second line of .gitattributes
// takes the eol attribute off src/main.rs, so core.autocrlf and core.eol decide that file's line endings.
const FIRST_FILES = {
  'README.md': '# Partsmith\n',
  'src/main.rs': 'fn main() {}\n',
  'docs/notes.md': '# Notes\n',
  '.gitattributes': '* text=auto eol=lf\nsrc/main.rs !eol\n',
}

// The text of src/main.rs in the second commit.
const MAIN_AFTER = 'fn main() {\n    println!("Partsmith");\n}\n'

// big.bin of the second commit: its tar is larger than a pipe's buffer (64 KiB on Linux).
const BIG_FILE = 'big.bin'
const BIG_FILE_SIZE = 1_048_576

// The output buffer of a plain git archive of the fixture, which holds big.bin.
const ARCHIVE_BUFFER_SIZE = 16 * BIG_FILE_SIZE

// The uncommitted change to README.md and the untracked file of the working tree, after the second commit.
const README_UNCOMMITTED = '# Partsmith, not committed\n'
const UNTRACKED_FILE = 'notes.txt'

// A stand-in for git that records, for each command, the value of GIT_ATTR_NOSYSTEM (or `unset`) and the command's
// arguments as one line, separated by NUL, in $GIT_CALLS, and hands every command to the real git at $REAL_GIT.
const GIT_STUB = `#!/usr/bin/env bash
printf '%s\\0' "\${GIT_ATTR_NOSYSTEM-unset}" "$@" >> "$GIT_CALLS"
printf '\\n' >> "$GIT_CALLS"
exec "$REAL_GIT" "$@"
`

// A stand-in for gzip that fails without reading its input.
const FAILING_GZIP = '#!/usr/bin/env bash\nexit 1\n'

// The commands the script runs, which the test of a PATH without sha256sum links into a folder of their own.
const SCRIPT_COMMANDS = ['bash', 'dirname', 'git', 'gzip', 'wc', 'tr', 'cut', 'mv', 'rm']

// A stand-in for macOS's shasum, run by this Node: it appends its arguments, each followed by NUL, and a line break to
// $SHASUM_CALLS, and prints the SHA-256 of its standard input as `shasum -a 256` does, followed by two spaces and `-`.
const SHASUM_STUB = `#!${process.execPath}
const { appendFileSync, readFileSync } = require('node:fs')
const { createHash } = require('node:crypto')
const callArguments = process.argv.slice(2)
appendFileSync(process.env.SHASUM_CALLS, callArguments.map((argument) => argument + '\\0').join('') + '\\n')
process.stdout.write(createHash('sha256').update(readFileSync(0)).digest('hex') + '  -\\n')
`

// The git that the stub hands its commands to.
const realGit = spawnSync('bash', ['-c', 'command -v git'], { encoding: 'utf8' }).stdout.trim()

/** The path of each command of `names` on this machine's `PATH`. */
function commandPaths(names) {
  return names.map((name) => {
    const found = spawnSync('bash', ['-c', 'command -v "$1"', 'bash', name], { encoding: 'utf8' })
    assert.equal(found.status, 0, `${name} is not on PATH`)
    return found.stdout.trim()
  })
}

/** The not-a-clone message of section 4 for the folder `root`. */
function notAClone(root) {
  return `${root} is not the top folder of a Git clone. Run ${PARTSMITH_SCRIPT} in a clone of the Partsmith repository, whose checked-out commit it archives (docs/spec/delivery.md section 2.13).`
}

/** The SHA-256 of `bytes` in hexadecimal, as sha256sum prints it. */
function sha256Of(bytes) {
  return createHash('sha256').update(bytes).digest('hex')
}

// The fixture every test shares: its temporary root, the isolated Git environment, the repository, its two commits,
// the folder runs start in, and the result and bytes of a first run into an empty folder.
let fixture

before(async () => {
  fixture = { root: await mkdtemp(join(tmpdir(), 'partsmith-source-archive-')) }
  fixture.environment = await isolatedGitEnvironment(fixture.root)
  fixture.repository = join(fixture.root, 'repository')
  fixture.elsewhere = join(fixture.root, 'elsewhere')
  await mkdir(fixture.elsewhere)
  Object.assign(fixture, await makeRepository(fixture.environment, fixture.repository))
  const out = await emptyFolder()
  const result = runScript(join(fixture.repository, PARTSMITH_SCRIPT), [out])
  fixture.first = {
    out,
    result,
    bytes: await readFile(join(out, PARTSMITH_ARCHIVE)).catch(() => null),
  }
})

after(() => fixture && rm(fixture.root, { recursive: true, force: true }))

/**
 * Makes the fixture repository at `repository`: a first commit of FIRST_FILES and a copy of the script at its path,
 * then a second commit that changes src/main.rs and adds big.bin, then an uncommitted change to README.md and an
 * untracked file. Resolves to both commits as `{ first, commit }`, `commit` being the second, the one checked out.
 */
async function makeRepository(environment, repository) {
  await mkdir(join(repository, dirname(PARTSMITH_SCRIPT)), { recursive: true })
  git(environment, repository, 'init', '--initial-branch=main')
  await writeFiles(repository, FIRST_FILES)
  await copyFile(scriptPath, join(repository, PARTSMITH_SCRIPT))
  await chmod(join(repository, PARTSMITH_SCRIPT), 0o755)
  git(environment, repository, 'add', '--all')
  git(environment, repository, 'commit', '--message', 'First')
  const first = git(environment, repository, 'rev-parse', 'HEAD')
  await writeFiles(repository, {
    'src/main.rs': MAIN_AFTER,
    [BIG_FILE]: Buffer.alloc(BIG_FILE_SIZE),
  })
  git(environment, repository, 'add', '--all')
  git(environment, repository, 'commit', '--message', 'Second')
  const commit = git(environment, repository, 'rev-parse', 'HEAD')
  await writeFiles(repository, {
    'README.md': README_UNCOMMITTED,
    [UNTRACKED_FILE]: 'not tracked\n',
  })
  return { first, commit }
}

/** A new empty folder in the fixture's root. */
function emptyFolder() {
  return mkdtemp(join(fixture.root, 'out-'))
}

/**
 * Runs `bash <script> <scriptArguments>` from the folder `cwd`, by default the fixture's folder `elsewhere`, in the
 * fixture's isolated Git environment plus `environment`.
 */
function runScript(script, scriptArguments, environment = {}, cwd = fixture.elsewhere) {
  return spawnSync('bash', [script, ...scriptArguments], {
    cwd,
    env: { ...fixture.environment, ...environment },
    encoding: 'utf8',
  })
}

/**
 * The bytes of the archive that a run of `script` from `cwd`, which must pass, writes into a new empty folder, as
 * `runScript` runs it.
 */
async function archiveBytes(
  script = join(fixture.repository, PARTSMITH_SCRIPT),
  environment = {},
  cwd = undefined,
) {
  const out = await emptyFolder()
  const result = runScript(script, [out], environment, cwd)
  assert.equal(result.status, 0, result.stderr)
  assert.deepEqual(await readdir(out), [PARTSMITH_ARCHIVE])
  return readFile(join(out, PARTSMITH_ARCHIVE))
}

/** Asserts that a run failed with `status` (any status but 0 when it is not given) and left `out` without a file. */
async function assertFailed(result, out, status) {
  if (status === undefined) {
    assert.notEqual(result.status, 0)
  } else {
    assert.equal(result.status, status, result.stderr)
  }
  assert.deepEqual(await readdir(out), [])
}

/** Writes each stub of `stubTexts`, a map from a command's name to its script, into a new folder; resolves to it. */
async function stubFolder(stubTexts) {
  const folder = await mkdtemp(join(fixture.root, 'stubs-'))
  await Promise.all(
    Object.entries(stubTexts).map(async ([name, text]) => {
      await writeFile(join(folder, name), text)
      await chmod(join(folder, name), 0o755)
    }),
  )
  return folder
}

/** The argument lists a stub recorded in `path`: one per line, each argument followed by NUL. */
async function recordedCalls(path) {
  return (await readFile(path, 'utf8'))
    .split('\n')
    .filter((call) => call !== '')
    .map((call) => call.split('\0').slice(0, -1))
}

/**
 * The entries of the tar `tar`, in order, as `{ name, type, mtime, data }`, with a pax `path` record applied to the
 * entry after it and the pax headers themselves left out.
 */
function tarEntries(tar) {
  const entries = []
  let paxPath
  let offset = 0
  while (offset + 512 <= tar.length) {
    const header = tar.subarray(offset, offset + 512)
    if (header.every((byte) => byte === 0)) {
      break
    }
    const field = (start, length) =>
      header
        .subarray(start, start + length)
        .toString('latin1')
        .replace(/\0[\s\S]*$/, '')
    const size = Number.parseInt(field(124, 12).trim() || '0', 8)
    const type = field(156, 1)
    const data = tar.subarray(offset + 512, offset + 512 + size)
    offset += 512 + Math.ceil(size / 512) * 512
    if (type === 'x' || type === 'g') {
      if (type === 'x') {
        paxPath = /(?:^|\n)\d+ path=([^\n]*)\n/.exec(data.toString('utf8'))?.[1]
      }
      continue
    }
    const prefix = field(345, 155)
    const name = paxPath ?? (prefix === '' ? field(0, 100) : `${prefix}/${field(0, 100)}`)
    paxPath = undefined
    entries.push({ name, type, mtime: Number.parseInt(field(136, 12).trim(), 8), data })
  }
  return entries
}

test('117: the script writes the archive of the checked-out commit into an empty folder and prints its size and SHA-256', async () => {
  const { out, result, bytes } = fixture.first

  assert.equal(result.status, 0, result.stderr)
  assert.ok(bytes, `${out} holds no ${PARTSMITH_ARCHIVE}`)
  assert.equal(
    result.stdout,
    `Wrote ${out}/${PARTSMITH_ARCHIVE} from commit ${fixture.commit}: ${bytes.length} bytes, SHA-256 ${sha256Of(bytes)}.\n`,
  )
  assert.deepEqual(await readdir(out), [PARTSMITH_ARCHIVE])
})

test('117: the gzip header holds no file name and no time, and the tar names the commit', () => {
  const { bytes } = fixture.first
  const commitId = spawnSync('git', ['get-tar-commit-id'], {
    input: gunzipSync(bytes),
    env: fixture.environment,
    encoding: 'utf8',
  })

  // Byte 3 holds the flags, FNAME among them; bytes 4 to 7 hold the modification time.
  assert.equal(bytes[3], 0)
  assert.deepEqual([...bytes.subarray(4, 8)], [0, 0, 0, 0])
  assert.equal(commitId.stdout, `${fixture.commit}\n`)
})

test("117: the archive holds every file and folder of the commit, at the commit's bytes and time, and nothing else", () => {
  const folder = archiveFolder(fixture.commit)
  const entries = tarEntries(gunzipSync(fixture.first.bytes))
  const tracked = git(
    fixture.environment,
    fixture.repository,
    'ls-tree',
    '-r',
    '-t',
    '-z',
    '--format=%(objecttype) %(path)',
    fixture.commit,
  )
    .split('\0')
    .filter((line) => line !== '')
    .map((line) => {
      const [type, path] = [line.slice(0, line.indexOf(' ')), line.slice(line.indexOf(' ') + 1)]
      return type === 'tree' ? `${folder}${path}/` : `${folder}${path}`
    })
  const committerTime = Number(
    git(fixture.environment, fixture.repository, 'log', '-1', '--format=%ct', fixture.commit),
  )
  const data = (path) => entries.find((entry) => entry.name === `${folder}${path}`)?.data

  assert.deepEqual(entries.map((entry) => entry.name).toSorted(), [folder, ...tracked].toSorted())
  for (const entry of entries) {
    assert.equal(entry.mtime, committerTime, entry.name)
  }
  assert.ok(
    !entries.some((entry) => entry.name.endsWith(UNTRACKED_FILE)),
    `${UNTRACKED_FILE} is in the archive`,
  )
  // The committed texts, not the working tree's change.
  assert.equal(data('README.md')?.toString('utf8'), FIRST_FILES['README.md'])
  assert.equal(data('src/main.rs')?.toString('utf8'), MAIN_AFTER)
  assert.equal(data('docs/notes.md')?.toString('utf8'), FIRST_FILES['docs/notes.md'])
  assert.deepEqual(data(BIG_FILE), Buffer.alloc(BIG_FILE_SIZE))
})

test('117: the archive is the bytes of git archive with the options of section 2.13, piped to gzip -9 -n', async () => {
  const expected = join(await emptyFolder(), 'expected.tar.gz')
  writePartsmithArchive(fixture.environment, fixture.repository, fixture.commit, expected)

  assert.deepEqual(fixture.first.bytes, await readFile(expected))
})

test('117: a second run into another empty folder writes the same bytes', async () => {
  assert.deepEqual(await archiveBytes(), fixture.first.bytes)
})

// The Git settings, each as `[key, value]`, that would change a plain git archive of the commit: the line endings of
// src/main.rs, by core.autocrlf and by core.eol, the file modes, and a global attributes file that leaves out docs/.
async function changingSettings() {
  const attributes = join(fixture.root, 'global-attributes')
  await writeFile(attributes, 'docs/* export-ignore\n')
  return [
    ['core.autocrlf', 'true'],
    ['core.eol', 'crlf'],
    ['tar.umask', '0077'],
    ['core.attributesFile', attributes],
  ]
}

/** A new isolated Git environment whose global configuration holds `settings`, a list of `[key, value]`. */
async function environmentWith(settings) {
  const folder = await mkdtemp(join(fixture.root, 'configuration-'))
  const environment = await isolatedGitEnvironment(folder)
  for (const [key, value] of settings) {
    git(environment, folder, 'config', '--global', key, value)
  }
  return environment
}

test('117: each Git setting that changes a plain git archive leaves the bytes the same', async () => {
  const settings = await changingSettings()
  const plainArchive = (environment) =>
    spawnSync('git', ['archive', '--format=tar', fixture.commit], {
      cwd: fixture.repository,
      env: environment,
      // The tar holds big.bin and is larger than spawnSync's default buffer of 1 MiB.
      maxBuffer: ARCHIVE_BUFFER_SIZE,
    }).stdout
  const plain = plainArchive(fixture.environment)
  const environments = await Promise.all(settings.map((setting) => environmentWith([setting])))

  // The fixture must exercise each setting: without the script's options, each one changes the archive.
  for (const [i, environment] of environments.entries()) {
    assert.notDeepEqual(plainArchive(environment), plain, settings[i][0])
  }
  const archives = await Promise.all(
    environments.map((environment) =>
      archiveBytes(undefined, {
        GIT_CONFIG_GLOBAL: environment.GIT_CONFIG_GLOBAL,
        XDG_CONFIG_HOME: environment.XDG_CONFIG_HOME,
      }),
    ),
  )
  for (const [i, bytes] of archives.entries()) {
    assert.deepEqual(bytes, fixture.first.bytes, settings[i][0])
  }
})

test('117: a run in a shallow clone of the repository writes the same bytes', async () => {
  const clone = join(await mkdtemp(join(fixture.root, 'shallow-')), 'clone')
  git(
    fixture.environment,
    fixture.root,
    'clone',
    '--quiet',
    '--depth',
    '1',
    pathToFileURL(fixture.repository).href,
    clone,
  )

  assert.deepEqual(await archiveBytes(join(clone, PARTSMITH_SCRIPT)), fixture.first.bytes)
})

test('117: a run by a relative path with CDPATH exported writes the same bytes', async () => {
  // A folder of CDPATH with a scripts/ of its own, where cd would go and which it would print.
  const decoy = await mkdtemp(join(fixture.root, 'decoy-'))
  await mkdir(join(decoy, dirname(PARTSMITH_SCRIPT)))

  const bytes = await archiveBytes(PARTSMITH_SCRIPT, { CDPATH: decoy }, fixture.repository)

  assert.deepEqual(bytes, fixture.first.bytes)
})

test('117: without sha256sum the script prints the SHA-256 that shasum -a 256 gives', async () => {
  const commands = await mkdtemp(join(fixture.root, 'commands-'))
  const calls = join(fixture.root, 'shasum-calls')
  await writeFile(calls, '')
  await Promise.all(
    commandPaths(SCRIPT_COMMANDS).map((path, i) =>
      symlink(path, join(commands, SCRIPT_COMMANDS[i])),
    ),
  )
  await writeFile(join(commands, 'shasum'), SHASUM_STUB)
  await chmod(join(commands, 'shasum'), 0o755)
  const out = await emptyFolder()

  const result = runScript(join(fixture.repository, PARTSMITH_SCRIPT), [out], {
    PATH: commands,
    SHASUM_CALLS: calls,
  })

  assert.equal(result.status, 0, result.stderr)
  assert.deepEqual(await readFile(join(out, PARTSMITH_ARCHIVE)), fixture.first.bytes)
  assert.equal(result.stdout, fixture.first.result.stdout.replace(fixture.first.out, out))
  assert.deepEqual(await recordedCalls(calls), [['-a', '256']])
})

test('117: git archive runs with GIT_ATTR_NOSYSTEM=1, so no system attributes file reaches the bytes', async () => {
  const calls = join(fixture.root, 'git-calls')
  await writeFile(calls, '')
  const stubs = await stubFolder({ git: GIT_STUB })

  const bytes = await archiveBytes(undefined, {
    PATH: `${stubs}:${process.env.PATH}`,
    GIT_CALLS: calls,
    REAL_GIT: realGit,
  })
  const archiveCalls = (await recordedCalls(calls)).filter((call) => call.includes('archive'))

  assert.deepEqual(bytes, fixture.first.bytes)
  assert.ok(archiveCalls.length > 0, 'the script ran no git archive through the stub')
  for (const [nosystem, ...gitArguments] of archiveCalls) {
    assert.equal(
      nosystem,
      '1',
      `git ${gitArguments.join(' ')} ran with GIT_ATTR_NOSYSTEM ${nosystem}`,
    )
  }
})

test('117: an archive already in the folder is replaced by the new one', async () => {
  const out = await emptyFolder()
  await writeFile(join(out, PARTSMITH_ARCHIVE), 'an older archive\n')

  const result = runScript(join(fixture.repository, PARTSMITH_SCRIPT), [out])

  assert.equal(result.status, 0, result.stderr)
  assert.deepEqual(await readdir(out), [PARTSMITH_ARCHIVE])
  assert.deepEqual(await readFile(join(out, PARTSMITH_ARCHIVE)), fixture.first.bytes)
})

/** Asserts that a run with `scriptArguments` exits 2 with the usage line on standard error and leaves `out` empty. */
async function assertUsage(out, scriptArguments) {
  const result = runScript(join(fixture.repository, PARTSMITH_SCRIPT), scriptArguments)

  await assertFailed(result, out, 2)
  assert.equal(result.stderr, `${usageLine}\n`)
}

test('117: no argument exits 2 with the usage line', async () => {
  await assertUsage(await emptyFolder(), [])
})

test('117: two arguments exit 2 with the usage line', async () => {
  const out = await emptyFolder()
  await assertUsage(out, [out, 'extra'])
})

test('117: a missing folder fails, and the script does not make it', async () => {
  const parent = await emptyFolder()
  const out = join(parent, 'missing')

  const result = runScript(join(fixture.repository, PARTSMITH_SCRIPT), [out])

  assert.notEqual(result.status, 0)
  assert.deepEqual(await readdir(parent), [])
})

test('117: a failing gzip fails the script and leaves the folder without a file', async () => {
  const out = await emptyFolder()
  const stubs = await stubFolder({ gzip: FAILING_GZIP })

  const result = runScript(join(fixture.repository, PARTSMITH_SCRIPT), [out], {
    PATH: `${stubs}:${process.env.PATH}`,
  })

  await assertFailed(result, out)
})

/** Asserts that the script at `script` exits 1 with only the not-a-clone message for the real path of `root`. */
async function assertNotAClone(script, root) {
  const out = await emptyFolder()

  const result = runScript(script, [out])

  await assertFailed(result, out, 1)
  assert.equal(result.stderr, `${notAClone(await realpath(root))}\n`)
}

test('117: a copy of the script in a tree without .git exits 1 with the not-a-clone message', async () => {
  const plain = join(await mkdtemp(join(fixture.root, 'plain-')), 'tree')
  await mkdir(plain)
  // The files of the commit, its copy of the script among them, as an extracted archive holds them.
  const extracted = spawnSync(
    'bash',
    [
      '-c',
      'set -o pipefail; git -C "$1" archive "$2" | tar -x -C "$3"',
      'bash',
      fixture.repository,
      fixture.commit,
      plain,
    ],
    { env: fixture.environment, encoding: 'utf8' },
  )
  assert.equal(extracted.status, 0, extracted.stderr)

  await assertNotAClone(join(plain, PARTSMITH_SCRIPT), plain)
})

test('117: a copy of the script in a folder inside another clone exits 1 with the not-a-clone message', async (context) => {
  const vendored = join(fixture.repository, 'vendor', 'partsmith')
  context.after(() => rm(join(fixture.repository, 'vendor'), { recursive: true, force: true }))
  await mkdir(join(vendored, dirname(PARTSMITH_SCRIPT)), { recursive: true })
  await copyFile(scriptPath, join(vendored, PARTSMITH_SCRIPT))

  await assertNotAClone(join(vendored, PARTSMITH_SCRIPT), vendored)
})
