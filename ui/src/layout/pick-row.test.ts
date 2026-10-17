import { describe, expect, it } from 'vitest'
import { MESSAGES } from '../messages'
import { planePickingSession } from '../sketch/fixtures.testing'
import type { ActiveCommand } from '../state/store'
import { MAC_WEB } from '../test/fakes/app'
import type { FeatureRecord } from '../test/fakes/documents'
import { attachCall, cubeWithSketch, lostFaceSketch } from '../test/fakes/repairs'
import { bodyItem, datumItem, originItem } from '../test/fakes/selection'
import { settle } from '../test/fakes/settle'
import { cubeDocumentState, documentState, readyState } from './fixtures.testing'
import { activeFieldAccepts, pickRow, togglesSelection } from './pick-row'
import { createModelApp } from './test-app.testing'

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
  helper: null,
  changedPaths: [],
  history: [],
  future: [],
  preview: 'none',
}

describe('activeFieldAccepts', () => {
  it('is null without a command or without an active field', () => {
    const { app } = createModelApp({ state: readyState(cubeDocumentState()) })
    expect(activeFieldAccepts(app.store.getState())).toBeNull()
    app.store.setState({ command: { ...DATUM_COMMAND, activeField: null } })
    expect(activeFieldAccepts(app.store.getState())).toBeNull()
  })

  it('is what the active selection field accepts', () => {
    const { app } = createModelApp({
      state: readyState(cubeDocumentState(), { command: DATUM_COMMAND }),
    })
    expect(activeFieldAccepts(app.store.getState())).toBe('plane')
  })
})

describe('pickRow', () => {
  it('selects the item, replacing or toggling the selection', () => {
    const { app } = createModelApp({ state: readyState(cubeDocumentState()) })
    pickRow(app, originItem('XY'), false)
    pickRow(app, bodyItem('b-cube', 'Cube'), true)
    expect(app.store.getState().selection.map((item) => item.key)).toEqual([
      'origin:XY',
      'body:b-cube',
    ])
    pickRow(app, originItem('XZ'), false)
    expect(app.store.getState().selection.map((item) => item.key)).toEqual(['origin:XZ'])
  })

  it('fills the active field with an item it accepts instead of selecting it', async () => {
    const { app } = createModelApp({
      state: readyState(cubeDocumentState(), { command: DATUM_COMMAND }),
    })
    pickRow(app, originItem('YZ'), false)
    await settle()
    expect(app.store.getState().command?.params.base).toEqual({ kind: 'origin', plane: 'YZ' })
    expect(app.store.getState().selection).toEqual([])
  })

  it('selects an item the active field does not accept', () => {
    const { app } = createModelApp({
      state: readyState(cubeDocumentState(), { command: DATUM_COMMAND }),
    })
    pickRow(app, bodyItem('b-cube', 'Cube'), false)
    expect(app.store.getState().command?.params.base).toBeUndefined()
    expect(app.store.getState().selection.map((item) => item.key)).toEqual(['body:b-cube'])
  })
})

/** Sketch mode while a plane is picked for a new sketch (3.16.1). */
const PLANE_PICKING = planePickingSession()

/** The plane of the sketch the one `applyEdits` call added. */
function addedSketchPlane(calls: readonly (readonly unknown[])[]): unknown {
  expect(calls).toHaveLength(1)
  const [, label, edits] = calls[0] ?? []
  expect(label).toBe('New sketch')
  const [edit] = edits as readonly { readonly type: string; readonly feature: { plane: unknown } }[]
  expect(edit?.type).toBe('add_feature')
  return edit?.feature.plane
}

