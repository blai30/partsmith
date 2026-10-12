import { describe, expect, it } from 'vitest'
import {
  bodyItem,
  cubeDocumentState,
  datumItem,
  documentState,
  elementItem,
  featureItem,
  originItem,
  readyState,
  rectSketchDocumentState,
  regionItem,
  sketchPointItem,
  CYLINDRICAL_FACE,
} from '../layout/fixtures.testing'
import {
  createAppStore,
  type AppState,
  type DocumentState,
  type SelectionItem,
} from '../state/store'
import { catalogOf, TEST_CATALOG_ENTRIES } from '../test/fakes/catalog'
import {
  CUBE_TOP_FACE,
  CUBE_TOP_FRONT_EDGE,
  RECT_REGION,
  RECT_SKETCH_SOLUTION,
  TWO_BODIES_DOCUMENT,
} from '../test/fakes/documents'
import { suggestedCommands } from './suggestions'

// The next steps of the selection inspector (ui.md 3.8).

function stateFor(document: DocumentState, overrides: Partial<AppState> = {}): AppState {
  return createAppStore(readyState(document, overrides)).getState()
}

function suggested(
  items: readonly SelectionItem[],
  document: DocumentState = cubeDocumentState(),
  overrides: Partial<AppState> = {},
): readonly string[] {
  return suggestedCommands(stateFor(document, overrides), items)
}

describe('suggestedCommands (U-114)', () => {
  it('U-114 gives Move body, Mirror and Split body for one body', () => {
    expect(suggested([bodyItem('b-cube', 'Cube')])).toEqual([
      'feature.move_body',
      'feature.mirror',
      'feature.split_body',
    ])
  })

  it('U-114 gives Boolean for two bodies', () => {
    const twoBodies = documentState(TWO_BODIES_DOCUMENT)
    expect(suggested([bodyItem('b-cube', 'Cube'), bodyItem('b-peg', 'Peg')], twoBodies)).toEqual([
      'feature.boolean',
    ])
  })

  it('U-114 gives Mirror and Split body for a body and origin XZ, in either order', () => {
    expect(suggested([bodyItem('b-cube', 'Cube'), originItem('XZ')])).toEqual([
      'feature.mirror',
      'feature.split_body',
    ])
    expect(suggested([originItem('XZ'), bodyItem('b-cube', 'Cube')])).toEqual([
      'feature.mirror',
      'feature.split_body',
    ])
  })

  it('U-114 gives Sketch and Datum plane for origin XY, and for a datum plane', () => {
    expect(suggested([originItem('XY')])).toEqual(['feature.sketch', 'feature.datum_plane'])
    expect(suggested([datumItem('f-dat1', 'Datum plane 1')])).toEqual([
      'feature.sketch',
      'feature.datum_plane',
    ])
  })

  it('U-114 gives Sketch, Hole and Shell for one flat face, and Shell for another face', () => {
    expect(suggested([elementItem(CUBE_TOP_FACE)])).toEqual([
      'feature.sketch',
      'feature.hole',
      'feature.shell',
    ])
    expect(suggested([elementItem(CYLINDRICAL_FACE, 'Peg')])).toEqual(['feature.shell'])
  })

  it('U-114 gives Extrude and Revolve for the sketch feature of rect-sketch.ptsm', () => {
    expect(suggested([featureItem('f-sk1', 'Sketch 1')], rectSketchDocumentState())).toEqual([
      'feature.extrude',
      'feature.revolve',
    ])
  })

  it('U-114 gives Hole for a sketch point in Model context', () => {
    expect(
      suggested([sketchPointItem('f-sk1', 'e-p1')], rectSketchDocumentState(), {
        context: 'model',
      }),
    ).toEqual(['feature.hole'])
  })

  it('U-114 leaves out a type the catalog lacks: one body without split_body gives the first two', () => {
    const catalog = catalogOf(TEST_CATALOG_ENTRIES.filter((entry) => entry.type !== 'split_body'))
    expect(suggested([bodyItem('b-cube', 'Cube')], cubeDocumentState(), { catalog })).toEqual([
      'feature.move_body',
      'feature.mirror',
    ])
  })

  it('gives Fillet and Chamfer for edges, and Extrude and Revolve for regions', () => {
    expect(suggested([elementItem(CUBE_TOP_FRONT_EDGE)])).toEqual([
      'feature.fillet',
      'feature.chamfer',
    ])
    expect(suggested([regionItem('f-sk1', RECT_REGION)], rectSketchDocumentState())).toEqual([
      'feature.extrude',
      'feature.revolve',
    ])
  })

  it('gives Hole as well for a sketch with points that are not construction', () => {
    const solution = {
      ...RECT_SKETCH_SOLUTION,
      entities: [
        ...RECT_SKETCH_SOLUTION.entities,
        {
          id: 'e-p1',
          type: 'point' as const,
          position: [10, 5] as [number, number],
          constrained: 'under_constrained' as const,
        },
      ],
    }
    const document = rectSketchDocumentState({ sketches: { 'f-sk1': { solution, display: null } } })
    expect(suggested([featureItem('f-sk1', 'Sketch 1')], document)).toEqual([
      'feature.extrude',
      'feature.revolve',
      'feature.hole',
    ])
  })

  it('suggests nothing for origin axes, other features, mixed selections and in Sketch context', () => {
    expect(suggested([originItem('X')])).toEqual([])
    expect(suggested([featureItem('f-ext1', 'Extrude 1')])).toEqual([])
    expect(suggested([bodyItem('b-cube', 'Cube'), elementItem(CUBE_TOP_FACE)])).toEqual([])
    expect(
      suggested([elementItem(CUBE_TOP_FACE)], cubeDocumentState(), { context: 'sketch' }),
    ).toEqual([])
  })
})
