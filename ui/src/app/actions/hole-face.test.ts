import { describe, expect, it } from 'vitest'
import type { ElementDescriptor } from '../../generated/protocol'
import { MESSAGES } from '../../messages'
import { activeCommand, appliedEdits, type FakeApp, startWithDocument } from '../../test/fakes/app'
import {
  CUBE_DOCUMENT,
  CUBE_FRONT_FACE,
  CUBE_TOP_FACE,
  featuresOf,
} from '../../test/fakes/documents'
import { elementItem } from '../../test/fakes/selection'
import { settle } from '../../test/fakes/settle'
import { pointSelectionItem } from '../../sketch/hit'
import type { PointerRay } from '../../viewport/picking'
import { cancelCommand, capturePick, startFeatureCommand } from './command'

// Placing holes on a face (ui.md 3.7): a click on a flat face places a hole center in a helper
// sketch the command writes, so Hole works from a face as well as from sketch points.

/** A ray straight down onto the cube's top face at (x, y), with 0.1 mm per CSS pixel. */
function rayAt(x: number, y: number): PointerRay {
  return { origin: [x, y, 100], direction: [0, 0, -1], millimetersPerPixel: 0.1 }
}

async function holeOnCube(): Promise<FakeApp> {
  const fake = await startWithDocument('cube.ptsm', CUBE_DOCUMENT)
  await startFeatureCommand(fake.app, 'hole')
  await settle()
  fake.transport.clearRequests()
  return fake
}

const TOP = elementItem(CUBE_TOP_FACE, 'Cube')

/**
 * The cube's top after a Move body by (100, 0, 0): its world placement at (100, 0, 20), and the
 * same placement in body coordinates as its fallback (engine.md 3.9).
 */
const MOVED_TOP_FACE: ElementDescriptor = {
  ...CUBE_TOP_FACE,
  centroid: [100, 0, 20],
  placement: { origin: [100, 0, 20], x_axis: [1, 0, 0], normal: [0, 0, 1] },
  fallback: { origin: [0, 0, 20], x_axis: [1, 0, 0], normal: [0, 0, 1] },
}

