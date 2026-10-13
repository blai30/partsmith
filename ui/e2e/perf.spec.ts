/// <reference lib="dom" />
import { fileURLToPath } from 'node:url'
import { expect, test, type Locator, type Page } from '@playwright/test'
import { createServer, type ViteDevServer } from 'vite'
import {
  canvas,
  dropFile,
  expectHover,
  expectReady,
  inspector,
  inspectorInput,
  longTasks,
  measures,
  openApp,
  pageTime,
  percentile,
  runCommand,
  sketchToScreen,
  timelineItem,
  viewport,
  watchLongTasks,
  type PagePoint,
} from './helpers'
import type { measureTransfers } from './pages/transfer-harness'
import { wheelAt } from './support/print'
import { editSketchFromTimeline } from './support/sketch'

// The performance tests of ui.md 6.7, tagged @perf and run only with PARTSMITH_PERF=1, on the
// reference machine before a release until the fixed hardware runner exists (delivery.md 1.2).
// Each test reports every measured value as a `perf` annotation before it holds the value to its
// budget, so a run reports its numbers whether it passes or fails.

// One test at a time: tests running side by side would slow each other down.
test.describe.configure({ mode: 'default' })

/** The longest main-thread task allowed during orbit, drag or preview typing (ui.md 5). */
const LONG_TASK_BUDGET_MILLISECONDS = 50

/** How long a rebuild of part-50.ptsm may take before a test stops waiting for it. */
const REBUILD_TIMEOUT_MILLISECONDS = 30_000

/**
 * How long one hover's pick of dense.ptsm may take before P-7 stops waiting for it: without a GPU
 * the pick takes seconds, so the default expect timeout would end the run before it reports.
 */
const PICK_TIMEOUT_MILLISECONDS = 30_000

/** The time between pointer moves of a timed gesture: one per frame at 60 frames a second. */
const MOVE_INTERVAL_MILLISECONDS = 1_000 / 60

/** A measured value and the budget it is held to. */
type Measured = { readonly statistic: string; readonly value: number; readonly budget: number }

/** A value in milliseconds rounded to 0.01 ms, as the annotations write it. */
function formatMilliseconds(value: number): string {
  return String(Math.round(value * 100) / 100)
}

/**
 * Adds one `perf` annotation per measured value, written `{id} {statistic}: {value} ms (budget
 * {budget} ms)`, then holds every value to its budget (ui.md 6.7).
 */
function expectWithinBudgets(id: string, measured: readonly Measured[]): void {
  for (const { statistic, value, budget } of measured) {
    test.info().annotations.push({
      type: 'perf',
      description: `${id} ${statistic}: ${formatMilliseconds(value)} ms (budget ${String(budget)} ms)`,
    })
  }
  for (const { statistic, value, budget } of measured) {
    expect.soft(value, `${id} ${statistic}`).toBeLessThanOrEqual(budget)
  }
}

/** The longest of the long tasks, 0 when there was none, held to the 50 ms budget. */
function longestLongTask(durations: readonly number[]): Measured {
  return {
    statistic: 'longest long task',
    value: Math.max(0, ...durations),
    budget: LONG_TASK_BUDGET_MILLISECONDS,
  }
}

/** Waits until the test's clock (`performance.now()` in Node) reaches `due`. */
async function waitUntil(page: Page, due: number): Promise<void> {
  const remaining = due - performance.now()
  if (remaining > 0) {
    await page.waitForTimeout(remaining)
  }
}

/** `count` points evenly along the segment from `from` (excluded) to `to` (included). */
function pointsAlong(from: PagePoint, to: PagePoint, count: number): PagePoint[] {
  return Array.from({ length: count }, (_, i) => ({
    x: from.x + ((to.x - from.x) * (i + 1)) / count,
    y: from.y + ((to.y - from.y) * (i + 1)) / count,
  }))
}

