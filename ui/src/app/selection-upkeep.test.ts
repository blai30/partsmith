import { describe, expect, it } from 'vitest'
import type { Edit, ElementDescriptor } from '../generated/protocol'
import { currentDocument, selectionKeys, startWithDocument } from '../test/fakes/app'
import {
  CUBE_DOCUMENT,
  CUBE_FRONT_FACE,
  CUBE_TOP_FACE,
  CUBE_FEATURES_DOCUMENT,
  TWO_BODIES_DOCUMENT,
} from '../test/fakes/documents'
import { bodyItem, elementItem, featureItem } from '../test/fakes/selection'
import { settle } from '../test/fakes/settle'
import { applyEdits } from './actions/document'
import { runFixAction } from './actions/fixes'
import { select } from './actions/selection'

const RENAME: readonly Edit[] = [{ type: 'update_body', body: { id: 'b-cube', name: 'Box' } }]

describe('selection after rebuilds (3.6)', () => {
  it('U-82 after a mesh change, a face that resolves missing leaves the selection and one that resolves stays', async () => {
    const fake = await startWithDocument('cube.ptsm', CUBE_DOCUMENT)
    const top = elementItem(CUBE_TOP_FACE, 'Cube')
    const front = elementItem(CUBE_FRONT_FACE, 'Cube')
    select(fake.app, [top, front], 'replace')
    fake.transport.handle('resolve_references', (command) => ({
      result: {
        type: 'resolve_references',
        results: (command.references ?? []).map((reference) =>
          reference.reference.q === CUBE_TOP_FACE.reference.q
            ? { status: 'missing', matches: [] }
            : { status: 'resolved', matches: [] },
        ),
      },
    }))
    await applyEdits(fake.app, 'Rename', RENAME)
    await settle()
    expect(fake.transport.commandsOfType('resolve_references')).toEqual([
      {
        type: 'resolve_references',
        doc: currentDocument(fake.app).documentHandle,
        references: [
          { body: 'b-cube', reference: CUBE_TOP_FACE.reference },
          { body: 'b-cube', reference: CUBE_FRONT_FACE.reference },
        ],
      },
    ])
    expect(selectionKeys(fake.app)).toEqual([front.key])
  })

  it('resolves the selected faces of every body when several bodies are remeshed by one change', async () => {
    const fake = await startWithDocument('two-bodies.ptsm', TWO_BODIES_DOCUMENT)
    const pegSide: ElementDescriptor = {
      ...CUBE_FRONT_FACE,
      name: 'f-cyl1/side',
      body: 'b-peg',
      feature: 'f-cyl1',
      reference: { q: 'f-cyl1/side', fp: 'cylinder|n=0,0,1|c=0,0,20|a=502.65|adj=2,2' },
      surface: 'cylinder',
    }
    select(fake.app, [elementItem(CUBE_TOP_FACE, 'Cube'), elementItem(pegSide, 'Peg')], 'replace')
    fake.transport.handle('resolve_references', (command) => ({
      result: {
        type: 'resolve_references',
        results: (command.references ?? []).map(() => ({ status: 'missing', matches: [] })),
      },
    }))
    await applyEdits(fake.app, 'Rename', RENAME)
    await settle()
    // Both bodies get a new mesh in their own body_mesh event; the request that is answered
    // last must still ask about the faces of both.
    const asked = fake.transport.commandsOfType('resolve_references').at(-1)?.references ?? []
    expect(asked.map((reference) => reference.body)).toEqual(['b-cube', 'b-peg'])
    expect(selectionKeys(fake.app)).toEqual([])
  })

  it('an ambiguous face leaves the selection too', async () => {
    const fake = await startWithDocument('cube.ptsm', CUBE_DOCUMENT)
    select(fake.app, [elementItem(CUBE_TOP_FACE, 'Cube')], 'replace')
    fake.transport.handle('resolve_references', () => ({
      result: { type: 'resolve_references', results: [{ status: 'ambiguous', matches: [] }] },
    }))
    await applyEdits(fake.app, 'Rename', RENAME)
    await settle()
    expect(selectionKeys(fake.app)).toEqual([])
  })

  it('asks nothing when no face, edge or vertex is selected', async () => {
    const fake = await startWithDocument('cube.ptsm', CUBE_DOCUMENT)
    select(fake.app, [bodyItem('b-cube', 'Cube')], 'replace')
    await applyEdits(fake.app, 'Rename', RENAME)
    await settle()
    expect(fake.transport.commandTypes()).not.toContain('resolve_references')
    expect(selectionKeys(fake.app)).toEqual(['body:b-cube'])
  })

  it('features and bodies that no longer exist leave the selection', async () => {
    const fake = await startWithDocument('cube-features.ptsm', CUBE_FEATURES_DOCUMENT)
    select(
      fake.app,
      [featureItem('f-sh1', 'Shell 1'), featureItem('f-fil1', 'Fillet 1')],
      'replace',
    )
    await applyEdits(fake.app, 'Delete Shell 1', [{ type: 'remove_feature', id: 'f-sh1' }])
    await settle()
    expect(selectionKeys(fake.app)).toEqual(['feature:f-fil1'])
  })
})

describe('what a show action selected (4.2)', () => {
  it('stays shown while the selection stays, and goes when the selection changes', async () => {
    const fake = await startWithDocument('cube-features.ptsm', CUBE_FEATURES_DOCUMENT)
    await runFixAction(
      fake.app,
      {
        label: "Show 'Sketch 1'",
        run: { kind: 'show', feature: 'f-sk1' },
      },
      'panel',
    )
    expect(fake.app.store.getState().shown).toEqual({ feature: 'f-sk1', entity: null, ghost: null })
    await applyEdits(fake.app, 'Rename', RENAME)
    await settle()
    expect(fake.app.store.getState().shown).not.toBeNull()
    select(fake.app, [featureItem('f-ext1', 'Extrude 1')], 'replace')
    expect(fake.app.store.getState().shown).toBeNull()
  })
})
