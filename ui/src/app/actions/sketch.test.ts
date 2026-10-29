import { afterEach, describe, expect, it, vi } from 'vitest'
import type { Command, Edit, ElementDescriptor, Reference } from '../../generated/protocol'
import { MESSAGES } from '../../messages'
import { selectSketchRecord } from '../../state/selectors'
import { currentDocument, type FakeApp, startWithDocument } from '../../test/fakes/app'
import { deferred, type Deferred } from '../../test/fakes/deferred'
import {
  CUBE_DOCUMENT,
  CUBE_FEATURES_DOCUMENT,
  CUBE_REGION,
  CUBE_FRONT_FACE,
  CUBE_SKETCH_SOLUTION,
  CUBE_TOP_FACE,
} from '../../test/fakes/documents'
import { elementItem, originItem } from '../../test/fakes/selection'
import { stateWithDocument } from '../../test/fakes/state'
import { editsOf } from '../../test/fakes/protocol'
import { settle } from '../../test/fakes/settle'
import type { PickResult } from '../../viewport/picking'
import type { App } from '../createApp'
import { startFeatureCommand } from './command'
import { setContext } from './context'
import { pickAt, select } from './selection'
import {
  createSketchOn,
  enterSketch,
  finishSketch,
  setSketchTool,
  sketchFaceBody,
  startNewSketch,
  storedSketchParams,
} from './sketch'

type AddFeature = Extract<Edit, { type: 'add_feature' }>

/**
 * The top of a cube moved by (100, 0, 0) with a Move body (U-142): its sketch placement in world
 * coordinates at (100, 0, 30), and the same placement in body coordinates, the move undone, as its
 * fallback (engine.md 3.9).
 */
const MOVED_TOP_FACE: ElementDescriptor = {
  ...CUBE_TOP_FACE,
  centroid: [100, 0, 30],
  normal: [0, 0, 1],
  placement: { origin: [100, 0, 30], x_axis: [1, 0, 0], normal: [0, 0, 1] },
  fallback: { origin: [0, 0, 30], x_axis: [1, 0, 0], normal: [0, 0, 1] },
}

/** The full reference `pick_reference` gives the top face: anchors and merged names (engine.md 3.9). */
const FULL_TOP_REFERENCE: Reference = {
  q: 'f-ext1/end-cap#2',
  fp: 'plane|n=0,0,1|c=100,0,30|a=400|adj=4,4',
  nb: { 'f-ext1/end-cap#2': ['f-ext1/lateral/e-2'] },
  merged: { 'f-ext1/end-cap#2': ['f-ext2/end-cap'] },
}

function newSketchEdit(fake: FakeApp): {
  readonly command: Extract<Command, { type: 'apply_edits' }> | undefined
  readonly edit: AddFeature | null
} {
  const command = fake.transport.commandsOfType('apply_edits')[0]
  const edit = editsOf(command)[0]
  return { command, edit: edit?.type === 'add_feature' ? edit : null }
}

async function cube(recipe = CUBE_DOCUMENT): Promise<FakeApp> {
  return startWithDocument('cube.ptsm', recipe, {
    core: { solutions: { 'f-sk1': CUBE_SKETCH_SOLUTION } },
  })
}