/** Moves the mouse through `path` at an even pace, the last move `duration` ms after the start. */
async function movePaced(page: Page, path: readonly PagePoint[], duration: number): Promise<void> {
  const start = performance.now()
  for (const [index, point] of path.entries()) {
    // The moves are paced: each waits for its time, then moves.
    // oxlint-disable-next-line no-await-in-loop
    await waitUntil(page, start + (duration * (index + 1)) / path.length)
    // oxlint-disable-next-line no-await-in-loop
    await page.mouse.move(point.x, point.y)
  }
}

/**
 * Moves the mouse from `from` to `to` at an even speed for `duration` ms, one move per
 * `MOVE_INTERVAL_MILLISECONDS` or as fast as the page takes them, the last one at `to`.
 */
async function moveFor(
  page: Page,
  from: PagePoint,
  to: PagePoint,
  duration: number,
): Promise<void> {
  const start = performance.now()
  let fraction = 0
  for (let i = 1; fraction < 1; i++) {
    // The moves are paced: each waits for its time, then moves to where the gesture is by then.
    // oxlint-disable-next-line no-await-in-loop
    await waitUntil(page, start + i * MOVE_INTERVAL_MILLISECONDS)
    fraction = Math.min((performance.now() - start) / duration, 1)
    // oxlint-disable-next-line no-await-in-loop
    await page.mouse.move(from.x + (to.x - from.x) * fraction, from.y + (to.y - from.y) * fraction)
  }
}

/** How many entries named `name` started at or after `since`. */
async function measureCount(page: Page, name: string, since: number): Promise<number> {
  return (await measures(page, name, since)).length
}

/** Opens the Parameters panel and returns the expression input of the parameter `name` (3.11). */
async function parameterExpression(page: Page, name: string): Promise<Locator> {
  await runCommand(page, 'Parameters')
  const expression = page
    .getByTestId('parameters-panel')
    .getByRole('combobox', { name: `Expression of ${name}`, exact: true })
  await expect(expression).toBeVisible()
  return expression
}

/** The time of the `ps:usable` mark after `performance.timeOrigin`, or null before it is marked. */
function usableTime(page: Page): Promise<number | null> {
  return page.evaluate(
    () => performance.getEntriesByName('ps:usable', 'mark').at(0)?.startTime ?? null,
  )
}

test('P-1 cold start: ps:usable at most 4,000 ms after timeOrigin on a reload @perf', async ({
  page,
}) => {
  await openApp(page)
  await page.reload()
  await expectReady(page)
  await expect.poll(() => usableTime(page)).not.toBeNull()
  const usable = (await usableTime(page)) ?? Number.NaN
  expectWithinBudgets('P-1', [{ statistic: 'ps:usable', value: usable, budget: 4_000 }])
})

test('P-2 sketch drag, 500 constraints: 95th percentile of ps:sketch-drag at most 16 ms, no long task over 50 ms @perf', async ({
  page,
}) => {
  test.setTimeout(120_000)
  await watchLongTasks(page)
  await openApp(page)
  await dropFile(page, 'sketch-500.ptsm')
  await editSketchFromTimeline(page, 'Sketch 1')
  const fitted = sketchToScreen(await canvas(page), [0, 0], [500, 500]).toPage([500, 500])
  // At the fitted view the constraint glyphs cover the drawing, e-drag.end among it. Ten wheel
  // notches forward at a free spot 60 pixels to its right zoom in by 1.0015^1000 about that spot
  // (2.11 zoomAt, 3.13), which stays in place while the point moves away from it.
  const cursor = { x: fitted.x + 60, y: fitted.y }
  await wheelAt(page, cursor, -100, 10)
  const start = { x: cursor.x + (fitted.x - cursor.x) * 1.0015 ** 1_000, y: fitted.y }
  await expectHover(page, start.x, start.y, 'sketch_point')
  await expect(viewport(page)).toHaveAttribute('data-hover-key', 'point:f-sk1:e-drag.end')
  const since = await pageTime(page)
  await page.mouse.down()
  // A 2 s path of 120 pointer moves, 240 pixels to the left into free space.
  await movePaced(page, pointsAlong(start, { x: start.x - 240, y: start.y }, 120), 2_000)
  await page.mouse.up()
  const drags = await measures(page, 'ps:sketch-drag', since)
  const tasks = await longTasks(page, since)
  expect(drags.length, 'ps:sketch-drag measures during the drag').toBeGreaterThan(0)
  expectWithinBudgets('P-2', [
    { statistic: 'ps:sketch-drag 95th percentile', value: percentile(drags, 0.95), budget: 16 },
    longestLongTask(tasks),
  ])
})

