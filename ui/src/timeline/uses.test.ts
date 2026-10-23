import { describe, expect, it } from 'vitest'
import type { FeatureRecord, Use } from '../generated/protocol'
import type { AppState } from '../state/store'
import { stateWithDocument } from '../test/fakes/state'
import {
  datumOnFace,
  DROP_DOCUMENT,
  DROP_USES,
  edgeReference,
  extrudeRecord,
  faceReference,
  filletRecord,
  stateWithUses,
  USED_BY_DOCUMENT,
  USED_BY_USES,
  USED_POINT_USES,
  USED_SKETCH_DOCUMENT,
  USED_SKETCH_USES,
  usesDocument,
  xySketchRecord,
} from '../test/fakes/uses'
import { dropWarning, usedByText, usedEntities, usersOf } from './uses'

// What a timeline drop would break, who uses a feature and which sketch entities later features
// use, from `list_uses` (ui.md 3.9, 3.16.10, engine.md 3.24).

/** The store state of a document of `features` at revision 5 whose uses are known at `usesRevision`. */
function stateWith(
  features: readonly FeatureRecord[],
  uses: readonly Use[],
  usesRevision = 5,
): AppState {
  return stateWithUses(usesDocument(features), uses, usesRevision)
}

/** The state of U-135: 'Extrude 1', 'Extrude 2' and 'Fillet 1', whose edge 'Extrude 2' made. */
function dropState(uses: readonly Use[] = DROP_USES, usesRevision = 5): AppState {
  return stateWithUses(DROP_DOCUMENT, uses, usesRevision)
}

/** A revolve of 'Sketch 2' about `axis`. */
function revolveAbout(axis: Record<string, unknown>): FeatureRecord {
  return {
    id: 'f-rev1',
    type: 'revolve',
    body: 'b-1',
    profile: { sketch: 'f-sk2', regions: [] },
    axis,
    extent: { kind: 'full' },
    op: 'join',
  }
}

/** The state of U-136: 'Extrude 1' used by 'Fillet 1' and 'Datum plane 1', with `uses` known. */
function usedByState(uses: readonly Use[] = USED_BY_USES): AppState {
  return stateWithUses(USED_BY_DOCUMENT, uses)
}

/** The state of U-137: 'Sketch 1' and the features after it, with `uses` known. */
function sketchState(uses: readonly Use[]): AppState {
  return stateWithUses(USED_SKETCH_DOCUMENT, uses)
}

describe('dropWarning (3.9, U-135)', () => {
  it('U-135 warns when the user would come before the feature whose edge it uses', () => {
    expect(dropWarning(dropState(), 'f-fil1', 1)).toEqual({
      user: 'f-fil1',
      feature: 'f-ext2',
      what: 'an edge',
      moved: 'f-fil1',
    })
  })

  it('U-135 warns when the used feature would come after its user', () => {
    expect(dropWarning(dropState(), 'f-ext2', 3)).toEqual({
      user: 'f-fil1',
      feature: 'f-ext2',
      what: 'an edge',
      moved: 'f-ext2',
    })
  })

  it('U-135 is null over the gaps on either side of the dragged feature, which leave it in place', () => {
    expect(dropWarning(dropState(), 'f-fil1', 2)).toBeNull()
    expect(dropWarning(dropState(), 'f-fil1', 3)).toBeNull()
    expect(dropWarning(dropState(), 'f-ext2', 1)).toBeNull()
    expect(dropWarning(dropState(), 'f-ext2', 2)).toBeNull()
  })

  it('is null for a drop that keeps every use in order', () => {
    expect(dropWarning(dropState(), 'f-ext1', 2)).toBeNull()
    expect(dropWarning(dropState(), 'f-ext1', 3)).toBeNull()
    expect(dropWarning(dropState(), 'f-ext2', 0)).toBeNull()
  })

  it('U-135 is null while the uses are not known, or describe an older revision', () => {
    const unknown = stateWithDocument(DROP_DOCUMENT, {}, { revision: 5, uses: null })
    expect(dropWarning(unknown, 'f-fil1', 1)).toBeNull()
    expect(dropWarning(dropState(DROP_USES, 4), 'f-fil1', 1)).toBeNull()
  })

  it('ignores a use that is already out of order before the drop', () => {
    // A use of a later feature is broken before any drop, so no drop breaks it.
    const forward: readonly Use[] = [{ user: 'f-ext1', field: '/body', feature: 'f-fil1' }]
    expect(dropWarning(dropState(forward), 'f-ext1', 3)).toBeNull()
  })

  it('ignores a use of a feature that is not in the document', () => {
    const missing: readonly Use[] = [{ user: 'f-fil1', field: '/edges/0', feature: 'f-gone' }]
    expect(dropWarning(dropState(missing), 'f-fil1', 0)).toBeNull()
  })

  it('names the first use, in list_uses order, that the drop would break', () => {
    const uses: readonly Use[] = [
      { user: 'f-fil1', field: '/edges/0', feature: 'f-ext1' },
      { user: 'f-fil1', field: '/edges/1', feature: 'f-ext2' },
    ]
    expect(dropWarning(dropState(uses), 'f-fil1', 0)).toMatchObject({ feature: 'f-ext1' })
    expect(dropWarning(dropState(uses), 'f-fil1', 1)).toMatchObject({ feature: 'f-ext2' })
  })
})

