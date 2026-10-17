import { describe, expect, it } from 'vitest'
import type { ElementDescriptor } from '../../generated/protocol'
import { CYLINDRICAL_FACE, cubeDocumentState, readyState } from '../../layout/fixtures.testing'
import { createModelApp } from '../../layout/test-app.testing'
import { CUBE_TOP_FRONT_EDGE } from '../../test/fakes/documents'
import { elementItem } from '../../test/fakes/selection'
import { capturePick, setActiveField, startFeatureCommand } from './command'

// Axes picked on the model keep the side they were picked on (ui.md 2.12, features.md 2.5, U-143):
// a straight edge or a round face picked for an axis stores the descriptor's `axis_direction`, in
// body coordinates, as the axis's `direction`.

/** The cube's top front edge with the axis direction the engine reports for it (engine.md 3.9). */
const DIRECTED_EDGE: ElementDescriptor = { ...CUBE_TOP_FRONT_EDGE, axis_direction: [1, 0, 0] }

/** The peg's side with its axis direction in body coordinates. */
const DIRECTED_FACE: ElementDescriptor = { ...CYLINDRICAL_FACE, axis_direction: [0, 0, 1] }

/** A command of `featureType` on cube.ptsm with its Axis field active. */
async function axisCommand(featureType: string) {
  const { app } = createModelApp({ state: readyState(cubeDocumentState()) })
  await startFeatureCommand(app, featureType)
  setActiveField(app, 'axis')
  return app
}

describe('axis picks store the picked direction (2.12, U-143)', () => {
  it("U-143 an edge picked for Revolve's Axis stores its axis_direction as direction", async () => {
    const app = await axisCommand('revolve')
    await capturePick(app, elementItem(DIRECTED_EDGE))
    expect(app.store.getState().command?.params['axis']).toEqual({
      kind: 'edge',
      body: 'b-cube',
      edge: DIRECTED_EDGE.reference,
      direction: [1, 0, 0],
    })
  })

  it("U-143 a round face picked for Circular pattern's Axis stores its axis_direction as direction", async () => {
    const app = await axisCommand('circular_pattern')
    await capturePick(app, elementItem(DIRECTED_FACE, 'Peg'))
    expect(app.store.getState().command?.params['axis']).toEqual({
      kind: 'face',
      body: 'b-peg',
      face: DIRECTED_FACE.reference,
      direction: [0, 0, 1],
    })
  })

  it('U-143 an edge whose descriptor has no axis_direction stores no direction', async () => {
    const app = await axisCommand('revolve')
    await capturePick(app, elementItem(CUBE_TOP_FRONT_EDGE))
    const axis = app.store.getState().command?.params['axis']
    expect(axis).toEqual({ kind: 'edge', body: 'b-cube', edge: CUBE_TOP_FRONT_EDGE.reference })
    expect(Object.keys(axis as object)).not.toContain('direction')
  })

  it('U-143 a preselected edge fills the Axis of a Revolve with its direction (3.7 step 4)', async () => {
    const { app } = createModelApp({
      state: readyState(cubeDocumentState(), { selection: [elementItem(DIRECTED_EDGE)] }),
    })
    await startFeatureCommand(app, 'revolve')
    expect(app.store.getState().command?.params['axis']).toEqual({
      kind: 'edge',
      body: 'b-cube',
      edge: DIRECTED_EDGE.reference,
      direction: [1, 0, 0],
    })
  })
})