describe('createSketchOn (3.16.1)', () => {
  it('adds an empty sketch on an origin plane in one edit labeled "New sketch", then edits it', async () => {
    const fake = await cube()
    await createSketchOn(fake.app, originItem('XY'))
    await settle()
    const { command, edit } = newSketchEdit(fake)
    expect(command?.label).toBe('New sketch')
    expect(editsOf(command)).toHaveLength(1)
    expect(edit?.index).toBe(2)
    expect(edit?.feature).toMatchObject({
      type: 'sketch',
      plane: { kind: 'origin', plane: 'XY' },
      entities: [],
      constraints: [],
    })
    expect(edit?.feature.id).toMatch(/^f-[0-9a-f]{12}$/)
    expect('name' in (edit?.feature ?? {})).toBe(false)
    const state = fake.app.store.getState()
    expect(state.context).toBe('sketch')
    expect(state.sketch?.sketchId).toBe(edit?.feature.id)
  })

  it('a double-click on a plane adds one sketch', async () => {
    const fake = await cube()
    await Promise.all([
      createSketchOn(fake.app, originItem('XY')),
      createSketchOn(fake.app, originItem('XY')),
    ])
    await settle()
    expect(fake.transport.commandsOfType('apply_edits')).toHaveLength(1)
    expect(fake.app.store.getState().sketch?.sketchId).toBe(newSketchEdit(fake).edit?.feature.id)
  })

  it('a double-click on Sketch with a plane selected adds one sketch', async () => {
    const fake = await cube()
    select(fake.app, [originItem('XY')], 'replace')
    await Promise.all([startNewSketch(fake.app), startNewSketch(fake.app)])
    await settle()
    expect(fake.transport.commandsOfType('apply_edits')).toHaveLength(1)
  })

  it("U-142 a face moved by a Move body gives a face plane with the descriptor's fallback in body coordinates", async () => {
    const fake = await cube()
    await createSketchOn(fake.app, elementItem(MOVED_TOP_FACE, 'Cube'))
    await settle()
    const { command, edit } = newSketchEdit(fake)
    expect(command?.label).toBe('New sketch')
    expect(edit?.feature['plane']).toEqual({
      kind: 'face',
      body: 'b-cube',
      face: MOVED_TOP_FACE.reference,
      fallback: MOVED_TOP_FACE.fallback,
      fallback_frame: 'body',
    })
  })

  it("U-142 stores the face's full reference, anchors and merged names included", async () => {
    const fake = await cube()
    const face: ElementDescriptor = { ...MOVED_TOP_FACE, reference: FULL_TOP_REFERENCE }
    await createSketchOn(fake.app, elementItem(face, 'Cube'))
    await settle()
    expect(newSketchEdit(fake).edit?.feature['plane']).toMatchObject({
      face: FULL_TOP_REFERENCE,
    })
  })

  it('a face without a motion stores its fallback, which equals its placement, with fallback_frame body', async () => {
    const fake = await cube()
    await createSketchOn(fake.app, elementItem(CUBE_TOP_FACE, 'Cube'))
    await settle()
    expect(newSketchEdit(fake).edit?.feature['plane']).toEqual({
      kind: 'face',
      body: 'b-cube',
      face: CUBE_TOP_FACE.reference,
      fallback: CUBE_TOP_FACE.placement,
      fallback_frame: 'body',
    })
  })

  it('origin and datum planes carry no fallback or fallback_frame', async () => {
    const fake = await cube()
    await createSketchOn(fake.app, originItem('XZ'))
    await settle()
    expect(newSketchEdit(fake).edit?.feature['plane']).toEqual({ kind: 'origin', plane: 'XZ' })
  })

  it('inserts the sketch at the rollback feature when the marker is not at the end', async () => {
    const fake = await cube({ ...CUBE_FEATURES_DOCUMENT, rollback: 'f-fil1' })
    await createSketchOn(fake.app, originItem('XZ'))
    await settle()
    const { command, edit } = newSketchEdit(fake)
    expect(edit?.index).toBe(2)
    expect(editsOf(command).some((candidate) => candidate.type === 'set_rollback')).toBe(false)
  })
})

describe('plane picking (3.16.1)', () => {
  afterEach(() => {
    vi.useRealTimers()
  })

  it('a face that is not flat shows pick_plane in the sketch header for 3 s and adds nothing', async () => {
    const fake = await cube()
    await startNewSketch(fake.app)
    const roundFace: ElementDescriptor = {
      ...CUBE_FRONT_FACE,
      name: 'f-cyl1/side',
      surface: 'cylinder',
      placement: null,
    }
    vi.useFakeTimers()
    await createSketchOn(fake.app, elementItem(roundFace, 'Cube'))
    const state = fake.app.store.getState()
    expect(state.pickHint).toEqual({ message: MESSAGES.pick_plane(), field: null })
    expect(state.notifications).toEqual([])
    expect(state.sketch?.sketchId).toBeNull()
    expect(fake.transport.commandTypes()).not.toContain('apply_edits')
    vi.advanceTimersByTime(3_000)
    expect(fake.app.store.getState().pickHint).toBeNull()
  })
})

describe('startNewSketch (3.16.1)', () => {
  it('with exactly one flat face selected, creates the sketch on it', async () => {
    const fake = await cube()
    select(fake.app, [elementItem(CUBE_FRONT_FACE, 'Cube')], 'replace')
    await startNewSketch(fake.app)
    await settle()
    expect(newSketchEdit(fake).edit?.feature['plane']).toMatchObject({
      kind: 'face',
      body: 'b-cube',
      face: CUBE_FRONT_FACE.reference,
    })
  })

  it("U-142 with a moved face selected, stores the descriptor's fallback with fallback_frame body", async () => {
    const fake = await cube()
    select(fake.app, [elementItem(MOVED_TOP_FACE, 'Cube')], 'replace')
    await startNewSketch(fake.app)
    await settle()
    expect(newSketchEdit(fake).edit?.feature['plane']).toEqual({
      kind: 'face',
      body: 'b-cube',
      face: MOVED_TOP_FACE.reference,
      fallback: MOVED_TOP_FACE.fallback,
      fallback_frame: 'body',
    })
  })

  it('without a plane selected, enters plane picking: Sketch context with no sketch yet', async () => {
    const fake = await cube()
    await startNewSketch(fake.app)
    await settle()
    const state = fake.app.store.getState()
    expect(state.context).toBe('sketch')
    expect(state.sketch).toMatchObject({ sketchId: null, tool: null })
    expect(fake.transport.commandTypes()).not.toContain('apply_edits')
  })
})

