import { cleanup, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { installPopoverPolyfill } from '../components/popover-polyfill.testing'
import type { Document } from '../generated/protocol'
import { cubeDocumentState, documentState, readyState } from '../layout/fixtures.testing'
import { createModelApp, renderWithApp, stubMatchMedia } from '../layout/test-app.testing'
import { INITIAL_STATE, type ActiveCommand, type AppState, type DocumentState } from '../state/store'
import { CUBE_DOCUMENT, PARAMETRIC_DOCUMENT } from '../test/fakes/documents'
import { settle } from '../test/fakes/settle'
import { BODY_SWATCHES, FONT_CLASSES, VIEWPORT_COLORS } from '../theme/roles'

let uninstallPopovers: () => void

beforeEach(() => {
  uninstallPopovers = installPopoverPolyfill()
  stubMatchMedia()
})

afterEach(() => {
  cleanup()
  uninstallPopovers()
  vi.unstubAllGlobals()
})

function renderBrowser(document: DocumentState = cubeDocumentState(), state: Partial<AppState> = {}) {
  const test = createModelApp({ state: readyState(document, state) })
  renderWithApp(test.app)
  return { ...test, user: userEvent.setup(), browser: screen.getByTestId('browser') }
}

function row(kind: string, id: string): HTMLElement {
  const found = screen
    .getAllByTestId('browser-row')
    .find((element) => element.getAttribute('data-kind') === kind && element.getAttribute('data-id') === id)
  if (found === undefined) {
    throw new Error(`No browser row ${kind} ${id}`)
  }
  return found
}

const DATUM_DOCUMENT: Document = {
  ...CUBE_DOCUMENT,
  features: [
    ...CUBE_DOCUMENT.features,
    { id: 'f-dp1', type: 'datum_plane', base: { kind: 'origin', plane: 'XY' }, offset: '10 mm' },
  ],
}

const DATUM_COMMAND: ActiveCommand = {
  featureType: 'datum_plane',
  featureId: 'f-dp9',
  editing: false,
  title: 'Datum plane',
  params: { offset: '10 mm' },
  activeField: 'base',
  fieldErrors: {},
  applied: false,
  newBody: null,
  preview: 'none',
}

describe('the browser tree (3.10)', () => {
  it('is a tree named "Browser" with the groups Origin, Bodies, Sketches, Construction and Parameters', () => {
    const { browser } = renderBrowser(documentState(PARAMETRIC_DOCUMENT))
    const tree = within(browser).getByRole('tree', { name: 'Browser' })
    for (const name of ['Origin', 'Bodies', 'Sketches', 'Construction', 'Parameters']) {
      expect(within(tree).getByText(name)).toBeInTheDocument()
    }
  })

  it('starts with Origin collapsed and the other groups expanded', () => {
    const { browser } = renderBrowser()
    const kinds = within(browser)
      .queryAllByTestId('browser-row')
      .map((element) => element.getAttribute('data-kind'))
    expect(kinds).not.toContain('origin_plane')
    expect(row('body', 'b-cube')).toBeInTheDocument()
    expect(row('sketch', 'f-sk1')).toBeInTheDocument()
  })

  it('expands Origin to the three planes, three axes and the origin point', async () => {
    const { user, browser } = renderBrowser()
    await user.click(within(browser).getByText('Origin'))
    for (const id of ['XY', 'XZ', 'YZ']) {
      expect(row('origin_plane', id)).toHaveTextContent(`${id} plane`)
    }
    for (const id of ['X', 'Y', 'Z']) {
      expect(row('origin_axis', id)).toHaveTextContent(`${id} axis`)
    }
    expect(row('origin_point', 'O')).toHaveTextContent('Origin')
  })

  it('lists datum planes under Construction and parameters as "{name} = {expr}"', () => {
    renderBrowser(documentState({ ...DATUM_DOCUMENT, parameters: PARAMETRIC_DOCUMENT.parameters }))
    expect(row('datum', 'f-dp1')).toHaveTextContent('Datum plane 1')
    expect(row('parameter', 'p-size')).toHaveTextContent('size = 20 mm')
  })

  it('shows the evaluated value of a parameter that is not a plain number, in the values font', () => {
    const document: Document = {
      ...PARAMETRIC_DOCUMENT,
      parameters: [{ id: 'p-size', name: 'size', expr: '2 * 10 mm' }],
    }
    renderBrowser(
      documentState(document, {
        parameterValues: { 'p-size': { id: 'p-size', value: 20, dimension: { length: 1, angle: 0 } } },
      }),
    )
    const value = within(row('parameter', 'p-size')).getByText('20 mm')
    expect(value).toHaveClass(...FONT_CLASSES.values.split(' '))
  })
})

describe('body rows (3.10)', () => {
  it('have color, lock and eye buttons named after the body', () => {
    renderBrowser()
    const body = row('body', 'b-cube')
    expect(body).toHaveTextContent('Cube')
    expect(within(body).getByRole('button', { name: 'Color of Cube', hidden: true })).toBeInTheDocument()
    expect(within(body).getByRole('button', { name: 'Lock Cube', hidden: true })).toHaveAttribute('aria-pressed', 'false')
    expect(within(body).getByRole('button', { name: 'Hide Cube', hidden: true })).toBeInTheDocument()
  })

  it('marks a body without model-layer geometry as "(empty)"', () => {
    renderBrowser(cubeDocumentState({ meshes: {} }))
    expect(row('body', 'b-cube')).toHaveTextContent('(empty)')
  })

  it('draws the body color as an SVG rect fill, the default body color without one', () => {
    renderBrowser()
    const swatch = within(row('body', 'b-cube')).getByRole('button', { name: 'Color of Cube', hidden: true })
    expect(swatch.querySelector('rect')?.getAttribute('fill')).toBe(VIEWPORT_COLORS.light.body)
    cleanup()
    const red: Document = { ...CUBE_DOCUMENT, bodies: [{ id: 'b-cube', name: 'Cube', color: '#e7000b' }] }
    renderBrowser(cubeDocumentState({ recipe: red }))
    const redSwatch = within(row('body', 'b-cube')).getByRole('button', { name: 'Color of Cube', hidden: true })
    expect(redSwatch.querySelector('rect')?.getAttribute('fill')).toBe('#e7000b')
  })

  it('selects a body on click', async () => {
    const { app, user } = renderBrowser()
    await user.click(within(row('body', 'b-cube')).getByText('Cube'))
    expect(app.store.getState().selection.map((item) => item.key)).toEqual(['body:b-cube'])
  })

  it('hides a body with the eye: an undoable "Hide {name}" edit', async () => {
    const { core, user } = renderBrowser()
    await user.click(within(row('body', 'b-cube')).getByRole('button', { name: 'Hide Cube', hidden: true }))
    await settle()
    expect(core.callsTo('applyEdits')).toEqual([
      [1, 'Hide Cube', [{ type: 'update_body', body: { id: 'b-cube', name: 'Cube', visible: false } }]],
    ])
  })

  it('shows a hidden body with "Show {name}"', async () => {
    const hidden: Document = { ...CUBE_DOCUMENT, bodies: [{ id: 'b-cube', name: 'Cube', visible: false }] }
    const { core, user } = renderBrowser(cubeDocumentState({ recipe: hidden }))
    await user.click(within(row('body', 'b-cube')).getByRole('button', { name: 'Show Cube', hidden: true }))
    await settle()
    const [[, label, edits] = []] = core.callsTo('applyEdits')
    expect(label).toBe('Show Cube')
    const [edit] = edits as readonly { type: string; body: { id: string; visible?: boolean } }[]
    expect(edit?.type).toBe('update_body')
    expect(edit?.body.visible ?? true).toBe(true)
  })

  it('locks a body for the session only', async () => {
    const { app, core, user } = renderBrowser()
    const lock = within(row('body', 'b-cube')).getByRole('button', { name: 'Lock Cube', hidden: true })
    await user.click(lock)
    expect(app.store.getState().view.locked).toEqual(['b-cube'])
    expect(within(row('body', 'b-cube')).getByRole('button', { name: 'Unlock Cube', hidden: true })).toHaveAttribute(
      'aria-pressed',
      'true',
    )
    expect(core.callsTo('applyEdits')).toEqual([])
  })

  it('offers the body swatches and commits a choice as "Change color"', async () => {
    const { core, user } = renderBrowser()
    await user.click(within(row('body', 'b-cube')).getByRole('button', { name: 'Color of Cube', hidden: true }))
    const popover = screen.getByTestId('color-popover')
    for (const swatch of BODY_SWATCHES) {
      expect(within(popover).getByRole('button', { name: swatch.name })).toBeInTheDocument()
    }
    await user.click(within(popover).getByRole('button', { name: 'Red' }))
    await settle()
    expect(core.callsTo('applyEdits')).toEqual([
      [1, 'Change color', [{ type: 'update_body', body: { id: 'b-cube', name: 'Cube', color: '#e7000b' } }]],
    ])
  })

  it('removes the color with the Default swatch', async () => {
    const red: Document = { ...CUBE_DOCUMENT, bodies: [{ id: 'b-cube', name: 'Cube', color: '#e7000b' }] }
    const { core, user } = renderBrowser(cubeDocumentState({ recipe: red }))
    await user.click(within(row('body', 'b-cube')).getByRole('button', { name: 'Color of Cube', hidden: true }))
    await user.click(within(screen.getByTestId('color-popover')).getByRole('button', { name: 'Default' }))
    await settle()
    expect(core.callsTo('applyEdits')).toEqual([
      [1, 'Change color', [{ type: 'update_body', body: { id: 'b-cube', name: 'Cube' } }]],
    ])
  })

  it('accepts a #rrggbb color in the hex field', async () => {
    const { core, user } = renderBrowser()
    await user.click(within(row('body', 'b-cube')).getByRole('button', { name: 'Color of Cube', hidden: true }))
    const field = within(screen.getByTestId('color-popover')).getByRole('textbox')
    expect(field).toHaveClass(...FONT_CLASSES.values.split(' '))
    await user.clear(field)
    await user.type(field, '#123456{Enter}')
    await settle()
    expect(core.callsTo('applyEdits')).toEqual([
      [1, 'Change color', [{ type: 'update_body', body: { id: 'b-cube', name: 'Cube', color: '#123456' } }]],
    ])
  })

  it('renames a body inline on double-click, labeled "Rename"', async () => {
    const { core, user } = renderBrowser()
    await user.dblClick(within(row('body', 'b-cube')).getByText('Cube'))
    const field = within(screen.getByTestId('browser')).getByRole('textbox')
    await user.clear(field)
    await user.type(field, 'Block{Enter}')
    await settle()
    expect(core.callsTo('applyEdits')).toEqual([
      [1, 'Rename', [{ type: 'update_body', body: { id: 'b-cube', name: 'Block' } }]],
    ])
  })
})

describe('sketch, datum and origin rows (3.10)', () => {
  it('shows and hides a sketch for the session only', async () => {
    const { app, core, user } = renderBrowser(cubeDocumentState(), {
      view: { ...INITIAL_STATE.view, sessionVisible: { 'feature:f-sk1': false } },
    })
    await user.click(within(row('sketch', 'f-sk1')).getByRole('button', { name: 'Show Sketch 1', hidden: true }))
    expect(app.store.getState().view.sessionVisible['feature:f-sk1']).toBe(true)
    expect(core.callsTo('applyEdits')).toEqual([])
  })

  it('selects a sketch row as a feature and a datum row as a datum plane', async () => {
    const { app, user } = renderBrowser(documentState(DATUM_DOCUMENT))
    await user.click(within(row('sketch', 'f-sk1')).getByText('Sketch 1'))
    expect(app.store.getState().selection).toEqual([expect.objectContaining({ key: 'feature:f-sk1', kind: 'feature' })])
    await user.click(within(row('datum', 'f-dp1')).getByText('Datum plane 1'))
    expect(app.store.getState().selection).toEqual([
      expect.objectContaining({ key: 'feature:f-dp1', kind: 'datum_plane' }),
    ])
  })

  it('enters the sketch on double-click', async () => {
    const { app, user } = renderBrowser()
    await user.dblClick(within(row('sketch', 'f-sk1')).getByText('Sketch 1'))
    await settle()
    expect(app.store.getState().context).toBe('sketch')
    expect(app.store.getState().sketch?.sketchId).toBe('f-sk1')
  })

  it('edits a datum plane on double-click', async () => {
    const { core, user } = renderBrowser(documentState(DATUM_DOCUMENT))
    await user.dblClick(within(row('datum', 'f-dp1')).getByText('Datum plane 1'))
    await settle()
    expect(core.callsTo('beginTransaction')).toEqual([[1, 'Edit Datum plane 1']])
  })

  it('captures an origin plane row into the active plane field of a command', async () => {
    const { app, user, browser } = renderBrowser(cubeDocumentState(), { command: DATUM_COMMAND })
    await user.click(within(browser).getByText('Origin'))
    await user.click(within(row('origin_plane', 'XZ')).getByText('XZ plane'))
    await settle()
    expect(app.store.getState().command?.params.base).toEqual({ kind: 'origin', plane: 'XZ' })
    expect(app.store.getState().selection).toEqual([])
  })

  it('selects an origin plane row outside a command', async () => {
    const { app, user, browser } = renderBrowser()
    await user.click(within(browser).getByText('Origin'))
    await user.click(within(row('origin_plane', 'XY')).getByText('XY plane'))
    expect(app.store.getState().selection).toEqual([
      expect.objectContaining({ key: 'origin:XY', kind: 'origin_plane', origin: 'XY' }),
    ])
  })
})

describe('parameters group (3.10)', () => {
  it('opens the Parameters panel from "Edit parameters"', async () => {
    const { app, user, browser } = renderBrowser(documentState(PARAMETRIC_DOCUMENT))
    await user.click(within(browser).getByRole('button', { name: 'Edit parameters', hidden: true }))
    expect(app.store.getState().panels.side).toBe('parameters')
  })

  it('opens the Parameters panel on a double-click of a parameter row', async () => {
    const { app, user } = renderBrowser(documentState(PARAMETRIC_DOCUMENT))
    await user.dblClick(within(row('parameter', 'p-size')).getByText('size = 20 mm'))
    expect(app.store.getState().panels.side).toBe('parameters')
  })
})

describe('keyboard (3.10)', () => {
  it('moves between rows with ArrowDown and ArrowUp as a tree', async () => {
    const { user, browser } = renderBrowser()
    const tree = within(browser).getByRole('tree')
    const items = within(tree).getAllByRole('treeitem')
    const focusable = items.filter((element) => element.tabIndex === 0)
    expect(focusable).toHaveLength(1)
    focusable[0]?.focus()
    await user.keyboard('{ArrowDown}')
    expect(document.activeElement).not.toBe(focusable[0])
    expect(document.activeElement?.getAttribute('role')).toBe('treeitem')
    await user.keyboard('{ArrowUp}')
    expect(document.activeElement).toBe(focusable[0])
  })

  it('takes no row buttons into the tab order', () => {
    const { browser } = renderBrowser()
    const buttons = within(browser).queryAllByRole('button', { hidden: true })
    const tabbable = buttons.filter(
      (button) => button.closest('[data-testid="browser-row"]') !== null && button.tabIndex !== -1,
    )
    expect(tabbable).toEqual([])
  })
})
