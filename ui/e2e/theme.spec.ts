/// <reference lib="dom" />
import { expect, test, type Locator, type Page } from '@playwright/test'
import {
  dropFile,
  expectReady,
  inspector,
  inspectorField,
  openApp,
  runCommand,
  timelineItem,
} from './helpers'
import { chooseContext } from './support/print'
import { editSketchFromTimeline } from './support/sketch'

// Themes, inline styles and fonts (ui.md 2.16, 3.12, 6.5 E-56 and E-60 to E-62) against the
// production build.

/** The key the settings are stored under in localStorage (ui.md 2.15). */
const SETTINGS_KEY = 'partsmith.settings.v1'

/** An element with a `style` attribute, as E-61 checks it. */
type StyledElement = {
  /** The start of its markup, so a failure names it. */
  readonly markup: string
  readonly positioned: boolean
  /** The properties its `style` attribute declares. */
  readonly properties: readonly string[]
  /** Whether it lies inside the view cube, whose rotor is the one element turned by `transform`. */
  readonly inViewCube: boolean
}

/** The root element, which carries the `dark` class in the dark theme (ui.md 2.16.8). */
function root(page: Page): Locator {
  return page.locator('html')
}

/** The stored settings: whether they parse to an object, and its `theme`. */
function storedSettings(
  page: Page,
): Promise<{ readonly isObject: boolean; readonly theme: unknown }> {
  return page.evaluate((key) => {
    const text = localStorage.getItem(key)
    const parsed: unknown = text === null ? null : JSON.parse(text)
    const isObject = typeof parsed === 'object' && parsed !== null && !Array.isArray(parsed)
    return { isObject, theme: isObject ? Reflect.get(parsed, 'theme') : undefined }
  }, SETTINGS_KEY)
}

/** Expects the stored settings to be an object whose `theme` is `theme`. */
async function expectStoredTheme(page: Page, theme: 'light' | 'dark'): Promise<void> {
  await expect.poll(() => storedSettings(page)).toEqual({ isObject: true, theme })
}

/** Reloads the page and waits until the core is ready again. */
async function reload(page: Page): Promise<void> {
  await page.reload()
  await expectReady(page)
}

/** Whether the root element has the `dark` class right now, read once without waiting. */
function isDarkNow(page: Page): Promise<boolean> {
  return page.evaluate(() => document.documentElement.classList.contains('dark'))
}

/** The computed value of a CSS property of the first element `locator` matches. */
function computed(locator: Locator, property: string): Promise<string> {
  return locator.evaluate(
    (element, name) => getComputedStyle(element).getPropertyValue(name),
    property,
  )
}

/** Every element with a `style` attribute, with what E-61 checks about it. */
function styledElements(page: Page): Promise<StyledElement[]> {
  return page.evaluate(() =>
    [...document.querySelectorAll('[style]')].map((element) => {
      const style =
        element instanceof HTMLElement || element instanceof SVGElement ? element.style : null
      return {
        markup: element.outerHTML.slice(0, 160),
        positioned: element.hasAttribute('data-positioned'),
        properties:
          style === null ? [] : Array.from({ length: style.length }, (_, i) => style.item(i)),
        inViewCube: element.closest('[data-testid=view-cube]') !== null,
      }
    }),
  )
}

/**
 * Expects every element with a `style` attribute to be a runtime position (ui.md 3.12, 6.1): it has
 * `data-positioned` and declares only `translate`, or, for the view cube rotor, only `transform`.
 */
async function expectOnlyRuntimePositions(page: Page): Promise<void> {
  const elements = await styledElements(page)
  expect(elements).not.toEqual([])
  const misplaced = elements.filter((element) => {
    const allowed = element.inViewCube ? 'transform' : 'translate'
    return (
      !element.positioned || element.properties.length !== 1 || element.properties[0] !== allowed
    )
  })
  expect(misplaced).toEqual([])
}

/** Expects the computed `font-family` of the first element `locator` matches to start with `family`. */
async function expectFontFamily(locator: Locator, family: string): Promise<void> {
  await expect(locator.first()).toBeVisible()
  const computedFamily = await computed(locator.first(), 'font-family')
  expect(computedFamily.startsWith(family), `font-family ${computedFamily}`).toBe(true)
}