test('P-3 feature preview: median ps:preview at most 250 ms over 20 distances, no long task over 50 ms @perf', async ({
  page,
}) => {
  test.setTimeout(120_000)
  await watchLongTasks(page)
  await openApp(page)
  await dropFile(page, 'part-50.ptsm')
  await timelineItem(page, 'Extrude 1')
    .getByRole('button', { name: 'Extrude 1', exact: true })
    .dblclick()
  await expect(inspector(page)).toHaveAccessibleName('Extrude 1')
  const distance = inspectorInput(page, 'Distance')
  await expect(distance).toHaveValue('8 mm')
  await expect(inspector(page)).not.toHaveAttribute('data-preview', 'pending')
  const since = await pageTime(page)
  let lastChange = since
  const start = performance.now()
  for (let i = 0; i < 20; i++) {
    // One distance every 300 ms: 8.1 mm, 8.2 mm, ... 10.0 mm.
    // oxlint-disable-next-line no-await-in-loop
    await waitUntil(page, start + i * 300)
    // oxlint-disable-next-line no-await-in-loop
    lastChange = await pageTime(page)
    // oxlint-disable-next-line no-await-in-loop
    await distance.fill(`${(8 + (i + 1) / 10).toFixed(1)} mm`)
  }
  await expect(inspector(page)).toHaveAttribute('data-preview', 'shown', { timeout: 15_000 })
  // The last change is never superseded, so its preview is measured.
  await expect.poll(() => measureCount(page, 'ps:preview', lastChange)).toBeGreaterThan(0)
  const previews = await measures(page, 'ps:preview', since)
  const tasks = await longTasks(page, since)
  expectWithinBudgets('P-3', [
    { statistic: 'ps:preview median', value: percentile(previews, 0.5), budget: 250 },
    longestLongTask(tasks),
  ])
})

test('P-4 full rebuild of 50 features: ps:rebuild at most 3,000 ms @perf', async ({ page }) => {
  test.setTimeout(60_000)
  await openApp(page)
  await dropFile(page, 'part-50.ptsm')
  const base = await parameterExpression(page, 'base')
  await base.fill('250 mm')
  const since = await pageTime(page)
  await base.press('Enter')
  await expect
    .poll(() => measureCount(page, 'ps:rebuild', since), { timeout: REBUILD_TIMEOUT_MILLISECONDS })
    .toBeGreaterThan(0)
  const [rebuild] = await measures(page, 'ps:rebuild', since)
  expectWithinBudgets('P-4', [
    { statistic: 'ps:rebuild', value: rebuild ?? Number.NaN, budget: 3_000 },
  ])
})

test('P-5 warm open of 50 features: the second ps:open at most 1,000 ms @perf', async ({
  page,
}) => {
  test.setTimeout(60_000)
  await openApp(page)
  await dropFile(page, 'part-50.ptsm')
  await runCommand(page, 'New')
  await expect(page.getByTestId('document-name')).not.toHaveText('part-50')
  const since = await pageTime(page)
  await dropFile(page, 'part-50.ptsm')
  const [open] = await measures(page, 'ps:open', since)
  expectWithinBudgets('P-5', [{ statistic: 'ps:open', value: open ?? Number.NaN, budget: 1_000 }])
})

