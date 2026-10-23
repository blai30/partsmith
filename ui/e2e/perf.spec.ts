/// <reference lib="dom" />
import { expect, test, type Locator, type Page } from '@playwright/test'
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
import { serveTestPage } from './support/test-pages'

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

/**
 * The nearest-rank percentile of `values`, or NaN when there is none, so the test still reports
 * its other values and then fails this one's budget.
 */
function measuredPercentile(values: readonly number[], fraction: number): number {
  return values.length === 0 ? Number.NaN : percentile(values, fraction)
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
  expect.soft(drags.length, 'ps:sketch-drag measures during the drag').toBeGreaterThan(0)
  expectWithinBudgets('P-2', [
    {
      statistic: 'ps:sketch-drag 95th percentile',
      value: measuredPercentile(drags, 0.95),
      budget: 16,
    },
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
    { statistic: 'ps:preview median', value: measuredPercentile(previews, 0.5), budget: 250 },
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
  // Opening dense.ptsm takes about 25 s; without a GPU each frame of the orbit takes about a second.
  test.setTimeout(600_000)
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
  // A 3 s orbit across the canvas center (Shift + middle drag, 3.13): 300 pixels, 120 degrees, in
  // 180 pointer moves, one per frame at 60 frames a second.
  await movePaced(page, pointsAlong(from, { x: box.cx + 150, y: box.cy }, 180), 3_000)
  await page.mouse.up({ button: 'middle' })
  await page.keyboard.up('Shift')
  const frames = await measures(page, 'ps:frame', since)
  const tasks = await longTasks(page, since)
  expect.soft(frames.length, 'ps:frame measures during the orbit').toBeGreaterThan(0)
  expectWithinBudgets('P-6', [
    {
      statistic: 'ps:frame 95th percentile',
      value: measuredPercentile(frames, 0.95),
      budget: 16.7,
    },
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
    { statistic: 'ps:pick 95th percentile', value: measuredPercentile(picks, 0.95), budget: 1 },
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
    { statistic: 'ps:palette 95th percentile', value: measuredPercentile(ranks, 0.95), budget: 8 },
  ])
})

test('P-9 timeline drop check over 300 features: 95th percentile of ps:drop-check at most 1 ms @perf', async ({
  page,
}) => {
  test.setTimeout(240_000)
  await openApp(page)
  await dropFile(page, 'part-300.ptsm')
  const items = page.getByTestId('timeline-item')
  await expect(items).toHaveCount(300)
  // A drag asks for list_uses when it starts (3.9); opening an item's menu asks the same and shows
  // "Used by" once they are known, so every drop check of the drag runs over the 300 features' uses.
  await timelineItem(page, 'Extrude 1')
    .getByRole('button', { name: 'Extrude 1', exact: true })
    .click({ button: 'right' })
  await expect(
    page.getByTestId('context-menu').getByRole('menuitem', { name: /^Used by / }),
  ).toBeVisible()
  await page.keyboard.press('Escape')
  await expect(page.getByTestId('context-menu')).toBeHidden()

  const last = items.last()
  await last.scrollIntoViewIfNeeded()
  const from = await last.boundingBox()
  const strip = await page
    .getByTestId('timeline')
    .getByRole('toolbar', { name: 'Features' })
    .boundingBox()
  if (from === null || strip === null) {
    throw new Error('The timeline strip or its last item is not visible.')
  }
  const start = { x: from.x + from.width / 2, y: from.y + from.height / 2 }
  await page.mouse.move(start.x, start.y)
  const since = await pageTime(page)
  await page.mouse.down()
  // The last feature dragged to the start of the strip in 120 pointer moves, over every gap shown.
  await page.mouse.move(strip.x + 4, start.y, { steps: 120 })
  // Esc cancels the drag (3.9), so the drop rebuilds nothing.
  await page.keyboard.press('Escape')
  await page.mouse.up()
  const checks = await measures(page, 'ps:drop-check', since)
  expect.soft(checks.length, 'ps:drop-check measures during the drag').toBeGreaterThan(0)
  expectWithinBudgets('P-9', [
    {
      statistic: 'ps:drop-check 95th percentile',
      value: measuredPercentile(checks, 0.95),
      budget: 1,
    },
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
    {
      statistic: 'ps:apply-events 95th percentile',
      value: measuredPercentile(applied, 0.95),
      budget: 8,
    },
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
    { statistic: 'ps:expression median', value: measuredPercentile(validations, 0.5), budget: 50 },
  ])
})

test.describe('worker transfer', () => {
  /** The bytes of the one buffer of each transferred frame. */
  const BUFFER_BYTES = 50_000_000
  const TRANSFER_COUNT = 20

  const transferPage = serveTestPage('transfer.html')

  test('P-12 worker transfer of a 50 MB response frame: median at most 2 ms, every sent buffer detached @perf', async ({
    page,
  }) => {
    test.setTimeout(120_000)
    await page.goto(transferPage.url)
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
        value: measuredPercentile(
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
