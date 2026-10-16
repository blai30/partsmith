import { act, cleanup, fireEvent, screen, within } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { notify } from '../app/actions/notifications'
import { installPopoverPolyfill, isPopoverOpen } from '../components/popover-polyfill.testing'
import type { Document } from '../generated/protocol'
import { ChoiceField } from '../inspector/fields/ChoiceField'
import type { InspectorField } from '../inspector/model'
import { sketchSession } from '../sketch/fixtures.testing'
import { INITIAL_STATE, type AppState } from '../state/store'
import { CUBE_DOCUMENT } from '../test/fakes/documents'
import { bodyItem } from '../test/fakes/selection'
import { createFakeViewport } from '../test/fakes/viewport'
import { checkResult, finding, printReport } from '../test/fakes/state'
import { COLOR_CLASSES } from '../theme/roles'
import {
  cubeDocumentState,
  documentState,
  FILLET_EDGES_COMMAND,
  rectSketchDocumentState,
} from './fixtures.testing'
import { renderModelApp, stubMatchMedia } from './test-app.testing'

// Flat islands: no outlines, a transparent edge for forced colors, fewer lines inside islands,
// rules on scrolling bodies and state outlines (ui.md 3.2, 3.23, 3.24; ADR 0032).

beforeEach(installPopoverPolyfill)

beforeEach(() => {
  stubMatchMedia()
})

afterEach(() => {
  vi.unstubAllGlobals()
  vi.restoreAllMocks()
})

/** The classes of a role facet, one by one. */
function classesOf(classes: string): string[] {
  return classes.split(' ')
}

/** Whether an element has every class of a role facet. */
function hasFacet(element: Element, classes: string): boolean {
  return classesOf(classes).every((name) => element.classList.contains(name))
}

/** Whether an element is shown: not inside a closed popover, such as a tooltip at rest. */
function isShown(element: Element): boolean {
  const popover = element.closest('[popover]')
  return popover === null || isPopoverOpen(popover)
}

/** The shown elements with a `shadow-md` or `shadow-lg` class. */
function shadowedElements(): Element[] {
  return Array.from(document.querySelectorAll('*')).filter(
    (element) =>
      isShown(element) &&
      (element.classList.contains('shadow-md') || element.classList.contains('shadow-lg')),
  )
}

/** The elements with a class that names a zinc border step without a variant. */
function zincBorderElements(): Element[] {
  return Array.from(document.querySelectorAll('*')).filter((element) =>
    [...element.classList].some((name) => name.startsWith('border-zinc-')),
  )
}

const DIVIDER_CLASSES = classesOf(COLOR_CLASSES.divider.border)
const BORDER_CONTROL_CLASSES = classesOf(COLOR_CLASSES.borderControl.border)

/** The elements whose zinc border classes are not `borderControl`'s. */
function zincBordersOtherThanControls(): Element[] {
  return zincBorderElements().filter(
    (element) => !BORDER_CONTROL_CLASSES.every((name) => element.classList.contains(name)),
  )
}

/** The surfaces that float over the islands themselves: on `surfaceRaised` (3.2). */
function isRaised(element: Element): boolean {
  return (
    element.getAttribute('data-testid') === 'document-menu' ||
    element.getAttribute('data-testid') === 'palette' ||
    element.getAttribute('data-testid') === 'notification'
  )
}

/** A short name of an element for a failure message: its test id or the start of its markup. */
function nameOf(element: Element): string {
  return element.getAttribute('data-testid') ?? element.outerHTML.slice(0, 120)
}

/** The shadowed elements without their 1-pixel forcedEdge border or their surface (3.2). */
function surfaceDefects(): string[] {
  return shadowedElements()
    .filter((element) => {
      const surface = isRaised(element)
        ? COLOR_CLASSES.surfaceRaised.background
        : COLOR_CLASSES.surface.background
      return !(
        element.classList.contains('border') &&
        hasFacet(element, COLOR_CLASSES.forcedEdge.border) &&
        hasFacet(element, surface)
      )
    })
    .map(nameOf)
}

