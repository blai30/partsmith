import { describe, expect, it } from 'vitest'
import type { ElementDescriptor } from '../../generated/protocol'
import { startWithDocument, type FakeApp } from '../../test/fakes/app'
import { CUBE_DOCUMENT, CUBE_TOP_FACE, CUBE_TOP_FRONT_EDGE } from '../../test/fakes/documents'
import { elementItem } from '../../test/fakes/selection'
import { settle } from '../../test/fakes/settle'
import { capturePick, startFeatureCommand } from './command'

// Faces for edges (ui.md 3.7): a face clicked while Fillet or Chamfer takes edges stands for every
// edge around it, with references from list_elements so the UI never builds one.

/** The edge around the top face shared with side `index`, as list_elements describes it. */
function topEdge(index: number): ElementDescriptor {
  const name = `edge(f-ext1/end-cap|f-ext1/side-${String(index)})`
  return { ...CUBE_TOP_FRONT_EDGE, name, reference: { q: name, fp: `line|side=${String(index)}` } }
}

const TOP_EDGES = [topEdge(0), topEdge(1), topEdge(2), topEdge(3)]

async function filletOnCube(): Promise<FakeApp> {
  const fake = await startWithDocument('cube.ptsm', CUBE_DOCUMENT, {
    core: { elements: { 'b-cube': TOP_EDGES.toReversed() } },
  })
  await startFeatureCommand(fake.app, 'fillet')
  await settle()
  return fake
}

function edgesOf(fake: FakeApp): unknown {
  return fake.app.store.getState().command?.params['edges']
}

const TOP = elementItem(CUBE_TOP_FACE, 'Cube')

describe('faces for edges (U-123)', () => {
  it('U-123 a face adds the edges around it in adjacent order, and the body with them', async () => {
    const fake = await filletOnCube()
    await capturePick(fake.app, TOP)
    expect(edgesOf(fake)).toEqual(TOP_EDGES.map((edge) => edge.reference))
    expect(fake.app.store.getState().command?.params['body']).toBe('b-cube')
  })

  it('U-123 the same face again removes them all', async () => {
    const fake = await filletOnCube()
    await capturePick(fake.app, TOP)
    await capturePick(fake.app, TOP)
    expect(edgesOf(fake)).toBeUndefined()
  })

  it('U-123 with two of them held, it adds the other two', async () => {
    const fake = await filletOnCube()
    await capturePick(fake.app, elementItem(TOP_EDGES[2] ?? topEdge(2), 'Cube'))
    await capturePick(fake.app, elementItem(TOP_EDGES[0] ?? topEdge(0), 'Cube'))
    await capturePick(fake.app, TOP)
    expect(edgesOf(fake)).toEqual([
      topEdge(2).reference,
      topEdge(0).reference,
      topEdge(1).reference,
      topEdge(3).reference,
    ])
  })

  it('lists the edges of a body once per model mesh', async () => {
    const fake = await filletOnCube()
    await capturePick(fake.app, TOP)
    await capturePick(fake.app, TOP)
    expect(fake.transport.commandTypes().filter((type) => type === 'list_elements')).toHaveLength(1)
  })

  it('records the face as one change of Edges', async () => {
    const fake = await filletOnCube()
    await capturePick(fake.app, TOP)
    expect(fake.app.store.getState().command?.history.map((step) => step.field)).toEqual(['Edges'])
  })
})
