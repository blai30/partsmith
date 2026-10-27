import { describe, expect, it } from 'vitest'
import type { BodyPrint, Finding, PrintReport } from '../generated/protocol'
import { DEFAULT_SETTINGS } from '../settings/settings'
import type { AppState } from '../state/store'
import { CUBE_BOUNDS, meshSummary, PEG_BOUNDS } from '../test/fakes/state'
import { findingAt } from './checks'
import {
  findingPoint,
  focusedFindingPoint,
  hasPrintableBody,
  healthRows,
  locationCenter,
  missingReport,
  withOutdatedSuffix,
} from './health'
import {
  CUBE,
  HOLLOW_DRAIN,
  HOLLOW_DRAIN_KEY,
  PEG,
  SLAB_SPLIT,
  SLAB_SPLIT_KEY,
  bodyPrint,
  documentState,
  finding,
  printReport,
  printState,
  stateWith,
  suggestionState,
  type BodyFindings,
  type OmittedCounts,
} from './fixtures.testing'

const LIMIT = 0.7853981633974483

function overhang(message: string): Finding {
  return finding({
    check: 'overhangs',
    code: 'overhang',
    severity: 'warning',
    message,
    limit: LIMIT,
  })
}

const OVERHANG_LARGE = overhang('Overhang steeper than 45 degrees over 80 mm². Add supports.')
const OVERHANG_SMALL = overhang('Overhang steeper than 45 degrees over 12 mm². Add supports.')
const OVERHANG_PEG = overhang('Overhang steeper than 45 degrees over 30 mm². Add supports.')
const TOO_LARGE = finding({
  check: 'bed_fit',
  code: 'footprint_too_large',
  severity: 'error',
  message: 'The part is 300 × 40 mm, but the bed is 256 × 256 mm. Split it or scale it down.',
})
const OPEN_EDGES = finding({
  check: 'mesh_validity',
  code: 'open_edges',
  severity: 'error',
  message: 'The mesh has 4 open edges, so it is not watertight. Repair or rebuild the body.',
})
const ISLAND = finding({
  check: 'islands',
  code: 'island',
  severity: 'warning',
  message: 'A region starts 12 mm above the bed with nothing below it. Add supports.',
})
const THIN_WALL = finding({
  check: 'thin_walls',
  code: 'thin_wall',
  severity: 'warning',
  message: 'A wall is 0.4 mm thick, thinner than 0.8 mm. Make it thicker.',
})
const HOLLOW = finding({
  check: 'trapped_volumes',
  code: 'enclosed_hollow',
  severity: 'info',
  message: 'The body encloses a hollow of 120 mm³. Slicers fill it with infill.',
})

const BODIES_WITH_GEOMETRY = {
  bodies: [CUBE, PEG],
  meshes: { [CUBE.id]: meshSummary(1, CUBE_BOUNDS), [PEG.id]: meshSummary(2, PEG_BOUNDS) },
}

type ReportState = {
  readonly report: PrintReport | null
  readonly hiddenChecks?: readonly string[]
}

// The cube document at revision 3 with the Cube and Peg bodies and a report of that revision.
function printingState(options: ReportState): AppState {
  return stateWith({
    context: 'print',
    document: documentState({ revision: 3, ...BODIES_WITH_GEOMETRY }),
    print: printState({
      report: options.report,
      reportRevision: options.report === null ? null : 3,
    }),
    settings: { ...DEFAULT_SETTINGS, hiddenChecks: options.hiddenChecks ?? [] },
  })
}

function cube(findings: BodyFindings, omitted: OmittedCounts = {}): BodyPrint {
  return bodyPrint({ body: CUBE.id, meshId: 1, findings, omitted })
}

function peg(findings: BodyFindings): BodyPrint {
  return bodyPrint({ body: PEG.id, meshId: 2, findings })
}