test.describe('themes', () => {
  test('E-56 the theme commands choose dark, light or the system theme', async ({ page }) => {
    // The system theme starts light, so "Theme: Dark" is a change.
    await page.emulateMedia({ colorScheme: 'light' })
    await openApp(page)
    await expect(root(page)).not.toContainClass('dark')
    await runCommand(page, 'Theme: Dark')
    await expect(root(page)).toContainClass('dark')
    await reload(page)
    await expect(root(page)).toContainClass('dark')
    await runCommand(page, 'Theme: Light')
    await expect(root(page)).not.toContainClass('dark')
    await page.emulateMedia({ colorScheme: 'dark' })
    await runCommand(page, 'Theme: System')
    await expect(root(page)).toContainClass('dark')
  })

  test('E-60 the theme toggle switches and stores an explicit theme that wins over the system', async ({
    page,
  }) => {
    await page.emulateMedia({ colorScheme: 'light' })
    await openApp(page)
    const toggle = page.getByTestId('theme-toggle')
    await expect(root(page)).not.toContainClass('dark')
    await expect(toggle).toHaveAttribute('aria-pressed', 'false')
    const topBar = page.getByTestId('top-bar')
    const lightBackground = await computed(topBar, 'background-color')
    await toggle.click()
    await expect(root(page)).toContainClass('dark')
    await expect(toggle).toHaveAttribute('aria-pressed', 'true')
    await expectStoredTheme(page, 'dark')
    await expect.poll(() => computed(topBar, 'background-color')).not.toBe(lightBackground)
    await reload(page)
    // The theme is applied before the first render (2.16.8), so it is there once the app is ready.
    expect(await isDarkNow(page)).toBe(true)
    await toggle.click()
    await expect(root(page)).not.toContainClass('dark')
    await expectStoredTheme(page, 'light')
    await page.emulateMedia({ colorScheme: 'dark' })
    await reload(page)
    // The explicit choice wins over the system theme.
    await expect(root(page)).not.toContainClass('dark')
    await runCommand(page, 'Theme: System')
    await expect(root(page)).toContainClass('dark')
  })
})

test.describe('E-61 no inline styles except runtime positions', () => {
  test('E-61 (a) right after openApp', async ({ page }) => {
    await openApp(page)
    await expectOnlyRuntimePositions(page)
  })

  test('E-61 (b) a sketch with dimensions and constraint glyphs', async ({ page }) => {
    await openApp(page)
    await dropFile(page, 'rect-sketch.ptsm')
    await editSketchFromTimeline(page, 'Sketch 1')
    // A label or glyph centered on its anchor sits in a positioned wrapper, since the inline
    // translate would replace its own centering translate classes (3.12).
    for (const testId of ['sketch-dimension', 'constraint-glyph']) {
      const first = page.getByTestId(testId).first()
      // oxlint-disable-next-line no-await-in-loop
      await expect(first).toBeVisible()
      // oxlint-disable-next-line no-await-in-loop
      await expect(first.locator('xpath=..')).toHaveAttribute('data-positioned', '')
    }
    await expectOnlyRuntimePositions(page)
  })

  test('E-61 (c) the inspector of an edited feature and the Document menu', async ({ page }) => {
    await openApp(page)
    await dropFile(page, 'cube.ptsm')
    await timelineItem(page, 'Extrude 1')
      .getByRole('button', { name: 'Extrude 1', exact: true })
      .dblclick()
    await expect(inspector(page)).toHaveAccessibleName('Extrude 1')
    await expect(inspector(page)).toHaveAttribute('data-positioned', '')
    await page.getByTestId('document-name').click()
    await expect(page.getByTestId('document-menu')).toBeVisible()
    await expectOnlyRuntimePositions(page)
  })

  test('E-61 (d) Print with the overhang legend', async ({ page }) => {
    await openApp(page)
    await dropFile(page, 'overhang.ptsm')
    await chooseContext(page, 'Print')
    await expect(page.getByTestId('overhang-legend')).toBeVisible({ timeout: 20_000 })
    await expectOnlyRuntimePositions(page)
  })
})

test.describe('fonts', () => {
  test('E-62 interface text is Inter and values are JetBrains Mono', async ({ page }) => {
    await openApp(page)
    await dropFile(page, 'rect-sketch.ptsm')
    await editSketchFromTimeline(page, 'Sketch 1')
    await expectFontFamily(page.getByTestId('document-name'), 'Inter')
    await expectFontFamily(page.getByTestId('status-units'), 'Inter')
    await expectFontFamily(page.getByTestId('sketch-header'), 'Inter')
    // Chrome keeps the quotes of a family name with a space.
    await expectFontFamily(page.getByTestId('sketch-dimension'), '"JetBrains Mono"')
    await page.getByRole('button', { name: 'Finish sketch', exact: true }).click()
    await expect(page.getByTestId('app')).toHaveAttribute('data-context', 'model')
    await page.keyboard.press('e')
    await expect(inspector(page)).toHaveAccessibleName('Extrude')
    const distance = inspectorField(page, 'extent.expr')
    await expectFontFamily(distance.getByTestId('expression-result'), '"JetBrains Mono"')
    await expectFontFamily(distance.locator('label'), 'Inter')
  })
})