test('P-6 orbit, 2 M triangles: 95th percentile of ps:frame at most 16.7 ms, no long task over 50 ms @perf', async ({
  page,
}) => {
  test.setTimeout(180_000)
  await watchLongTasks(page)
  await openApp(page)
  await dropFile(page, 'dense.ptsm')
  await page.getByRole('button', { name: 'Home view', exact: true }).click()
  await expect(viewport(page)).toHaveAttribute('data-view', 'iso')
  const box = await canvas(page)
  const from = { x: box.cx - 150, y: box.cy }
  await page.mouse.move(from.x, from.y)
  const since = await pageTime(page)
  await page.keyboard.down('Shift')
  await page.mouse.down({ button: 'middle' })
  // A 3 s orbit across the canvas center (Shift + middle drag, 3.13): 300 pixels, 120 degrees.
  await moveFor(page, from, { x: box.cx + 150, y: box.cy }, 3_000)
  await page.mouse.up({ button: 'middle' })
  await page.keyboard.up('Shift')
  const frames = await measures(page, 'ps:frame', since)
  const tasks = await longTasks(page, since)
  expect(frames.length, 'ps:frame measures during the orbit').toBeGreaterThan(0)
  expectWithinBudgets('P-6', [
    { statistic: 'ps:frame 95th percentile', value: percentile(frames, 0.95), budget: 16.7 },
    longestLongTask(tasks),
  ])
})

test('P-7 hover picks, 2 M triangles: 95th percentile of ps:pick at most 1 ms @perf', async ({
  page,
}) => {
  // Opening dense.ptsm takes about 25 s; without a GPU each hover's pick takes about a second.
  test.setTimeout(600_000)
  await openApp(page)
  await dropFile(page, 'dense.ptsm')
  const box = await canvas(page)
  const since = await pageTime(page)
  for (let i = 0; i < 100; i++) {
    // A 10 by 10 grid over the middle of the canvas, where the model is; the camera stays still.
    const column = i % 10
    const row = Math.floor(i / 10)
    // oxlint-disable-next-line no-await-in-loop
    await page.mouse.move(
      box.cx + (column - 4.5) * 0.06 * box.width,
      box.cy + (row - 4.5) * 0.06 * box.height,
    )
    // oxlint-disable-next-line no-await-in-loop
    await expect
      .poll(() => measureCount(page, 'ps:pick', since), { timeout: PICK_TIMEOUT_MILLISECONDS })
      .toBeGreaterThan(i)
  }
  const picks = await measures(page, 'ps:pick', since)
  expectWithinBudgets('P-7', [
    { statistic: 'ps:pick 95th percentile', value: percentile(picks, 0.95), budget: 1 },
  ])
})

test('P-8 palette re-rank, 2,000 items: 95th percentile of ps:palette at most 8 ms @perf', async ({
  page,
}) => {
  test.setTimeout(60_000)
  await openApp(page)
  await dropFile(page, 'palette-2000.ptsm')
  await page.keyboard.press('ControlOrMeta+K')
  await expect(page.getByTestId('palette-input')).toBeFocused()
  const since = await pageTime(page)
  for (const [index, character] of [...'parameters'].entries()) {
    // oxlint-disable-next-line no-await-in-loop
    await page.keyboard.press(character)
    // oxlint-disable-next-line no-await-in-loop
    await expect.poll(() => measureCount(page, 'ps:palette', since)).toBeGreaterThan(index)
  }
  const ranks = await measures(page, 'ps:palette', since)
  expectWithinBudgets('P-8', [
    { statistic: 'ps:palette 95th percentile', value: percentile(ranks, 0.95), budget: 8 },
  ])
})

test('P-10 applying document_changed plus recomputed for 50 features: 95th percentile of ps:apply-events at most 8 ms @perf', async ({
  page,
}) => {
  test.setTimeout(240_000)
  await openApp(page)
  await dropFile(page, 'part-50.ptsm')
  const base = await parameterExpression(page, 'base')
  const since = await pageTime(page)
  for (let i = 0; i < 20; i++) {
    // base is 240 mm, so every set is a change: 250 mm, 240 mm, 250 mm, ...
    // oxlint-disable-next-line no-await-in-loop
    await base.fill(i % 2 === 0 ? '250 mm' : '240 mm')
    // oxlint-disable-next-line no-await-in-loop
    await base.press('Enter')
    // oxlint-disable-next-line no-await-in-loop
    await expect
      .poll(() => measureCount(page, 'ps:rebuild', since), {
        timeout: REBUILD_TIMEOUT_MILLISECONDS,
      })
      .toBeGreaterThan(i)
  }
  const applied = await measures(page, 'ps:apply-events', since)
  expect.soft(applied, 'one ps:apply-events measure per change').toHaveLength(20)
  expectWithinBudgets('P-10', [
    { statistic: 'ps:apply-events 95th percentile', value: percentile(applied, 0.95), budget: 8 },
  ])
})

