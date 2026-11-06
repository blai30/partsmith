// Temporary experiment (removed in the next commit): starts Firefox again and again, as a
// Playwright worker starts it, and records whether each new browser gives a canvas a WebGL2
// context, the reason Firefox gives when it does not, every line the browser prints and how long
// the runner stalled while the browser started.
// The workflow .github/workflows/firefox-webgl-experiment.yml runs it under xvfb-run, as
// ci.yml's e2e-engines runs the tests. Run it from ui/:
//   xvfb-run --auto-servernum node e2e-experiment/probe.mjs --variant current-2 --workers 2 --waves 20
import { execFile } from 'node:child_process'
import { appendFile, mkdir, readdir, readFile } from 'node:fs/promises'
import { createServer } from 'node:http'
import { dirname } from 'node:path'
import { setTimeout as sleep } from 'node:timers/promises'
import { parseArgs, promisify } from 'node:util'

const { values: options } = parseArgs({
  options: {
    variant: { type: 'string', default: 'current-2' },
    row: { type: 'string' },
    workers: { type: 'string', default: '2' },
    waves: { type: 'string', default: '1000' },
    minutes: { type: 'string', default: '14' },
    output: { type: 'string' },
    'force-webgl': { type: 'boolean', default: false },
    headless: { type: 'boolean', default: false },
    'drop-caches': { type: 'boolean', default: false },
  },
})
const row = options.row ?? options.variant
const workers = Number(options.workers)
const waves = Number(options.waves)
const output = options.output ?? `test-results/firefox-webgl/${row}.jsonl`

// The longest line kept from the browser's output or from about:support.
const LINE_LIMIT = 500

// Playwright logs each line the browser prints as `[pid=N][out]` or `[err]` under pw:browser.
// Its debug module reads these variables when it loads, so they are set before the import.
process.env.DEBUG = 'pw:browser'
process.env.DEBUG_COLORS = '0'

/** The lines each browser printed, by process id, with the time each arrived. */
const browserOutput = new Map()
const BROWSER_LINE = /pw:browser \[pid=(\d+)\]\[(?:out|err)\] (.*)$/
const writeStandardError = process.stderr.write.bind(process.stderr)
process.stderr.write = (chunk, ...rest) => {
  const text = String(chunk)
  if (!text.includes(' pw:browser ')) {
    return writeStandardError(chunk, ...rest)
  }
  const match = BROWSER_LINE.exec(text.trimEnd())
  if (match) {
    const lines = browserOutput.get(Number(match[1])) ?? []
    lines.push({ arrived: performance.now(), text: match[2].slice(0, LINE_LIMIT) })
    browserOutput.set(Number(match[1]), lines)
  }
  return true
}

const { devices, firefox } = await import('@playwright/test')