/** Whether an element may carry `divider`: a menu separator or the palette's header row (3.2). */
function mayDivide(element: Element): boolean {
  const separator = element.tagName === 'HR' && element.closest('[role="menu"]') !== null
  return separator || element.parentElement?.getAttribute('data-testid') === 'palette'
}

/** The elements with zinc border steps other than `borderControl`'s, except `divider` where it stays. */
function misplacedLines(): string[] {
  return zincBordersOtherThanControls()
    .filter((element) => !(mayDivide(element) && hasFacet(element, COLOR_CLASSES.divider.border)))
    .map(nameOf)
}

/** Expects a top rule in forcedEdge that the overflowTop facet colors (3.2). */
function expectTopRule(body: Element | null): void {
  expect(body).toHaveClass('border-t', ...classesOf(COLOR_CLASSES.forcedEdge.border))
  expect(body).toHaveClass(...classesOf(COLOR_CLASSES.divider.overflowTop))
}

/** The elements with both the accent outline and the transparent state outline. */
function doublyOutlined(): string[] {
  return Array.from(document.querySelectorAll('*'))
    .filter(
      (element) =>
        hasFacet(element, COLOR_CLASSES.accent.outline) &&
        hasFacet(element, COLOR_CLASSES.forcedEdge.outline),
    )
    .map(nameOf)
}

/** The cube with the parameters `size` = 20 mm and `gap` = size / 4, as in parametric.ptsm. */
const PARAMETRIC_DOCUMENT: Document = {
  ...CUBE_DOCUMENT,
  parameters: [
    { id: 'p-size', name: 'size', expr: '20 mm' },
    { id: 'p-gap', name: 'gap', expr: 'size / 4' },
  ],
}

const LENGTH = { length: 1, angle: 0 } as const

function parametric() {
  return documentState(PARAMETRIC_DOCUMENT, {
    name: 'parametric',
    parameterValues: {
      'p-size': { id: 'p-size', value: 20, dimension: LENGTH },
      'p-gap': { id: 'p-gap', value: 5, dimension: LENGTH },
    },
  })
}

const PARAMETERS_OPEN: AppState['panels'] = {
  ...INITIAL_STATE.panels,
  browserOpen: true,
  side: 'parameters',
}

/** The cube with the Parameters panel open, the timeline expanded, "Cube" selected and an info notification. */
function renderShell(panels: Partial<AppState['panels']> = {}) {
  const rendered = renderModelApp({
    document: parametric(),
    state: {
      panels: { ...PARAMETERS_OPEN, timelineCollapsed: false, ...panels },
      selection: [bodyItem('b-cube', 'Cube')],
    },
  })
  act(() => {
    notify(rendered.app, { severity: 'info', message: 'Saved.', actions: [] })
  })
  return rendered
}

/** The print context with one overhang finding on the cube and the print check panel open. */
function printState(): Partial<AppState> {
  const report = printReport({
    'b-cube': [checkResult('overhangs', [finding('overhangs', 'warning')])],
  })
  return {
    context: 'print',
    print: { ...INITIAL_STATE.print, report, reportRevision: cubeDocumentState().revision },
    panels: { ...INITIAL_STATE.panels, side: 'health' },
  }
}

/** The body after a side panel's `PanelHeader`, found by the panel's title. */
function bodyAfterHeader(title: string): Element | null {
  const heading = within(screen.getByTestId('side-panel')).getByRole('heading', { name: title })
  return heading.parentElement?.nextElementSibling ?? null
}

/** The elements that carry a `divider` overflow facet. */
function overflowRuled(): Element[] {
  return Array.from(document.querySelectorAll('[class*="data-overflow-"]'))
}