// Both bodies with findings of every severity in several checks.
const MIXED_REPORT = printReport([
  cube({
    bed_fit: [TOO_LARGE],
    islands: [ISLAND],
    overhangs: [OVERHANG_LARGE, OVERHANG_SMALL],
    trapped_volumes: [HOLLOW],
  }),
  peg({ overhangs: [OVERHANG_PEG], mesh_validity: [OPEN_EDGES], thin_walls: [THIN_WALL] }),
])

describe('healthRows', () => {
  it('gives one row per finding with its key, severity, message and body name', () => {
    const report = printReport([cube({ overhangs: [OVERHANG_LARGE] })])
    expect(healthRows(printingState({ report }))).toEqual([
      {
        key: { body: CUBE.id, check: 'overhangs', index: 0 },
        severity: 'warning',
        message: OVERHANG_LARGE.message,
        suggestion: null,
        bodyName: 'Cube',
      },
    ])
  })

  it('sorts errors first, then warnings, then info', () => {
    const rows = healthRows(printingState({ report: MIXED_REPORT }))
    expect(rows.map((row) => row.severity)).toEqual([
      'error',
      'error',
      'warning',
      'warning',
      'warning',
      'warning',
      'warning',
      'info',
    ])
  })

  it('sorts rows of one severity by the check order, then by report order', () => {
    const rows = healthRows(printingState({ report: MIXED_REPORT }))
    expect(rows.map((row) => row.message)).toEqual([
      TOO_LARGE.message,
      OPEN_EDGES.message,
      OVERHANG_LARGE.message,
      OVERHANG_SMALL.message,
      OVERHANG_PEG.message,
      THIN_WALL.message,
      ISLAND.message,
      HOLLOW.message,
    ])
  })

  it('keys each row by its body, its check and its index in that check', () => {
    const rows = healthRows(printingState({ report: MIXED_REPORT }))
    expect(rows.map((row) => row.key)).toEqual([
      { body: CUBE.id, check: 'bed_fit', index: 0 },
      { body: PEG.id, check: 'mesh_validity', index: 0 },
      { body: CUBE.id, check: 'overhangs', index: 0 },
      { body: CUBE.id, check: 'overhangs', index: 1 },
      { body: PEG.id, check: 'overhangs', index: 0 },
      { body: PEG.id, check: 'thin_walls', index: 0 },
      { body: CUBE.id, check: 'islands', index: 0 },
      { body: CUBE.id, check: 'trapped_volumes', index: 0 },
    ])
    expect(rows.map((row) => row.bodyName)).toEqual([
      'Cube',
      'Peg',
      'Cube',
      'Cube',
      'Peg',
      'Peg',
      'Cube',
      'Cube',
    ])
  })

  it('leaves out hidden checks', () => {
    const rows = healthRows(
      printingState({ report: MIXED_REPORT, hiddenChecks: ['overhangs', 'thin_walls'] }),
    )
    expect(rows.map((row) => row.message)).toEqual([
      TOO_LARGE.message,
      OPEN_EDGES.message,
      ISLAND.message,
      HOLLOW.message,
    ])
  })

  it('never leaves out mesh validity, even when stored settings hide it', () => {
    const report = printReport([peg({ mesh_validity: [OPEN_EDGES] })])
    const rows = healthRows(printingState({ report, hiddenChecks: ['mesh_validity'] }))
    expect(rows.map((row) => row.message)).toEqual([OPEN_EDGES.message])
  })

  it('lists only the listed findings of a check, not its omitted count', () => {
    const report = printReport([cube({ overhangs: [OVERHANG_LARGE] }, { overhangs: 3 })])
    expect(healthRows(printingState({ report }))).toHaveLength(1)
  })

  it('is empty without a report', () => {
    expect(healthRows(printingState({ report: null }))).toEqual([])
  })
})

