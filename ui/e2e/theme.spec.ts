/// <reference lib="dom" />
import { expect, test, type Locator, type Page } from '@playwright/test'
import {
  bodyRow,
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

// Themes, inline styles and fonts (ui.md 2.16, 3.12, 6.5 E-56, E-60 to E-62, E-143 and E-144)
// against the production build.

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

/** The computed `font-family` of an element up to its first comma, without quotes (E-62). */
function firstFamily(fontFamily: string): string {
  return (fontFamily.split(',')[0] ?? '').trim().replaceAll('"', '').replaceAll("'", '')
}

/** Expects the first family of the first element `locator` matches to be `family`. */
async function expectFirstFamily(locator: Locator, family: string): Promise<void> {
  await expect(locator.first()).toBeVisible()
  expect(firstFamily(await computed(locator.first(), 'font-family'))).toBe(family)
}

/** Expects a computed property of the first element `locator` matches to be `value`. */
async function expectComputed(locator: Locator, property: string, value: string): Promise<void> {
  await expect(locator.first()).toBeVisible()
  expect(await computed(locator.first(), property)).toBe(value)
}

/** Expects the computed `font-feature-settings` of the first element `locator` matches to hold `ss05`. */
async function expectSerifedCapitalI(locator: Locator): Promise<void> {
  await expect(locator.first()).toBeVisible()
  expect(await computed(locator.first(), 'font-feature-settings')).toContain('ss05')
}

/** The elements of the page that break E-143's rules, as the start of their markup. */
function typeRuleOffenders(page: Page): Promise<{
  readonly withoutSerifedI: readonly string[]
  readonly slashedZero: readonly string[]
}> {
  return page.evaluate(() => {
    const elements = [...document.querySelectorAll('*')]
    const geist = elements.filter((element) => {
      const family = getComputedStyle(element).fontFamily.split(',')[0] ?? ''
      return family.trim().replaceAll('"', '').replaceAll("'", '') === 'Geist'
    })
    return {
      withoutSerifedI: geist
        .filter((element) => !getComputedStyle(element).fontFeatureSettings.includes('ss05'))
        .map((element) => element.outerHTML.slice(0, 120)),
      slashedZero: elements
        .filter((element) => getComputedStyle(element).fontVariantNumeric.includes('slashed-zero'))
        .map((element) => element.outerHTML.slice(0, 120)),
    }
  })
}

/** The font faces of `document.fonts` as E-62 reads them. */
function fontFaces(page: Page): Promise<
  readonly {
    readonly family: string
    readonly weight: string
    readonly display: string
    readonly sizeAdjust: string
    readonly status: string
  }[]
> {
  return page.evaluate(() =>
    [...document.fonts].map((face) => ({
      family: face.family.replaceAll('"', '').replaceAll("'", ''),
      weight: face.weight,
      display: face.display,
      // The DOM types of TypeScript lack the descriptor, which Chromium has.
      sizeAdjust: String(Reflect.get(face, 'sizeAdjust')),
      status: face.status,
    })),
  )
}

/** Whether `document.fonts.check` is true for a font specifier. */
function fontAvailable(page: Page, specifier: string): Promise<boolean> {
  return page.evaluate((font) => document.fonts.check(font), specifier)
}

/** The browser row of the parameter `size` of parametric.ptsm. */
function sizeParameterRow(page: Page): Locator {
  return page.locator('[data-testid=browser-row][data-kind=parameter][data-id=p-size]')
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
  test('E-62 interface text and numbers are Geist, code is JetBrains Mono', async ({ page }) => {
    await openApp(page)
    await dropFile(page, 'rect-sketch.ptsm')
    await editSketchFromTimeline(page, 'Sketch 1')
    for (const testId of ['document-name', 'status-units', 'sketch-header', 'sketch-dimension']) {
      // oxlint-disable-next-line no-await-in-loop
      await expectFirstFamily(page.getByTestId(testId), 'Geist')
    }
    await expectComputed(
      page.getByTestId('sketch-dimension'),
      'font-variant-numeric',
      'tabular-nums',
    )
    await expectComputed(page.getByTestId('status-dof'), 'font-variant-numeric', 'tabular-nums')
    await expectComputed(page.getByTestId('sketch-header'), 'font-variant-numeric', 'normal')

    await page.getByRole('button', { name: 'Finish sketch', exact: true }).click()
    await expect(page.getByTestId('app')).toHaveAttribute('data-context', 'model')
    await page.keyboard.press('e')
    await expect(inspector(page)).toHaveAccessibleName('Extrude')
    const distance = inspectorField(page, 'extent.expr')
    const distanceInput = distance.getByRole('combobox', { name: 'Distance', exact: true })
    const result = distance.getByTestId('expression-result')
    await expectFirstFamily(result, 'Geist')
    await expectFirstFamily(distance.locator('label'), 'Geist')
    await expectFirstFamily(distanceInput, 'Geist')
    await expectComputed(distanceInput, 'font-variant-numeric', 'tabular-nums')
    await expectComputed(result, 'font-variant-numeric', 'tabular-nums')

    await page.keyboard.press('Escape')
    await expect(inspector(page)).toBeHidden()
    await dropFile(page, 'parametric.ptsm')
    await runCommand(page, 'Parameters')
    const panel = page.getByTestId('parameters-panel')
    for (const input of [
      panel.getByRole('textbox', { name: 'Name of size', exact: true }),
      panel.getByRole('combobox', { name: 'Expression of size', exact: true }),
    ]) {
      // oxlint-disable-next-line no-await-in-loop
      await expectFirstFamily(input, 'JetBrains Mono')
      // oxlint-disable-next-line no-await-in-loop
      await expectComputed(input, 'font-variant-ligatures', 'no-contextual')
      // oxlint-disable-next-line no-await-in-loop
      await expectComputed(input, 'font-feature-settings', 'normal')
    }
    const value = panel.getByTestId('parameter-value')
    await expectFirstFamily(value, 'Geist')
    await expectComputed(value, 'font-variant-numeric', 'tabular-nums')
    const rowText = sizeParameterRow(page).getByText('size = 20 mm', { exact: true })
    await expectFirstFamily(rowText, 'JetBrains Mono')
    await expectComputed(rowText, 'font-variant-ligatures', 'normal')

    await page.keyboard.press('ControlOrMeta+K')
    await page.getByTestId('palette-input').fill('size')
    const option = page.locator('[data-testid=palette-option][data-kind=parameter]')
    await expectFirstFamily(option.getByText('size = 20 mm', { exact: true }), 'JetBrains Mono')
    await expectFirstFamily(option.getByText('Parameter', { exact: true }), 'Geist')

    expect(await fontAvailable(page, '14px Geist')).toBe(true)
    expect(await fontAvailable(page, '14px "JetBrains Mono"')).toBe(true)
    const faces = await fontFaces(page)
    expect(faces.find((face) => face.family === 'Geist')).toMatchObject({
      weight: '100 900',
      display: 'swap',
      status: 'loaded',
    })
    expect(faces.find((face) => face.family === 'JetBrains Mono')).toMatchObject({
      weight: '100 800',
      sizeAdjust: '96%',
      status: 'loaded',
    })
  })

  test('E-143 interface text keeps ss05 and numbers in fields are tabular', async ({ page }) => {
    await openApp(page)
    await dropFile(page, 'cube.ptsm')
    await timelineItem(page, 'Extrude 1')
      .getByRole('button', { name: 'Extrude 1', exact: true })
      .dblclick()
    await expect(inspector(page)).toHaveAccessibleName('Extrude 1')
    const distance = inspectorField(page, 'extent.expr')
    const label = distance.locator('label')
    const input = distance.getByRole('combobox', { name: 'Distance', exact: true })
    const ok = inspector(page).getByRole('button', { name: 'OK', exact: true })
    const documentName = page.getByTestId('document-name')
    for (const element of [documentName, label, input, ok, page.getByTestId('status-units')]) {
      // oxlint-disable-next-line no-await-in-loop
      await expectSerifedCapitalI(element)
    }
    for (const element of [input, distance.getByTestId('expression-result')]) {
      // oxlint-disable-next-line no-await-in-loop
      await expectComputed(element, 'font-variant-numeric', 'tabular-nums')
    }
    await expectComputed(
      page.getByTestId('status-recompute'),
      'font-variant-numeric',
      'tabular-nums',
    )
    for (const element of [documentName, label, ok]) {
      // oxlint-disable-next-line no-await-in-loop
      await expectComputed(element, 'font-variant-numeric', 'normal')
    }
    expect(await typeRuleOffenders(page)).toEqual({ withoutSerifedI: [], slashedZero: [] })

    // The footer's Cancel, beside OK; the header's close button has the same name.
    await inspector(page)
      .getByRole('button', { name: 'Cancel', exact: true })
      .filter({ hasText: 'Cancel' })
      .click()
    await expect(inspector(page)).not.toHaveAttribute('data-mode', 'command')
    await bodyRow(page, 'Cube').click()
    const numbers = page.getByTestId('body-size').locator('.tabular-nums')
    await expect(numbers).toHaveCount(3)
    for (const number of await numbers.all()) {
      // oxlint-disable-next-line no-await-in-loop
      await expectSerifedCapitalI(number)
      // oxlint-disable-next-line no-await-in-loop
      await expectComputed(number, 'font-variant-numeric', 'tabular-nums')
    }
    await expectComputed(page.getByTestId('selection-title'), 'font-variant-numeric', 'normal')
    expect(await typeRuleOffenders(page)).toEqual({ withoutSerifedI: [], slashedZero: [] })
  })

  test('E-144 no font request leaves the app', async ({ page }) => {
    const requests: { readonly url: URL; readonly type: string }[] = []
    page.on('request', (request) => {
      requests.push({ url: new URL(request.url()), type: request.resourceType() })
    })
    const requestsFor = (path: string) =>
      requests.filter((request) => request.url.pathname === path)
    await openApp(page)
    await page.evaluate(() => document.fonts.ready)
    expect(await fontAvailable(page, '14px Geist')).toBe(true)
    expect(requestsFor('/fonts/Geist-Variable.woff2')).toHaveLength(1)
    expect(requestsFor('/fonts/JetBrainsMono-Variable.woff2')).toHaveLength(0)

    await dropFile(page, 'parametric.ptsm')
    await runCommand(page, 'Parameters')
    await expect.poll(() => fontAvailable(page, '14px "JetBrains Mono"')).toBe(true)
    expect(requestsFor('/fonts/JetBrainsMono-Variable.woff2')).toHaveLength(1)

    const origin = new URL(page.url()).origin
    const strayFonts = requests
      .filter((request) => request.type === 'font')
      .filter(
        (request) => request.url.origin !== origin || !request.url.pathname.startsWith('/fonts/'),
      )
    expect(strayFonts.map((request) => request.url.href)).toEqual([])
    const googleFonts = requests.filter((request) =>
      ['fonts.googleapis.com', 'fonts.gstatic.com'].includes(request.url.hostname),
    )
    expect(googleFonts.map((request) => request.url.href)).toEqual([])
  })
})