// MOZ_GFX_DEBUG=1 makes the startup graphics probe (gfxtest) log each step to stderr, and
// gfx.logging.level 2 prints the warning Firefox writes when that probe fails in any way.
const preferences = {
  'gfx.logging.level': 2,
  ...(options['force-webgl'] ? { 'webgl.force-enabled': true } : {}),
}
// The most lines of browser output kept per launch.
const OUTPUT_LIMIT = 300
// Lines of about:support worth keeping: the WebGL rows, the decisions and the failure log.
const SUPPORT_LINE = /webgl|glxtest|feature_failure|blocklist|blocked|failure|decision|\[GFX/i
// The driver extension lists of about:support are long and say nothing about a refusal.
const EXTENSIONS_LINE = /extensions/i
// Failing launches per row that read about:support besides the first launch. In a row where
// every launch fails (slow-probe), reading it each time would take most of the row's time.
const ABOUT_SUPPORT_LIMIT = 5
// A launch that has not ended after this long is recorded as stalled and its browser is killed,
// so that one hung browser cannot hold up the row until the step's timeout.
const LAUNCH_LIMIT = 150_000
// How long Playwright may take to start a browser before the launch fails.
const START_LIMIT = 90_000

let failingAboutSupportReads = 0

/** The first line of an error's message, or the whole message when `whole` is set. */
function messageOf(error, whole = false) {
  const message = error instanceof Error ? error.message : String(error)
  return whole ? message.slice(0, 4000) : message.split('\n')[0]
}

/** The headers of the `/*` route of public/_headers, which every page of the app gets. */
async function appHeaders() {
  const text = await readFile(new URL('../public/_headers', import.meta.url), 'utf8')
  const headers = {}
  let inAppRoute = false
  for (const line of text.split('\n')) {
    if (!line.startsWith(' ')) {
      inAppRoute = line.trim() === '/*'
    } else if (inAppRoute) {
      const [name, ...value] = line.trim().split(':')
      headers[name] = value.join(':').trim()
    }
  }
  return headers
}

/** Serves the check page with the app's headers on a free local port and returns its URL. */
async function servePage() {
  const headers = await appHeaders()
  const pages = {
    '/': {
      body: await readFile(new URL('page/index.html', import.meta.url)),
      type: 'text/html; charset=utf-8',
    },
    '/check.js': {
      body: await readFile(new URL('page/check.js', import.meta.url)),
      type: 'text/javascript; charset=utf-8',
    },
  }
  const server = createServer((request, response) => {
    const page = pages[new URL(request.url, 'http://localhost').pathname]
    if (page === undefined) {
      // No content for the favicon request, which would log a failed load to the console.
      response.writeHead(request.url === '/favicon.ico' ? 204 : 404, headers).end()
      return
    }
    response.writeHead(200, { ...headers, 'Content-Type': page.type }).end(page.body)
  })
  await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve))
  return { server, url: `http://127.0.0.1:${String(server.address().port)}/` }
}

/** The `total=` of the `some` line of a /proc/pressure file in milliseconds, if it has one. */
function stallMilliseconds(pressure) {
  const match = /^some .*total=(\d+)$/m.exec(pressure)
  return match === null ? undefined : Number(match[1]) / 1000
}

/**
 * The runner's cumulative counters: the time some task waited for a CPU, for the disk and for
 * memory (pressure stall information), the time the hypervisor took the CPUs away (steal, summed
 * over the CPUs) and the page faults that read the disk.
 */
async function runnerCounters() {
  const [cpu, io, memory, stat, vmstat] = await Promise.all(
    ['pressure/cpu', 'pressure/io', 'pressure/memory', 'stat', 'vmstat'].map((name) =>
      readFile(`/proc/${name}`, 'utf8').catch(() => ''),
    ),
  )
  // The first line of /proc/stat: cpu user nice system idle iowait irq softirq steal, in 10 ms ticks.
  const steal = Number(stat.split('\n')[0].trim().split(/\s+/)[8]) * 10
  const majorFaults = Number(/^pgmajfault (\d+)$/m.exec(vmstat)?.[1])
  return {
    cpuStall: stallMilliseconds(cpu),
    ioStall: stallMilliseconds(io),
    memoryStall: stallMilliseconds(memory),
    steal: Number.isFinite(steal) ? steal : undefined,
    majorFaults: Number.isFinite(majorFaults) ? majorFaults : undefined,
  }
}

/** How much each counter grew since `before`, leaving out the counters the runner lacks. */
function growth(before, after) {
  return Object.fromEntries(
    Object.entries(after)
      .filter(([name, value]) => value !== undefined && before[name] !== undefined)
      .map(([name, value]) => [name, Math.round(value - before[name])]),
  )
}

/** Opens the check page in a new context, as each test does, and returns what it found. */
async function checkPage(browser, url) {
  const context = await browser.newContext({ ...devices['Desktop Firefox'] })
  const messages = []
  try {
    const page = await context.newPage()
    page.on('console', (message) => messages.push(`${message.type()}: ${message.text()}`))
    page.on('pageerror', (error) => messages.push(`pageerror: ${messageOf(error)}`))
    const started = performance.now()
    await page.goto(url)
    const result = await page.evaluate(() => globalThis.probeResult)
    if (!result.webgl2.created) {
      // Firefox's own warning for the refusal can arrive just after the evaluation's result.
      await sleep(500)
    }
    return { ...result, milliseconds: Math.round(performance.now() - started), console: messages }
  } finally {
    await context.close()
  }
}

