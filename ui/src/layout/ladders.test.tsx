import { act, cleanup, fireEvent, screen, waitFor, within } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { commandComplete, startFeatureCommand } from '../app/actions/command'
import { pickAt } from '../app/actions/selection'
import type { App } from '../app/createApp'
import { installPopoverPolyfill } from '../components/popover-polyfill.testing'
import type { ElementDescriptor } from '../generated/protocol'
import { sketchSession } from '../sketch/fixtures.testing'
import type { ActiveCommand, AppState } from '../state/store'
import { coreErrorFor, type FakeCoreApi } from '../test/fakes/core-api'
import { deferred, type Deferred } from '../test/fakes/deferred'
import { CUBE_TOP_FACE, CUBE_TOP_FRONT_EDGE, RECT_REGION } from '../test/fakes/documents'
import { bodyItem, elementItem, meshPick, sketchLineItem } from '../test/fakes/selection'
import { settle } from '../test/fakes/settle'
import { commandEdges, selectionKeys } from '../test/fakes/test-app'
import {
  cubeDocumentState,
  FILLET_EDGES_COMMAND,
  readyState,
  rectSketchDocumentState,
} from './fixtures.testing'
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

const EDITING_SKETCH = sketchSession()

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

// Enter, Mod+Z and Esc act on what the input made before the key leaves (ui.md 3.5, 3.20, U-244,
// U-245). Each test holds a core answer with a deferred promise and answers it after the key.

const EDGE_CLICK = meshPick(CUBE_TOP_FRONT_EDGE, 4)
const FACE_CLICK = meshPick(CUBE_TOP_FACE, 0)

/** A Fillet on the cube's top front edge: complete, its preview drawn. */
const COMPLETE_FILLET: ActiveCommand = {
  ...FILLET_EDGES_COMMAND,
  params: { body: 'b-cube', radius: '2 mm', edges: [CUBE_TOP_FRONT_EDGE.reference] },
  applied: true,
  preview: 'shown',
}

/** Holds the core's answer to every pick until the test resolves or rejects the returned promise. */
function holdPicks(core: FakeCoreApi): Deferred<ElementDescriptor> {
  const picked = deferred<ElementDescriptor>()
  core.stub('pickReference', () => picked.promise)
  return picked
}

/** Answers the held picks with `element` and lets what waited for them land. */
async function answerPick(
  picked: Deferred<ElementDescriptor>,
  click: Promise<void>,
  element: ElementDescriptor = CUBE_TOP_FRONT_EDGE,
): Promise<void> {
  await act(async () => {
    picked.resolve(element)
    await click
    await settle()
  })
}

function pressEnter(target: Element): boolean {
  return fireEvent.keyDown(target, { key: 'Enter' })
}

/** Whether an `applyEdits` call carried the Fillet with `edges`. */
function appliedWithEdges(core: FakeCoreApi, edges: unknown): boolean {
  return core
    .callsTo('applyEdits')
    .some((call) => JSON.stringify(call[2]).includes(JSON.stringify(edges)))
}