/** A click on the cube's top face, which pick_reference turns into its descriptor. */
const TOP_FACE_PICK: PickResult = {
  kind: 'mesh',
  body: 'b-cube',
  meshId: 1,
  element: 'face',
  index: 0,
  key: 'mesh:1:face:0',
  world: [0, 0, 20],
}

/**
 * Holds the core's answer to every pick_reference, the top face, until the test resolves the
 * returned promise. The fake core answers in request order, so every later request waits too.
 */
function holdTopFacePicks(fake: FakeApp): Deferred<undefined> {
  const picked = deferred<undefined>()
  fake.transport.handle('pick_reference', async () => {
    await picked.promise
    return { result: { type: 'pick_reference', element: CUBE_TOP_FACE } }
  })
  return picked
}

/** Clicks the top face with its pick_reference answered only once Sketch has been pressed. */
async function clickTopFaceThenSketch(fake: FakeApp): Promise<void> {
  const picked = holdTopFacePicks(fake)
  const click = pickAt(fake.app, TOP_FACE_PICK, { toggle: false })
  const started = startNewSketch(fake.app)
  picked.resolve(undefined)
  await click
  await started
  await settle()
}

describe('a click the core answers after Sketch is pressed (3.16.1)', () => {
  it('a face clicked right before Sketch takes the new sketch', async () => {
    const fake = await cube()
    await clickTopFaceThenSketch(fake)
    expect(newSketchEdit(fake).edit?.feature['plane']).toMatchObject({
      kind: 'face',
      body: 'b-cube',
      face: CUBE_TOP_FACE.reference,
    })
    expect(fake.app.store.getState().sketch?.sketchId).toBe(newSketchEdit(fake).edit?.feature.id)
  })

  it('a face clicked right before Sketch replaces the plane selected before it', async () => {
    const fake = await cube()
    select(fake.app, [originItem('XZ')], 'replace')
    await clickTopFaceThenSketch(fake)
    expect(fake.transport.commandsOfType('apply_edits')).toHaveLength(1)
    expect(newSketchEdit(fake).edit?.feature['plane']).toMatchObject({
      kind: 'face',
      face: CUBE_TOP_FACE.reference,
    })
  })
})

describe('a feature key pressed right after Sketch (3.7, 3.16.1)', () => {
  // A new sketch starts from the Sketch command and from the Sketch context alike.
  const SKETCH_STARTS = [
    ['the Sketch command', (app: App) => startFeatureCommand(app, 'sketch')],
    ['the Sketch context', (app: App) => setContext(app, 'sketch')],
  ] as const

  it.each(SKETCH_STARTS)(
    'with a click on its way, opens its command after %s has put the sketch on the face',
    async (_start, startSketch) => {
      const fake = await cube()
      const picked = holdTopFacePicks(fake)
      const click = pickAt(fake.app, TOP_FACE_PICK, { toggle: false })
      const sketched = startSketch(fake.app)
      const started = startFeatureCommand(fake.app, 'chamfer')
      picked.resolve(undefined)
      await Promise.all([click, sketched, started])
      await settle()
      const types = fake.transport.commandTypes()
      expect(
        types.filter((type) => type === 'apply_edits' || type === 'begin_transaction'),
      ).toEqual(['apply_edits', 'begin_transaction'])
      expect(newSketchEdit(fake).command?.label).toBe('New sketch')
      expect(newSketchEdit(fake).edit?.feature['plane']).toMatchObject({
        kind: 'face',
        face: CUBE_TOP_FACE.reference,
      })
      expect(fake.app.store.getState().notifications).toEqual([])
      expect(fake.app.store.getState()).toMatchObject({
        context: 'model',
        sketch: null,
        command: { featureType: 'chamfer' },
      })
    },
  )

  it.each(SKETCH_STARTS)(
    'opens its command after %s, still canceling the open command, has opened plane picking',
    async (_start, startSketch) => {
      const fake = await cube()
      await startFeatureCommand(fake.app, 'fillet')
      await settle()
      fake.transport.clearRequests()
      const canceled = deferred<undefined>()
      fake.transport.handle('cancel_transaction', async (command, core, buffers) => {
        await canceled.promise
        return core.reply(command, buffers)
      })
      const sketched = startSketch(fake.app)
      const started = startFeatureCommand(fake.app, 'chamfer')
      canceled.resolve(undefined)
      await Promise.all([sketched, started])
      await settle()
      expect(fake.transport.commandTypes()).toEqual(['cancel_transaction', 'begin_transaction'])
      expect(fake.app.store.getState().notifications).toEqual([])
      expect(fake.app.store.getState()).toMatchObject({
        context: 'model',
        sketch: null,
        command: { featureType: 'chamfer' },
      })
    },
  )
})

