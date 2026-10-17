import { act, cleanup, fireEvent, screen, waitFor, within } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { commandComplete } from '../app/actions/command'
import { installPopoverPolyfill } from '../components/popover-polyfill.testing'
import type { ActiveCommand, AppState, SketchSession } from '../state/store'
import { deferred } from '../test/fakes/deferred'
import { CUBE_TOP_FRONT_EDGE, RECT_REGION } from '../test/fakes/documents'
import { bodyItem, elementItem, sketchLineItem } from '../test/fakes/selection'
import { settle } from '../test/fakes/settle'
import { cubeDocumentState, readyState, rectSketchDocumentState } from './fixtures.testing'
import { escapeStep } from './ladders'
import { createModelApp, renderModelApp, renderWithApp, stubMatchMedia } from './test-app.testing'

beforeEach(installPopoverPolyfill)

beforeEach(() => {
  stubMatchMedia()
})

afterEach(() => {
  cleanup()
  vi.unstubAllGlobals()
})

/** A new extrude whose required profile is set, so it can be confirmed. */
const COMPLETE_EXTRUDE: ActiveCommand = {
  featureType: 'extrude',
  featureId: 'f-new1',
  editing: false,
  title: 'Extrude',
  params: {
    profile: { sketch: 'f-sk1', regions: [RECT_REGION.reference] },
    extent: { kind: 'distance', expr: '10 mm' },
    op: 'new',
  },
  activeField: null,
  fieldErrors: {},
  applied: true,
  newBody: null,
  helper: null,
  changedPaths: [],
  history: [],
  future: [],
  preview: 'shown',
}

const INCOMPLETE_EXTRUDE: ActiveCommand = {
  ...COMPLETE_EXTRUDE,
  params: { extent: { kind: 'distance', expr: '10 mm' }, op: 'new' },
  activeField: 'profile',
  applied: false,
  preview: 'none',
}

const EDITING_SKETCH: SketchSession = {
  sketchId: 'f-sk1',
  reattaching: null,
  tool: null,
  construction: false,
  showConstraints: true,
  selection: [],
  dragging: false,
  cameraBefore: null,
}

function renderApp(state: Partial<AppState>) {
  return renderModelApp({ document: rectSketchDocumentState(), state })
}

function viewportSurface(): HTMLElement {
  return within(screen.getByTestId('viewport')).getByRole('application')
}

describe('commandComplete', () => {
  it('is false without a command and true once every required field is set', () => {
    const { app } = createModelApp({ state: readyState(rectSketchDocumentState()) })
    expect(commandComplete(app.store.getState())).toBe(false)
    app.store.setState({ command: INCOMPLETE_EXTRUDE })
    expect(commandComplete(app.store.getState())).toBe(false)
    app.store.setState({ command: COMPLETE_EXTRUDE })
    expect(commandComplete(app.store.getState())).toBe(true)
  })

  it('is false while a field has an error', () => {
    const { app } = createModelApp({
      state: readyState(rectSketchDocumentState(), {
        command: { ...COMPLETE_EXTRUDE, fieldErrors: { 'extent.expr': 'Distance needs a value.' } },
      }),
    })
    expect(commandComplete(app.store.getState())).toBe(false)
  })
})

