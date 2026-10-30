/// <reference lib="dom" />
import { expect, test, type Page, type Worker } from '@playwright/test'
import { dropFile, openApp, reportedViolations, runCommand } from './helpers'
import { chooseContext, expectCurrentReport, openPrintCheck } from './support/print'
import { editSketchFromTimeline } from './support/sketch'

// Content security policy (ui.md 6.5 E-161, delivery.md 2.7, ADR 0049): the page and its core
// worker report no violation from their start, and both enforce the policy. It carries no @engines
// tag, so it runs in Chromium alone (delivery.md open issue 19). Its `test` is Playwright's, not the
// one of helpers.ts, whose check of the page at the end would fail on the refusals made on purpose.

/** The path of the core worker's script (delivery.md 2.6). */
const CORE_WORKER_PATH = /^\/assets\/core\.worker-[^/]*\.js$/

/** Where the test loads from on purpose, so the policy has something to refuse. */
const ELSEWHERE = 'https://example.com/'

/** The one worker of the page that runs the core. */
function coreWorker(page: Page): Worker {
  const workers = page
    .workers()
    .filter((worker) => CORE_WORKER_PATH.test(new URL(worker.url()).pathname))
  expect(workers.map((worker) => worker.url())).toHaveLength(1)
  const [worker] = workers
  if (worker === undefined) {
    throw new Error('The page has no core worker.')
  }
  return worker
}

test.describe('content security policy (delivery.md 2.7)', () => {
  test('E-161 no content security policy violation, in the page or in the core worker', async ({
    page,
  }) => {
    // A refusal that stops the core while its worker starts (its module or its WebAssembly) fails
    // here with the core's fatal text; any other refusal is in the reports read at the end.
    await openApp(page)
    const worker = coreWorker(page)

    await dropFile(page, 'cube.ptsm')
    // Entering the sketch loads the sketch overlay's chunk (3.1).
    await editSketchFromTimeline(page, 'Sketch 1')
    await page.keyboard.press('ControlOrMeta+Enter')
    await expect(page.getByTestId('app')).toHaveAttribute('data-context', 'model')
    await chooseContext(page, 'Print')
    await expectCurrentReport(page)
    await openPrintCheck(page)
    await chooseContext(page, 'Model')
    await page.getByTestId('theme-toggle').click()
    await runCommand(page, 'Parameters')
    await expect(page.getByTestId('side-panel')).toBeVisible()

    // Last, two loads the policy refuses: they show that the reports are read and that the page
    // and the worker both enforce the policy. Reports arrive as queued tasks, so polling until
    // these two arrive also lets every earlier one arrive.
    await page.evaluate((source) => {
      const image = document.createElement('img')
      image.src = `${source}blocked.png`
      document.body.append(image)
    }, ELSEWHERE)
    await worker.evaluate((source) => {
      fetch(`${source}blocked`).catch(() => undefined)
    }, ELSEWHERE)
    await expect
      .poll(() => page.evaluate(reportedViolations), { message: 'the violations of the page' })
      .toEqual([`img-src ${ELSEWHERE}blocked.png`])
    await expect
      .poll(() => worker.evaluate(reportedViolations), {
        message: 'the violations of the core worker',
      })
      .toEqual([`connect-src ${ELSEWHERE}blocked`])
  })
})