describe('no outlines (U-209)', () => {
  it('U-209 gives every shadowed island, card and popup a forcedEdge border on its surface', () => {
    renderShell()
    expect(shadowedElements().length).toBeGreaterThanOrEqual(8)
    expect(surfaceDefects()).toEqual([])
    expect(misplacedLines()).toEqual([])
  })

  it('U-209 the same holds with the palette open', () => {
    renderShell({ paletteOpen: true })
    expect(screen.getByTestId('palette')).toBeInTheDocument()
    expect(shadowedElements().length).toBeGreaterThanOrEqual(8)
    expect(surfaceDefects()).toEqual([])
    expect(misplacedLines()).toEqual([])
    const header = screen.getByTestId('palette-input').parentElement
    expect(header).toHaveClass('border-b', ...DIVIDER_CLASSES)
  })

  it('U-209 the same holds with the Document menu open, whose separators keep the divider', () => {
    renderShell()
    fireEvent.click(screen.getByTestId('document-name'))
    const menu = screen.getByTestId('document-menu')
    expect(isPopoverOpen(menu)).toBe(true)
    expect(shadowedElements().length).toBeGreaterThanOrEqual(8)
    expect(surfaceDefects()).toEqual([])
    expect(misplacedLines()).toEqual([])
    const separators = menu.querySelectorAll(':scope > hr')
    expect(separators.length).toBeGreaterThan(0)
    for (const separator of separators) {
      expect(separator).toHaveClass(...DIVIDER_CLASSES)
    }
  })

  it('U-209 draws no rule under panel headers, between parameter rows or above the palette footer', () => {
    renderShell({ paletteOpen: true })
    const header = within(screen.getByTestId('side-panel')).getByRole('heading', {
      name: 'Parameters',
    }).parentElement
    const rows = screen.getAllByTestId('parameter-row')
    expect(rows.length).toBeGreaterThan(0)
    for (const element of [header, ...rows, screen.getByTestId('palette-footer')]) {
      expect(element).not.toHaveClass('border-t')
      expect(element).not.toHaveClass('border-b')
    }
  })

  it('U-209 draws no rule above the command inspector footer', () => {
    renderModelApp({ state: { command: FILLET_EDGES_COMMAND } })
    const footer = screen.getByTestId('inspector').lastElementChild
    expect(footer).toContainElement(screen.getByTestId('inspector-ok'))
    expect(footer).not.toHaveClass('border-t')
    expect(footer).not.toHaveClass('border-b')
  })

  it.each([
    ['Model', {}, 0],
    [
      'Sketch',
      {
        document: rectSketchDocumentState(),
        state: { context: 'sketch', sketch: sketchSession() },
      },
      1,
    ],
  ] as const)(
    'U-209 keeps the %s toolbar separators as gaps without a line',
    (_name, options, minimum) => {
      renderModelApp(options)
      const separators = screen.getByTestId('top-bar').querySelectorAll('hr')
      expect(separators.length).toBeGreaterThanOrEqual(minimum)
      for (const separator of separators) {
        expect(separator).toHaveClass('mx-1', 'h-6', 'w-0', 'shrink-0', 'border-0')
        expect(separator).not.toHaveClass('border-l')
        expect(hasFacet(separator, COLOR_CLASSES.divider.border)).toBe(false)
      }
    },
  )

  it('U-209 draws no rule inside the sketch header', () => {
    renderModelApp({
      document: rectSketchDocumentState(),
      state: { context: 'sketch', sketch: sketchSession() },
    })
    const header = screen.getByTestId('sketch-header')
    for (const element of [header, ...header.querySelectorAll('*')]) {
      expect(element).not.toHaveClass('border-l')
    }
  })

  it('U-209 draws no rule above the sections of the Keyboard and mouse panel', () => {
    renderModelApp({ state: { panels: { ...INITIAL_STATE.panels, side: 'keys' } } })
    const sections = within(screen.getByTestId('side-panel')).getAllByRole('heading', { level: 3 })
    expect(sections.length).toBeGreaterThan(0)
    for (const heading of sections) {
      expect(heading.parentElement).not.toHaveClass('border-t')
    }
  })

  it('U-209 draws no rule under the print check header block', () => {
    renderModelApp({ state: printState() })
    expect(screen.getByTestId('health-list').firstElementChild).not.toHaveClass('border-b')
  })

  it('U-209 raises the browser and the side panel to surfaceRaised while they float over the free area', () => {
    renderShell()
    for (const island of [screen.getByTestId('browser'), screen.getByTestId('side-panel')]) {
      expect(island).toHaveClass(...classesOf(COLOR_CLASSES.surface.background))
      expect(island).toHaveClass(...classesOf(COLOR_CLASSES.surfaceRaised.overlay))
      expect(island).toHaveClass('@max-5xl/app:z-20', '@max-5xl/app:shadow-lg')
    }
    for (const id of ['top-bar', 'timeline', 'status-bar', 'inspector']) {
      expect(hasFacet(screen.getByTestId(id), COLOR_CLASSES.surfaceRaised.overlay)).toBe(false)
    }
  })
})

