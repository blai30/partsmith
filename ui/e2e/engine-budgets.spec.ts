/// <reference lib="dom" />
import { readFile } from 'node:fs/promises'
import { expect, test, type Page, type Route } from '@playwright/test'
import { percentile } from './helpers'
import type { Answer } from './pages/engine-worker'
import {
  MEMORY_ROW,
  TIMED_ROWS,
  ok,
  openEngine,
  warmUp,
  type Engine,
} from './support/engine-budgets'
import { expectWithinBudgets } from './support/perf'
import { serveTestPage } from './support/test-pages'

// The web column of the engine's performance budgets (engine.md section 5, tests 155 to 157). The rows are tagged
// @perf and run only with PARTSMITH_PERF=1, by hand on the reference machine; tests 156 and 157 check the driver of
// the engine page on every run, each on a page of its own.

// One test after another in one worker, despite the configuration's fullyParallel: rows side by side would time each
// other's load. Not serial, which would skip the rows after a failed one.
test.describe.configure({ mode: 'default' })

/**
 * The samples each timed row takes, engine.md section 5's 7; their median counts. The test holds the rows to this
 * number of its own, apart from the support module's, so a row that takes another number fails.
 */
const SAMPLES = 7

/** The module's maximum memory, 4 GiB (`-sMAXIMUM_MEMORY=4GB`), which row 1's bytes stay below. */
const MAXIMUM_MEMORY_BYTES = 4_294_967_296

/** How long one row, or the warm-up before the rows, may take. */
const ROW_TIMEOUT_MILLISECONDS = 300_000

/** A WebAssembly memory grows by whole pages of this size. */
const WASM_PAGE_BYTES = 65_536

/** The desktop bench, whose rows the web rows port under the same names (engine.md section 5, "In step"). */
const DESKTOP_BENCH = new URL('../../crates/partsmith-engine/benches/budgets.rs', import.meta.url)

/** The name of row 1 in engine.md section 5's list. */
const MEMORY_ROW_NAME = 'wasm memory with a 300-feature document open'

/** Rows 2 to 17 of engine.md section 5's list, in its order, each with its web budget in milliseconds. */
const TIMED_ROW_BUDGETS: readonly (readonly [name: string, budget: number])[] = [
  [
    'apply_edits inside a transaction changing one parameter that affects the last 5 features of the reference part',
    250,
  ],
  ['open_document of the reference part, cold session', 3_000],
  ['close_document then open_document of the same text in the same session', 1_000],
  ['export of the reference part to binary STL at standard quality', 900],
  ['export_python of the reference part', 50],
  [
    'apply_fix of a SetField of one of the last 5 features of the reference part, inside a transaction',
    250,
  ],
  [
    'apply_edits inside a transaction of a parameter change that loses 5 references of the last features of the reference part',
    250,
  ],
  [
    'set_document_units with keep_sizes on the reference part, including the edit set, recompute, tessellation of changed bodies and frame encoding',
    250,
  ],
  ['list_uses of the reference part', 15],
  ['ghost capture of one lost edge on the reference part', 15],
  ['drag_sketch frame on a 500-constraint sketch, including frame encoding', 12],
  ['pick_reference, first pick in a new state of a 1000-face body', 2],
  ['pick_reference, later picks in the same state', 2],
  ['list_elements of a 1000-face body, first call for a new state', 25],
  ['list_elements of a 1000-face body, repeated', 25],
  ['analyze_print of one 200 000-triangle body', 900],
]

/** The text in a request frame at which the stand-in core of test 157 stops. */
const STOP_MARKER = 'stop the core'

/**
 * The stand-in for `core/partsmith_core.mjs` of test 157: the real factory, whose instance's `_ps_call` traps, as
 * `unreachable` does, on a request frame that holds `STOP_MARKER`, and calls the real one otherwise.
 */
const STOPPING_CORE = [
  "import createRealCore from './partsmith_core.mjs?real'",
  'export default async function createPartsmithCore(...options) {',
  '  const core = await createRealCore(...options)',
  '  const call = core._ps_call',
  '  core._ps_call = (request, requestLength, responseLengthOut) => {',
  '    const frame = core.HEAPU8.slice(request, request + requestLength)',
  `    if (new TextDecoder().decode(frame).includes(${JSON.stringify(STOP_MARKER)})) {`,
  "      throw new WebAssembly.RuntimeError('unreachable')",
  '    }',
  '    return call(request, requestLength, responseLengthOut)',
  '  }',
  '  return core',
  '}',
].join('\n')

/** The path of the engine page's worker script, which test 157 answers in place of the dev server. */
const WORKER_SCRIPT = '/e2e/pages/engine-worker.ts'

