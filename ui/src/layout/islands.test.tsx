import { act, fireEvent, screen, within } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { notify } from '../app/actions/notifications'
import { installPopoverPolyfill } from '../components/popover-polyfill.testing'
import { DEFAULT_SETTINGS } from '../settings/settings'
import { sketchSession } from '../sketch/fixtures.testing'
import { createAppStore, INITIAL_STATE, type AppState } from '../state/store'
import { bodyItem } from '../test/fakes/selection'
import { checkResult, finding, printReport } from '../test/fakes/state'
import { COLOR_CLASSES } from '../theme/roles'
import {
  cubeDocumentState,
  FILLET_EDGES_COMMAND,
  rectSketchDocumentState,
} from './fixtures.testing'
import { renderModelApp, stubMatchMedia } from './test-app.testing'

// The floating islands over a full-window viewport (ui.md 3.2, 3.14, 3.23; ADR 0030).

beforeEach(installPopoverPolyfill)

beforeEach(() => {
  stubMatchMedia()
})

afterEach(() => {
  vi.unstubAllGlobals()
  vi.restoreAllMocks()
})

const PARAMETERS_OPEN: AppState['panels'] = {
  ...INITIAL_STATE.panels,
  browserOpen: true,
  side: 'parameters',
}

/** The classes of a role facet, one by one. */
function classesOf(classes: string): string[] {
  return classes.split(' ')
}

/** The classes of a frame island an element lacks: its shape, the surface, its outline and its shadow color. */
function missingIslandClasses(element: HTMLElement, corner: 'rounded-xl' | 'rounded-lg'): string[] {
  return [
    'relative',
    'z-10',
    corner,
    'border',
    'shadow-md',
    ...classesOf(COLOR_CLASSES.surface.background),
    ...classesOf(COLOR_CLASSES.border.border),
    ...classesOf(COLOR_CLASSES.elevation.shadow),
  ].filter((name) => !element.classList.contains(name))
}

function renderIslands() {
  return renderModelApp({
    state: { panels: PARAMETERS_OPEN, selection: [bodyItem('b-cube', 'Cube')] },
  })
}

/** Stubs the window width in CSS pixels and the root font size in pixels. */
function stubWindow(width: number, rootFontPixels = 16): void {
  vi.stubGlobal('innerWidth', width)
  const computed = window.getComputedStyle.bind(window)
  vi.spyOn(window, 'getComputedStyle').mockImplementation((element, pseudo) =>
    element === document.documentElement
      ? ({ fontSize: `${String(rootFontPixels)}px` } as CSSStyleDeclaration)
      : computed(element, pseudo),
  )
}

/** The app with the browser and the Problems panel open. */
function renderPanels() {
  return renderModelApp({ state: { panels: { ...PARAMETERS_OPEN, side: 'problems' } } })
}

/** Presses the status bar, outside both panels, and returns the panels state after it. */
function pressOutsidePanels(): AppState['panels'] {
  const { app } = renderPanels()
  fireEvent.pointerDown(screen.getByTestId('status-bar'))
  return app.store.getState().panels
}

/** The free-area layer: the last child of the viewport element (3.2). */
function freeAreaLayer(): HTMLElement {
  const layer = screen.getByTestId('viewport').lastElementChild
  if (!(layer instanceof HTMLElement)) {
    throw new Error('The viewport has no free-area layer.')
  }
  return layer
}

