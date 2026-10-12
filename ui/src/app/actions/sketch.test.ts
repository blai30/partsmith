import { afterEach, describe, expect, it, vi } from 'vitest'
import type { Command, Edit, ElementDescriptor } from '../../generated/protocol'
import { MESSAGES } from '../../messages'
import { currentDocument, type FakeApp, startWithDocument } from '../../test/fakes/app'
import {
  CUBE_DOCUMENT,
  CUBE_FEATURES_DOCUMENT,
  CUBE_FRONT_FACE,
  CUBE_SKETCH_SOLUTION,
  CUBE_TOP_FACE,
} from '../../test/fakes/documents'
import { elementItem, originItem } from '../../test/fakes/selection'
import { editsOf } from '../../test/fakes/protocol'
import { settle } from '../../test/fakes/settle'
import { select } from './selection'
import { createSketchOn, enterSketch, finishSketch, setSketchTool, startNewSketch } from './sketch'

type AddFeature = Extract<Edit, { type: 'add_feature' }>

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

  it('a flat face gives a face plane with its body, reference and placement as the fallback', async () => {
    const fake = await cube()
    await createSketchOn(fake.app, elementItem(CUBE_TOP_FACE, 'Cube'))
    await settle()
    expect(newSketchEdit(fake).edit?.feature['plane']).toEqual({
      kind: 'face',
      body: 'b-cube',
      face: CUBE_TOP_FACE.reference,
      fallback: CUBE_TOP_FACE.placement,
    })
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
