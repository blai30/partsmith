import { describe, expect, it } from 'vitest'
import type { Diagnostic, FeatureStatus, FixAction, Repair } from '../generated/protocol'
import type { ActiveCommand, AppState } from '../state/store'
import { CUBE_DOCUMENT } from '../test/fakes/documents'
import {
  R16_CANDIDATE_FRONT,
  R16_CANDIDATE_TOP,
  R16_DOCUMENT,
  R16_MESSAGE,
  R16_REPAIR,
  R16_STATUS,
  R16_USE_FRONT,
  R16_USE_TOP,
  r16Fix,
  SPLIT_REPAIR,
  SPLIT_SOLUTION,
  SPLIT_STATUS,
} from '../test/fakes/repairs'
import { stateWithDocument } from '../test/fakes/state'
import {
  candidateAction,
  ghostSegments,
  regionCandidatePlaces,
  repairView,
  selectRegionCandidates,
  selectRepairMode,
} from './repair'

// Repair mode's data (ui.md 3.7): the current repair and its numbered candidates, the action that
// stores each candidate, found by its label (open issue 18), and the outline of a ghost.

function action(label: string, index = 0): FixAction {
  return { label, run: { kind: 'apply', fix: r16Fix(index) } }
}

function withActions(actions: readonly FixAction[]): Diagnostic {
  return { ...R16_MESSAGE, actions: [...actions] }
}

describe('candidateAction (3.7)', () => {
  it('finds the apply action labeled "Use {description}"', () => {
    expect(candidateAction([R16_MESSAGE], R16_CANDIDATE_TOP.description)).toEqual(R16_USE_TOP)
  })

  it('finds "Use only {description}" of a widened reference', () => {
    const useOnly = action("Use only the top edge of the cut made by 'Extrude 1'")
    expect(
      candidateAction([withActions([useOnly])], "the top edge of the cut made by 'Extrude 1'"),
    ).toEqual(useOnly)
  })

  it('finds "Attach to {description}" and "Attach \'{sketch}\' to {description}"', () => {
    const attach = action("Attach to the top of 'Extrude 3'")
    const attachSketch = action("Attach 'Sketch 2' to the front of 'Extrude 3'", 1)
    const messages = [withActions([attach, attachSketch])]
    expect(candidateAction(messages, "the top of 'Extrude 3'")).toEqual(attach)
    expect(candidateAction(messages, "the front of 'Extrude 3'")).toEqual(attachSketch)
  })

  it('takes the first matching action', () => {
    const first = action("Use the top edge of the cut made by 'Extrude 1'", 3)
    const messages = [withActions([first]), withActions([R16_USE_TOP])]
    expect(candidateAction(messages, R16_CANDIDATE_TOP.description)).toEqual(first)
  })

  it('takes only apply actions', () => {
    const pick: FixAction = {
      label: "Use the top edge of the cut made by 'Extrude 1'",
      run: { kind: 'pick', feature: 'f-fil1', path: '/edges/0' },
    }
    expect(candidateAction([withActions([pick])], R16_CANDIDATE_TOP.description)).toBeNull()
  })

  it('matches the whole label only', () => {
    const longer = action("Use the top edge of the cut made by 'Extrude 1' and its neighbor")
    expect(candidateAction([withActions([longer])], R16_CANDIDATE_TOP.description)).toBeNull()
  })

  it('is null when no action stores the candidate', () => {
    expect(candidateAction([R16_MESSAGE], "the back edge of 'Box 1'")).toBeNull()
    expect(candidateAction([], R16_CANDIDATE_TOP.description)).toBeNull()
  })
})