describe('islands (U-174)', () => {
  it('U-174 draws the top bar, browser, side panel and timeline as rounded-xl frame islands and the status bar rounded-lg', () => {
    renderIslands()
    for (const id of ['top-bar', 'browser', 'side-panel', 'timeline']) {
      expect(missingIslandClasses(screen.getByTestId(id), 'rounded-xl')).toEqual([])
    }
    expect(missingIslandClasses(screen.getByTestId('status-bar'), 'rounded-lg')).toEqual([])
  })

  it('U-174 rules a side panel header off its body with a divider hairline', () => {
    renderIslands()
    const title = within(screen.getByTestId('side-panel')).getByRole('heading', {
      name: 'Parameters',
    })
    const header = title.parentElement
    expect(header).toHaveClass('h-12', 'px-4', 'border-b')
    expect(header).toHaveClass(...classesOf(COLOR_CLASSES.divider.border))
  })

  it('U-174 makes the collapsed timeline rounded-lg', () => {
    renderIslands()
    fireEvent.click(screen.getByRole('button', { name: 'Collapse timeline' }))
    const timeline = screen.getByTestId('timeline')
    expect(timeline).toHaveClass('rounded-lg')
    expect(timeline).not.toHaveClass('rounded-xl')
  })

  it('U-174 scrolls the browser tree inside the island, never the island', () => {
    renderIslands()
    const browser = screen.getByTestId('browser')
    expect(browser).toHaveClass('flex', 'flex-col', 'overflow-hidden')
    expect(browser).not.toHaveClass('overflow-y-auto')
    const body = within(browser).getByRole('tree').parentElement
    expect(body).toHaveClass('min-h-0', 'flex-1', 'overflow-y-auto')
  })

  it('U-174 pads the app root by the gutter, with no grid gap, on the stage role', () => {
    renderIslands()
    const root = screen.getByTestId('app')
    expect(root).toHaveClass('p-2', '@container/app')
    for (const name of classesOf(COLOR_CLASSES.stage.background)) {
      expect(root).toHaveClass(name)
    }
    expect([...root.classList].filter((name) => name.startsWith('gap-'))).toEqual([])
  })

  it('U-174 keeps each island 0.5rem from the free area by a margin on the side facing it', () => {
    renderIslands()
    expect(screen.getByTestId('top-bar')).toHaveClass('mb-2')
    expect(screen.getByTestId('browser')).toHaveClass('mr-2')
    expect(screen.getByTestId('side-panel')).toHaveClass('ml-2')
    expect(screen.getByTestId('timeline')).toHaveClass('mt-2')
    expect(screen.getByTestId('status-bar')).toHaveClass('mt-2')
  })

  it('U-174 leaves the viewport unpositioned, with the canvas holder filling the window and the free-area layer last', () => {
    renderIslands()
    const viewport = screen.getByTestId('viewport')
    for (const name of ['relative', 'absolute', 'fixed']) {
      expect(viewport).not.toHaveClass(name)
    }
    const holder = viewport.querySelector('canvas')?.parentElement
    expect(holder).toHaveClass('absolute', 'inset-0', 'peer')
    const layer = freeAreaLayer()
    expect(layer).toHaveClass(
      'pointer-events-none',
      'relative',
      'isolate',
      'size-full',
      'rounded-xl',
      'peer-focus-visible:outline-2',
      'peer-focus-visible:-outline-offset-2',
    )
    for (const name of classesOf(COLOR_CLASSES.accent.outline)) {
      expect(layer).toHaveClass(name)
    }
    for (const id of ['view-cube', 'viewport-controls', 'inspector']) {
      expect(layer).toContainElement(screen.getByTestId(id))
    }
  })

  it('U-174 puts the notifications region after the status bar, over the free area and above the overlay islands', () => {
    const { app } = renderIslands()
    act(() => {
      notify(app, { severity: 'error', message: 'Something broke.', actions: [] })
    })
    const card = screen.getByTestId('notification')
    const region = card.parentElement
    expect(region).toHaveClass('relative', 'z-30', 'col-start-2', 'row-start-2')
    const statusRegion = screen.getByTestId('status-bar')
    expect(
      statusRegion.compareDocumentPosition(region ?? document.body) &
        Node.DOCUMENT_POSITION_FOLLOWING,
    ).toBeTruthy()
  })

  it('U-174 portals the sketch layer outside the free-area layer and keeps the sketch header in it', () => {
    renderModelApp({
      document: rectSketchDocumentState(),
      state: { context: 'sketch', sketch: sketchSession() },
    })
    const viewport = screen.getByTestId('viewport')
    const layer = freeAreaLayer()
    const overlay = screen.getByTestId('sketch-overlay')
    expect(layer).not.toContainElement(overlay)
    const host = overlay.parentElement
    expect(host?.parentElement).toBe(viewport)
    expect(host).toHaveClass('pointer-events-none', 'absolute', 'inset-0')
    expect(layer).toContainElement(screen.getByTestId('sketch-header'))
  })
})