describe('cards of the free area (U-214)', () => {
  it('U-214 marks the cards and chips of the free area, whose boxes hide the labels under them', () => {
    renderShell()
    const cards = [
      screen.getByTestId('view-cube'),
      screen.getByRole('button', { name: 'Home view' }),
      screen.getByTestId('viewport-controls'),
      screen.getByTestId('inspector'),
    ]
    for (const card of cards) {
      expect(card).toHaveAttribute('data-card', '')
    }
    expect(document.querySelectorAll('[data-card]')).toHaveLength(cards.length)
  })

  it('U-214 marks the empty-state card, the sketch header and its hint', () => {
    renderModelApp({ document: documentState({ ...CUBE_DOCUMENT, features: [] }) })
    expect(screen.getByTestId('empty-state')).toHaveAttribute('data-card', '')
    cleanup()
    renderModelApp({
      document: rectSketchDocumentState(),
      state: { context: 'sketch', sketch: sketchSession() },
    })
    expect(screen.getByTestId('sketch-header')).toHaveAttribute('data-card', '')
    const hint = screen.getByTestId('sketch-hint')
    expect(hint.hasAttribute('data-card')).toBe(!hint.classList.contains('sr-only'))
  })
})

describe('buttons and boundaries in the shell (U-211)', () => {
  it('U-211 draws Search and Export as ghost buttons without a border, like the toolbar buttons', () => {
    renderShell()
    const search = screen.getByTestId('palette-button')
    const exportButton = within(screen.getByTestId('top-bar')).getByRole('button', {
      name: 'Export',
    })
    for (const button of [search, exportButton]) {
      expect(button).not.toHaveClass('border')
      expect(hasFacet(button, COLOR_CLASSES.forcedEdge.border)).toBe(false)
      expect(button).toHaveClass(...classesOf(COLOR_CLASSES.hover.background))
      expect(hasFacet(button, COLOR_CLASSES.field.background)).toBe(false)
      expect(hasFacet(button, COLOR_CLASSES.tonal.background)).toBe(false)
    }
    expect(search).toHaveClass(...classesOf(COLOR_CLASSES.textMuted.foreground))
    expect(exportButton).toHaveClass(...classesOf(COLOR_CLASSES.text.foreground))
  })

  it('U-211 gives the toolbar primary button Finish sketch a border in its fill color', () => {
    renderModelApp({
      document: rectSketchDocumentState(),
      state: { context: 'sketch', sketch: sketchSession() },
    })
    const finish = within(screen.getByTestId('top-bar')).getByRole('button', {
      name: /^Finish sketch/,
    })
    expect(finish).toHaveClass('border', ...classesOf(COLOR_CLASSES.accent.border))
  })

  it('U-211 keeps the border of a disabled toolbar primary button, transparent, so it keeps its size', () => {
    renderModelApp({
      state: { context: 'print', panels: { ...INITIAL_STATE.panels, browserOpen: false } },
    })
    const toolbar = screen.getByRole('toolbar', { name: 'Print tools' })
    const exportEnabled = within(toolbar).getByRole('button', { name: 'Export 3MF' })
    expect(exportEnabled).not.toHaveAttribute('aria-disabled')
    expect(exportEnabled).toHaveClass('border', ...classesOf(COLOR_CLASSES.accent.border))
    cleanup()
    // Export works on the model state, so it waits while a feature command is open (3.18).
    renderModelApp({ state: { context: 'print', command: FILLET_EDGES_COMMAND } })
    const exportDisabled = within(screen.getByRole('toolbar', { name: 'Print tools' })).getByRole(
      'button',
      { name: 'Export 3MF' },
    )
    expect(exportDisabled).toHaveAttribute('aria-disabled', 'true')
    expect(exportDisabled).toHaveClass('border', ...classesOf(COLOR_CLASSES.forcedEdge.border))
    expect(hasFacet(exportDisabled, COLOR_CLASSES.accent.border)).toBe(false)
    expect(hasFacet(exportDisabled, COLOR_CLASSES.accent.background)).toBe(false)
  })

  it('U-211 keeps the borderControl boundary on a choice shown as a select', () => {
    const options = ['a', 'b', 'c', 'd', 'e'].map((value) => ({
      value,
      label: value.toUpperCase(),
    }))
    const field: InspectorField = {
      path: 'style',
      label: 'Style',
      help: null,
      essential: true,
      required: false,
      control: { type: 'choice', presentation: 'select', options },
      value: 'a',
      error: null,
      attachedTo: null,
      axis: null,
    }
    renderModelApp({
      element: <ChoiceField field={field} options={options} presentation="select" />,
    })
    const select = screen.getByRole('combobox', { name: 'Style' })
    expect(select).toHaveClass('border', ...classesOf(COLOR_CLASSES.borderControl.border))
  })

  it('U-211 leaves the palette input without a boundary, with the muted placeholder', () => {
    renderShell({ paletteOpen: true })
    const input = screen.getByTestId('palette-input')
    expect(input).toHaveClass(...classesOf(COLOR_CLASSES.textMuted.placeholder))
    expect(input).not.toHaveClass('border')
  })
})

