/// <reference lib="dom" />
import { expect, test, type Page } from '@playwright/test'
import { bodyRow, canvas, dropFile, fitView, inspector, openApp, runCommand, timelineItem, toScreen } from './helpers'
import { editSketchFromTimeline } from './support/sketch'

// Temporary probe of the fixer pass; deleted before the pass ends.

const OUT = 'test-results/fixer/'

async function open(page: Page, scheme: 'light' | 'dark', scale: number, width = 1280, height = 800): Promise<void> {
  await page.setViewportSize({ width, height })
  if (scale !== 1) {
    await page.addInitScript((value) => {
      localStorage.setItem('partsmith.settings.v1', JSON.stringify({ uiScale: value }))
    }, scale)
  }
  await page.emulateMedia({ colorScheme: scheme })
  await openApp(page)
}

async function park(page: Page): Promise<void> {
  const bar = await page.getByTestId('status-bar').boundingBox()
  if (bar !== null) await page.mouse.move(bar.x + 4, bar.y + bar.height / 2)
  await page.waitForTimeout(400)
}

async function shot(page: Page, name: string): Promise<void> {
  await page.evaluate(() => document.fonts.ready)
  await page.screenshot({ path: `${OUT}${name}.png` })
}

for (const scale of [1, 2]) {
  test(`empty ${scale}`, async ({ page }) => {
    await open(page, 'light', scale)
    await park(page)
    await shot(page, `empty-light-${scale}`)
  })
  test(`palette ${scale}`, async ({ page }) => {
    await open(page, 'light', scale)
    await page.keyboard.press('Control+k')
    await page.keyboard.type('exp')
    await page.waitForTimeout(300)
    await shot(page, `palette-light-${scale}`)
  })
  test(`sketch-pick ${scale}`, async ({ page }) => {
    await open(page, 'dark', scale)
    await page.getByTestId('empty-state').getByRole('button', { name: 'Sketch' }).click()
    await park(page)
    await shot(page, `sketch-pick-dark-${scale}`)
  })
  test(`move ${scale}`, async ({ page }) => {
    await open(page, 'light', scale)
    await dropFile(page, 'cube.ptsm')
    await runCommand(page, 'Move body')
    await park(page)
    await shot(page, `move-light-${scale}`)
  })
  test(`edit ${scale}`, async ({ page }) => {
    await open(page, 'dark', scale)
    await dropFile(page, 'cube.ptsm')
    await timelineItem(page, 'Extrude 1').getByRole('button', { name: 'Extrude 1' }).dblclick()
    await expect(inspector(page)).toHaveAttribute('data-mode', 'command')
    await park(page)
    await shot(page, `edit-dark-${scale}`)
  })
  test(`sketch ${scale}`, async ({ page }) => {
    await open(page, 'light', scale)
    await dropFile(page, 'rect-sketch.ptsm')
    await editSketchFromTimeline(page, 'Sketch 1')
    await park(page)
    await shot(page, `sketch-light-${scale}`)
  })
  test(`notification ${scale}`, async ({ page }) => {
    await open(page, 'dark', scale)
    await dropFile(page, 'fillet-error.ptsm')
    await park(page)
    await shot(page, `notification-dark-${scale}`)
  })
}

test('selection face', async ({ page }) => {
  await open(page, 'light', 1)
  await dropFile(page, 'cube.ptsm')
  await fitView(page, 'Front')
  const cube = toScreen(await canvas(page), { min: [-10, -10, 0], max: [10, 10, 20] }, 'Front')
  const center = cube.toPage([0, -10, 10])
  await page.mouse.click(center.x, center.y)
  await park(page)
  await shot(page, 'selection-light-1')
})

test('sketchfront', async ({ page }) => {
  await open(page, 'light', 1)
  await dropFile(page, 'revolve-sketch.ptsm')
  await timelineItem(page, 'Sketch 1').getByRole('button', { name: 'Sketch 1' }).dblclick()
  await park(page)
  await shot(page, 'sketchfront-light-1')
})

test('parameters panel', async ({ page }) => {
  await open(page, 'light', 1)
  await dropFile(page, 'cube.ptsm')
  await runCommand(page, 'Parameters')
  await bodyRow(page, 'Cube').click()
  await park(page)
  await shot(page, 'parameters-light-1')
})

test('tooltip', async ({ page }) => {
  await open(page, 'light', 1)
  await page.getByTestId('theme-toggle').hover()
  await expect(page.getByRole('tooltip')).toBeVisible()
  await shot(page, 'tooltip-light-1')
})