/** Whether this launch reads about:support: the first launch, and the first failing ones. */
function readsAboutSupport(record) {
  if (record.launch === 1) {
    return true
  }
  if (record.pages[0].webgl2.created || failingAboutSupportReads >= ABOUT_SUPPORT_LIMIT) {
    return false
  }
  failingAboutSupportReads += 1
  return true
}

/** The WebGL and failure lines of about:support, or why they could not be read. */
async function readAboutSupport(browser) {
  const context = await browser.newContext()
  try {
    const page = await context.newPage()
    await page.goto('about:support', { timeout: 15_000 })
    await page.waitForFunction(() => document.body.innerText.includes('WebGL 2'), undefined, {
      timeout: 10_000,
    })
    const text = await page.evaluate(() => document.body.innerText)
    return {
      lines: text
        .split('\n')
        .filter((line) => SUPPORT_LINE.test(line) && !EXTENSIONS_LINE.test(line))
        .map((line) => line.slice(0, LINE_LIMIT))
        .slice(0, 80),
    }
  } catch (error) {
    return { error: messageOf(error) }
  } finally {
    await context.close()
  }
}

/** The process id of the browser this process started with `marker` in its environment. */
async function browserProcessId(marker) {
  const names = (await readdir('/proc')).filter((name) => /^\d+$/.test(name))
  const matches = await Promise.all(
    names.map(async (name) => {
      try {
        const [environment, status] = await Promise.all([
          readFile(`/proc/${name}/environ`, 'latin1'),
          readFile(`/proc/${name}/status`, 'latin1'),
        ])
        const ours = status.includes(`\nPPid:\t${String(process.pid)}\n`)
        return ours && environment.split('\0').includes(marker) ? Number(name) : undefined
      } catch {
        // The process ended, or it is not ours to read.
        return undefined
      }
    }),
  )
  return matches.find((match) => match !== undefined)
}

/** Kills the browser and everything it started, gfxtest included. */
function killBrowser(processId) {
  if (processId === undefined) {
    return
  }
  try {
    // Playwright starts the browser as the leader of its own process group.
    process.kill(-processId, 'SIGKILL')
  } catch {
    // The browser has already exited.
  }
}

/** Whether `promise` settles within `milliseconds`. */
async function settlesWithin(promise, milliseconds) {
  let timer
  const timeout = new Promise((resolve) => {
    timer = setTimeout(resolve, milliseconds, false)
  })
  try {
    return await Promise.race([promise.then(() => true), timeout])
  } finally {
    clearTimeout(timer)
  }
}

/** Starts one browser into `record`, checks two pages in it and closes it. Never throws. */
async function runLaunch(record, url, waveCounters) {
  // This marker in the browser's environment finds its process id. The id ties the output lines
  // Playwright logs to this launch.
  const marker = `PARTSMITH_PROBE_LAUNCH=${String(record.launch)}`
  const started = performance.now()
  let browser
  try {
    try {
      browser = await firefox.launch({
        headless: options.headless,
        firefoxUserPrefs: preferences,
        timeout: START_LIMIT,
        env: { ...process.env, MOZ_GFX_DEBUG: '1', PARTSMITH_PROBE_LAUNCH: String(record.launch) },
      })
    } finally {
      record.launchMilliseconds = Math.round(performance.now() - started)
      // The counters grew from the start of the wave to the end of this launch: the window in
      // which the browsers of the wave started and their graphics probes ran.
      record.startupStalls = growth(waveCounters, await runnerCounters())
    }
    record.launched = true
    record.processId = await browserProcessId(marker)
    // Two pages one after the other, as a worker runs two tests: a refusal that holds for the
    // whole browser refuses both, one that holds for a page refuses the first alone.
    record.pages.push(await checkPage(browser, url))
    record.pages.push(await checkPage(browser, url))
    if (readsAboutSupport(record)) {
      record.aboutSupport = await readAboutSupport(browser)
    }
  } catch (error) {
    // A stalled launch keeps its stall as the error, not the error its killed browser then gave.
    record.error ??= messageOf(error, browser === undefined)
  } finally {
    await browser?.close().catch((error) => {
      record.closeError = messageOf(error)
    })
  }
}