describe('U-244 Enter and Mod+Z wait for a click on its way (3.5, 3.20)', () => {
  it('U-244 (a) a click on an edge and Enter confirm the Fillet the edge completes, however late the core answers', async () => {
    const { app, core } = renderCube({ command: FILLET_EDGES_COMMAND })
    const picked = holdPicks(core)
    const click = pickAt(app, EDGE_CLICK, { toggle: false })
    viewportSurface().focus()
    expect(pressEnter(viewportSurface())).toBe(false)
    pressEnter(viewportSurface())
    await settle()
    expect(core.callsTo('commitTransaction')).toEqual([])
    await answerPick(picked, click)
    await waitFor(() => {
      expect(core.callsTo('commitTransaction')).toEqual([[1]])
    })
    expect(appliedWithEdges(core, [CUBE_TOP_FRONT_EDGE.reference])).toBe(true)
    expect(app.store.getState().command).toBeNull()
  })

  it('U-244 (b) guard: an Enter that waited for a click the core refuses commits nothing, and a later Enter does nothing', async () => {
    const { app, core } = renderCube({ command: FILLET_EDGES_COMMAND })
    const picked = holdPicks(core)
    const click = pickAt(app, EDGE_CLICK, { toggle: false })
    viewportSurface().focus()
    pressEnter(viewportSurface())
    const refusal = await coreErrorFor({
      code: 'stale_pick',
      message: 'The picked geometry changed since it was displayed. Pick again.',
    })
    await act(async () => {
      picked.reject(refusal)
      await click
      await settle()
    })
    pressEnter(viewportSurface())
    await settle()
    expect(core.callsTo('commitTransaction')).toEqual([])
    expect(app.store.getState().command?.featureType).toBe('fillet')
    expect(commandEdges(app)).toEqual([])
    expect(app.store.getState().notifications).toEqual([])
  })

  it('U-244 (c) a click with nothing selected and Enter repeat the last command on what the click selected', async () => {
    const { app, core } = renderCube({ lastFeatureType: 'fillet' })
    const picked = holdPicks(core)
    const click = pickAt(app, EDGE_CLICK, { toggle: false })
    viewportSurface().focus()
    pressEnter(viewportSurface())
    await answerPick(picked, click)
    await waitFor(() => {
      expect(app.store.getState().command?.featureType).toBe('fillet')
    })
    expect(commandEdges(app)).toEqual([CUBE_TOP_FRONT_EDGE.reference])
  })

  it('U-244 (d) Esc pressed after an Enter that waits for a click cancels nothing, and the Fillet is committed', async () => {
    const { app, core } = renderCube({ command: FILLET_EDGES_COMMAND })
    const picked = holdPicks(core)
    const click = pickAt(app, EDGE_CLICK, { toggle: false })
    viewportSurface().focus()
    pressEnter(viewportSurface())
    fireEvent.keyDown(viewportSurface(), { key: 'Escape' })
    await answerPick(picked, click)
    await waitFor(() => {
      expect(core.callsTo('commitTransaction')).toEqual([[1]])
    })
    expect(core.callsTo('cancelTransaction')).toEqual([])
    expect(app.store.getState().command).toBeNull()
  })

  it('U-244 (e) guard: Enter, a click on a face and Enter while OK runs commit once and repeat nothing', async () => {
    const { app, core } = renderCube({ command: COMPLETE_FILLET })
    core.stub('pickReference', () => Promise.resolve(CUBE_TOP_FACE))
    const committed = deferred<{ revision: number; recorded: boolean }>()
    core.stub('commitTransaction', () => committed.promise)
    viewportSurface().focus()
    pressEnter(viewportSurface())
    await settle()
    const click = pickAt(app, FACE_CLICK, { toggle: false })
    pressEnter(viewportSurface())
    await act(async () => {
      committed.resolve({ revision: 2, recorded: true })
      await click
      await settle()
    })
    expect(core.callsTo('commitTransaction')).toEqual([[1]])
    expect(core.callsTo('beginTransaction')).toEqual([])
    expect(app.store.getState().command).toBeNull()
    expect(selectionKeys(app)).toEqual([`element:b-cube:${CUBE_TOP_FACE.name}`])
  })

  it('U-244 (f) guard: a click, Enter and a feature key open that command with what the click selected and confirm nothing', async () => {
    const { app, core } = renderCube({ lastFeatureType: 'chamfer' })
    const picked = holdPicks(core)
    const begun = deferred<undefined>()
    core.stub('beginTransaction', () => begun.promise)
    const click = pickAt(app, EDGE_CLICK, { toggle: false })
    viewportSurface().focus()
    pressEnter(viewportSurface())
    const started = startFeatureCommand(app, 'fillet')
    // The core answers in request order: the pick, then the transaction.
    await answerPick(picked, click)
    await act(async () => {
      begun.resolve(undefined)
      await started
      await settle()
    })
    expect(app.store.getState().command).toMatchObject({ featureType: 'fillet', history: [] })
    expect(commandEdges(app)).toEqual([CUBE_TOP_FRONT_EDGE.reference])
    expect(core.callsTo('beginTransaction')).toEqual([[1, 'Fillet']])
    expect(core.callsTo('commitTransaction')).toEqual([])
  })

  it('U-244 (g) guard: the OK button, disabled while the command is incomplete, does nothing with a click on its way', async () => {
    const { app, core } = renderCube({ command: FILLET_EDGES_COMMAND })
    const picked = holdPicks(core)
    const click = pickAt(app, EDGE_CLICK, { toggle: false })
    const [ok] = within(screen.getByTestId('inspector')).getAllByRole('button', { name: 'OK' })
    expect(ok).toHaveAttribute('aria-disabled', 'true')
    fireEvent.click(ok ?? document.body)
    await answerPick(picked, click)
    expect(core.callsTo('commitTransaction')).toEqual([])
    expect(commandEdges(app)).toEqual([CUBE_TOP_FRONT_EDGE.reference])
  })

  it('U-244 (h) Enter in the Radius field waits for a click on its way and confirms the Fillet it completes', async () => {
    const { app, core } = renderCube({ command: FILLET_EDGES_COMMAND })
    await act(async () => {
      await settle()
    })
    const picked = holdPicks(core)
    const click = pickAt(app, EDGE_CLICK, { toggle: false })
    const radius = within(screen.getByTestId('inspector')).getByRole('combobox', { name: 'Radius' })
    radius.focus()
    pressEnter(radius)
    await settle()
    expect(core.callsTo('commitTransaction')).toEqual([])
    await answerPick(picked, click)
    await waitFor(() => {
      expect(core.callsTo('commitTransaction')).toEqual([[1]])
    })
  })

  it('U-244 (i) a click on an edge, F and Enter confirm the Fillet on that edge, however late the core answers', async () => {
    const { app, core } = renderCube({ lastFeatureType: 'extrude' })
    const picked = holdPicks(core)
    const begun = deferred<undefined>()
    core.stub('beginTransaction', () => begun.promise)
    const click = pickAt(app, EDGE_CLICK, { toggle: false })
    const started = startFeatureCommand(app, 'fillet')
    viewportSurface().focus()
    pressEnter(viewportSurface())
    await answerPick(picked, click)
    await act(async () => {
      begun.resolve(undefined)
      await started
      await settle()
    })
    await waitFor(() => {
      expect(core.callsTo('commitTransaction')).toEqual([[1]])
    })
    expect(core.callsTo('beginTransaction')).toEqual([[1, 'Fillet']])
    expect(appliedWithEdges(core, [CUBE_TOP_FRONT_EDGE.reference])).toBe(true)
    expect(app.store.getState().command).toBeNull()
  })

  it('U-244 (j) a click on an edge and Mod+Z leave the Fillet as it was before the click, not canceled', async () => {
    const { app, core } = renderCube({ command: FILLET_EDGES_COMMAND })
    const picked = holdPicks(core)
    const click = pickAt(app, EDGE_CLICK, { toggle: false })
    const undone = app.commands.run(app, 'edit.undo', 'shortcut')
    await answerPick(picked, click)
    await act(async () => {
      await undone
      await settle()
    })
    expect(core.callsTo('cancelTransaction')).toEqual([])
    expect(app.store.getState().command?.featureType).toBe('fillet')
    expect(commandEdges(app)).toEqual([])
  })
})