describe('the Enter ladder (3.5)', () => {
  it('confirms a complete command with Enter on the focused viewport', async () => {
    const { core } = renderApp({ command: COMPLETE_EXTRUDE })
    viewportSurface().focus()
    fireEvent.keyDown(viewportSurface(), { key: 'Enter' })
    await waitFor(() => {
      expect(core.callsTo('commitTransaction')).toEqual([[1]])
    })
  })

  it('a held Enter (repeated presses) confirms the command once', async () => {
    const { core, app } = renderApp({ command: COMPLETE_EXTRUDE })
    viewportSurface().focus()
    fireEvent.keyDown(viewportSurface(), { key: 'Enter' })
    fireEvent.keyDown(viewportSurface(), { key: 'Enter', repeat: true })
    fireEvent.keyDown(viewportSurface(), { key: 'Enter', repeat: true })
    await settle()
    expect(core.callsTo('commitTransaction')).toEqual([[1]])
    expect(app.store.getState().notifications).toEqual([])
  })

  it('does nothing while the command is incomplete', async () => {
    const { core } = renderApp({ command: INCOMPLETE_EXTRUDE })
    fireEvent.keyDown(viewportSurface(), { key: 'Enter' })
    await settle()
    expect(core.callsTo('commitTransaction')).toEqual([])
  })

  it('leaves Mod+Enter to the shortcuts', async () => {
    const { core } = renderApp({ command: COMPLETE_EXTRUDE })
    fireEvent.keyDown(viewportSurface(), { key: 'Enter', ctrlKey: true })
    await settle()
    expect(core.callsTo('commitTransaction')).toEqual([])
  })

  it('confirms with Enter pressed in an inspector field before its text is evaluated', async () => {
    const result = deferred<{ value: number; dimension: { length: number; angle: number } }>()
    const { core, user } = renderApp({ command: COMPLETE_EXTRUDE })
    await settle()
    core.stub('evaluate', () => result.promise)
    const distance = within(screen.getByTestId('inspector')).getByRole('combobox', {
      name: 'Distance',
    })
    await user.clear(distance)
    await user.type(distance, '25{Enter}')
    await settle()
    expect(core.callsTo('commitTransaction')).toEqual([])
    await act(async () => {
      result.resolve({ value: 25, dimension: { length: 0, angle: 0 } })
      await settle()
    })
    await waitFor(() => {
      expect(core.callsTo('commitTransaction')).toEqual([[1]])
    })
  })

  it('lets a focused button in the inspector take its own Enter', async () => {
    const { core } = renderApp({ command: COMPLETE_EXTRUDE })
    const cancel = within(screen.getByTestId('inspector')).getAllByRole('button', {
      name: 'Cancel',
    })[0]
    fireEvent.keyDown(cancel ?? document.body, { key: 'Enter' })
    await settle()
    expect(core.callsTo('commitTransaction')).toEqual([])
  })
})

function renderCube(state: Partial<AppState>) {
  const test = createModelApp({ state: readyState(cubeDocumentState(), state) })
  renderWithApp(test.app)
  return test
}

describe('Repeat in the Enter ladder (3.5, step 4)', () => {
  it('starts the last feature command on the selection with Enter on the focused viewport', async () => {
    const { app } = renderCube({
      selection: [elementItem(CUBE_TOP_FRONT_EDGE)],
      lastFeatureType: 'chamfer',
    })
    viewportSurface().focus()
    fireEvent.keyDown(viewportSurface(), { key: 'Enter' })
    await waitFor(() => {
      expect(app.store.getState().command?.featureType).toBe('chamfer')
    })
    expect(app.store.getState().command?.params['edges']).toEqual([CUBE_TOP_FRONT_EDGE.reference])
  })

  it('ignores the repeated presses of a held key', async () => {
    const { app, core } = renderCube({
      selection: [elementItem(CUBE_TOP_FRONT_EDGE)],
      lastFeatureType: 'chamfer',
    })
    fireEvent.keyDown(viewportSurface(), { key: 'Enter', repeat: true })
    await settle()
    expect(core.callsTo('beginTransaction')).toEqual([])
    expect(app.store.getState().command).toBeNull()
  })

  it('says there is no command to repeat yet before any feature command ran', async () => {
    const { app } = renderCube({ selection: [elementItem(CUBE_TOP_FRONT_EDGE)] })
    fireEvent.keyDown(viewportSurface(), { key: 'Enter' })
    await settle()
    expect(app.store.getState().notifications.map((shown) => shown.message)).toEqual([
      'No command to repeat yet.',
    ])
  })

  it('does nothing without a selection', async () => {
    const { app, core } = renderCube({ lastFeatureType: 'chamfer' })
    fireEvent.keyDown(viewportSurface(), { key: 'Enter' })
    await settle()
    expect(core.callsTo('beginTransaction')).toEqual([])
    expect(app.store.getState().notifications).toEqual([])
  })
})

