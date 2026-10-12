import { describe, expect, it } from 'vitest'
import type { Document } from '../../generated/protocol'
import type { SketchChange } from '../../sketch/session'
import { currentDocument, startWithDocument } from '../../test/fakes/app'
import { CUBE_DOCUMENT, CUBE_SKETCH, CUBE_SKETCH_SOLUTION } from '../../test/fakes/documents'
import { editsOf } from '../../test/fakes/protocol'
import { settle } from '../../test/fakes/settle'
import { addSketchShape, enterSketch, finishSketch } from './sketch'

// A sketch change waits in the mutation queue behind a slow rebuild; it still belongs to the sketch
// the user drew it in, and finishing or moving to another sketch waits for it (3.16.3, 3.16.11).

/** cube.ptsm with a second, empty sketch `f-sk2` on the XZ plane. */
const TWO_SKETCHES: Document = {
  ...CUBE_DOCUMENT,
  features: [
    ...(CUBE_DOCUMENT.features ?? []),
    { id: 'f-sk2', type: 'sketch', plane: { kind: 'origin', plane: 'XZ' }, entities: [], constraints: [] },
  ],
}

/** A line from (0, 0) to (30, 0), with nothing to probe. */
const LINE: SketchChange = {
  add: { entities: [{ id: 'e-new', type: 'line', start: [0, 0], end: [30, 0] }], constraints: [] },
  requested: [],
  inferred: [],
}

async function editing(sketchId: string) {
  const fake = await startWithDocument('cube.ptsm', TWO_SKETCHES, {
    core: { solutions: { 'f-sk1': CUBE_SKETCH_SOLUTION } },
  })
  await enterSketch(fake.app, sketchId)
  await settle()
  fake.transport.clearRequests()
  return fake
}

/** The ids of the sketches that the update_feature edits sent so far name. */
function updatedSketches(fake: Awaited<ReturnType<typeof editing>>): readonly string[] {
  return fake.transport
    .commandsOfType('apply_edits')
    .flatMap((command) => editsOf(command))
    .flatMap((edit) => (edit.type === 'update_feature' ? [edit.feature.id] : []))
}

describe('sketch changes queued behind a slow rebuild', () => {
  it('a line drawn just before Finish sketch lands in the sketch, and finishing waits for it', async () => {
    const fake = await editing('f-sk1')
    fake.transport.pause()
    const added = addSketchShape(fake.app, 'Line', LINE)
    const finished = finishSketch(fake.app, { selectResult: true })
    await settle()
    // Finishing waits for the line's commit, so the session is still the sketch's.
    expect(fake.app.store.getState().context).toBe('sketch')
    fake.transport.resume()
    expect(await added).toBe(true)
    await finished
    await settle()
    const state = fake.app.store.getState()
    expect(state.context).toBe('model')
    expect(state.sketch).toBeNull()
    expect(state.notifications).toEqual([])
    expect(updatedSketches(fake)).toEqual(['f-sk1'])
    const record = currentDocument(fake.app).recipe.features?.find((feature) => feature.id === 'f-sk1')
    expect(record?.['entities']).toHaveLength((CUBE_SKETCH['entities'] as unknown[]).length + 1)
  })

  it('a line drawn just before moving to another sketch lands in the sketch it was drawn in', async () => {
    const fake = await editing('f-sk1')
    fake.transport.pause()
    const added = addSketchShape(fake.app, 'Line', LINE)
    const entered = enterSketch(fake.app, 'f-sk2')
    await settle()
    expect(fake.app.store.getState().sketch?.sketchId).toBe('f-sk1')
    fake.transport.resume()
    expect(await added).toBe(true)
    await entered
    await settle()
    expect(fake.app.store.getState().sketch?.sketchId).toBe('f-sk2')
    expect(updatedSketches(fake)).toEqual(['f-sk1'])
    const second = currentDocument(fake.app).recipe.features?.find((feature) => feature.id === 'f-sk2')
    expect(second?.['entities']).toEqual([])
  })
})