describe('placing holes on a face (U-100)', () => {
  it('U-100 gives the command a helper sketch on the face with a point where the ray met it', async () => {
    const fake = await holeOnCube()
    await capturePick(fake.app, TOP, rayAt(3, 4))
    const { helper, params } = activeCommand(fake.app)
    expect(helper?.params.plane).toEqual({
      kind: 'face',
      body: 'b-cube',
      face: CUBE_TOP_FACE.reference,
      fallback: CUBE_TOP_FACE.fallback,
      fallback_frame: 'body',
    })
    expect(helper?.placement).toEqual(CUBE_TOP_FACE.placement)
    expect(helper?.params.constraints).toEqual([])
    const [entity] = helper?.params.entities ?? []
    expect(entity).toEqual({
      id: expect.stringMatching(/^e-/) as unknown,
      type: 'point',
      position: [3, 4],
    })
    expect(params['points']).toEqual([entity?.id])
    expect(params['sketch']).toBe(helper?.id)
    expect(params['body']).toBe('b-cube')
  })

  it('U-100 adds a point for a second click on the same face', async () => {
    const fake = await holeOnCube()
    await capturePick(fake.app, TOP, rayAt(3, 4))
    await capturePick(fake.app, TOP, rayAt(-2, 1))
    const { helper, params } = activeCommand(fake.app)
    const entities = helper?.params.entities ?? []
    expect(entities.map((entity) => (entity.type === 'point' ? entity.position : null))).toEqual([
      [3, 4],
      [-2, 1],
    ])
    expect(params['points']).toEqual(entities.map((entity) => entity.id))
  })

  it('U-100 refuses another face with hole_one_face and leaves the params alone', async () => {
    const fake = await holeOnCube()
    await capturePick(fake.app, TOP, rayAt(3, 4))
    const before = activeCommand(fake.app).params
    const front: ElementDescriptor = CUBE_FRONT_FACE
    await capturePick(fake.app, elementItem(front, 'Cube'), {
      origin: [0, -100, 10],
      direction: [0, 1, 0],
      millimetersPerPixel: 0.1,
    })
    expect(activeCommand(fake.app).params).toEqual(before)
    expect(fake.app.store.getState().pickHint).toEqual({
      message: MESSAGES.hole_one_face(),
      field: 'points',
    })
  })

  it('U-100 replaces points of another sketch with the helper point', async () => {
    const fake = await holeOnCube()
    await capturePick(fake.app, pointSelectionItem('f-sk2', 'e-p1'))
    expect(activeCommand(fake.app).params).toMatchObject({ points: ['e-p1'], sketch: 'f-sk2' })
    await capturePick(fake.app, TOP, rayAt(3, 4))
    const { helper, params } = activeCommand(fake.app)
    expect(params['points']).toEqual([helper?.params.entities?.[0]?.id])
    expect(params['sketch']).toBe(helper?.id)
  })

  it("U-100 on a face moved by a Move body, stores the descriptor's fallback and places the center in the face's world placement", async () => {
    const fake = await holeOnCube()
    await capturePick(fake.app, elementItem(MOVED_TOP_FACE, 'Cube'), rayAt(103, 4))
    const { helper } = activeCommand(fake.app)
    expect(helper?.params.plane).toEqual({
      kind: 'face',
      body: 'b-cube',
      face: MOVED_TOP_FACE.reference,
      fallback: MOVED_TOP_FACE.fallback,
      fallback_frame: 'body',
    })
    expect(helper?.placement).toEqual(MOVED_TOP_FACE.placement)
    const [entity] = helper?.params.entities ?? []
    expect(entity?.type === 'point' ? entity.position : null).toEqual([3, 4])
  })

  it('U-100 places a second center on a moved face in its world placement too', async () => {
    const fake = await holeOnCube()
    const moved = elementItem(MOVED_TOP_FACE, 'Cube')
    await capturePick(fake.app, moved, rayAt(103, 4))
    await capturePick(fake.app, moved, rayAt(98, 1))
    const entities = activeCommand(fake.app).helper?.params.entities ?? []
    expect(entities.map((entity) => (entity.type === 'point' ? entity.position : null))).toEqual([
      [3, 4],
      [-2, 1],
    ])
  })

  it('U-100 snaps a ray within 8 times millimetersPerPixel of the centroid to it', async () => {
    const fake = await holeOnCube()
    await capturePick(fake.app, TOP, rayAt(0.5, 0.5))
    const [entity] = activeCommand(fake.app).helper?.params.entities ?? []
    expect(entity?.type === 'point' ? entity.position : null).toEqual([0, 0])
  })

  it('refuses a face that is not flat with pick_points', async () => {
    const fake = await holeOnCube()
    const round: ElementDescriptor = { ...CUBE_TOP_FACE, surface: 'cylinder' }
    await capturePick(fake.app, elementItem(round, 'Cube'), rayAt(3, 4))
    expect(activeCommand(fake.app).helper).toBeNull()
    expect(fake.app.store.getState().pickHint).toEqual({
      message: MESSAGES.pick_points(),
      field: 'points',
    })
  })

  it('clicking a helper point removes it, and the last one takes the helper with it', async () => {
    const fake = await holeOnCube()
    await capturePick(fake.app, TOP, rayAt(3, 4))
    await capturePick(fake.app, TOP, rayAt(-2, 1))
    const helper = activeCommand(fake.app).helper
    const [first, second] = helper?.params.entities ?? []
    if (helper === null || first === undefined || second === undefined) {
      throw new Error('The helper has no two points.')
    }
    await capturePick(fake.app, pointSelectionItem(helper.id, first.id))
    expect(activeCommand(fake.app).params['points']).toEqual([second.id])
    expect(activeCommand(fake.app).helper?.params.entities).toEqual([second])
    await capturePick(fake.app, pointSelectionItem(helper.id, second.id))
    expect(activeCommand(fake.app).helper).toBeNull()
    expect(activeCommand(fake.app).params['sketch']).toBeUndefined()
  })
})

describe('the helper sketch in previews (U-101)', () => {
  it('U-101 adds the helper right before the hole, updates both, and removes it when unused', async () => {
    const fake = await holeOnCube()
    const index = featuresOf(CUBE_DOCUMENT).length
    await capturePick(fake.app, TOP, rayAt(3, 4))
    await settle()
    const helperId = activeCommand(fake.app).helper?.id
    const featureId = activeCommand(fake.app).featureId
    const [first] = appliedEdits(fake)
    expect(first?.map((edit) => edit.type)).toEqual(['add_feature', 'add_feature'])
    expect(first?.[0]).toMatchObject({ type: 'add_feature', index, feature: { id: helperId } })
    expect(first?.[1]).toMatchObject({
      type: 'add_feature',
      index: index + 1,
      feature: { id: featureId, type: 'hole' },
    })
    expect(activeCommand(fake.app).helper?.applied).toBe(true)

    await capturePick(fake.app, TOP, rayAt(-2, 1))
    await settle()
    const second = appliedEdits(fake)[1] ?? []
    expect(second.map((edit) => edit.type)).toEqual(['update_feature', 'update_feature'])
    expect(second[0]).toMatchObject({ feature: { id: helperId } })
    expect(second[1]).toMatchObject({ feature: { id: featureId } })

    await capturePick(fake.app, pointSelectionItem('f-sk1', 'e-1.start'))
    await settle()
    const third = appliedEdits(fake)[2] ?? []
    expect(third).toContainEqual({ type: 'remove_feature', id: helperId })
    expect(activeCommand(fake.app).helper).toBeNull()
  })

  it('U-101 cancelCommand calls cancelTransaction and nothing else', async () => {
    const fake = await holeOnCube()
    await capturePick(fake.app, TOP, rayAt(3, 4))
    await settle()
    fake.transport.clearRequests()
    await cancelCommand(fake.app)
    await settle()
    expect(fake.transport.commandTypes()).toEqual(['cancel_transaction'])
  })
})