describe('docking by the window width in rem (U-175)', () => {
  it('U-175 floats the panels at 1000 pixels (62.5rem): a press outside closes both, Esc a reopened browser', () => {
    stubWindow(1000)
    const { app } = renderPanels()
    fireEvent.pointerDown(screen.getByTestId('status-bar'))
    expect(app.store.getState().panels.browserOpen).toBe(false)
    expect(app.store.getState().panels.side).toBeNull()
    act(() => {
      app.store.setState((state) => ({ panels: { ...state.panels, browserOpen: true } }))
    })
    fireEvent.keyDown(window, { key: 'Escape' })
    expect(app.store.getState().panels.browserOpen).toBe(false)
  })

  it('U-175 floats the panels at 1030 pixels, a content box of 63.375rem inside the gutter', () => {
    stubWindow(1030)
    expect(pressOutsidePanels()).toMatchObject({ browserOpen: false, side: null })
  })

  it('U-175 docks the panels at 1100 pixels: neither a press outside nor Esc closes them', () => {
    stubWindow(1100)
    const { app } = renderPanels()
    fireEvent.pointerDown(screen.getByTestId('status-bar'))
    fireEvent.keyDown(window, { key: 'Escape' })
    expect(app.store.getState().panels.browserOpen).toBe(true)
    expect(app.store.getState().panels.side).toBe('problems')
  })

  it('U-175 floats the panels at 1280 pixels with a 32 pixel root font (40rem)', () => {
    stubWindow(1280, 32)
    expect(pressOutsidePanels()).toMatchObject({ browserOpen: false, side: null })
  })

  it.each([
    [1000, 1, false],
    [1030, 1, false],
    [1280, 2, false],
    [1040, 1, true],
    [1280, 1, true],
  ] as const)(
    'U-175 createAppStore at %d pixels and a stored UI scale of %d starts with the browser open: %s',
    (width, uiScale, open) => {
      stubWindow(width)
      const store = createAppStore({ settings: { ...DEFAULT_SETTINGS, uiScale } })
      expect(store.getState().panels.browserOpen).toBe(open)
    },
  )

  it('U-175 gives the browser and the side panel the overlay island classes below the container width', () => {
    renderPanels()
    const overlay = [
      '@max-5xl/app:absolute',
      '@max-5xl/app:inset-y-0',
      '@max-5xl/app:z-20',
      '@max-5xl/app:shadow-lg',
      '@max-5xl/app:col-start-2',
      '@max-5xl/app:col-end-3',
      '@max-5xl/app:row-end-3',
      'max-w-full',
    ]
    expect(screen.getByTestId('browser')).toHaveClass(...overlay, '@max-5xl/app:left-0')
    expect(screen.getByTestId('side-panel')).toHaveClass(...overlay, '@max-5xl/app:right-0')
  })
})

describe('corners and slot (U-186)', () => {
  it('U-186 puts the controls first in the free area left column and the overhang legend last', () => {
    const report = printReport({
      'b-cube': [checkResult('overhangs', [finding('overhangs', 'warning')])],
    })
    renderModelApp({
      state: {
        context: 'print',
        print: { ...INITIAL_STATE.print, report, reportRevision: cubeDocumentState().revision },
      },
    })
    const column = screen.getByTestId('viewport-controls').parentElement
    expect(column).toHaveClass(
      'pointer-events-none',
      'absolute',
      'inset-y-4',
      'left-4',
      'flex',
      'flex-col',
      'flex-wrap',
      'content-start',
      'items-start',
      'gap-2',
    )
    expect(column?.firstElementChild).toBe(screen.getByTestId('viewport-controls'))
    const legend = screen.getByTestId('overhang-legend')
    expect(column?.lastElementChild).toBe(legend)
    expect(legend).toHaveClass('mt-auto', 'rounded-xl', 'border', 'shadow-md')
  })

  it('U-186 shows the empty state as a card in the top-center slot', () => {
    renderModelApp({
      document: cubeDocumentState({
        recipe: { ...cubeDocumentState().recipe, features: [] },
      }),
    })
    const card = screen.getByTestId('empty-state')
    expect(card).toHaveRole('group')
    expect(card).toHaveAccessibleName('First step')
    expect(card).toHaveClass('pointer-events-auto', 'rounded-xl', 'border', 'shadow-md', 'p-3')
    for (const name of [
      ...classesOf(COLOR_CLASSES.surfaceRaised.background),
      ...classesOf(COLOR_CLASSES.elevation.shadow),
    ]) {
      expect(card).toHaveClass(name)
    }
    const column = card.parentElement
    expect(column).toHaveAttribute('data-testid', 'top-center-slot')
    expect(column?.parentElement).toHaveClass('pointer-events-none', 'absolute', 'inset-4')
  })

  it('U-186 shows the print hint as a one-line chip in the top-center slot', () => {
    renderModelApp({
      state: { context: 'print', print: { ...INITIAL_STATE.print, pickingFace: true } },
    })
    const hint = screen.getByTestId('print-hint')
    expect(hint).toHaveClass('rounded-lg', 'border', 'shadow-md')
    expect(hint.parentElement).toHaveAttribute('data-testid', 'top-center-slot')
  })
})