// Every source file outside the tests, to look for forced-colors classes.
const SOURCES = import.meta.glob<string>(
  [
    '/src/**/*.{ts,tsx}',
    '!/src/generated/**',
    '!/src/**/*.test.{ts,tsx}',
    '!/src/**/*.testing.{ts,tsx}',
  ],
  { query: '?raw', import: 'default', eager: true },
)

describe('control boundaries and forced colors (U-211)', () => {
  it('U-211 keeps the borderControl boundary on fields, selection fields, switches and the export radios', async () => {
    const { user } = renderModelApp({
      document: parametric(),
      state: { panels: { ...INITIAL_STATE.panels, side: 'parameters' } },
    })
    const name = within(screen.getByTestId('side-panel')).getByRole('textbox', {
      name: 'Name of size',
    })
    expect(name).toHaveClass('border', ...classesOf(COLOR_CLASSES.borderControl.border))
    await user.click(within(screen.getByTestId('top-bar')).getByRole('button', { name: 'Export' }))
    const radios = within(screen.getByRole('radiogroup', { name: 'Bodies' })).getAllByRole('radio')
    expect(radios.length).toBeGreaterThan(1)
    for (const radio of radios) {
      const boundary = (radio as HTMLInputElement).checked
        ? ['border-4', ...classesOf(COLOR_CLASSES.accent.border)]
        : ['border', ...classesOf(COLOR_CLASSES.borderControl.border)]
      expect(radio).toHaveClass(...boundary)
    }
  })

  it('U-211 keeps a boundary on the selection field of a command', () => {
    renderModelApp({ state: { command: FILLET_EDGES_COMMAND } })
    const field = screen.getAllByTestId('selection-field')[0]
    const boundaries = [
      COLOR_CLASSES.borderControl.border,
      COLOR_CLASSES.accent.border,
      COLOR_CLASSES.errorMark.border,
    ]
    expect(field).toHaveClass('border')
    expect(boundaries.some((facet) => field !== undefined && hasFacet(field, facet))).toBe(true)
  })

  it('U-211 opts only the swatches and the ramp bar out of forced colors, with no forced-colors variant', () => {
    const files = Object.entries(SOURCES)
      .filter(([, text]) => text.includes('forced-color'))
      .map(([path]) => path)
      .toSorted()
    expect(files).toEqual(['/src/browser/BodyColor.tsx', '/src/print/OverhangLegend.tsx'])
    for (const path of files) {
      expect(SOURCES[path]).not.toContain('forced-colors:')
    }
  })
})