describe('repairView (3.7)', () => {
  it('numbers the candidates of the current repair in the order received, with their actions', () => {
    expect(repairView(R16_STATUS, 'edges')).toEqual({
      repair: R16_REPAIR,
      candidates: [
        {
          number: 1,
          description: R16_CANDIDATE_FRONT.description,
          relation: R16_CANDIDATE_FRONT.relation,
          element: R16_CANDIDATE_FRONT.element,
          action: R16_USE_FRONT,
        },
        {
          number: 2,
          description: R16_CANDIDATE_TOP.description,
          relation: R16_CANDIDATE_TOP.relation,
          element: R16_CANDIDATE_TOP.element,
          action: R16_USE_TOP,
        },
      ],
    })
  })

  it('keeps the number of a candidate no action stores, with a null action', () => {
    const renamed: FeatureStatus = {
      ...R16_STATUS,
      repairs: [
        {
          ...R16_REPAIR,
          candidates: [
            R16_CANDIDATE_FRONT,
            { ...R16_CANDIDATE_TOP, description: "the top edge of 'Box 1'" },
          ],
        },
      ],
    }
    expect(repairView(renamed, 'edges')?.candidates.map((candidate) => candidate.number)).toEqual([
      1, 2,
    ])
    expect(repairView(renamed, 'edges')?.candidates[1]?.action).toBeNull()
  })

  const PROFILE_REPAIR: Repair = {
    kind: 'region',
    path: '/profile/regions/0',
    sketch: 'f-sk1',
    region: { key: 'e-1,e-2,e-3,e-4', fp: 'c=40,25|a=4000' },
    resolution: 'missing',
    description: "the region of 'Sketch 1' around (40, 25)",
    candidates: [
      {
        regions: [{ key: 'e-1,e-2,e-6,e-4', fp: 'c=40,25|a=3600' }],
        description: "the region of 'Sketch 1' around (40, 22)",
      },
    ],
  }

  const TWO_REPAIRS: FeatureStatus = {
    ...R16_STATUS,
    messages: [R16_MESSAGE, withActions([action("Use the region of 'Sketch 1' around (40, 22)")])],
    repairs: [PROFILE_REPAIR, R16_REPAIR],
  }

  it('takes the first repair whose path maps to the active field', () => {
    expect(repairView(TWO_REPAIRS, 'edges')?.repair).toEqual(R16_REPAIR)
  })

  it('takes the first repair without an active field, or when the active field holds none', () => {
    expect(repairView(TWO_REPAIRS, null)?.repair).toEqual(PROFILE_REPAIR)
    expect(repairView(TWO_REPAIRS, 'radius')?.repair).toEqual(PROFILE_REPAIR)
  })

  it('gives a region candidate no relation and no element', () => {
    expect(repairView(TWO_REPAIRS, 'profile')?.candidates).toEqual([
      {
        number: 1,
        description: "the region of 'Sketch 1' around (40, 22)",
        relation: '',
        element: null,
        action: action("Use the region of 'Sketch 1' around (40, 22)"),
      },
    ])
  })

  it('is null for a status without repairs', () => {
    expect(repairView({ ...R16_STATUS, repairs: [] }, 'edges')).toBeNull()
  })
})

/** The segments of a flat list of points, two points per segment, each as its sorted endpoints. */
function segmentSet(positions: Float32Array): string[] {
  const segments: string[] = []
  for (let i = 0; i + 6 <= positions.length; i += 6) {
    const start = Array.from(positions.subarray(i, i + 3)).join(',')
    const end = Array.from(positions.subarray(i + 3, i + 6)).join(',')
    segments.push([start, end].toSorted().join(' '))
  }
  return segments.toSorted()
}

describe('ghostSegments (3.7)', () => {
  it('gives the segments of lines as given', () => {
    const lines = Float32Array.of(-10, -10, 20, 10, -10, 20, 10, -10, 20, 10, 10, 20)
    expect(Array.from(ghostSegments('lines', lines))).toEqual(Array.from(lines))
  })

  it('gives the three sides of a single triangle', () => {
    const triangle = Float32Array.of(0, 0, 0, 1, 0, 0, 0, 1, 0)
    expect(segmentSet(ghostSegments('triangles', triangle))).toEqual(
      ['0,0,0 1,0,0', '0,0,0 0,1,0', '0,1,0 1,0,0'].toSorted(),
    )
  })

  it('leaves out the sides two triangles share, so a square face is its outline', () => {
    const square = Float32Array.of(0, 0, 0, 1, 0, 0, 1, 1, 0, 0, 0, 0, 1, 1, 0, 0, 1, 0)
    expect(segmentSet(ghostSegments('triangles', square))).toEqual(
      ['0,0,0 1,0,0', '1,0,0 1,1,0', '0,1,0 1,1,0', '0,0,0 0,1,0'].toSorted(),
    )
  })

  it('gives no segments for points, which are drawn as rings', () => {
    expect(ghostSegments('points', Float32Array.of(0, -10, 20)).length).toBe(0)
  })
})

