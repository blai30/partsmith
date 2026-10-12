import { cleanup, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { installPopoverPolyfill } from '../components/popover-polyfill.testing'
import type { Edit } from '../generated/protocol'
import type { ActiveCommand, AppState } from '../state/store'
import { CUBE_TOP_FRONT_EDGE, RECT_REGION } from '../test/fakes/documents'
import { settle } from '../test/fakes/settle'
import { cubeDocumentState, readyState, rectSketchDocumentState } from './fixtures.testing'
import { createModelApp, renderWithApp, stubMatchMedia } from './test-app.testing'

// Typing values (ui.md 3.7, U-104): a value typed on the viewport or a selection field box goes to
// the command's value field, so pick, type 2, Enter makes a 2 mm fillet.

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

const FILLET: ActiveCommand = {
  featureType: 'fillet',
  featureId: 'f-fil9',
  editing: false,
  title: 'Fillet',
  params: { edges: [CUBE_TOP_FRONT_EDGE.reference], body: 'b-cube', radius: '1 mm' },
  activeField: 'edges',
  fieldErrors: {},
  applied: false,
  newBody: null,
  helper: null,
  changedPaths: [],
  history: [],
  future: [],
  preview: 'none',
}

const EXTRUDE: ActiveCommand = {
  ...FILLET,
  featureType: 'extrude',
  featureId: 'f-ext9',
  title: 'Extrude',
  params: {
    profile: { sketch: 'f-sk1', regions: [RECT_REGION.reference] },
    extent: { kind: 'two_sides', expr: '10 mm', opposite_expr: '4 mm' },
    op: 'new',
  },
  activeField: 'profile',
}

function renderApp(state: Partial<AppState>, document = cubeDocumentState()) {
  const test = createModelApp({ state: readyState(document, state) })
  renderWithApp(test.app)
  const viewport = within(screen.getByTestId('viewport')).getByRole('application')
  return { ...test, viewport, user: userEvent.setup() }
}

function lastFeatureEdit(edits: readonly (readonly unknown[])[]): Edit | undefined {
  const calls = edits.map((call) => call[2] as readonly Edit[])
  return calls
    .flat()
    .findLast((edit) => edit.type === 'add_feature' || edit.type === 'update_feature')
}

describe('typing values (U-104)', () => {
  it('U-104 with the viewport focused, 2 and Enter commit a fillet of radius 2', async () => {
    const { app, core, viewport, user } = renderApp({ command: FILLET })
    viewport.focus()
    await user.keyboard('2')
    const radius = screen.getByRole<HTMLInputElement>('combobox', { name: 'Radius' })
    expect(radius).toHaveFocus()
    expect(radius).toHaveValue('2')
    expect(radius.selectionStart).toBe(1)
    await user.keyboard('{Enter}')
    await waitFor(() => {
      expect(core.callsTo('commitTransaction')).toEqual([[1]])
    })
    const edit = lastFeatureEdit(core.callsTo('applyEdits'))
    expect(
      edit?.type === 'add_feature' || edit?.type === 'update_feature'
        ? edit.feature['radius']
        : null,
    ).toBe('2')
    expect(app.store.getState().command).toBeNull()
  })

  it('U-104 .5 goes to the first essential expression field of an Extrude', async () => {
    const { app, viewport, user } = renderApp({ command: EXTRUDE }, rectSketchDocumentState())
    viewport.focus()
    await user.keyboard('.5')
    expect(getExtent(app.store.getState())).toMatchObject({ expr: '.5' })
  })

  it('U-104 after the user edited Opposite distance, a typed value goes there', async () => {
    const { app, viewport, user } = renderApp(
      { command: { ...EXTRUDE, changedPaths: ['extent.opposite_expr'] } },
      rectSketchDocumentState(),
    )
    viewport.focus()
    await user.keyboard('3')
    expect(getExtent(app.store.getState())).toMatchObject({ expr: '10 mm', opposite_expr: '3' })
  })

  it('U-104 a letter types nothing and keeps focus on the viewport', async () => {
    const { app, viewport, user } = renderApp({ command: FILLET })
    viewport.focus()
    await user.keyboard('e')
    expect(viewport).toHaveFocus()
    expect(app.store.getState().command?.params['radius']).toBe('1 mm')
  })

  it('U-104 Tab on the viewport selects the value field’s text', async () => {
    const { viewport, user } = renderApp({ command: FILLET })
    viewport.focus()
    await user.keyboard('{Tab}')
    const radius = screen.getByRole<HTMLInputElement>('combobox', { name: 'Radius' })
    expect(radius).toHaveFocus()
    expect(radius.selectionStart).toBe(0)
    expect(radius.selectionEnd).toBe('1 mm'.length)
  })

  it('takes a value typed on a selection field box', async () => {
    const { app, user } = renderApp({ command: FILLET })
    const box = screen.getByTestId('selection-field')
    box.focus()
    await user.keyboard('(')
    expect(app.store.getState().command?.params['radius']).toBe('(')
    expect(screen.getByRole('combobox', { name: 'Radius' })).toHaveFocus()
    await settle()
  })

  it('records the typed value as one change of the command', async () => {
    const { app, viewport, user } = renderApp({ command: FILLET })
    viewport.focus()
    await user.keyboard('25')
    expect(app.store.getState().command?.params['radius']).toBe('25')
    expect(app.store.getState().command?.history.map((step) => step.field)).toEqual(['Radius'])
  })
})

function getExtent(state: AppState): unknown {
  return state.command?.params['extent']
}
