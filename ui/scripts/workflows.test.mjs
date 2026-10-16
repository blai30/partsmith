// Acceptance tests 49 to 51 of docs/spec/delivery.md section 6 for the workflow files.
import assert from 'node:assert/strict'
import { spawnSync } from 'node:child_process'
import { chmod, mkdir, mkdtemp, readFile, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { test } from 'node:test'
import { fileURLToPath } from 'node:url'

const workflowsDirectory = fileURLToPath(new URL('../../.github/workflows/', import.meta.url))

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
