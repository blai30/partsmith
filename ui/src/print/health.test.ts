import { describe, expect, it } from 'vitest'
import type { BodyPrint, Finding, PrintReport } from '../generated/protocol'
import { DEFAULT_SETTINGS } from '../settings/settings'
import type { AppState } from '../state/store'
import { healthRows } from './health'
import {
  CUBE,
  CUBE_BOUNDS,
  PEG,
  PEG_BOUNDS,
  bodyPrint,
  documentState,
  finding,
  meshSummary,
  printReport,
  printState,
  stateWith,
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
      printingState({ report: MIXED_REPORT, hiddenChecks: ['overhangs', 'mesh_validity'] }),
    )
    expect(rows.map((row) => row.message)).toEqual([
      TOO_LARGE.message,
      THIN_WALL.message,
      ISLAND.message,
      HOLLOW.message,
    ])
  })

  it('lists only the listed findings of a check, not its omitted count', () => {
    const report = printReport([cube({ overhangs: [OVERHANG_LARGE] }, { overhangs: 3 })])
    expect(healthRows(printingState({ report }))).toHaveLength(1)
  })

  it('is empty without a report', () => {
    expect(healthRows(printingState({ report: null }))).toEqual([])
  })
})