describe('what a drop warning names (3.9)', () => {
  it('names a face for a plane field that holds a face', () => {
    const state = stateWith(
      [
        extrudeRecord('f-ext1', 'f-sk1', 'new'),
        {
          id: 'f-sk2',
          type: 'sketch',
          plane: { kind: 'face', body: 'b-1', face: faceReference('f-ext1/end-cap') },
          entities: [],
          constraints: [],
        },
      ],
      [{ user: 'f-sk2', field: '/plane/face', feature: 'f-ext1' }],
    )
    expect(dropWarning(state, 'f-sk2', 0)?.what).toBe('a face')
  })

  it('names a plane for a plane field that holds a datum plane', () => {
    const state = stateWith(
      [
        datumOnFace('f-dp1', faceReference('f-ext0/end-cap')),
        {
          id: 'f-sk2',
          type: 'sketch',
          plane: { kind: 'datum', feature: 'f-dp1' },
          entities: [],
          constraints: [],
        },
      ],
      [{ user: 'f-sk2', field: '/plane/feature', feature: 'f-dp1' }],
    )
    expect(dropWarning(state, 'f-dp1', 2)?.what).toBe('a plane')
  })

  it('names an edge or a face for an axis field by the kind of its value', () => {
    const edge = stateWith(
      [
        extrudeRecord('f-ext1', 'f-sk1', 'new'),
        revolveAbout({
          kind: 'edge',
          body: 'b-1',
          edge: edgeReference('edge(f-ext1/lateral/e-1)'),
        }),
      ],
      [{ user: 'f-rev1', field: '/axis/edge', feature: 'f-ext1' }],
    )
    expect(dropWarning(edge, 'f-rev1', 0)?.what).toBe('an edge')
    const face = stateWith(
      [
        extrudeRecord('f-ext1', 'f-sk1', 'new'),
        revolveAbout({ kind: 'face', body: 'b-1', face: faceReference('f-ext1/lateral/e-1') }),
      ],
      [{ user: 'f-rev1', field: '/axis/face', feature: 'f-ext1' }],
    )
    expect(dropWarning(face, 'f-rev1', 0)?.what).toBe('a face')
  })

  it('names a region, a point and a sketch by the selection kind of the field', () => {
    const sketch = xySketchRecord('f-sk1', [], [{ id: 'e-p1', at: [0, 0] }])
    const hole: FeatureRecord = {
      id: 'f-hole1',
      type: 'hole',
      body: 'b-1',
      points: ['e-p1'],
      sketch: 'f-sk1',
      diameter: '3 mm',
      hole_type: { kind: 'simple' },
      extent: { kind: 'through_all' },
    }
    const region = stateWith(
      [sketch, extrudeRecord('f-ext1', 'f-sk1', 'new')],
      [{ user: 'f-ext1', field: '/profile/sketch', feature: 'f-sk1' }],
    )
    expect(dropWarning(region, 'f-ext1', 0)?.what).toBe('a region')
    const point = stateWith(
      [sketch, hole],
      [{ user: 'f-hole1', field: '/points/0', feature: 'f-sk1' }],
    )
    expect(dropWarning(point, 'f-hole1', 0)?.what).toBe('a point')
    const linked = stateWith(
      [sketch, hole],
      [{ user: 'f-hole1', field: '/sketch', feature: 'f-sk1' }],
    )
    expect(dropWarning(linked, 'f-hole1', 0)?.what).toBe('a sketch')
  })

  it('names a shape for a field no hint maps', () => {
    const state = stateWith(
      [extrudeRecord('f-ext1', 'f-sk1', 'new'), filletRecord('f-fil1', [])],
      [{ user: 'f-fil1', field: '/radius', feature: 'f-ext1' }],
    )
    expect(dropWarning(state, 'f-fil1', 0)?.what).toBe('a shape')
  })
})

