import { cleanup, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { installPopoverPolyfill } from '../components/popover-polyfill.testing'
import type { AppState, SketchSession } from '../state/store'
import { cubeDocumentState, readyState } from './fixtures.testing'
import { createModelApp, renderWithApp, stubMatchMedia } from './test-app.testing'

// The context toolbar's fit to its width (ui.md 3.2), over a synthetic layout: jsdom lays nothing
// out, so each toolbar part gets a width from what it shows, side by side without gaps.

/** Widths in CSS pixels of a button with its label, of an icon-only button and of a separator. */
const LABELED = 100
const ICON = 36
const SEPARATOR = 9

let available = 0
function widthOf(element: Element): number {
  if (element.tagName === 'HR') {
    return SEPARATOR
  }
  // A labeled button shows its label as a text span; an icon-only one has none.
  return element.querySelector('span') === null ? ICON : LABELED
}

/** The parts of the toolbar that holds `element`, in order, which the fit measures. */
function partsBeside(element: Element): readonly Element[] {
  const toolbar = element.closest('[role="toolbar"]')
  return toolbar === null ? [] : [...toolbar.querySelectorAll('[data-toolbar-item], hr')]
}

function layOut(): void {
  vi.spyOn(HTMLElement.prototype, 'clientWidth', 'get').mockImplementation(function (
    this: HTMLElement,
  ) {
    return this.getAttribute('role') === 'toolbar' ? available : 0
  })
  vi.spyOn(HTMLElement.prototype, 'offsetWidth', 'get').mockImplementation(function (
    this: HTMLElement,
  ) {
    return widthOf(this)
  })
  vi.spyOn(HTMLElement.prototype, 'offsetHeight', 'get').mockReturnValue(ICON)
  vi.spyOn(Element.prototype, 'getBoundingClientRect').mockImplementation(function (this: Element) {
    const parts = partsBeside(this)
    const index = parts.indexOf(this)
    const left = parts.slice(0, Math.max(0, index)).reduce((sum, part) => sum + widthOf(part), 0)
    return DOMRect.fromRect({ x: left, y: 0, width: widthOf(this), height: ICON })
  })
}

beforeEach(installPopoverPolyfill)

beforeEach(() => {
  stubMatchMedia()
  layOut()
})

afterEach(() => {
  cleanup()
  vi.unstubAllGlobals()
  vi.restoreAllMocks()
})

function renderToolbar(width: number, state: Partial<AppState> = {}, name = 'Model tools') {
  available = width
  const test = createModelApp({ state: readyState(cubeDocumentState(), state) })
  renderWithApp(test.app)
  return { ...test, user: userEvent.setup(), toolbar: screen.getByRole('toolbar', { name }) }
}

/** The names of the toolbar's buttons and whether each shows its label. */
function shown(toolbar: HTMLElement): readonly (readonly [string, boolean])[] {
  return within(toolbar)
    .getAllByRole('button')
    .map((button) => [
      button.getAttribute('aria-label') ?? '',
      button.querySelector('span') !== null,
    ])
}

const MODEL_TOOLS = [
  'Sketch',
  'Extrude',
  'Revolve',
  'Hole',
  'Fillet',
  'Chamfer',
  'Shell',
  'Shapes',
  'Pattern',
  'Mirror',
  'Boolean',
]

const EDITING_SKETCH: SketchSession = {
  sketchId: 'f-sk1',
  tool: null,
  construction: false,
  showConstraints: true,
  selection: [],
  dragging: false,
  cameraBefore: null,
}

describe('the context toolbar fits its width (3.2)', () => {
  it('shows every label when the labeled toolbar fits', () => {
    const { toolbar } = renderToolbar(MODEL_TOOLS.length * LABELED)
    expect(shown(toolbar)).toEqual(MODEL_TOOLS.map((name) => [name, true]))
  })

  it('shows icons only when the labels do not fit', () => {
    const { toolbar } = renderToolbar(MODEL_TOOLS.length * LABELED - 1)
    expect(shown(toolbar)).toEqual(MODEL_TOOLS.map((name) => [name, false]))
  })

  it('keeps the label of Finish sketch while every other tool fits as an icon', () => {
    const { toolbar } = renderToolbar(
      600,
      { context: 'sketch', sketch: EDITING_SKETCH },
      'Sketch tools',
    )
    const labeled = shown(toolbar).filter(([, label]) => label)
    expect(labeled).toEqual([['Finish sketch', true]])
  })

  it('moves the last tools into More tools when even the icons do not fit, and lists them there', async () => {
    // Seven icons and More tools: 8 × 36 = 288.
    const { toolbar, user } = renderToolbar(300)
    expect(shown(toolbar).map(([name]) => name)).toEqual([...MODEL_TOOLS.slice(0, 7), 'More tools'])
    await user.click(within(toolbar).getByRole('button', { name: 'More tools' }))
    const menu = screen.getByRole('menu', { name: 'More tools' })
    expect(
      within(menu)
        .getAllByRole('menuitem')
        .map((item) => item.textContent),
    ).toEqual([
      expect.stringContaining('Shapes'),
      expect.stringContaining('Pattern'),
      expect.stringContaining('Mirror'),
      expect.stringContaining('Boolean'),
    ])
  })

  it('never scrolls, and its padding keeps the focus ring of every button whole (3.23)', () => {
    const { toolbar } = renderToolbar(300)
    expect(toolbar).toHaveClass('p-1', 'overflow-hidden')
    expect(toolbar).not.toHaveClass('overflow-x-auto')
  })

  it('keeps the label of Finish sketch and moves the last tools into More tools when they do not fit beside it', () => {
    // Finish sketch labeled, the separator, More tools and four icons: 100 + 9 + 5 × 36 = 289.
    const { toolbar } = renderToolbar(
      300,
      { context: 'sketch', sketch: EDITING_SKETCH },
      'Sketch tools',
    )
    expect(shown(toolbar)).toEqual([
      ['Line', false],
      ['Rectangle', false],
      ['Circle', false],
      ['Arc', false],
      ['Finish sketch', true],
      ['More tools', false],
    ])
  })

  it('shows Finish sketch as an icon only when its label does not fit beside More tools', () => {
    // Finish sketch as an icon, the separator, More tools and one icon: 9 + 3 × 36 = 117.
    const { toolbar } = renderToolbar(
      140,
      { context: 'sketch', sketch: EDITING_SKETCH },
      'Sketch tools',
    )
    expect(shown(toolbar)).toEqual([
      ['Line', false],
      ['Finish sketch', false],
      ['More tools', false],
    ])
  })

  it('never moves Finish sketch into More tools', () => {
    const { toolbar } = renderToolbar(
      3 * ICON + SEPARATOR,
      { context: 'sketch', sketch: EDITING_SKETCH },
      'Sketch tools',
    )
    const names = shown(toolbar).map(([name]) => name)
    expect(names).toContain('Finish sketch')
    expect(names.at(-1)).toBe('More tools')
  })

  it('opens the printer menu under More tools when Printer moved there (3.17)', async () => {
    // Only Export 3MF, the separator and More tools fit.
    const { toolbar, user } = renderToolbar(
      2 * ICON + SEPARATOR,
      { context: 'print' },
      'Print tools',
    )
    expect(shown(toolbar).map(([name]) => name)).toEqual(['Export 3MF', 'More tools'])
    expect(within(toolbar).queryByRole('button', { name: /^Printer/ })).toBeNull()
    await user.click(within(toolbar).getByRole('button', { name: 'More tools' }))
    await user.click(screen.getByRole('menuitem', { name: /Choose printer/ }))
    expect(screen.getByTestId('printer-menu')).toBeInTheDocument()
  })
})
