/// <reference lib="dom" />
import { expect, type Locator, type Page } from '@playwright/test'
import {
  bodyRow,
  boxOf,
  canvas,
  dropFile,
  expectHover,
  fitView,
  openApp,
  runCommand,
  test,
  viewport,
  type Box,
} from './helpers'

// Layout (ui.md 6.5 E-105 to E-107 and E-133 to E-138, E-141): boxes measured in the real
// browser, because jsdom has no layout. Each test names the section whose layout it holds.

/** Sub-pixel rounding of rendered boxes. */
const TOLERANCE_PIXELS = 0.5

/** The UI scales of the settings (2.16.9) that these tests check. */
const SCALES = [1, 2] as const

/** Opens the app at a UI scale, stored in the settings before the first load (2.15). */
async function openAtScale(page: Page, uiScale: number): Promise<void> {
  await page.addInitScript((scale) => {
    localStorage.setItem('partsmith.settings.v1', JSON.stringify({ uiScale: scale }))
  }, uiScale)
  await openApp(page)
  // The UI scale sets the root font size as a percentage of the browser's 16 pixels (2.16.9).
  await expect
    .poll(() => page.evaluate(() => getComputedStyle(document.documentElement).fontSize))
    .toBe(`${String(16 * uiScale)}px`)
  await page.evaluate(() => document.fonts.ready)
}

/** Expects `inner` to lie within `outer`, up to sub-pixel rounding. */
function expectInside(inner: Box, outer: Box, what: string): void {
  expect(inner.x, `${what}: left`).toBeGreaterThanOrEqual(outer.x - TOLERANCE_PIXELS)
  expect(inner.y, `${what}: top`).toBeGreaterThanOrEqual(outer.y - TOLERANCE_PIXELS)
  expect(inner.x + inner.width, `${what}: right`).toBeLessThanOrEqual(
    outer.x + outer.width + TOLERANCE_PIXELS,
  )
  expect(inner.y + inner.height, `${what}: bottom`).toBeLessThanOrEqual(
    outer.y + outer.height + TOLERANCE_PIXELS,
  )
}

/** Expects a box to be (x, y, width, height), each within sub-pixel rounding. */
function expectBox(
  actual: Box,
  expected: readonly [number, number, number, number],
  what: string,
): void {
  const [x, y, width, height] = expected
  expect(Math.abs(actual.x - x), `${what}: x ${String(actual.x)}`).toBeLessThanOrEqual(
    TOLERANCE_PIXELS,
  )
  expect(Math.abs(actual.y - y), `${what}: y ${String(actual.y)}`).toBeLessThanOrEqual(
    TOLERANCE_PIXELS,
  )
  expect(
    Math.abs(actual.width - width),
    `${what}: width ${String(actual.width)}`,
  ).toBeLessThanOrEqual(TOLERANCE_PIXELS)
  expect(
    Math.abs(actual.height - height),
    `${what}: height ${String(actual.height)}`,
  ).toBeLessThanOrEqual(TOLERANCE_PIXELS)
}

/** Whether two boxes overlap by more than sub-pixel rounding. */
function intersects(first: Box, second: Box): boolean {
  return (
    first.x + first.width > second.x + TOLERANCE_PIXELS &&
    second.x + second.width > first.x + TOLERANCE_PIXELS &&
    first.y + first.height > second.y + TOLERANCE_PIXELS &&
    second.y + second.height > first.y + TOLERANCE_PIXELS
  )
}

/** A computed style property of an element. */
function computed(locator: Locator, property: string): Promise<string> {
  return locator.evaluate(
    (element, name) => getComputedStyle(element).getPropertyValue(name),
    property,
  )
}

/**
 * The page boxes of the view cube's 8 corner buttons as the camera turns it, its 3D transform
 * applied (3.14). Each holds one corner of the cube, and the cube's outline is the hull of its
 * corners, so the cube is inside a rectangle exactly when these boxes are.
 */