describe('enterSketch, setSketchTool and finishSketch (3.16)', () => {
  it('enterSketch opens a session for the sketch in the Sketch context', async () => {
    const fake = await cube()
    await enterSketch(fake.app, 'f-sk1')
    await settle()
    const state = fake.app.store.getState()
    expect(state.context).toBe('sketch')
    expect(state.sketch).toMatchObject({
      sketchId: 'f-sk1',
      tool: null,
      selection: [],
      dragging: false,
    })
    expect(currentDocument(fake.app).sketches['f-sk1']?.solution).toEqual(CUBE_SKETCH_SOLUTION)
  })

  it('entering clears the app selection, so a command run from the sketch takes what it made (3.3, 3.16.1)', async () => {
    const fake = await cube()
    select(fake.app, [originItem('XY')], 'replace')
    await enterSketch(fake.app, 'f-sk1')
    await settle()
    expect(fake.app.store.getState().selection).toEqual([])
    await startFeatureCommand(fake.app, 'extrude')
    await settle()
    expect(fake.app.store.getState().command?.params['profile']).toEqual({
      sketch: 'f-sk1',
      regions: [CUBE_REGION.reference],
    })
  })

  it('a sketch started on the selected plane starts with an empty app selection', async () => {
    const fake = await cube()
    select(fake.app, [originItem('XY')], 'replace')
    await startNewSketch(fake.app)
    await settle()
    expect(fake.app.store.getState().sketch?.sketchId).not.toBeNull()
    expect(fake.app.store.getState().selection).toEqual([])
  })

  it('setSketchTool sets and clears the active tool', async () => {
    const fake = await cube()
    await enterSketch(fake.app, 'f-sk1')
    setSketchTool(fake.app, 'line')
    expect(fake.app.store.getState().sketch?.tool).toBe('line')
    setSketchTool(fake.app, null)
    expect(fake.app.store.getState().sketch?.tool).toBeNull()
  })

  it('finishSketch returns to Model, clears the session and keeps the sketch visible for the session', async () => {
    const fake = await cube()
    await enterSketch(fake.app, 'f-sk1')
    await finishSketch(fake.app)
    await settle()
    const state = fake.app.store.getState()
    expect(state.context).toBe('model')
    expect(state.sketch).toBeNull()
    expect(state.view.sessionVisible['feature:f-sk1']).toBe(true)
  })
})

describe('sketch record readers', () => {
  const FACE_SKETCH = {
    id: 'f-sk2',
    type: 'sketch',
    plane: {
      kind: 'face',
      body: 'b-cube',
      face: { q: 'f-ext1/end-cap', fp: 'plane|n=0,0,1|c=0,0,20|a=400|adj=4,4' },
      fallback: { origin: [0, 0, 20], normal: [0, 0, 1], x_axis: [1, 0, 0] },
    },
    entities: [],
    constraints: [],
  }
  const state = stateWithDocument({
    ...CUBE_DOCUMENT,
    features: [...(CUBE_DOCUMENT.features ?? []), FACE_SKETCH],
  })

  it('selectSketchRecord finds a sketch record and is null for other features and no id', () => {
    expect(selectSketchRecord(state, 'f-sk1')?.id).toBe('f-sk1')
    expect(selectSketchRecord(state, 'f-ext1')).toBeNull()
    expect(selectSketchRecord(state, null)).toBeNull()
  })

  it('storedSketchParams reads the stored plane, entities and constraints', () => {
    expect(storedSketchParams(state, 'f-sk2')).toEqual({
      plane: FACE_SKETCH.plane,
      entities: [],
      constraints: [],
    })
    expect(storedSketchParams(state, 'f-ext1')).toBeNull()
  })

  it('sketchFaceBody is the body of a face sketch and null for a sketch on a plane', () => {
    expect(sketchFaceBody(state, 'f-sk2')).toBe('b-cube')
    expect(sketchFaceBody(state, 'f-sk1')).toBeNull()
    expect(sketchFaceBody(state, null)).toBeNull()
  })
})