/** The stand-in for the worker script of test 157, a module that throws as it runs. */
const THROWING_WORKER = "throw new Error('stop the worker')"

/** Answers a routed request of test 157 with a 404. */
function notFound(route: Route): Promise<void> {
  return route.fulfill({ status: 404, body: 'Not found' })
}

/** An answer of `close_document` with request `id` that took 0.25 ms and returned `messages`. */
function closeDocumentAnswer(id: number, messages: Answer['messages']): Answer {
  return { type: 'close_document', id, milliseconds: 0.25, messages }
}

/** The text with which the driver refuses an instance that is not running. */
function notRunning(instance: number): string {
  return `No engine instance ${String(instance)} is running; start() makes one and end() drops it.`
}

/** Sends `new_document` to `instance` and checks the answer: request `id`, a new document `doc`, an ok result. */
async function expectNewDocument(
  engine: Engine,
  instance: number,
  id: number,
  doc: number,
): Promise<void> {
  const answer = await engine.call(instance, { type: 'new_document' })
  const result = { kind: 'ok', type: 'new_document', doc }
  expect(answer.type).toBe('new_document')
  expect(answer.id).toBe(id)
  expect(Number.isFinite(answer.milliseconds), `milliseconds ${String(answer.milliseconds)}`).toBe(
    true,
  )
  expect(answer.milliseconds).toBeGreaterThanOrEqual(0)
  expect(answer.messages.at(-1)).toEqual({ kind: 'response', id, result })
  expect(ok(answer)).toEqual(result)
}

const enginePage = serveTestPage('engine.html')

test('ok returns the result of an ok response and throws for an error or a missing response', () => {
  const closed = { kind: 'ok', type: 'close_document' } as const

  expect(ok(closeDocumentAnswer(6, [{ kind: 'response', id: 6, result: closed }]))).toEqual(closed)
  expect(() =>
    ok(
      closeDocumentAnswer(7, [
        {
          kind: 'response',
          id: 7,
          result: { kind: 'error', code: 'unknown_document', message: 'Document 3 is not open.' },
        },
      ]),
    ),
  ).toThrow(
    new Error(
      'close_document (request 7) failed in the web core with unknown_document: Document 3 is not open.',
    ),
  )
  const noResponse = new Error('close_document (request 8) got no response from the web core.')
  expect(() => ok(closeDocumentAnswer(8, []))).toThrow(noResponse)
  expect(() =>
    ok(
      closeDocumentAnswer(8, [
        { kind: 'request', id: 8, command: { type: 'close_document', doc: 3 } },
      ]),
    ),
  ).toThrow(noResponse)
})

test("engine test 156: the engine page's driver", async ({ page }) => {
  // A page of its own, not the rows', so its instance numbers start at 1 whatever ran before.
  const engine = await openEngine(page, enginePage.url)
  const [worker, ...otherWorkers] = page.workers()
  expect(otherWorkers).toHaveLength(0)
  expect(await worker?.evaluate(() => self.crossOriginIsolated)).toBe(true)

  expect(await engine.start()).toBe(1)
  expect(await engine.start()).toBe(2)
  // Each instance has a session of its own: both answer with document 1.
  await expectNewDocument(engine, 1, 1, 1)
  await expectNewDocument(engine, 2, 1, 1)
  await expectNewDocument(engine, 1, 2, 2)

  const bytes = await engine.memoryBytes(1)
  expect(bytes).toBeGreaterThan(0)
  expect(bytes % WASM_PAGE_BYTES).toBe(0)

  const closed = await engine.call(2, { type: 'close_document', doc: 9 })
  expect(closed.type).toBe('close_document')
  expect(closed.id).toBe(2)
  expect(() => ok(closed)).toThrow(
    new Error(
      'close_document (request 2) failed in the web core with unknown_document: Document 9 is not open.',
    ),
  )

  await engine.end(1)
  await expect(engine.call(1, { type: 'engine_info' })).rejects.toThrow(notRunning(1))
  await expect(engine.memoryBytes(1)).rejects.toThrow(notRunning(1))
  await expect(engine.end(1)).rejects.toThrow(notRunning(1))
  await expect(engine.call(3, { type: 'engine_info' })).rejects.toThrow(notRunning(3))
  expect(ok(await engine.call(2, { type: 'engine_info' }))).toMatchObject({
    kind: 'ok',
    type: 'engine_info',
  })
  await engine.end(2)

  // The rows are section 5's, and the desktop bench still names each timed row, so a desktop row renamed or removed
  // fails here.
  expect([MEMORY_ROW.name, ...TIMED_ROWS.map((row) => row.name)]).toEqual([
    MEMORY_ROW_NAME,
    ...TIMED_ROW_BUDGETS.map(([name]) => name),
  ])
  expect(TIMED_ROWS.map((row) => [row.name, row.budget])).toEqual(TIMED_ROW_BUDGETS)
  const desktopBench = await readFile(DESKTOP_BENCH, 'utf8')
  for (const row of TIMED_ROWS) {
    expect.soft(desktopBench, `budgets.rs names "${row.name}"`).toContain(`"${row.name}"`)
  }
})