async function viewCubeCornerBoxes(page: Page): Promise<Box[]> {
  const corners = page
    .getByRole('group', { name: 'View cube' })
    .getByRole('button', { name: /corner view$/ })
  await expect(corners).toHaveCount(8)
  return Promise.all((await corners.all()).map((corner) => boxOf(corner)))
}

/** The segmented choices and selects of the inspector, each with its field's path. */
function inspectorChoices(page: Page): Locator {
  return page
    .getByTestId('inspector')
    .getByTestId('inspector-field')
    .filter({ has: page.locator('[role=radiogroup], select') })
}

/** Expects every choice of the inspector to lie within its field (3.7). */
async function expectChoicesInsideFields(page: Page): Promise<void> {
  const fields = inspectorChoices(page)
  await expect(fields).not.toHaveCount(0)
  await Promise.all(
    (await fields.all()).map(async (field) => {
      const path = (await field.getAttribute('data-path')) ?? ''
      const control = field.locator('[role=radiogroup], select').first()
      expectInside(await boxOf(control), await boxOf(field), `the ${path} choice`)
    }),
  )
}

/** Clicks Sketch in the empty state and expects the plane-picking header clear of the corners. */
async function expectHeaderClear(page: Page): Promise<Box> {
  await page.getByTestId('empty-state').getByRole('button', { name: 'Sketch', exact: true }).click()
  const header = page.getByTestId('sketch-header')
  await expect(header).toHaveAttribute('data-state', 'picking')
  const [area, headerBox, controlsBox] = await Promise.all([
    boxOf(viewport(page)),
    boxOf(header),
    boxOf(page.getByTestId('viewport-controls')),
  ])
  expectInside(headerBox, area, 'the sketch header')
  expect(intersects(headerBox, controlsBox), 'the header and the controls').toBe(false)
  for (const corner of await viewCubeCornerBoxes(page)) {
    expect(intersects(headerBox, corner), 'the header and a cube corner').toBe(false)
  }
  const home = await boxOf(page.getByRole('button', { name: 'Home view', exact: true }))
  expect(intersects(headerBox, home), 'the header and Home view').toBe(false)
  // Level with Home view, the header keeps 0.5rem from it, so the two chips never touch (3.2).
  if (headerBox.y < home.y + home.height) {
    const rem = await page.evaluate(() =>
      Number.parseFloat(getComputedStyle(document.documentElement).fontSize),
    )
    expect(home.x - (headerBox.x + headerBox.width), 'the gap to Home view').toBeGreaterThanOrEqual(
      0.5 * rem - TOLERANCE_PIXELS,
    )
  }
  return { ...headerBox, y: headerBox.y - area.y }
}