describe('healthRows memoization', () => {
  it('returns the same rows while the report, the hidden checks and the bodies stay', () => {
    const state = printingState({ report: MIXED_REPORT })
    const rows = healthRows(state)
    expect(healthRows({ ...state, selection: [] })).toBe(rows)
  })

  it('names a body the recipe no longer has by its id', () => {
    const report = printReport([cube({ overhangs: [OVERHANG_LARGE] })])
    const state = printingState({ report })
    const withoutBodies = {
      ...state,
      document:
        state.document === null
          ? null
          : { ...state.document, recipe: { ...state.document.recipe, bodies: [] } },
    }
    expect(healthRows(withoutBodies).map((row) => row.bodyName)).toEqual([CUBE.id])
  })
})

describe('findingAt', () => {
  it('finds a finding by body, check and index', () => {
    expect(findingAt(MIXED_REPORT, { body: CUBE.id, check: 'overhangs', index: 1 })).toBe(
      OVERHANG_SMALL,
    )
  })

  it('is undefined for a key the report does not have, and without a report', () => {
    expect(findingAt(MIXED_REPORT, { body: CUBE.id, check: 'overhangs', index: 2 })).toBeUndefined()
    expect(
      findingAt(MIXED_REPORT, { body: 'b-gone', check: 'overhangs', index: 0 }),
    ).toBeUndefined()
    expect(findingAt(null, { body: CUBE.id, check: 'overhangs', index: 0 })).toBeUndefined()
  })
})

describe('locationCenter', () => {
  it('is the point of a point location', () => {
    expect(locationCenter({ kind: 'point', at: [1, 2, 3] })).toEqual([1, 2, 3])
  })

  it('is the center of a bounds location', () => {
    expect(locationCenter({ kind: 'bounds', min: [0, -10, 4], max: [8, 10, 10] })).toEqual([
      4, 0, 7,
    ])
  })
})

describe('focusedFindingPoint', () => {
  const BOXED = finding({
    check: 'bed_fit',
    code: 'footprint_too_large',
    severity: 'error',
    message: 'Too large.',
    location: { kind: 'bounds', min: [0, 0, 0], max: [300, 40, 10] },
  })
  const REPORT = printReport([cube({ bed_fit: [BOXED] })])

  function focusedState(focusedFinding: { body: string; check: string; index: number } | null) {
    return stateWith({
      context: 'print',
      document: documentState({ revision: 3, ...BODIES_WITH_GEOMETRY }),
      print: printState({ report: REPORT, reportRevision: 3, focusedFinding }),
    })
  }

  it("is the center of the focused finding's location", () => {
    expect(
      focusedFindingPoint(focusedState({ body: CUBE.id, check: 'bed_fit', index: 0 })),
    ).toEqual([150, 20, 5])
  })

  it('is null without a focused finding or when the report no longer has it', () => {
    expect(focusedFindingPoint(focusedState(null))).toBeNull()
    expect(
      focusedFindingPoint(focusedState({ body: CUBE.id, check: 'islands', index: 0 })),
    ).toBeNull()
  })

  it('returns the same point while the report and the focused finding stay', () => {
    const state = focusedState({ body: CUBE.id, check: 'bed_fit', index: 0 })
    const point = focusedFindingPoint(state)
    expect(focusedFindingPoint({ ...state, selection: [] })).toBe(point)
  })
})

describe('hasPrintableBody', () => {
  it('is true with a visible part body that has model geometry', () => {
    expect(hasPrintableBody(printingState({ report: null }))).toBe(true)
  })

  it('is false without bodies, without geometry, or with hidden or modifier bodies only', () => {
    const cases = [
      documentState({ bodies: [], meshes: {} }),
      documentState({ bodies: [CUBE], meshes: {} }),
      documentState({ bodies: [{ ...CUBE, visible: false }] }),
      documentState({ bodies: [{ ...CUBE, role: 'negative_volume' }] }),
    ]
    for (const document of cases) {
      expect(hasPrintableBody(stateWith({ document }))).toBe(false)
    }
    expect(hasPrintableBody(stateWith({ document: null }))).toBe(false)
  })
})