/** The R16 document with 'Fillet 1' in `status` and these state overrides. */
function stateWith(overrides: Partial<AppState>, status: FeatureStatus = R16_STATUS): AppState {
  return stateWithDocument(R16_DOCUMENT, overrides, { featureStatus: { [status.id]: status } })
}

describe('selectRepairMode (3.7)', () => {
  const EDIT_FILLET: ActiveCommand = {
    featureType: 'fillet',
    featureId: 'f-fil1',
    editing: true,
    title: 'Fillet 1',
    params: {},
    activeField: 'edges',
    fieldErrors: {},
    applied: true,
    newBody: null,
    helper: null,
    changedPaths: [],
    history: [],
    future: [],
    preview: 'shown',
  }

  it('shows the repairs of the feature being edited, with the repair of the active field', () => {
    const mode = selectRepairMode(stateWith({ command: EDIT_FILLET }))
    expect(mode).toMatchObject({ feature: 'f-fil1', repairs: R16_STATUS.repairs })
    expect(mode?.view.repair).toEqual(R16_REPAIR)
  })

  it('shows nothing during a command that creates a feature, whatever AppState.repair holds', () => {
    const state = stateWith({
      command: { ...EDIT_FILLET, editing: false },
      repair: { feature: 'f-fil1' },
    })
    expect(selectRepairMode(state)).toBeNull()
  })

  it('shows the feature AppState.repair names while no command is active (a focused Problems row)', () => {
    expect(selectRepairMode(stateWith({ repair: { feature: 'f-fil1' } }))).toMatchObject({
      feature: 'f-fil1',
    })
    expect(selectRepairMode(stateWith({}))).toBeNull()
  })

  it('ends when the feature has no repair left', () => {
    expect(
      selectRepairMode(stateWith({ command: EDIT_FILLET }, { ...R16_STATUS, repairs: [] })),
    ).toBeNull()
  })
})

/** The cube with its sketch split in two and the extrude's region lost, with `repair` focused. */
function splitState(repair: AppState['repair']): AppState {
  return stateWithDocument(
    CUBE_DOCUMENT,
    { repair },
    {
      sketches: { 'f-sk1': { solution: SPLIT_SOLUTION, display: null } },
      featureStatus: { 'f-ext1': SPLIT_STATUS },
    },
  )
}

describe('region candidates (3.7)', () => {
  it("finds each candidate's regions by key and puts its number at its first region's centroid", () => {
    expect(regionCandidatePlaces(SPLIT_REPAIR, SPLIT_SOLUTION)).toEqual({
      sketch: 'f-sk1',
      regions: [[1], [0, 1]],
      anchors: [
        [5, 0, 0],
        [-5, 0, 0],
      ],
    })
  })

  it('leaves out a region the sketch no longer has, and a candidate with none has no number', () => {
    const gone = {
      ...SPLIT_REPAIR,
      candidates: [{ regions: [{ key: 'e-9', fp: 'c=0,0|a=1' }], description: 'gone' }],
    }
    expect(regionCandidatePlaces(gone, SPLIT_SOLUTION)).toMatchObject({
      regions: [[]],
      anchors: [null],
    })
  })

  it('places the candidates while repair mode shows a region repair, and nothing otherwise', () => {
    expect(selectRegionCandidates(splitState({ feature: 'f-ext1' }))?.regions).toEqual([
      [1],
      [0, 1],
    ])
    expect(selectRegionCandidates(splitState(null))).toBeNull()
  })
})