describe('escapeStep (3.5)', () => {
  it('clears the in-sketch selection when no tool is active', () => {
    const { app } = createModelApp({
      state: readyState(rectSketchDocumentState(), {
        context: 'sketch',
        sketch: { ...EDITING_SKETCH, selection: [sketchLineItem('f-sk1', 'e-1')] },
      }),
    })
    expect(escapeStep(app)).toBe(true)
    expect(app.store.getState().sketch?.selection).toEqual([])
    expect(escapeStep(app)).toBe(false)
    expect(app.store.getState().context).toBe('sketch')
  })

  it('names the way out of the sketch for 3 s when nothing is left to do (3.5)', () => {
    vi.useFakeTimers()
    try {
      const { app } = createModelApp({
        state: readyState(rectSketchDocumentState(), { context: 'sketch', sketch: EDITING_SKETCH }),
      })
      expect(app.store.getState().pickHint).toBeNull()
      expect(escapeStep(app)).toBe(false)
      expect(app.store.getState().pickHint).toEqual({
        message: 'To leave the sketch, click Finish sketch or press Ctrl+Enter.',
        field: null,
      })
      expect(app.store.getState().context).toBe('sketch')
      vi.advanceTimersByTime(3000)
      expect(app.store.getState().pickHint).toBeNull()
    } finally {
      vi.useRealTimers()
    }
  })

  it('shows no sketch_leave hint while there is still a step to take', () => {
    const { app } = createModelApp({
      state: readyState(rectSketchDocumentState(), {
        context: 'sketch',
        sketch: { ...EDITING_SKETCH, tool: 'line' },
      }),
    })
    expect(escapeStep(app)).toBe(true)
    expect(app.store.getState().pickHint).toBeNull()
  })

  it('clears the app selection, and then has nothing left to do', () => {
    const { app } = createModelApp({
      state: readyState(cubeDocumentState(), { selection: [bodyItem('b-cube', 'Cube')] }),
    })
    expect(escapeStep(app)).toBe(true)
    expect(app.store.getState().selection).toEqual([])
    expect(escapeStep(app)).toBe(false)
  })

  it('cancels the face pick of Lay on face before clearing the selection (step 5)', () => {
    const { app } = createModelApp({
      state: readyState(cubeDocumentState(), {
        context: 'print',
        selection: [bodyItem('b-cube', 'Cube')],
      }),
    })
    app.store.setState((state) => ({ print: { ...state.print, pickingFace: true } }))
    expect(escapeStep(app)).toBe(true)
    expect(app.store.getState().print.pickingFace).toBe(false)
    expect(app.store.getState().selection).toHaveLength(1)
  })

  it('E-102 five Esc presses in an edited sketch never leave sketch mode (3.5)', () => {
    vi.useFakeTimers()
    try {
      const { app } = renderApp({
        context: 'sketch',
        sketch: {
          ...EDITING_SKETCH,
          tool: 'line',
          selection: [sketchLineItem('f-sk1', 'e-1')],
        },
      })
      for (let i = 0; i < 5; i++) {
        fireEvent.keyDown(document.body, { key: 'Escape' })
      }
      const state = app.store.getState()
      expect(state.context).toBe('sketch')
      expect(state.sketch).toMatchObject({ sketchId: 'f-sk1', tool: null, selection: [] })
      expect(state.pickHint?.message).toBe(
        'To leave the sketch, click Finish sketch or press Ctrl+Enter.',
      )
    } finally {
      vi.useRealTimers()
    }
  })
})
