/// <reference lib="dom" />
import { execFile } from 'node:child_process'
import { createRequire } from 'node:module'
import { fileURLToPath } from 'node:url'
import { promisify } from 'node:util'
import { expect } from '@playwright/test'
import { longTasks, measures, openApp, pageTime, percentile, test, watchLongTasks } from './helpers'

/** How long the busy task of the long task check runs, and the least duration it must report. */
const BUSY_MILLISECONDS = 120
const BUSY_REPORTED_MILLISECONDS = 100

/** The title of the startup check, the one test of the engine subset without an E-test id (ui.md 6.8). */
const STARTUP_CHECK =
  'the production build starts the WebAssembly core in a cross-origin isolated page'

/** The E-tests of the engine subset (ui.md 6.8), which run in every project. */
const ENGINE_TEST_IDS = [
  'E-1',
  'E-2',
  'E-3',
  'E-4',
  'E-5',
  'E-6',
  'E-8',
  'E-9',
  'E-10',
  'E-12',
  'E-13',
  'E-29',
  'E-33',
  'E-41',
  'E-47',
  'E-52',
  'E-53',
  'E-56',
  'E-127',
  'E-128',
  'E-144',
  'E-147',
  'E-156',
]

/** The projects of `ui/playwright.config.ts`, in order: Chromium runs every test, the other two the subset. */
const PROJECTS = ['chromium', 'firefox', 'webkit']

/**
 * One test of a listing in one project. Its place, the file, line and column of its declaration with its title, is the
 * same in every project; a test declared in a loop shares its line with the others of the loop, not its title.
 */
type ListedTest = { place: string; title: string; project: string }

/** The suites of Playwright's JSON report, as far as a listing uses them. */
type ReportSuite = {
  specs: {
    file: string
    line: number
    column: number
    title: string
    tests: { projectName: string }[]
  }[]
  suites?: ReportSuite[]
}

/** The `ui` folder, where `pnpm e2e` runs. */
const uiFolder = fileURLToPath(new URL('..', import.meta.url))

/** The command line of `@playwright/test`, which `pnpm e2e` runs. */
const playwrightCommand = createRequire(import.meta.url).resolve('@playwright/test/cli')

/** Every test of a JSON report's suites, once per project it runs in. */
function listedTests(suites: ReportSuite[]): ListedTest[] {
  return suites.flatMap((suite) => [
    ...suite.specs.flatMap((spec) =>
      spec.tests.map((listed) => ({
        place: `${spec.file}:${String(spec.line)}:${String(spec.column)} ${spec.title}`,
        title: spec.title,
        project: listed.projectName,
      })),
    ),
    ...listedTests(suite.suites ?? []),
  ])
}

/**
 * The tests `playwright test --list` gives with `projectArguments`, read from its JSON report. A listing loads the
 * configuration and the test files and starts no browser and no web server.
 */
async function listTests(projectArguments: string[]): Promise<ListedTest[]> {
  const { stdout } = await promisify(execFile)(
    process.execPath,
    [playwrightCommand, 'test', '--list', '--reporter=json', ...projectArguments],
    { cwd: uiFolder, maxBuffer: 64 * 1024 * 1024 },
  )
  const report = JSON.parse(stdout) as { suites: ReportSuite[] }
  return listedTests(report.suites)
}

/** The E-test id that starts a title, or the whole title for the startup check. */
function engineTestId(title: string): string {
  return /^(E-\d+) /.exec(title)?.[1] ?? title
}

test(STARTUP_CHECK, { tag: '@engines' }, async ({ page }) => {
  await page.goto('/')
  await expect(page).toHaveTitle('Partsmith')
  const app = page.getByTestId('app')
  // Compiling the core's WebAssembly can take longer than the default expect timeout on a cold runner.
  await expect(app).not.toHaveAttribute('data-core-status', 'loading', { timeout: 30_000 })
  await expect(app).toHaveAttribute('data-core-status', 'ready')
  expect(await page.evaluate(() => window.crossOriginIsolated)).toBe(true)
  // The first frame with the startup document and the core ready (ui.md 2.18).
  await expect
    .poll(() => page.evaluate(() => performance.getEntriesByName('ps:usable').length))
    .toBe(1)
})

// The helpers that only the @perf tests use (ui.md 6.3, 6.7), checked on every run.