describe('pickRow while a plane is picked (3.16.1)', () => {
  it('starts the new sketch on a clicked origin plane row', async () => {
    const { app, core } = createModelApp({
      state: readyState(cubeDocumentState(), { context: 'sketch', sketch: PLANE_PICKING }),
    })
    pickRow(app, originItem('XZ'), false)
    await settle()
    expect(addedSketchPlane(core.callsTo('applyEdits'))).toEqual({ kind: 'origin', plane: 'XZ' })
    expect(app.store.getState().selection).toEqual([])
  })

  it('starts the new sketch on a clicked datum row', async () => {
    const { app, core } = createModelApp({
      state: readyState(cubeDocumentState(), { context: 'sketch', sketch: PLANE_PICKING }),
    })
    pickRow(app, datumItem('f-dp1', 'Datum plane 1'), false)
    await settle()
    expect(addedSketchPlane(core.callsTo('applyEdits'))).toEqual({
      kind: 'datum',
      feature: 'f-dp1',
    })
  })

  it('selects any other row, and plane rows once a sketch is being edited', async () => {
    const { app, core } = createModelApp({
      state: readyState(cubeDocumentState(), { context: 'sketch', sketch: PLANE_PICKING }),
    })
    pickRow(app, bodyItem('b-cube', 'Cube'), false)
    app.store.setState({ sketch: { ...PLANE_PICKING, sketchId: 'f-sk1' } })
    pickRow(app, originItem('XY'), true)
    await settle()
    expect(core.callsTo('applyEdits')).toEqual([])
    expect(app.store.getState().selection.map((item) => item.key)).toEqual([
      'body:b-cube',
      'origin:XY',
    ])
  })
})

/** 'Sketch 2', a circle on a face the cube lost, with its fallback in body coordinates. */
const SKETCH_2 = lostFaceSketch()

/** Re-attaching 'Sketch 2' at the end of cube.ptsm, before the features `after` (3.16.1). */
function reattachingApp(after: readonly FeatureRecord[] = []) {
  return createModelApp({
    state: readyState(documentState(cubeWithSketch(SKETCH_2, after), { name: 'cube' }), {
      context: 'sketch',
      sketch: planePickingSession('f-sk2'),
    }),
  })
}

describe('pickRow while a sketch is re-attached (3.16.1)', () => {
  it('attaches the sketch to a clicked origin plane row in one update labeled "Attach \'Sketch 2\'", then edits it', async () => {
    const { app, core } = reattachingApp()
    pickRow(app, originItem('XY'), false)
    await settle()
    expect(core.callsTo('applyEdits')).toEqual([
      attachCall(SKETCH_2, { kind: 'origin', plane: 'XY' }),
    ])
    expect(app.store.getState().sketch).toMatchObject({ sketchId: 'f-sk2', reattaching: null })
  })

  it('attaches once for two quick picks', async () => {
    const { app, core } = reattachingApp()
    pickRow(app, originItem('XY'), false)
    pickRow(app, originItem('XZ'), false)
    await settle()
    const calls = core.callsTo('applyEdits')
    expect(calls).toHaveLength(1)
    expect(calls[0]?.[2]).toMatchObject([{ feature: { plane: { kind: 'origin', plane: 'XY' } } }])
  })

  it('refuses the row of a datum plane after the sketch with attach_after in the sketch hint', async () => {
    const datum: FeatureRecord = {
      id: 'f-dp1',
      type: 'datum_plane',
      base: { kind: 'origin', plane: 'XY' },
      offset: '30 mm',
    }
    const { app, core } = reattachingApp([datum])
    pickRow(app, datumItem('f-dp1', 'Datum plane 1'), false)
    await settle()
    expect(core.callsTo('applyEdits')).toEqual([])
    expect(app.store.getState().pickHint).toEqual({
      message: MESSAGES.attach_after('Sketch 2', 'Datum plane 1'),
      field: null,
    })
    expect(app.store.getState().sketch).toMatchObject({ sketchId: null, reattaching: 'f-sk2' })
  })
})

describe('togglesSelection', () => {
  const none = { shiftKey: false, metaKey: false, ctrlKey: false }

  it('toggles with Shift everywhere', () => {
    const { app } = createModelApp()
    expect(togglesSelection(app, { ...none, shiftKey: true })).toBe(true)
    expect(togglesSelection(app, none)).toBe(false)
  })

  it('toggles with Ctrl outside macOS and with Cmd on macOS', () => {
    const linux = createModelApp().app
    const mac = createModelApp({ platform: MAC_WEB }).app
    expect(togglesSelection(linux, { ...none, ctrlKey: true })).toBe(true)
    expect(togglesSelection(linux, { ...none, metaKey: true })).toBe(false)
    expect(togglesSelection(mac, { ...none, metaKey: true })).toBe(true)
    expect(togglesSelection(mac, { ...none, ctrlKey: true })).toBe(false)
  })
})