test.describe('layout', () => {
  for (const scale of SCALES) {
    test(`E-105 the view cube keeps every corner inside the viewport in the iso view at ${String(scale * 100)}% UI scale (3.14)`, async ({
      page,
    }) => {
      await openAtScale(page, scale)
      await expect(viewport(page)).toHaveAttribute('data-view', 'iso')
      const bounds = await boxOf(viewport(page))
      for (const corner of await viewCubeCornerBoxes(page)) {
        expectInside(corner, bounds, 'a view cube corner')
      }
    })
  }

  test('E-106 the Extrude inspector clips no choice, and choosing an option keeps the control (3.7)', async ({
    page,
  }) => {
    await openApp(page)
    await dropFile(page, 'cube.ptsm')
    await page.evaluate(() => document.fonts.ready)
    await page.keyboard.press('e')
    const inspector = page.getByTestId('inspector')
    await expect(inspector).toHaveAccessibleName('Extrude')
    await expectChoicesInsideFields(page)
    const operation = inspector.getByRole('radiogroup', { name: 'Operation', exact: true })
    // Each choice draws its label bold, so each is checked and measured in turn.
    for (const name of ['Intersect', 'Cut', 'Join', 'New body']) {
      const radio = operation.getByRole('radio', { name, exact: true })
      // oxlint-disable-next-line no-await-in-loop
      await radio.check()
      // oxlint-disable-next-line no-await-in-loop
      await expect(radio).toBeChecked()
      // oxlint-disable-next-line no-await-in-loop
      await expectChoicesInsideFields(page)
    }
  })

  test('E-107 at 200% UI scale the plane-picking header keeps its prompt and plane buttons inside the viewport (3.16.1)', async ({
    page,
  }) => {
    await openAtScale(page, 2)
    await page
      .getByTestId('empty-state')
      .getByRole('button', { name: 'Sketch', exact: true })
      .click()
    const header = page.getByTestId('sketch-header')
    await expect(header).toHaveAttribute('data-state', 'picking')
    const bounds = await boxOf(page.getByTestId('viewport'))
    expectInside(await boxOf(header), bounds, 'the sketch header')
    expectInside(
      await boxOf(header.getByText('Pick a plane or a flat face to draw on')),
      bounds,
      'the prompt',
    )
    await Promise.all(
      ['Top plane (XY)', 'Front plane (XZ)', 'Right plane (YZ)'].map(async (plane) => {
        const button = header.getByRole('button', { name: `Sketch on ${plane}`, exact: true })
        expectInside(await boxOf(button), bounds, `the ${plane} button`)
      }),
    )
  })

  test('E-133 islands float over a window-filling canvas at 1280 by 720 and 100% (3.2)', async ({
    page,
  }) => {
    await openApp(page)
    expectBox(await boxOf(viewport(page).locator('canvas')), [0, 0, 1280, 720], 'the canvas')
    expectBox(await boxOf(viewport(page)), [280, 64, 992, 548], 'the free area')
    const islands = [
      ['top-bar', [8, 8, 1264, 48], '12px'],
      ['browser', [8, 64, 264, 548], '12px'],
      ['timeline', [8, 620, 1264, 56], '12px'],
      ['status-bar', [8, 684, 1264, 28], '8px'],
    ] as const
    for (const [id, box, radius] of islands) {
      const island = page.getByTestId(id)
      // oxlint-disable-next-line no-await-in-loop
      expectBox(await boxOf(island), box, id)
      // oxlint-disable-next-line no-await-in-loop
      expect(await computed(island, 'border-top-left-radius'), `${id} radius`).toBe(radius)
      // oxlint-disable-next-line no-await-in-loop
      expect(await computed(island, 'box-shadow'), `${id} shadow`).not.toBe('none')
    }
    await runCommand(page, 'Parameters')
    expectBox(await boxOf(page.getByTestId('side-panel')), [912, 64, 360, 548], 'the side panel')
    expectBox(await boxOf(viewport(page)), [280, 64, 624, 548], 'the free area beside both')
    await page.getByRole('button', { name: 'Toggle browser' }).click()
    expectBox(await boxOf(viewport(page)), [8, 64, 896, 548], 'the free area without the browser')
    await page.getByRole('button', { name: 'Collapse timeline' }).click()
    const timeline = page.getByTestId('timeline')
    expectBox(await boxOf(timeline), [8, 648, 1264, 28], 'the collapsed timeline')
    expect(await computed(timeline, 'border-top-left-radius')).toBe('8px')
    expectBox(await boxOf(viewport(page)), [8, 64, 896, 576], 'the free area, timeline collapsed')
  })

  test('E-134 fit centers the model in the free area, and closing an island moves it to the new center (2.11)', async ({
    page,
  }) => {
    await openApp(page)
    await dropFile(page, 'cube.ptsm')
    await fitView(page, 'Front')
    const before = await canvas(page)
    await expectHover(page, before.cx, before.cy, 'face')
    await expectHover(page, before.cx + 0.4 * before.H - 10, before.cy, 'face')
    await expectHover(page, before.cx + 0.4 * before.H + 12, before.cy, 'none')
    await page.getByRole('button', { name: 'Toggle browser' }).click()
    await expect.poll(async () => (await canvas(page)).cx).toBeCloseTo(before.cx - 136, 0)
    const after = await canvas(page)
    expect(after.H).toBeCloseTo(before.H, 1)
    await expectHover(page, after.cx, after.cy, 'face')
    await expectHover(page, after.cx + 0.4 * after.H - 10, after.cy, 'face')
    await expectHover(page, after.cx + 0.4 * after.H + 12, after.cy, 'none')
    await expect(viewport(page)).toHaveAttribute('data-view', 'front')
    await page.getByRole('button', { name: 'Fit to view', exact: true }).click()
    await expectHover(page, after.cx, after.cy, 'face')
    await expectHover(page, after.cx + 0.4 * after.H - 10, after.cy, 'face')
    await expectHover(page, after.cx + 0.4 * after.H + 12, after.cy, 'none')
  })

  test('E-135 nothing is picked or zoomed through an island (3.13)', async ({ page }) => {
    await openApp(page)
    await dropFile(page, 'cube.ptsm')
    await fitView(page, 'Front')
    const { cx, cy } = await canvas(page)
    await page.mouse.move(cx, cy)
    for (let i = 0; i < 10; i += 1) {
      // oxlint-disable-next-line no-await-in-loop
      await page.mouse.wheel(0, -100)
    }
    await expectHover(page, cx, cy, 'face')
    const browser = await boxOf(page.getByTestId('browser'))
    const browserRight = browser.x + browser.width
    await expectHover(page, browserRight - 20, cy, 'none')
    const target = await viewport(page).getAttribute('data-camera-target')
    await page.mouse.wheel(0, 100)
    await expect(viewport(page)).toHaveAttribute('data-camera-target', target ?? '')
    await expectHover(page, browserRight + 4, cy, 'face')
    const topBar = await boxOf(page.getByTestId('top-bar'))
    await expectHover(page, topBar.x + topBar.width / 2, topBar.y + topBar.height - 4, 'none')
  })

  test('E-136 the view cube and the viewport controls follow the free area corners (3.14)', async ({
    page,
  }) => {
    await openApp(page)
    await dropFile(page, 'cube.ptsm')
    const cube = page.getByTestId('view-cube')
    const controls = page.getByTestId('viewport-controls')
    async function expectCorners(): Promise<{ cubeRight: number; controlsLeft: number }> {
      const [area, cubeBox, controlsBox] = await Promise.all([
        boxOf(viewport(page)),
        boxOf(cube),
        boxOf(controls),
      ])
      const cubeRight = cubeBox.x + cubeBox.width
      expect(Math.abs(area.x + area.width - cubeRight - 36)).toBeLessThanOrEqual(TOLERANCE_PIXELS)
      expect(Math.abs(cubeBox.y - area.y - 44)).toBeLessThanOrEqual(TOLERANCE_PIXELS)
      expect(Math.abs(controlsBox.x - area.x - 16)).toBeLessThanOrEqual(TOLERANCE_PIXELS)
      expect(Math.abs(controlsBox.y - area.y - 16)).toBeLessThanOrEqual(TOLERANCE_PIXELS)
      return { cubeRight, controlsLeft: controlsBox.x }
    }
    const docked = await expectCorners()
    await runCommand(page, 'Parameters')
    await expect(page.getByTestId('side-panel')).toBeVisible()
    const withPanel = await expectCorners()
    expect(Math.abs(docked.cubeRight - withPanel.cubeRight - 368)).toBeLessThanOrEqual(
      TOLERANCE_PIXELS,
    )
    await page.getByRole('button', { name: 'Toggle browser' }).click()
    await expect(page.getByTestId('browser')).toBeHidden()
    const withoutBrowser = await expectCorners()
    expect(
      Math.abs(withPanel.controlsLeft - withoutBrowser.controlsLeft - 272),
    ).toBeLessThanOrEqual(TOLERANCE_PIXELS)
  })

  test('E-137 the top-center slot clears the cube and the controls in a narrow free area (3.2)', async ({
    page,
  }) => {
    await page.setViewportSize({ width: 1040, height: 720 })
    await openApp(page)
    await runCommand(page, 'Parameters')
    expect((await boxOf(viewport(page))).width).toBeCloseTo(384, 0)
    const header = await expectHeaderClear(page)
    expect(header.y).toBeGreaterThanOrEqual(192 - TOLERANCE_PIXELS)
  })

  test('E-137 the top-center slot clears the cube and the controls in a wide free area (3.2)', async ({
    page,
  }) => {
    await openApp(page)
    await runCommand(page, 'Parameters')
    const header = await expectHeaderClear(page)
    expect(Math.abs(header.y - 16)).toBeLessThanOrEqual(TOLERANCE_PIXELS)
  })

  test('E-137 the top-center slot clears the cube and the controls at 200% UI scale (3.2)', async ({
    page,
  }) => {
    await openAtScale(page, 2)
    const header = await expectHeaderClear(page)
    expect(Math.abs(header.y - 32)).toBeLessThanOrEqual(TOLERANCE_PIXELS)
  })

  test('E-137 the top-center slot rises in a free area too short for the drop at 200% (3.2)', async ({
    page,
  }) => {
    await page.setViewportSize({ width: 2080, height: 720 })
    await openAtScale(page, 2)
    await runCommand(page, 'Parameters')
    await expect(page.getByTestId('browser')).toBeVisible()
    expectBox(await boxOf(viewport(page)), [560, 128, 768, 376], 'the free area beside both')
    await page
      .getByTestId('empty-state')
      .getByRole('button', { name: 'Sketch', exact: true })
      .click()
    const header = page.getByTestId('sketch-header')
    await expect(header).toHaveAttribute('data-state', 'picking')
    const area = await boxOf(viewport(page))
    expectInside(await boxOf(header), area, 'the sketch header')
    for (const plane of ['Top plane (XY)', 'Front plane (XZ)', 'Right plane (YZ)']) {
      const button = header.getByRole('button', { name: `Sketch on ${plane}`, exact: true })
      // oxlint-disable-next-line no-await-in-loop
      expectInside(await boxOf(button), area, `the ${plane} button`)
    }
  })

  test('E-138 at 200% the browser and side panel float over the free area (3.2, 3.24)', async ({
    page,
  }) => {
    await openAtScale(page, 2)
    // The top bar under 48rem: the document name whole beside the context switcher.
    const name = page.getByTestId('document-name')
    const nameBox = await boxOf(name)
    const switcher = await boxOf(page.getByRole('radiogroup', { name: 'Context' }))
    expect(nameBox.x + nameBox.width).toBeLessThanOrEqual(switcher.x + TOLERANCE_PIXELS)
    expect(await name.evaluate((element) => element.scrollWidth <= element.clientWidth)).toBe(true)
    const browser = page.getByTestId('browser')
    await expect(browser).toBeHidden()
    const area = await boxOf(viewport(page))
    expectBox(area, [16, 128, 1248, 376], 'the free area')
    await page.getByRole('button', { name: 'Toggle browser' }).click()
    await expect(browser).toBeVisible()
    const browserBox = await boxOf(browser)
    expect(Math.abs(browserBox.x - area.x)).toBeLessThanOrEqual(TOLERANCE_PIXELS)
    expectInside(browserBox, area, 'the browser')
    expectBox(await boxOf(viewport(page)), [16, 128, 1248, 376], 'the free area with the browser')
    await page.keyboard.press('Escape')
    await expect(browser).toBeHidden()
    await runCommand(page, 'Parameters')
    const side = page.getByTestId('side-panel')
    await expect(side).toBeVisible()
    const sideBox = await boxOf(side)
    expectInside(sideBox, area, 'the side panel')
    expect(sideBox.width).toBeCloseTo(720, 0)
    await page.keyboard.press('Escape')
    await expect(side).toBeHidden()
    const corners = await viewCubeCornerBoxes(page)
    for (const corner of corners) {
      expectInside(corner, area, 'a view cube corner')
    }
    expectInside(await boxOf(page.getByRole('button', { name: 'Home view' })), area, 'Home view')
    const controlButtons = await page.getByTestId('viewport-controls').getByRole('button').all()
    for (const button of controlButtons) {
      // oxlint-disable-next-line no-await-in-loop
      const box = await boxOf(button)
      expectInside(box, area, 'a viewport control')
      for (const corner of corners) {
        expect(intersects(box, corner), 'a control and a cube corner').toBe(false)
      }
    }
    await dropFile(page, 'cube.ptsm')
    await page
      .getByRole('radiogroup', { name: 'Context' })
      .getByRole('radio', { name: 'Print' })
      .click()
    await expect(page.getByTestId('health-badge')).toHaveAttribute('data-stale', 'false')
    const legend = await boxOf(page.getByTestId('overhang-legend'))
    expectInside(legend, await boxOf(viewport(page)), 'the overhang legend')
    expect(intersects(legend, await boxOf(page.getByTestId('viewport-controls')))).toBe(false)
    expect(intersects(legend, await boxOf(page.getByTestId('timeline')))).toBe(false)
    for (const corner of await viewCubeCornerBoxes(page)) {
      expect(intersects(legend, corner), 'the legend and a cube corner').toBe(false)
    }
  })

  test('E-142 an inspector dragged to the right edge comes back into the free area when a side panel docks (3.7)', async ({
    page,
  }) => {
    await openApp(page)
    await dropFile(page, 'cube.ptsm')
    await bodyRow(page, 'Cube').click()
    const card = page.getByTestId('inspector')
    await expect(card).toBeVisible()
    const header = await boxOf(card.getByText('Selection', { exact: true }))
    await page.mouse.move(header.x + header.width / 2, header.y + header.height / 2)
    await page.mouse.down()
    await page.mouse.move(header.x + 2000, header.y + header.height / 2, { steps: 5 })
    await page.mouse.up()
    const dragged = await boxOf(card)
    expect(dragged.x + dragged.width).toBeCloseTo(1272, 0)
    await runCommand(page, 'Parameters')
    await expect(page.getByTestId('side-panel')).toBeVisible()
    const area = await boxOf(viewport(page))
    await expect
      .poll(async () => (await boxOf(card)).x + 320)
      .toBeLessThanOrEqual(area.x + area.width + 1)
    expectInside(await boxOf(card), area, 'the inspector')
    // Its close button is hit by a click, not covered by the side panel.
    const close = await boxOf(card.getByRole('button', { name: 'Clear selection' }))
    const hit = await page.evaluate(
      ([x, y]) =>
        document
          .elementFromPoint(x ?? 0, y ?? 0)
          ?.closest('[data-testid]')
          ?.getAttribute('data-testid') ?? null,
      [close.x + close.width / 2, close.y + close.height / 2],
    )
    expect(hit).toBe('inspector')
  })

  test('E-141 the viewport menu opens at the pointer, in canvas pixels (2.11, 3.13)', async ({
    page,
  }) => {
    await openApp(page)
    await dropFile(page, 'cube.ptsm')
    await fitView(page, 'Front')
    const { cx, cy } = await canvas(page)
    const target = await viewport(page).getAttribute('data-camera-target')
    await page.mouse.move(cx + 200, cy)
    await page.mouse.wheel(0, 100)
    await expect(viewport(page)).not.toHaveAttribute('data-camera-target', target ?? '')
    // Beside the model, so the click selects nothing and no inspector opens.
    await page.mouse.click(cx + 250, cy + 50, { button: 'right' })
    const menu = page.getByTestId('viewport-menu')
    await expect(menu).toBeVisible()
    const box = await boxOf(menu)
    expect(Math.abs(box.x - (cx + 250))).toBeLessThanOrEqual(4)
    expect(Math.abs(box.y - (cy + 50))).toBeLessThanOrEqual(4)
    await menu.getByRole('menuitem', { name: /^Fit to view/ }).click()
    await expect(viewport(page)).toHaveAttribute('data-camera-target', '0,0,10')
  })
})