/** The app on the cube with fakes, not rendered: `escapeStep` is called directly. */
function cubeApp(state: Partial<AppState> = {}) {
  return createModelApp({ state: readyState(cubeDocumentState(), state) })
}

/** The index in the core's calls of the first call to `method`, or -1. */
function callIndex(core: FakeCoreApi, method: keyof App['core']): number {
  return core.calls.findIndex((call) => call.method === method)
}

describe('U-245 Esc waits for what was asked before it (3.5)', () => {
  it('U-245 (a) a click on a face and Esc end with nothing selected, however late the core answers', async () => {
    const { app, core } = cubeApp()
    const picked = holdPicks(core)
    const click = pickAt(app, FACE_CLICK, { toggle: false })
    expect(escapeStep(app)).toBe(true)
    picked.resolve(CUBE_TOP_FACE)
    await click
    await settle()
    expect(app.store.getState().selection).toEqual([])
  })

  it('U-245 (b) F and Esc cancel the Fillet once it has opened', async () => {
    const { app, core } = cubeApp()
    const begun = deferred<undefined>()
    core.stub('beginTransaction', () => begun.promise)
    const started = startFeatureCommand(app, 'fillet')
    expect(escapeStep(app)).toBe(true)
    begun.resolve(undefined)
    await started
    await settle()
    expect(core.callsTo('cancelTransaction')).toHaveLength(1)
    expect(callIndex(core, 'cancelTransaction')).toBeGreaterThan(
      callIndex(core, 'beginTransaction'),
    )
    expect(app.store.getState().command).toBeNull()
  })

  it('U-245 (c) Sketch, while it still cancels a command, and Esc end the plane picking it opens', async () => {
    const { app, core } = cubeApp({ command: FILLET_EDGES_COMMAND })
    const canceled = deferred<number>()
    core.stub('cancelTransaction', () => canceled.promise)
    const started = startFeatureCommand(app, 'sketch')
    expect(escapeStep(app)).toBe(true)
    canceled.resolve(2)
    await started
    await settle()
    expect(app.store.getState()).toMatchObject({ context: 'model', sketch: null, command: null })
  })
})