test('P-11 expression validation round trip: median ps:expression at most 50 ms @perf', async ({
  page,
}) => {
  test.setTimeout(60_000)
  await openApp(page)
  await dropFile(page, 'part-50.ptsm')
  const base = await parameterExpression(page, 'base')
  await base.selectText()
  const since = await pageTime(page)
  for (let i = 0; i < 20; i++) {
    // oxlint-disable-next-line no-await-in-loop
    await page.keyboard.press('2')
    // oxlint-disable-next-line no-await-in-loop
    await expect.poll(() => measureCount(page, 'ps:expression', since)).toBeGreaterThan(i)
  }
  await expect(base).toHaveValue('2'.repeat(20))
  const validations = await measures(page, 'ps:expression', since)
  expect.soft(validations, 'one ps:expression measure per keystroke').toHaveLength(20)
  expectWithinBudgets('P-11', [
    { statistic: 'ps:expression median', value: percentile(validations, 0.5), budget: 50 },
  ])
})

test.describe('worker transfer', () => {
  /** The bytes of the one buffer of each transferred frame. */
  const BUFFER_BYTES = 50_000_000
  const TRANSFER_COUNT = 20

  const uiDirectory = fileURLToPath(new URL('..', import.meta.url))
  let server: ViteDevServer | null = null
  let pageUrl = ''

  // The production preview serves only the app, so P-12 serves its page from a Vite dev server.
  test.beforeAll(async () => {
    server = await createServer({
      root: uiDirectory,
      logLevel: 'error',
      clearScreen: false,
      server: { port: 0, hmr: false },
      optimizeDeps: { entries: ['e2e/pages/transfer.html'] },
    })
    await server.listen()
    const base = server.resolvedUrls?.local[0]
    if (base === undefined) {
      throw new Error('The Vite dev server reported no local URL.')
    }
    pageUrl = new URL('e2e/pages/transfer.html', base).href
  })

  test.afterAll(async () => {
    await server?.close()
  })

  test('P-12 worker transfer of a 50 MB response frame: median at most 2 ms, every sent buffer detached @perf', async ({
    page,
  }) => {
    test.setTimeout(120_000)
    await page.goto(pageUrl)
    await expect(page.locator('body')).toHaveAttribute('data-ready', 'true', { timeout: 15_000 })
    const transfers = await page.evaluate(
      ({ count, bytes }) => {
        const harness = Reflect.get(window, 'partsmithTransfer') as {
          readonly measureTransfers: typeof measureTransfers
        }
        return harness.measureTransfers(count, bytes)
      },
      { count: TRANSFER_COUNT, bytes: BUFFER_BYTES },
    )
    expect(transfers).toHaveLength(TRANSFER_COUNT)
    expectWithinBudgets('P-12', [
      {
        statistic: 'transfer median',
        value: percentile(
          transfers.map((transfer) => transfer.milliseconds),
          0.5,
        ),
        budget: 2,
      },
    ])
    for (const [index, transfer] of transfers.entries()) {
      expect.soft(transfer.sentDetached, `frame ${String(index + 1)} detached`).toBe(true)
      expect.soft(transfer.sentByteLength, `frame ${String(index + 1)} byteLength`).toBe(0)
      expect.soft(transfer.messages).toEqual([
        {
          kind: 'response',
          id: index + 1,
          result: { kind: 'ok', type: 'undo', revision: 1, label: 'Edit' },
        },
      ])
      expect.soft(transfer.bufferCount).toBe(1)
      expect.soft(transfer.bufferType).toBe('Uint8Array')
      expect.soft(transfer.bufferLength).toBe(BUFFER_BYTES)
    }
  })
})