test("engine test 157: the engine page's failures", async ({ page, context }) => {
  // In Chromium a page's routes also serve its workers' requests, as stallCore relies on (ui.md 6.3).
  await page.route((url) => url.pathname.endsWith('/core/partsmith_core.wasm'), notFound)
  const unbuilt = await openEngine(page, enginePage.url)
  await expect(unbuilt.start()).rejects.toThrow('The web core did not start: TypeError: ')

  const stoppingPage = await context.newPage()
  await stoppingPage.route(
    (url) => url.pathname.endsWith('/core/partsmith_core.mjs') && url.search === '',
    (route) => route.fulfill({ contentType: 'text/javascript', body: STOPPING_CORE }),
  )
  const stopping = await openEngine(stoppingPage, enginePage.url)
  expect(await stopping.start()).toBe(1)
  await expect(
    stopping.call(1, { type: 'open_document', text: STOP_MARKER, assets: [] }),
  ).rejects.toThrow(
    'The web core stopped during open_document (request 1): RuntimeError: unreachable',
  )
  // A stopped instance counts as ended.
  await expect(stopping.call(1, { type: 'engine_info' })).rejects.toThrow(notRunning(1))
  expect(await stopping.start()).toBe(2)
  expect(ok(await stopping.call(2, { type: 'engine_info' }))).toMatchObject({
    kind: 'ok',
    type: 'engine_info',
  })
  await stopping.end(2)

  // The worker's own failures reach openEngine's rejection instead of its timeout.
  const throwingPage = await context.newPage()
  await throwingPage.route(
    (url) => url.pathname.endsWith(WORKER_SCRIPT),
    // The dev server's response with another body: a worker of a cross-origin isolated page needs its headers to load.
    async (route) => route.fulfill({ response: await route.fetch(), body: THROWING_WORKER }),
  )
  await expect(openEngine(throwingPage, enginePage.url)).rejects.toThrow(
    "The engine page's worker did not start: Uncaught Error: stop the worker",
  )
  const missingPage = await context.newPage()
  await missingPage.route((url) => url.pathname.endsWith(WORKER_SCRIPT), notFound)
  await expect(openEngine(missingPage, enginePage.url)).rejects.toThrow(
    "The engine page's worker did not start: its script did not load",
  )
})

test.describe('web-column budgets', () => {
  let rowsPage: Page | null = null
  let rowsEngine: Engine | null = null

  test.beforeAll(async ({ browser }) => {
    test.setTimeout(ROW_TIMEOUT_MILLISECONDS)
    rowsPage = await browser.newPage()
    const engine = await openEngine(rowsPage, enginePage.url)
    // Every row, a row selected alone with -g too, starts with the code it shares with opening the reference part
    // already optimized; code only one row runs warms up in that row's first samples (engine.md section 5).
    await warmUp(engine)
    rowsEngine = engine
  })

  test.afterAll(async () => {
    await rowsPage?.close()
  })

  /** The engine of the page the rows share, once `beforeAll` has opened and warmed it. */
  function rowsEngineOrThrow(): Engine {
    if (rowsEngine === null) {
      throw new Error('The engine page of the rows has not opened.')
    }
    return rowsEngine
  }

  test(`engine test 155: ${MEMORY_ROW.name} @perf`, async () => {
    test.setTimeout(ROW_TIMEOUT_MILLISECONDS)
    const bytes = await MEMORY_ROW.measure(rowsEngineOrThrow())
    test.info().annotations.push({
      type: 'perf',
      description: `${MEMORY_ROW.name}: ${String(bytes)} bytes (budget below ${String(MAXIMUM_MEMORY_BYTES)} bytes)`,
    })
    expect(bytes, MEMORY_ROW.name).toBeLessThan(MAXIMUM_MEMORY_BYTES)
  })

  for (const row of TIMED_ROWS) {
    test(`engine test 155: ${row.name} @perf`, async () => {
      test.setTimeout(ROW_TIMEOUT_MILLISECONDS)
      const samples = await row.measure(rowsEngineOrThrow())
      expect(samples, `${row.name} samples`).toHaveLength(SAMPLES)
      expectWithinBudgets(row.name, [
        { statistic: 'median', value: percentile(samples, 0.5), budget: row.budget },
      ])
    })
  }
})
