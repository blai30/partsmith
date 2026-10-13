import { describe, expect, it } from 'vitest'
import type { Document } from '../../generated/protocol'
import { documentState, readyState } from '../../layout/fixtures.testing'
import { createModelApp } from '../../layout/test-app.testing'
import { activeCommand, appliedEdits, startWithDocument } from '../../test/fakes/app'
import {
  CUBE_DOCUMENT,
  CUBE_REGION,
  CUBE_SKETCH_SOLUTION,
  CUBE_TOP_FACE,
  featuresOf,
} from '../../test/fakes/documents'
import { bodyItem, originItem, regionItem } from '../../test/fakes/selection'
import { settle } from '../../test/fakes/settle'
import { CUBE_BOUNDS, meshSummary } from '../../test/fakes/state'
import { capturePick, setCommandField, startFeatureCommand } from './command'
import { select } from './selection'

// Bodies a command creates (ui.md 2.12): new bodies for copies and split parts, and the cut
// direction of an Extrude sketched on a face of its body (3.7).

describe('new bodies for copies and split parts (U-105)', () => {
  it('U-105 a split keeps both parts: the first application adds a body for Target', async () => {
    const fake = await startWithDocument('cube.ptsm', CUBE_DOCUMENT)
    select(fake.app, [bodyItem('b-cube', 'Cube'), originItem('XZ')], 'replace')
    await startFeatureCommand(fake.app, 'split_body')
    await settle()
    const newBody = activeCommand(fake.app).newBody
    expect(newBody).toMatchObject({ name: 'Body 1', path: 'target' })
    const [first] = appliedEdits(fake)
    expect(first?.map((edit) => edit.type)).toEqual(['add_body', 'add_feature'])
    expect(first?.[0]).toMatchObject({ body: { id: newBody?.id, name: 'Body 1' } })
    expect(first?.[1]).toMatchObject({ feature: { target: newBody?.id, body: 'b-cube' } })

    setCommandField(fake.app, 'target', undefined)
    await settle()
    const second = appliedEdits(fake)[1] ?? []
    expect(second.map((edit) => edit.type)).toEqual(['update_feature', 'remove_body'])
    expect(second[0]).toMatchObject({ type: 'update_feature' })
    expect(
      second[0]?.type === 'update_feature' ? second[0].feature['target'] : null,
    ).toBeUndefined()
    expect(second[1]).toEqual({ type: 'remove_body', id: newBody?.id })
    expect(activeCommand(fake.app).newBody).toBeNull()
  })

  it('U-105 a mirror makes a new body only with op new, and removes it when op goes back', async () => {
    const fake = await startWithDocument('cube.ptsm', CUBE_DOCUMENT)
    select(fake.app, [originItem('XZ')], 'replace')
    await startFeatureCommand(fake.app, 'mirror')
    await settle()
    expect(activeCommand(fake.app).params).toMatchObject({ op: 'join', body: 'b-cube' })
    expect(activeCommand(fake.app).params['target']).toBeUndefined()
    fake.transport.clearRequests()

    setCommandField(fake.app, 'op', 'new')
    const newBody = activeCommand(fake.app).newBody
    expect(newBody).toMatchObject({ name: 'Body 1', path: 'target' })
    expect(activeCommand(fake.app).params['target']).toBe(newBody?.id)
    await settle()
    const [added] = appliedEdits(fake)
    expect(added?.map((edit) => edit.type)).toEqual(['add_body', 'update_feature'])

    setCommandField(fake.app, 'op', 'join')
    await settle()
    const removed = appliedEdits(fake)[1] ?? []
    expect(removed.map((edit) => edit.type)).toEqual(['update_feature', 'remove_body'])
    expect(activeCommand(fake.app).params['target']).toBeUndefined()
    expect(activeCommand(fake.app).params['body']).toBe('b-cube')
  })

  it('U-105 an extrude with op new still creates its body in Body', async () => {
    const fake = await startWithDocument(
      'rect.ptsm',
      {
        ...CUBE_DOCUMENT,
        bodies: [],
        features: featuresOf(CUBE_DOCUMENT).slice(0, 1),
      },
      { core: { solutions: { 'f-sk1': CUBE_SKETCH_SOLUTION } } },
    )
    select(fake.app, [regionItem('f-sk1', CUBE_REGION)], 'replace')
    await startFeatureCommand(fake.app, 'extrude')
    await settle()
    const newBody = activeCommand(fake.app).newBody
    expect(newBody).toMatchObject({ path: 'body' })
    expect(activeCommand(fake.app).params['body']).toBe(newBody?.id)
    expect(activeCommand(fake.app).params['target']).toBeUndefined()
  })
})

/** cube.ptsm plus `f-sk2`, a sketch on the cube's top face with one region. */
const FACE_SKETCH_DOCUMENT: Document = {
  ...CUBE_DOCUMENT,
  features: [
    ...featuresOf(CUBE_DOCUMENT),
    {
      id: 'f-sk2',
      type: 'sketch',
      plane: {
        kind: 'face',
        body: 'b-cube',
        face: CUBE_TOP_FACE.reference,
        fallback: CUBE_TOP_FACE.placement,
      },
      entities: [],
      constraints: [],
    },
  ],
}

/** An Extrude of a region of `sketch` on the cube, with its body the cube. */
async function cubeExtrudeOf(sketch: 'f-sk1' | 'f-sk2') {
  const test = createModelApp({
    state: readyState(
      documentState(FACE_SKETCH_DOCUMENT, {
        meshes: { 'b-cube': meshSummary(1, CUBE_BOUNDS) },
        sketches: {
          'f-sk1': { solution: CUBE_SKETCH_SOLUTION, display: null },
          'f-sk2': { solution: { ...CUBE_SKETCH_SOLUTION, feature: 'f-sk2' }, display: null },
        },
      }),
    ),
  })
  await startFeatureCommand(test.app, 'extrude')
  await capturePick(test.app, regionItem(sketch, CUBE_REGION))
  return test
}

describe('cuts go into the part (3.7, U-106)', () => {
  it('U-106 a cut sketched on a face of the body points into it, a join out of it', async () => {
    const { app } = await cubeExtrudeOf('f-sk2')
    expect(app.store.getState().command?.params).toMatchObject({ op: 'join', body: 'b-cube' })
    setCommandField(app, 'op', 'cut')
    expect(app.store.getState().command?.params['reverse']).toBe(true)
    setCommandField(app, 'op', 'join')
    expect(app.store.getState().command?.params['reverse']).toBe(false)
  })

  it('U-106 leaves Reverse alone once the user set it', async () => {
    const { app } = await cubeExtrudeOf('f-sk2')
    setCommandField(app, 'op', 'cut')
    setCommandField(app, 'reverse', false)
    setCommandField(app, 'op', 'intersect')
    expect(app.store.getState().command?.params['reverse']).toBe(false)
    setCommandField(app, 'op', 'join')
    expect(app.store.getState().command?.params['reverse']).toBe(false)
  })

  it('U-106 never touches Reverse for a sketch on an origin plane', async () => {
    const { app } = await cubeExtrudeOf('f-sk1')
    setCommandField(app, 'op', 'cut')
    expect(app.store.getState().command?.params['reverse']).toBe(false)
    setCommandField(app, 'op', 'join')
    expect(app.store.getState().command?.params['reverse']).toBe(false)
  })
})