// Every source file, to look for a class no file may use; the needle is assembled so this file
// does not contain it itself.
const SOURCES = import.meta.glob<string>(['/src/**/*.{ts,tsx}', '!/src/generated/**'], {
  query: '?raw',
  import: 'default',
  eager: true,
})
const SMALL_SHADOW = ['shadow', 'sm'].join('-')

describe('shapes of the shell (U-188)', () => {
  it('U-188 gives the inspector and a notification the card shape', () => {
    const { app } = renderModelApp({ state: { selection: [bodyItem('b-cube', 'Cube')] } })
    act(() => {
      notify(app, { severity: 'info', message: 'Saved.', actions: [] })
    })
    // The inspector card never scrolls as a whole; its body under the header does.
    const inspector = screen.getByTestId('inspector')
    expect(inspector).toHaveClass('flex', 'flex-col', 'overflow-hidden')
    expect(inspector.lastElementChild).toHaveClass('min-h-0', 'flex-1', 'overflow-y-auto')
    expect(inspector.lastElementChild).toContainElement(screen.getByTestId('selection-title'))
    for (const card of [inspector, screen.getByTestId('notification')]) {
      expect(card).toHaveClass('rounded-xl', 'border', 'shadow-lg')
      for (const name of classesOf(COLOR_CLASSES.elevation.shadow)) {
        expect(card).toHaveClass(name)
      }
    }
  })

  it('U-188 keeps the command footer in view under the scrolling body of the inspector (3.7)', () => {
    renderModelApp({ state: { command: FILLET_EDGES_COMMAND } })
    const inspector = screen.getByTestId('inspector')
    const footer = screen.getByTestId('inspector-ok').closest('div.border-t')
    expect(footer?.parentElement).toBe(inspector)
    expect(footer).toBe(inspector.lastElementChild)
    expect(footer).toHaveClass('shrink-0', ...classesOf(COLOR_CLASSES.divider.border))
    const body = footer?.previousElementSibling
    expect(body).toHaveClass('min-h-0', 'flex-1', 'overflow-y-auto')
    expect(body).toContainElement(screen.getAllByTestId('inspector-field')[0] ?? null)
  })

  it('U-188 gives the palette dialog the card shape', () => {
    renderModelApp({ state: { panels: { ...INITIAL_STATE.panels, paletteOpen: true } } })
    const palette = screen.getByTestId('palette')
    expect(palette).toHaveClass('rounded-xl', 'border', 'shadow-lg')
    expect(palette).toHaveClass(...classesOf(COLOR_CLASSES.elevation.shadow))
  })

  it('U-188 gives every rendered shadow the elevation role, so it shows in the dark theme too', () => {
    const { app } = renderModelApp({
      state: {
        selection: [bodyItem('b-cube', 'Cube')],
        panels: { ...INITIAL_STATE.panels, paletteOpen: true, side: 'parameters' },
      },
    })
    act(() => {
      notify(app, { severity: 'info', message: 'Saved.', actions: [] })
    })
    const shadowed = Array.from(
      document.querySelectorAll('[class*="shadow-md"], [class*="shadow-lg"]'),
    )
    // The frame islands, the inspector, the notification, the palette and the viewport controls.
    expect(shadowed.length).toBeGreaterThanOrEqual(8)
    for (const element of shadowed) {
      expect(element).toHaveClass(...classesOf(COLOR_CLASSES.elevation.shadow))
    }
  })

  it('U-188 draws the focus rings of the status bar and the collapsed timeline inside their controls', () => {
    renderModelApp({
      state: { panels: { ...INITIAL_STATE.panels, timelineCollapsed: true } },
    })
    const inset = classesOf(COLOR_CLASSES.focusRing.inset)
    const outline = classesOf(COLOR_CLASSES.focusRing.outline)
    const buttons = [
      ...within(screen.getByTestId('status-bar')).getAllByRole('button'),
      ...within(screen.getByTestId('timeline')).getAllByRole('button'),
    ]
    expect(buttons.length).toBeGreaterThan(2)
    for (const button of buttons) {
      expect(button).toHaveClass(...inset)
      expect(outline.every((name) => button.classList.contains(name))).toBe(false)
    }
  })

  it('U-188 uses the smallest Tailwind shadow class nowhere', () => {
    const offenders = Object.entries(SOURCES)
      .filter(([, text]) => text.includes(SMALL_SHADOW))
      .map(([path]) => path)
    expect(offenders).toEqual([])
  })
})