describe('rules of scrolling bodies (U-212)', () => {
  it.each([
    ['parameters', 'Parameters'],
    ['problems', 'Problems'],
    ['keys', 'Keyboard and mouse'],
  ] as const)('U-212 rules the body of the %s panel under its header', (side, title) => {
    renderModelApp({ state: { panels: { ...INITIAL_STATE.panels, side } } })
    const body = bodyAfterHeader(title)
    expectTopRule(body)
    expect(overflowRuled()).toEqual([body])
  })

  it('U-212 rules the print check list under its header block', () => {
    renderModelApp({ state: printState() })
    const list = within(screen.getByTestId('health-list')).getByRole('list', { name: 'Findings' })
    expectTopRule(list)
    expect(overflowRuled()).toEqual([list])
  })

  it('U-212 rules the selection inspector body under its header', () => {
    renderModelApp({ state: { selection: [bodyItem('b-cube', 'Cube')] } })
    const body = screen.getByTestId('inspector').lastElementChild
    expect(body).toContainElement(screen.getByTestId('selection-title'))
    expectTopRule(body)
    expect(overflowRuled()).toEqual([body])
  })

  it('U-212 rules the command inspector body above and below, between its header and footer', () => {
    renderModelApp({ state: { command: FILLET_EDGES_COMMAND } })
    const body = screen.getByTestId('inspector').lastElementChild?.previousElementSibling ?? null
    expect(body).toHaveClass('min-h-0', 'flex-1', 'overflow-y-auto', 'border-y')
    expect(body).toHaveClass(...classesOf(COLOR_CLASSES.forcedEdge.border))
    expect(body).toHaveClass(...classesOf(COLOR_CLASSES.divider.overflowTop))
    expect(body).toHaveClass(...classesOf(COLOR_CLASSES.divider.overflowBottom))
    expect(overflowRuled()).toEqual([body])
  })

  it('U-212 rules the palette list above its footer', () => {
    renderModelApp({ state: { panels: { ...INITIAL_STATE.panels, paletteOpen: true } } })
    const list = screen.getByRole('listbox', { name: 'Results' })
    expect(list).toHaveClass('border-b', ...classesOf(COLOR_CLASSES.forcedEdge.border))
    expect(list).toHaveClass(...classesOf(COLOR_CLASSES.divider.overflowBottom))
    expect(hasFacet(list, COLOR_CLASSES.divider.overflowTop)).toBe(false)
    expect(overflowRuled()).toEqual([list])
  })
})