describe('usersOf (3.9, U-136)', () => {
  it('U-136 lists each feature that uses Extrude 1 once, in timeline order', () => {
    expect(usersOf(usedByState(), 'f-ext1')).toEqual(['f-fil1', 'f-dp1'])
  })

  it('U-136 keeps timeline order whatever order the uses come in', () => {
    expect(usersOf(usedByState(USED_BY_USES.toReversed()), 'f-ext1')).toEqual(['f-fil1', 'f-dp1'])
  })

  it('U-136 is empty for a feature that nothing uses', () => {
    expect(usersOf(usedByState(), 'f-dp1')).toEqual([])
  })

  it('U-136 ignores a use by a feature that is not in the document, and the feature itself', () => {
    const uses: readonly Use[] = [
      ...USED_BY_USES,
      { user: 'f-gone', field: '/edges/0', feature: 'f-ext1' },
      { user: 'f-ext1', field: '/profile/sketch', feature: 'f-ext1' },
    ]
    expect(usersOf(usedByState(uses), 'f-ext1')).toEqual(['f-fil1', 'f-dp1'])
  })

  it('counts a feature link as a use: Extrude 1 uses Sketch 1', () => {
    expect(usersOf(usedByState(), 'f-sk1')).toEqual(['f-ext1'])
  })
})

describe('usedByText (3.9, 3.16.10)', () => {
  it('names the users by their labels', () => {
    expect(usedByText(usedByState(), ['f-fil1', 'f-dp1'])).toBe(
      "Used by 'Fillet 1', 'Datum plane 1'",
    )
  })
})

describe('usedEntities (3.16.10, U-137)', () => {
  it('U-137 maps e-3 to Fillet 1, through Extrude 1, which links Sketch 1', () => {
    expect(usedEntities(sketchState(USED_SKETCH_USES), 'f-sk1')).toEqual({
      'e-3': ['f-fil1'],
    })
  })

  it('U-137 maps a used point entity to its user as well', () => {
    expect(usedEntities(sketchState(USED_POINT_USES), 'f-sk1')).toEqual({
      'e-3': ['f-fil1'],
      'e-p1': ['f-cha1'],
    })
  })

  it('lists the users of one entity once each, in timeline order', () => {
    const uses: readonly Use[] = [
      { user: 'f-ext1', field: '/profile/sketch', feature: 'f-sk1' },
      { user: 'f-cha1', field: '/edges/0', feature: 'f-ext1', entity: 'e-3' },
      { user: 'f-fil1', field: '/edges/0', feature: 'f-ext1', entity: 'e-3' },
      { user: 'f-fil1', field: '/edges/1', feature: 'f-ext1', entity: 'e-3' },
    ]
    expect(usedEntities(sketchState(uses), 'f-sk1')).toEqual({ 'e-3': ['f-fil1', 'f-cha1'] })
  })

  it('leaves out entity uses of a feature that does not link the sketch', () => {
    const uses: readonly Use[] = [
      { user: 'f-fil1', field: '/edges/0', feature: 'f-ext1', entity: 'e-3' },
    ]
    expect(usedEntities(sketchState(uses), 'f-sk1')).toEqual({})
  })

  it('is empty for a sketch whose entities nothing uses', () => {
    const uses: readonly Use[] = [{ user: 'f-ext1', field: '/profile/sketch', feature: 'f-sk1' }]
    expect(usedEntities(sketchState(uses), 'f-sk1')).toEqual({})
  })
})