describe('missingReport', () => {
  it('is null with a report, also a stale one', () => {
    const report = printReport([cube({})])
    expect(
      missingReport(stateWith({ print: printState({ report, reportRevision: 1 }) })),
    ).toBeNull()
  })

  it('is checking while the first analysis runs, and unchecked before', () => {
    expect(missingReport(stateWith({ print: printState({ running: true }) }))).toBe('checking')
    expect(missingReport(stateWith({ print: printState() }))).toBe('unchecked')
  })
})

describe('withOutdatedSuffix', () => {
  const warnings = { state: 'warning', errors: 0, warnings: 2, stale: false } as const

  it('marks the text of a stale badge', () => {
    expect(withOutdatedSuffix({ ...warnings, stale: true }, '2 warnings')).toBe(
      '2 warnings (out of date)',
    )
  })

  it('leaves the text of a current or running badge as it is', () => {
    expect(withOutdatedSuffix(warnings, '2 warnings')).toBe('2 warnings')
    expect(withOutdatedSuffix({ ...warnings, state: 'running', stale: true }, 'Checking')).toBe(
      'Checking',
    )
  })
})

describe('healthRows suggestions (3.17)', () => {
  it("gives a row the words of its finding's suggestion, and null without one", () => {
    const rows = healthRows(stateWith(suggestionState()))
    expect(rows.map((row) => [row.key, row.suggestion])).toEqual([
      [SLAB_SPLIT_KEY, 'Split at x = 200 mm and y = 200 mm'],
      [HOLLOW_DRAIN_KEY, 'Add a drain hole near (-8, -8, 2 mm)'],
    ])
    const report = printReport([cube({ overhangs: [OVERHANG_LARGE] })])
    expect(healthRows(printingState({ report })).map((row) => row.suggestion)).toEqual([null])
  })

  it('gives a split without planes no words', () => {
    const empty = finding({
      check: 'bed_fit',
      code: 'footprint_too_large',
      severity: 'error',
      message: 'Too large.',
      suggestion: { kind: 'split', planes: [] },
    })
    const report = printReport([cube({ bed_fit: [empty] })])
    expect(healthRows(printingState({ report })).map((row) => row.suggestion)).toEqual([null])
  })

  it('words suggestions in millimeters in a document in inches too', () => {
    const state = stateWith(suggestionState())
    const inches = {
      ...state,
      document:
        state.document === null
          ? null
          : { ...state.document, recipe: { ...state.document.recipe, units: 'in' as const } },
    }
    expect(healthRows(inches).map((row) => row.suggestion)).toEqual([
      'Split at x = 200 mm and y = 200 mm',
      'Add a drain hole near (-8, -8, 2 mm)',
    ])
  })
})

describe('findingPoint (3.17)', () => {
  it("is a drain hole suggestion's point", () => {
    expect(findingPoint(HOLLOW_DRAIN)).toEqual([-8, -8, 2])
  })

  it('is the center of the bounds of a finding with a split suggestion', () => {
    expect(findingPoint(SLAB_SPLIT)).toEqual([200, 200, 5])
  })

  it('is the point of a point location without a suggestion', () => {
    const pointed = finding({
      check: 'islands',
      code: 'island',
      severity: 'warning',
      message: 'An island.',
      location: { kind: 'point', at: [1, 2, 3] },
    })
    expect(findingPoint(pointed)).toEqual([1, 2, 3])
  })
})

describe('focusedFindingPoint with suggestions (3.17)', () => {
  it("is the focused drain hole's point, not the center of the hollow", () => {
    expect(focusedFindingPoint(stateWith(suggestionState(HOLLOW_DRAIN_KEY)))).toEqual([-8, -8, 2])
  })

  it("is the center of a focused split finding's box", () => {
    expect(focusedFindingPoint(stateWith(suggestionState(SLAB_SPLIT_KEY)))).toEqual([200, 200, 5])
  })
})