describe('state outlines (U-213)', () => {
  const STATE_OUTLINE = [
    'outline',
    '-outline-offset-1',
    ...classesOf(COLOR_CLASSES.forcedEdge.outline),
  ]

  it('U-213 outlines the selected browser row and rests the others with the transparent outline', () => {
    renderModelApp({ state: { selection: [bodyItem('b-cube', 'Cube')] } })
    const rows = within(screen.getByTestId('browser')).getAllByTestId('browser-row')
    const cube = rows.find((row) => row.getAttribute('data-id') === 'b-cube')
    expect(cube).toHaveClass(...STATE_OUTLINE, ...classesOf(COLOR_CLASSES.selection.background))
    for (const row of rows.filter((other) => other !== cube)) {
      expect(row).toHaveClass(...classesOf(COLOR_CLASSES.forcedEdge.outline))
      expect(row).not.toHaveClass('outline')
    }
    expect(doublyOutlined()).toEqual([])
  })

  it('U-213 outlines a selected timeline item, and the item being edited in the accent', async () => {
    const { user } = renderModelApp()
    const item = within(screen.getByTestId('timeline')).getByRole('button', { name: 'Extrude 1' })
    await user.click(item)
    expect(item).toHaveClass(...STATE_OUTLINE, ...classesOf(COLOR_CLASSES.selection.background))
    await user.dblClick(item)
    expect(item).toHaveClass('outline-2', '-outline-offset-2')
    expect(item).toHaveClass(...classesOf(COLOR_CLASSES.accent.outline))
    expect(item).not.toHaveClass('-outline-offset-1')
    expect(hasFacet(item, COLOR_CLASSES.forcedEdge.outline)).toBe(false)
    expect(doublyOutlined()).toEqual([])
  })

  it('U-213 outlines a viewport toggle once it is pressed', async () => {
    const { app, user } = renderModelApp()
    // The camera mirrors its projection in the store (3.13), as the fake viewport does here.
    const viewport = createFakeViewport()
    viewport.toggleProjection.mockImplementation(() => {
      app.store.setState((state) => ({ view: { ...state.view, projection: 'orthographic' } }))
    })
    app.attachViewport(viewport)
    const orthographic = within(screen.getByTestId('viewport-controls')).getByRole('button', {
      name: 'Orthographic',
    })
    expect(orthographic).toHaveAttribute('aria-pressed', 'false')
    expect(orthographic).not.toHaveClass('outline')
    await user.click(orthographic)
    expect(orthographic).toHaveAttribute('aria-pressed', 'true')
    expect(orthographic).toHaveClass(
      ...STATE_OUTLINE,
      ...classesOf(COLOR_CLASSES.selection.background),
    )
    expect(doublyOutlined()).toEqual([])
  })

  it('U-213 shows the pressed top bar view toggles with the neutral active fill, not selection', () => {
    renderModelApp({ state: { settings: { ...INITIAL_STATE.settings, theme: 'dark' } } })
    const topBar = within(screen.getByTestId('top-bar'))
    const toggles = [
      topBar.getByRole('button', { name: 'Toggle browser' }),
      screen.getByTestId('theme-toggle'),
    ]
    for (const toggle of toggles) {
      expect(toggle).toHaveAttribute('aria-pressed', 'true')
      expect(toggle).toHaveClass(...STATE_OUTLINE, ...classesOf(COLOR_CLASSES.hover.active))
      expect(toggle).toHaveClass(...classesOf(COLOR_CLASSES.text.foreground))
      expect(hasFacet(toggle, COLOR_CLASSES.selection.background)).toBe(false)
    }
  })

  it('U-213 outlines the active option of the palette and no other', async () => {
    const { user } = renderModelApp({
      state: { panels: { ...INITIAL_STATE.panels, paletteOpen: true } },
    })
    await user.type(screen.getByTestId('palette-input'), 'ext')
    const options = screen.getAllByTestId('palette-option')
    const active = options.find((option) => option.getAttribute('aria-selected') === 'true')
    expect(active).toHaveClass(...STATE_OUTLINE, ...classesOf(COLOR_CLASSES.hover.active))
    for (const option of options.filter((other) => other !== active)) {
      expect(option).not.toHaveClass('outline')
    }
    expect(doublyOutlined()).toEqual([])
  })

  it('U-213 outlines the keyboard-active item of the Document menu', async () => {
    const { user } = renderModelApp()
    await user.click(screen.getByTestId('document-name'))
    await user.keyboard('{ArrowDown}')
    const menu = screen.getByTestId('document-menu')
    const focused = document.activeElement
    expect(menu).toContainElement(focused instanceof HTMLElement ? focused : null)
    expect(focused).toHaveClass(...STATE_OUTLINE, ...classesOf(COLOR_CLASSES.hover.active))
    expect(doublyOutlined()).toEqual([])
  })
})