test('percentile is the nearest-rank percentile of the values in any order', () => {
  const oneToTwenty = Array.from({ length: 20 }, (_, i) => i + 1)
  expect(percentile([5], 0.95)).toBe(5)
  expect(percentile(oneToTwenty.slice(0, 10).toReversed(), 0.95)).toBe(10)
  expect(percentile(oneToTwenty, 0.5)).toBe(10)
  expect(percentile(oneToTwenty, 0.95)).toBe(19)
  // The 4th of 7, which median of partsmith-budgets takes, as the engine's web rows use it (engine.md section 5).
  expect(percentile([7, 1, 6, 2, 5, 3, 4], 0.5)).toBe(4)
  expect(() => percentile([], 0.5)).toThrow('A percentile needs at least one value.')
})

test('longTasks and measures report what started at or after a page time', async ({ page }) => {
  await watchLongTasks(page)
  await openApp(page)
  await page.evaluate(() => {
    performance.measure('helpers-check', { start: performance.now(), duration: 1 })
  })
  const since = await pageTime(page)
  // The busy loop runs in a task of the page's own, since script that Playwright evaluates
  // directly is not a task the page reports.
  await page.evaluate(
    (milliseconds) =>
      new Promise<void>((resolve) => {
        setTimeout(() => {
          const end = performance.now() + milliseconds
          while (performance.now() < end) {
            // Busy, so this task is a long task.
          }
          performance.measure('helpers-check', { start: performance.now(), duration: 2 })
          resolve()
        })
      }),
    BUSY_MILLISECONDS,
  )
  const after = await pageTime(page)
  const longest = async (start: number) => Math.max(0, ...(await longTasks(page, start)))
  await expect.poll(() => longest(since)).toBeGreaterThanOrEqual(BUSY_REPORTED_MILLISECONDS)
  expect(await longest(after)).toBeLessThan(BUSY_REPORTED_MILLISECONDS)
  expect(await measures(page, 'helpers-check', since)).toEqual([2])
  expect(await measures(page, 'helpers-check')).toEqual([1, 2])
})

// The browser engines (ui.md 6.8).

test(
  'E-156 every project has the viewport of the tests, and Firefox and WebKit run the subset',
  { tag: '@engines' },
  ({ browserName }) => {
    const { config, project: running } = test.info()
    const projects = config.projects

    expect(projects.map((project) => project.name)).toEqual(PROJECTS)
    for (const project of projects) {
      expect(project.use.viewport, project.name).toEqual({ width: 1280, height: 720 })
      expect(project.use.deviceScaleFactor, project.name).toBe(1)
    }
    const [chromium, firefox, webkit] = projects
    for (const engine of [firefox, webkit]) {
      expect(engine.grep, engine.name).toEqual(/@engines/)
      expect(engine.use.launchOptions?.executablePath, engine.name).toBeUndefined()
    }
    // Playwright's default grep, which matches every title.
    expect(chromium.grep).toEqual(/.*/)
    expect(chromium.use.launchOptions?.executablePath).toBe(
      process.env.PLAYWRIGHT_CHROMIUM_EXECUTABLE,
    )
    // Firefox runs headed; the other two keep Playwright's default, headless.
    expect(firefox.use.headless).toBe(false)
    expect(chromium.use.headless).toBeUndefined()
    expect(webkit.use.headless).toBeUndefined()
    expect(browserName).toBe(running.name)
  },
)

test('E-157 Firefox and WebKit list the engine subset, and the default project is Chromium with every test', async () => {
  // Four listings take a few seconds each, longer on a busy runner.
  test.setTimeout(120_000)
  const [firefox, webkit, everyProject, byDefault] = await Promise.all([
    listTests(['--project', 'firefox']),
    listTests(['--project', 'webkit']),
    listTests(PROJECTS.flatMap((project) => ['--project', project])),
    listTests([]),
  ])
  const subset = [STARTUP_CHECK, ...ENGINE_TEST_IDS].toSorted()

  for (const [project, listed] of [
    ['firefox', firefox],
    ['webkit', webkit],
  ] as const) {
    expect(new Set(listed.map((entry) => entry.project)), project).toEqual(new Set([project]))
    expect(listed.map((entry) => engineTestId(entry.title)).toSorted(), project).toEqual(subset)
  }
  expect(new Set(byDefault.map((entry) => entry.project))).toEqual(new Set(['chromium']))
  // A test of another engine that Chromium does not run would be a place the default listing lacks.
  const defaultPlaces = byDefault.map((entry) => entry.place)
  expect(new Set(defaultPlaces).size, 'each test once').toBe(defaultPlaces.length)
  expect(new Set(defaultPlaces)).toEqual(new Set(everyProject.map((entry) => entry.place)))
})
