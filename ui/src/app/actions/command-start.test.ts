import { describe, expect, it } from 'vitest'
import type { RegionInfo, SketchEntity } from '../../generated/protocol'
import {
  cubeDocumentState,
  elementItem,
  featureItem,
  readyState,
  rectSketchDocumentState,
  regionItem,
} from '../../layout/fixtures.testing'
import { createModelApp } from '../../layout/test-app.testing'
import { MESSAGES } from '../../messages'
import {
  circleEntity,
  documentState,
  pointEntity,
  sketchRecord,
  sketchSession,
  solutionFor,
  solvedSketch,
} from '../../sketch/fixtures.testing'
import type { AppState } from '../../state/store'
import { CUBE_TOP_FRONT_EDGE, RECT_REGION, RECT_SKETCH_SOLUTION } from '../../test/fakes/documents'
import { settle } from '../../test/fakes/settle'
import { capturePick, setCommandField, startFeatureCommand } from './command'

// How a feature command opens (ui.md 3.3, 3.7 steps 4 and 5): the finished sketch hands off to the
// next feature, a selected sketch stands for its region or points, and the active field.

/** A state editing sketch `f-sk1` with these entities. */
function editingSketch(entities: SketchEntity[], regions: RegionInfo[] = []): Partial<AppState> {
  return readyState(
    documentState({
      features: [sketchRecord({ entities })],
      sketches: { 'f-sk1': solvedSketch(solutionFor(entities, { regions })) },
    }),
    { context: 'sketch', sketch: sketchSession() },
  )
}

function region(key: string): RegionInfo {
  return { reference: { key, fp: `fp-${key}` }, area: 100, centroid: [5, 5] }
}

describe('the finished sketch hands off to the next feature (3.3, U-99)', () => {
  it('U-99 a Hole run from Sketch takes the sketch’s point entities', async () => {
    const { app } = createModelApp({
      state: editingSketch([pointEntity('e-p1', [0, 0]), pointEntity('e-p2', [5, 0])]),
    })
    await startFeatureCommand(app, 'hole')
    expect(app.store.getState().context).toBe('model')
    expect(app.store.getState().command?.params).toMatchObject({
      points: ['e-p1', 'e-p2'],
      sketch: 'f-sk1',
    })
  })

  it('U-99 leaves out construction points', async () => {
    const { app } = createModelApp({
      state: editingSketch([pointEntity('e-p1', [0, 0]), pointEntity('e-p2', [5, 0], true)]),
    })
    await startFeatureCommand(app, 'hole')
    expect(app.store.getState().command?.params['points']).toEqual(['e-p1'])
  })

  it('U-99 takes the centers of circles when the sketch has no point entities', async () => {
    const { app } = createModelApp({ state: editingSketch([circleEntity('e-5', [0, 0], 3)]) })
    await startFeatureCommand(app, 'hole')
    expect(app.store.getState().command?.params['points']).toEqual(['e-5.center'])
  })

  it('U-99 an Extrude run from Sketch takes the only region', async () => {
    const { app } = createModelApp({ state: editingSketch([], [region('r-1')]) })
    await startFeatureCommand(app, 'extrude')
    expect(app.store.getState().command?.params['profile']).toEqual({
      sketch: 'f-sk1',
      regions: [region('r-1').reference],
    })
  })

  it('U-99 a selected sketch row stands for its one region', async () => {
    const { app } = createModelApp({
      state: readyState(rectSketchDocumentState(), {
        selection: [featureItem('f-sk1', 'Sketch 1')],
      }),
    })
    await startFeatureCommand(app, 'extrude')
    expect(app.store.getState().command?.params['profile']).toEqual({
      sketch: 'f-sk1',
      regions: [RECT_REGION.reference],
    })
    expect(app.store.getState().selection).toEqual([])
  })

  it('U-99 a selected sketch of two regions leaves Profile empty and active', async () => {
    const twoRegions = rectSketchDocumentState({
      sketches: {
        'f-sk1': solvedSketch({ ...RECT_SKETCH_SOLUTION, regions: [region('r-1'), region('r-2')] }),
      },
    })
    const { app } = createModelApp({
      state: readyState(twoRegions, { selection: [featureItem('f-sk1', 'Sketch 1')] }),
    })
    await startFeatureCommand(app, 'extrude')
    const command = app.store.getState().command
    expect(command?.params['profile']).toBeUndefined()
    expect(command?.activeField).toBe('profile')
  })
})

describe('the active field (3.7 step 5, U-104)', () => {
  it('U-104 a Fillet started with a preselected edge keeps Edges active for more edges', async () => {
    const { app } = createModelApp({
      state: readyState(cubeDocumentState(), {
        selection: [elementItem(CUBE_TOP_FRONT_EDGE)],
      }),
    })
    await startFeatureCommand(app, 'fillet')
    const command = app.store.getState().command
    expect(command?.params).toMatchObject({ body: 'b-cube', radius: '1 mm' })
    expect(command?.activeField).toBe('edges')
  })

  it('U-104 a profile pick in Revolve moves the active field to Axis', async () => {
    const { app } = createModelApp({ state: readyState(rectSketchDocumentState()) })
    await startFeatureCommand(app, 'revolve')
    expect(app.store.getState().command?.activeField).toBe('profile')
    await capturePick(app, regionItem('f-sk1', RECT_REGION))
    expect(app.store.getState().command?.activeField).toBe('axis')
  })
})

describe('the body the first pick fills (2.12, U-117)', () => {
  it('U-117 clearing the edges of a fillet clears the body their first pick filled', async () => {
    const { app } = createModelApp({ state: readyState(cubeDocumentState()) })
    await startFeatureCommand(app, 'fillet')
    expect(app.store.getState().command?.params['body']).toBeUndefined()
    await capturePick(app, elementItem(CUBE_TOP_FRONT_EDGE))
    expect(app.store.getState().command?.params['body']).toBe('b-cube')
    setCommandField(app, 'edges', undefined)
    expect(app.store.getState().command?.params['body']).toBeUndefined()
    await settle()
  })

  it('U-117 renders pick_same_body with the plural noun', () => {
    expect(MESSAGES.pick_same_body('edges', 'Cube')).toBe(
      "All edges must be on 'Cube'. Remove them to pick edges of another body.",
    )
  })
})

describe('the command Repeat starts again (3.5)', () => {
  it('records the feature type of each started command, and not a new sketch', async () => {
    const { app } = createModelApp({ state: readyState(cubeDocumentState()) })
    expect(app.store.getState().lastFeatureType).toBeNull()
    await startFeatureCommand(app, 'fillet')
    expect(app.store.getState().lastFeatureType).toBe('fillet')
    await startFeatureCommand(app, 'sketch')
    expect(app.store.getState().lastFeatureType).toBe('fillet')
  })
})