/** Starts one browser, checks two pages in it, closes it and returns what it recorded. */
async function launch(url, runStarted, wave, slot, waveCounters) {
  const started = performance.now()
  const record = {
    variant: options.variant,
    row,
    launch: (wave - 1) * workers + slot,
    wave,
    slot,
    workers,
    forceWebgl: options['force-webgl'],
    headless: options.headless,
    dropCaches: options['drop-caches'],
    startedMilliseconds: Math.round(started - runStarted),
    launched: false,
    pages: [],
  }
  const work = runLaunch(record, url, waveCounters)
  if (!(await settlesWithin(work, LAUNCH_LIMIT))) {
    record.error = `stalled: the launch had not ended after ${String(LAUNCH_LIMIT / 1000)} s`
    killBrowser(record.processId)
    await settlesWithin(work, 30_000)
  }
  const lines = browserOutput.get(record.processId) ?? []
  browserOutput.delete(record.processId)
  record.outputLines = lines.length
  record.output = lines.slice(0, OUTPUT_LIMIT).map((line) => ({
    milliseconds: Math.round(line.arrived - started),
    text: line.text,
  }))
  return record
}

/** One line for the job log. */
function describe(record) {
  const name = `wave ${String(record.wave)} slot ${String(record.slot)}`
  const stalls = Object.entries(record.startupStalls ?? {})
    .map(([counter, value]) => `${counter} ${String(value)}`)
    .join(', ')
  if (!record.launched) {
    return `${name}: launch failed: ${messageOf(record.error)} (${stalls})`
  }
  const pages = record.pages.map(
    (page, i) =>
      `page ${String(i + 1)} ${page.webgl2.created ? 'WebGL2' : `no WebGL2 (${page.webgl2.reason ?? 'no reason given'})`}`,
  )
  if (pages.length === 0) {
    pages.push('no page checked')
  }
  const error = record.error === undefined ? '' : `, error: ${messageOf(record.error)}`
  return `${name}: ${pages.join(', ')}, launch ${String(record.launchMilliseconds)} ms (${stalls})${error}`
}

/** Empties the page cache. The next launches then read Firefox and Mesa from disk. */
async function dropPageCache() {
  await promisify(execFile)('sudo', ['-n', 'sh', '-c', 'sync && echo 3 > /proc/sys/vm/drop_caches'])
}

/** Starts `workers` browsers at the same moment, as Playwright's workers do at the start of a run. */
async function runWave(url, runStarted, wave) {
  if (options['drop-caches']) {
    await dropPageCache()
  }
  const counters = await runnerCounters()
  const records = await Promise.all(
    Array.from({ length: workers }, (_, i) => launch(url, runStarted, wave, i + 1, counters)),
  )
  await appendFile(output, records.map((record) => `${JSON.stringify(record)}\n`).join(''))
  for (const record of records) {
    console.log(describe(record))
  }
  // Xvfb resets when its last client leaves; the pause lets it finish before the next wave connects.
  await sleep(1000)
}

const { server, url } = await servePage()
await mkdir(dirname(output), { recursive: true })
const runStarted = performance.now()
const deadline = runStarted + Number(options.minutes) * 60_000
console.log(
  `${row}: up to ${String(waves)} waves of ${String(workers)} launches within ${options.minutes} minutes`,
)
for (let wave = 1; wave <= waves && performance.now() < deadline; wave += 1) {
  // The waves run one after another.
  // oxlint-disable-next-line no-await-in-loop
  await runWave(url, runStarted, wave)
}
server.close()
// A browser that a stalled launch could not kill would keep this process alive until the step's
// timeout, and the records are written, so the probe ends here.
process.exit(0)
