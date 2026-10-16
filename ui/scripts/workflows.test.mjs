// Acceptance tests 49 to 54 and 57 of docs/spec/delivery.md section 6 for the workflow files.
import assert from 'node:assert/strict'
import { spawnSync } from 'node:child_process'
import { chmod, lstat, mkdir, mkdtemp, readFile, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { test } from 'node:test'
import { fileURLToPath, pathToFileURL } from 'node:url'

import { git, isolatedGitEnvironment } from './isolated-git.mjs'

const repositoryRoot = fileURLToPath(new URL('../..', import.meta.url))
const workflowsDirectory = join(repositoryRoot, '.github/workflows')

// The source stamp at the root of every web build, which tests 52, 53 and 57 name (section 2.13).
const SOURCE_STAMP = 'SOURCE.txt'

// The script that writes the OCCT source archive of a release, which tests 54 and 57 name (section 2.13).
const ARCHIVE_SCRIPT = 'scripts/occt-source-archive.sh'

// The repository and a pull request as GitHub gives them to the source stamp's writer (section 2.13).
const REPOSITORY_URL = 'https://github.com/blai30/partsmith'
const PULL_REQUEST_URL = 'https://github.com/blai30/partsmith/pull/7'

// Two commits, 40 hexadecimal digits as GitHub writes them: the head of main that a night tests, and an older one.
const HEAD_COMMIT = 'a'.repeat(40)
const OTHER_COMMIT = 'b'.repeat(40)

// The arguments of the skip check's gh run list (section 2.11), one per element.
const SKIP_CHECK_ARGUMENTS = [
  'run',
  'list',
  '--workflow',
  'nightly.yml',
  '--branch',
  'main',
  '--status',
  'completed',
  '--limit',
  '1',
  '--json',
  'headSha',
  '--jq',
  '.[0].headSha // ""',
]

// The timeout-minutes of every job of each workflow file, in the order of its jobs (sections 2.4, 2.5 and 2.11).
const EXPECTED_TIMEOUTS = {
  'ci.yml': {
    'occt-native': 150,
    'occt-wasm': 150,
    'kernel-wasm': 60,
    rust: 60,
    deny: 60,
    ui: 60,
    schema: 60,
    'core-wasm': 60,
    web: 60,
    e2e: 60,
    python: 60,
    desktop: 60,
    deploy: 60,
  },
  'release.yml': { verify: 60, bundle: 150, draft: 60 },
  'nightly.yml': { properties: 20 },
}

// A stand-in for gh that appends its arguments, separated by NUL, as one line to $STUB_CALLS, then prints
// $STUB_ANSWER or, when $STUB_FAILS is 1, fails as an API error does.
const GH_STUB = `#!/usr/bin/env bash
printf '%s\\0' "$@" >> "$STUB_CALLS"
printf '\\n' >> "$STUB_CALLS"
if [ "$STUB_FAILS" = 1 ]; then
  echo 'HTTP 502: Bad Gateway' >&2
  exit 1
fi
printf '%s\\n' "$STUB_ANSWER"
`

/** Creates a temporary directory that is removed when the test ends. */
async function temporaryDirectory(context) {
  const directory = await mkdtemp(join(tmpdir(), 'partsmith-workflows-'))
  context.after(() => rm(directory, { recursive: true, force: true }))
  return directory
}

/** The text of the workflow file `file`. */
function workflowText(file) {
  return readFile(join(workflowsDirectory, file), 'utf8')
}

/**
 * The jobs of a workflow's text in order, as `{ id, lines }`, read by the two-space indentation of the workflow
 * files: a job is a line `  <id>:` under `jobs:`, and its lines are the lines below it up to the next job.
 */
function workflowJobs(text) {
  const jobs = []
  let inJobs = false
  for (const line of text.split('\n')) {
    if (/^[^\s#]/.test(line)) {
      inJobs = line === 'jobs:'
      continue
    }
    const job = inJobs ? /^ {2}([\w-]+):$/.exec(line) : null
    if (job !== null) {
      jobs.push({ id: job[1], lines: [] })
    } else if (inJobs && jobs.length > 0) {
      jobs.at(-1).lines.push(line)
    }
  }
  return jobs
}

/** The job `id` of the workflow file `file`; fails the test when the file has no such job. */
async function workflowJob(file, id) {
  const job = workflowJobs(await workflowText(file)).find((listed) => listed.id === id)
  assert.ok(job, `${file} has no job ${id}`)
  return job
}

/** The `timeout-minutes` of a job as a number, or undefined when it sets none. */
function timeoutMinutes(job) {
  const prefix = '    timeout-minutes: '
  const line = job.lines.find((jobLine) => jobLine.startsWith(prefix))
  return line === undefined ? undefined : Number(line.slice(prefix.length))
}

/**
 * The steps of a job, each as its non-blank lines with the step's indentation removed, so that its keys start
 * the lines. A step starts at a line `      - `; comments between steps belong to none.
 */
function jobSteps(job) {
  const steps = []
  for (const line of job.lines.slice(job.lines.indexOf('    steps:') + 1)) {
    if (line.startsWith('      - ')) {
      steps.push([line.slice(8)])
    } else if (line.startsWith('        ') && steps.length > 0) {
      steps.at(-1).push(line.slice(8))
    }
  }
  return steps
}

/**
 * The entries of the mapping `key` of a step, such as its `env` or `with`, as an object of their values as written
 * (`|` for a block scalar); undefined when the step has no such key.
 */
function stepMapping(step, key) {
  const index = step.indexOf(`${key}:`)
  if (index === -1) {
    return undefined
  }
  const entries = {}
  for (const line of step.slice(index + 1)) {
    // The lines of a block scalar belong to the entry above them.
    if (line.startsWith('    ')) {
      continue
    }
    const entry = /^ {2}([\w-]+): (.*)$/.exec(line)
    if (entry === null) {
      break
    }
    entries[entry[1]] = entry[2]
  }
  return entries
}

/** The index of the first step of `steps` for which `matches` is true; fails the test, naming `what`, when none is. */
function stepIndex(steps, what, matches) {
  const index = steps.findIndex(matches)
  assert.notEqual(index, -1, `no step ${what}`)
  return index
}

/** The index of the step of `steps` named `name`; fails the test when there is none. */
function namedStep(steps, name) {
  return stepIndex(steps, `named ${name}`, (step) => stepValue(step, 'name') === name)
}

/** The index of the step of `steps` whose `run` is `command`; fails the test when there is none. */
function commandStep(steps, command) {
  return stepIndex(steps, `that runs ${command}`, (step) => stepValue(step, 'run') === command)
}

/** The value of `key` in a step: the rest of its line, or for a block scalar (`|`) the lines below it. */
function stepValue(step, key) {
  const index = step.findIndex((line) => line.startsWith(`${key}:`))
  if (index === -1) {
    return undefined
  }
  const value = step[index].slice(key.length + 1).trim()
  if (value !== '|') {
    return value
  }
  const block = []
  for (const line of step.slice(index + 1)) {
    if (!line.startsWith('  ')) {
      break
    }
    block.push(line.slice(2))
  }
  return `${block.join('\n')}\n`
}

/**
 * Runs the skip check of nightly.yml as GitHub runs a step without a `shell` (`bash -e`), with the gh stub first on
 * `PATH` answering `answer`, or failing when `answer` is null. Resolves to its exit status, standard output, the
 * text it wrote to `$GITHUB_OUTPUT` and the argument lists gh received.
 */
async function runSkipCheck(context, answer) {
  const properties = await workflowJob('nightly.yml', 'properties')
  const step = jobSteps(properties).find((listed) => stepValue(listed, 'id') === 'skip')
  assert.ok(step, 'nightly.yml properties has no step with id skip')
  const directory = await temporaryDirectory(context)
  const stubDirectory = join(directory, 'stub')
  const stubPath = join(stubDirectory, 'gh')
  const scriptPath = join(directory, 'skip.sh')
  const outputPath = join(directory, 'github-output')
  const callsPath = join(directory, 'calls')
  await mkdir(stubDirectory)
  await writeFile(stubPath, GH_STUB)
  await chmod(stubPath, 0o755)
  await writeFile(scriptPath, stepValue(step, 'run'))
  await writeFile(outputPath, '')
  await writeFile(callsPath, '')
  const result = spawnSync('bash', ['-e', scriptPath], {
    encoding: 'utf8',
    env: {
      ...process.env,
      PATH: `${stubDirectory}:${process.env.PATH}`,
      GITHUB_SHA: HEAD_COMMIT,
      GITHUB_OUTPUT: outputPath,
      STUB_CALLS: callsPath,
      STUB_ANSWER: answer ?? '',
      STUB_FAILS: answer === null ? '1' : '0',
    },
  })
  const calls = (await readFile(callsPath, 'utf8'))
    .split('\n')
    .filter((call) => call !== '')
    .map((call) => call.split('\0').slice(0, -1))
  return {
    status: result.status,
    stdout: result.stdout,
    output: await readFile(outputPath, 'utf8'),
    calls,
  }
}

test('49: every job sets the timeout-minutes of sections 2.4, 2.5 and 2.11', async () => {
  const files = Object.keys(EXPECTED_TIMEOUTS)
  const texts = await Promise.all(files.map(workflowText))
  files.forEach((file, i) => {
    const actual = workflowJobs(texts[i]).map((job) => [job.id, timeoutMinutes(job)])
    assert.deepEqual(actual, Object.entries(EXPECTED_TIMEOUTS[file]), file)
  })
})

test('50: the schema job installs the packages first and checks the notices only when they are installed', async () => {
  const steps = jobSteps(await workflowJob('ci.yml', 'schema'))
  const commands = steps.map((step) => stepValue(step, 'run'))
  const install = steps.findIndex((step) => stepValue(step, 'id') === 'install')
  const schemaWrite = commands.indexOf('cargo run -p partsmith-cli -- schema --write')
  assert.notEqual(install, -1, 'ci.yml schema has no step with id install')
  assert.notEqual(schemaWrite, -1, 'ci.yml schema does not run schema --write')
  assert.equal(commands[install], 'pnpm -C ui install --frozen-lockfile')
  assert.ok(install < schemaWrite, 'the install comes after schema --write')
  assert.equal(stepValue(steps.at(-1), 'run'), 'pnpm -C ui check:notices')
  assert.equal(
    stepValue(steps.at(-1), 'if'),
    "${{ !cancelled() && steps.install.outcome == 'success' }}",
  )
})

test('51: the nightly skip check skips when the newest finished run tested the head of main', async (context) => {
  const result = await runSkipCheck(context, HEAD_COMMIT)

  assert.equal(result.status, 0)
  assert.deepEqual(result.calls, [SKIP_CHECK_ARGUMENTS])
  assert.equal(result.output, 'skip=true\n')
  assert.equal(
    result.stdout,
    `::notice title=Nightly skipped::The last nightly run already tested ${HEAD_COMMIT}, the head of main, so the property tests did not run.\n`,
  )
})

/** Asserts that the skip check ran gh once, as section 2.11 says, and decided to run the tests without a message. */
function assertTests(result) {
  assert.equal(result.status, 0)
  assert.deepEqual(result.calls, [SKIP_CHECK_ARGUMENTS])
  assert.equal(result.output, 'skip=false\n')
  assert.equal(result.stdout, '')
}

test('51: the nightly skip check tests when the newest finished run tested another commit', async (context) => {
  assertTests(await runSkipCheck(context, OTHER_COMMIT))
})

test('51: the nightly skip check tests when no nightly run has finished', async (context) => {
  assertTests(await runSkipCheck(context, ''))
})

test('51: the nightly skip check tests with a warning when gh fails', async (context) => {
  const result = await runSkipCheck(context, null)

  assert.equal(result.status, 0)
  assert.deepEqual(result.calls, [SKIP_CHECK_ARGUMENTS])
  assert.equal(result.output, 'skip=false\n')
  assert.equal(
    result.stdout,
    `::warning title=Nightly skip check failed::gh run list failed, so the property tests run without checking whether an earlier night tested ${HEAD_COMMIT}.\n`,
  )
})

/** The four lines of the source stamp of section 2.13 for `commit`, each with its LF. */
function stampText(commit) {
  return [
    'Source of this deployment of the Partsmith web app',
    `Repository: ${REPOSITORY_URL}`,
    `Commit: ${commit}`,
    `Source archive: ${REPOSITORY_URL}/archive/${commit}.tar.gz`,
  ]
    .map((line) => `${line}\n`)
    .join('')
}

/** The three lines a pull request's build adds to the source stamp (section 2.13), each with its LF. */
function pullRequestText(parents, tree) {
  return `Pull request: ${PULL_REQUEST_URL}\nParents: ${parents.join(' ')}\nTree: ${tree}\n`
}

/** The source-stamp error of section 4 for `commit`. */
function stampErrorMessage(commit) {
  return `::error title=Source stamp missing::ui/dist/${SOURCE_STAMP} does not name the commit ${commit}, so nothing was deployed. The web job writes it (docs/spec/delivery.md section 2.13).`
}

/** Runs a step's `run` script with `bash -e` in `directory` and `environment`, as GitHub runs a step without a `shell`. */
async function runStep(context, step, directory, environment) {
  const scriptPath = join(await temporaryDirectory(context), 'step.sh')
  await writeFile(scriptPath, stepValue(step, 'run'))
  return spawnSync('bash', ['-e', scriptPath], {
    cwd: directory,
    env: environment,
    encoding: 'utf8',
  })
}

/** The step `Write the source stamp` of the web job of ci.yml. */
async function stampWriter() {
  const steps = jobSteps(await workflowJob('ci.yml', 'web'))
  return steps[namedStep(steps, 'Write the source stamp')]
}

test('52: the web job writes the source stamp after the build and before the distribution check', async () => {
  const steps = jobSteps(await workflowJob('ci.yml', 'web'))
  const build = commandStep(steps, 'pnpm build')
  const writer = namedStep(steps, 'Write the source stamp')
  const check = commandStep(steps, 'pnpm check:dist')

  assert.ok(
    build < writer && writer < check,
    'Write the source stamp is not between pnpm build and pnpm check:dist',
  )
  assert.equal(stepValue(steps[writer], 'shell'), undefined)
  assert.deepEqual(stepMapping(steps[writer], 'env'), {
    REPOSITORY_URL: '${{ github.server_url }}/${{ github.repository }}',
    PULL_REQUEST_URL: '${{ github.event.pull_request.html_url }}',
  })
})

test('52: the source stamp of a push names the repository, the commit and its source archive', async (context) => {
  const build = await temporaryDirectory(context)
  const environment = await isolatedGitEnvironment(build)
  await mkdir(join(build, 'dist'))

  const result = await runStep(context, await stampWriter(), build, {
    ...environment,
    REPOSITORY_URL,
    PULL_REQUEST_URL: '',
    GITHUB_SHA: HEAD_COMMIT,
  })

  assert.equal(result.status, 0, result.stderr)
  assert.equal(await readFile(join(build, 'dist', SOURCE_STAMP), 'utf8'), stampText(HEAD_COMMIT))
})

/**
 * Makes a repository in `directory` whose tip is a merge, made with --no-ff, of a head commit on another branch into a
 * base commit, with a message whose second paragraph holds a decoy parent line and a decoy tree line. Returns the
 * repository's path and the merge commit, its two parents and its tree.
 */
async function mergedRepository(environment, directory) {
  const repository = join(directory, 'repository')
  await mkdir(repository)
  git(environment, repository, 'init', '--initial-branch=main')
  await writeFile(join(repository, 'base.txt'), 'base\n')
  git(environment, repository, 'add', '--all')
  git(environment, repository, 'commit', '--message', 'Base')
  const base = git(environment, repository, 'rev-parse', 'HEAD')
  git(environment, repository, 'switch', '--create', 'head')
  await writeFile(join(repository, 'head.txt'), 'head\n')
  git(environment, repository, 'add', '--all')
  git(environment, repository, 'commit', '--message', 'Head')
  const head = git(environment, repository, 'rev-parse', 'HEAD')
  git(environment, repository, 'switch', 'main')
  const decoy = `parent ${'c'.repeat(40)}\ntree ${'d'.repeat(40)}`
  git(
    environment,
    repository,
    'merge',
    '--no-ff',
    '--message',
    `Merge head into main\n\n${decoy}`,
    'head',
  )
  const merge = git(environment, repository, 'rev-parse', 'HEAD')
  assert.ok(git(environment, repository, 'cat-file', 'commit', merge).endsWith(`\n\n${decoy}`))
  return {
    repository,
    merge,
    parents: [base, head],
    tree: git(environment, repository, 'rev-parse', `${merge}^{tree}`),
  }
}

/** A shallow clone (depth 1) of `repository` in `directory`, holding an empty `dist/`, as the web job checks out. */
async function shallowClone(environment, repository, directory) {
  const clone = join(directory, 'clone')
  git(
    environment,
    directory,
    'clone',
    '--quiet',
    '--depth',
    '1',
    pathToFileURL(repository).href,
    clone,
  )
  await mkdir(join(clone, 'dist'))
  return clone
}

test('52: the source stamp of a pull request also names it, the two parents of the merge and its tree', async (context) => {
  const directory = await temporaryDirectory(context)
  const environment = await isolatedGitEnvironment(directory)
  const { repository, merge, parents, tree } = await mergedRepository(environment, directory)
  const clone = await shallowClone(environment, repository, directory)

  const result = await runStep(context, await stampWriter(), clone, {
    ...environment,
    REPOSITORY_URL,
    PULL_REQUEST_URL,
    GITHUB_SHA: merge,
  })

  assert.equal(result.status, 0, result.stderr)
  assert.equal(
    await readFile(join(clone, 'dist', SOURCE_STAMP), 'utf8'),
    stampText(merge) + pullRequestText(parents, tree),
  )
})

test('52: the source stamp of a pull request fails when the checkout does not hold the commit', async (context) => {
  const directory = await temporaryDirectory(context)
  const environment = await isolatedGitEnvironment(directory)
  const { repository, parents } = await mergedRepository(environment, directory)
  const clone = await shallowClone(environment, repository, directory)

  // The base is a parent of the clone's only commit, so the shallow clone does not hold it.
  const result = await runStep(context, await stampWriter(), clone, {
    ...environment,
    REPOSITORY_URL,
    PULL_REQUEST_URL,
    GITHUB_SHA: parents[0],
  })

  assert.notEqual(result.status, 0)
})

/** The steps of the deploy job of ci.yml and the index of its step `Check the source stamp`. */
async function deploySteps() {
  const steps = jobSteps(await workflowJob('ci.yml', 'deploy'))
  return { steps, check: namedStep(steps, 'Check the source stamp') }
}

test('53: the deploy job checks the source stamp after downloading web-dist and before Wrangler', async () => {
  const { steps, check } = await deploySteps()
  const download = stepIndex(
    steps,
    'that downloads web-dist',
    (step) =>
      stepValue(step, 'uses')?.startsWith('actions/download-artifact@') &&
      stepMapping(step, 'with')?.name === 'web-dist',
  )
  const pages = stepIndex(steps, 'with id pages', (step) => stepValue(step, 'id') === 'pages')

  assert.equal(stepValue(steps[check], 'if'), "steps.secrets.outputs.available == 'true'")
  assert.ok(
    download < check && check < pages,
    'Check the source stamp is not between the download and Wrangler',
  )
})

/**
 * Runs the deploy job's stamp check in a new folder with `GITHUB_SHA` forty `a` digits, after writing `stamp` to
 * `ui/dist/SOURCE.txt` (no file when it is null, and no `ui/dist` when it is undefined).
 */
async function runStampCheck(context, stamp) {
  const { steps, check } = await deploySteps()
  const workspace = await temporaryDirectory(context)
  if (stamp !== undefined) {
    await mkdir(join(workspace, 'ui/dist'), { recursive: true })
  }
  if (typeof stamp === 'string') {
    await writeFile(join(workspace, 'ui/dist', SOURCE_STAMP), stamp)
  }
  return runStep(context, steps[check], workspace, { ...process.env, GITHUB_SHA: HEAD_COMMIT })
}

/** Asserts that the stamp check passes and prints nothing with `stamp` in `ui/dist/SOURCE.txt`. */
async function assertStampAccepted(context, stamp) {
  const result = await runStampCheck(context, stamp)

  assert.equal(result.status, 0, result.stderr)
  assert.equal(result.stdout, '')
  assert.equal(result.stderr, '')
}

/** Asserts that the stamp check fails with exactly the source-stamp error, as `runStampCheck` takes `stamp`. */
async function assertStampRejected(context, stamp) {
  const result = await runStampCheck(context, stamp)

  assert.equal(result.status, 1)
  assert.equal(result.stdout, `${stampErrorMessage(HEAD_COMMIT)}\n`)
  assert.equal(result.stderr, '')
}

test('53: the stamp check passes silently when the stamp of a push names the commit', (context) =>
  assertStampAccepted(context, stampText(HEAD_COMMIT)))

test('53: the stamp check passes silently when the stamp of a pull request names the commit', (context) =>
  assertStampAccepted(
    context,
    stampText(HEAD_COMMIT) + pullRequestText([OTHER_COMMIT, 'c'.repeat(40)], 'd'.repeat(40)),
  ))

test('53: the stamp check fails with the source-stamp error when the stamp is missing', (context) =>
  assertStampRejected(context, null))

test('53: the stamp check fails with the source-stamp error when ui/dist is missing', (context) =>
  assertStampRejected(context, undefined))

test('53: the stamp check fails with the source-stamp error when the stamp names another commit', (context) =>
  assertStampRejected(context, stampText(OTHER_COMMIT)))

test('53: the stamp check fails with the source-stamp error when the commit line runs on', (context) =>
  assertStampRejected(
    context,
    stampText(HEAD_COMMIT).replace(`Commit: ${HEAD_COMMIT}\n`, `Commit: ${HEAD_COMMIT}0\n`),
  ))

test('54: the draft job adds the OCCT source archive after the notices and before SHA256SUMS', async () => {
  const steps = jobSteps(await workflowJob('release.yml', 'draft'))
  const notices = commandStep(steps, 'cp THIRD-PARTY-NOTICES.txt release/')
  const archive = commandStep(steps, `${ARCHIVE_SCRIPT} release`)
  const sums = namedStep(steps, 'Write SHA256SUMS')

  assert.ok(
    notices < archive && archive < sums,
    'the archive step is not between the notices and SHA256SUMS',
  )
  assert.match(stepValue(steps.at(-1), 'run'), /^gh release create .* release\/\*$/m)
})

test('54: the archive script is a regular file its owner can run', async () => {
  const status = await lstat(join(repositoryRoot, ARCHIVE_SCRIPT))

  assert.ok(status.isFile(), `${ARCHIVE_SCRIPT} is not a regular file`)
  assert.notEqual(status.mode & 0o100, 0, `${ARCHIVE_SCRIPT} is not executable by its owner`)
})

test('57: the notices statements name the stamp, the archive and its script', async () => {
  const pins = await readFile(join(repositoryRoot, 'scripts/occt-version.sh'), 'utf8')
  const version = /^OCCT_VERSION=(\S+)$/m.exec(pins)?.[1]
  assert.ok(version, 'scripts/occt-version.sh sets no OCCT_VERSION')
  const archiveName = `occt-${version}-source.tar.gz`
  const web = await readFile(join(repositoryRoot, 'ui/public/THIRD-PARTY-NOTICES.txt'), 'utf8')
  const desktop = await readFile(join(repositoryRoot, 'THIRD-PARTY-NOTICES.txt'), 'utf8')

  for (const name of [`/${SOURCE_STAMP}`, archiveName, ARCHIVE_SCRIPT]) {
    assert.ok(web.includes(name), `ui/public/THIRD-PARTY-NOTICES.txt does not name ${name}`)
  }
  for (const name of [archiveName, ARCHIVE_SCRIPT]) {
    assert.ok(desktop.includes(name), `THIRD-PARTY-NOTICES.txt does not name ${name}`)
  }
})
