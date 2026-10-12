import { describe, expect, it } from 'vitest'
import type { BodyPrint, Finding, PrintReport } from '../generated/protocol'
import { DEFAULT_SETTINGS } from '../settings/settings'
import type { AppState } from '../state/store'
import { healthBadge, healthRows } from './health'
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
} from './test-app.testing'

const LIMIT = 0.7853981633974483

function overhang(message: string): Finding {
  return finding({ check: 'overhangs', code: 'overhang', severity: 'warning', message, limit: LIMIT })
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
const FITS_ROTATED = finding({
  check: 'bed_fit',
  code: 'fits_rotated',
  severity: 'info',
  message: 'The part fits the bed only when turned about Z. Rotate it in the slicer.',
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
  readonly reportRevision?: number
  readonly documentRevision?: number
  readonly running?: boolean
  readonly hiddenChecks?: readonly string[]
}

function printingState(options: ReportState): AppState {
  return stateWith({
    context: 'print',
    document: documentState({ revision: options.documentRevision ?? 3, ...BODIES_WITH_GEOMETRY }),
    print: printState({
      report: options.report,
      reportRevision: options.report === null ? null : (options.reportRevision ?? 3),
      running: options.running ?? false,
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

describe('healthBadge', () => {
  it('U-81 is none before any report', () => {
    expect(healthBadge(printingState({ report: null }))).toEqual({
      state: 'none',
      errors: 0,
      warnings: 0,
      stale: false,
    })
  })

  it('U-81 is running while an analysis is in flight', () => {
    expect(healthBadge(printingState({ report: null, running: true })).state).toBe('running')
  })

  it('U-81 is running while an analysis is in flight after an earlier report', () => {
    const report = printReport([cube({ bed_fit: [TOO_LARGE] })])
    expect(healthBadge(printingState({ report, running: true })).state).toBe('running')
  })

  it('U-81 is ok for a report without errors or warnings', () => {
    expect(healthBadge(printingState({ report: printReport([cube({})]) }))).toEqual({
      state: 'ok',
      errors: 0,
      warnings: 0,
      stale: false,
    })
  })

  it('U-81 info findings never change the badge', () => {
    const report = printReport([cube({ bed_fit: [FITS_ROTATED], trapped_volumes: [HOLLOW] })])
    expect(healthBadge(printingState({ report }))).toEqual({
      state: 'ok',
      errors: 0,
      warnings: 0,
      stale: false,
    })
  })

  it('U-81 is warning for shown warnings without errors, counted over every body and check', () => {
    const report = printReport([
      cube({ overhangs: [OVERHANG_LARGE, OVERHANG_SMALL] }),
      peg({ islands: [ISLAND] }),
    ])
    expect(healthBadge(printingState({ report }))).toEqual({
      state: 'warning',
      errors: 0,
      warnings: 3,
      stale: false,
    })
  })

  it('U-81 is error for shown errors and counts both errors and warnings', () => {
    const report = printReport([
      cube({ bed_fit: [TOO_LARGE], overhangs: [OVERHANG_LARGE, OVERHANG_SMALL] }),
    ])
    expect(healthBadge(printingState({ report }))).toEqual({
      state: 'error',
      errors: 1,
      warnings: 2,
      stale: false,
    })
  })

  it("U-81 adds a check's omitted findings to the count of its listed findings' severity", () => {
    const report = printReport([
      cube({ overhangs: [OVERHANG_LARGE, OVERHANG_SMALL] }, { overhangs: 3 }),
    ])
    expect(healthBadge(printingState({ report }))).toMatchObject({ state: 'warning', warnings: 5 })
  })

  it('U-81 omitted info findings are not counted', () => {
    const report = printReport([cube({ trapped_volumes: [HOLLOW] }, { trapped_volumes: 4 })])
    expect(healthBadge(printingState({ report }))).toMatchObject({
      state: 'ok',
      errors: 0,
      warnings: 0,
    })
  })

  it('U-81 hidden checks are excluded from the counts', () => {
    const report = printReport([
      cube({ overhangs: [OVERHANG_LARGE, OVERHANG_SMALL] }),
      peg({ islands: [ISLAND] }),
    ])
    expect(healthBadge(printingState({ report, hiddenChecks: ['overhangs'] }))).toMatchObject({
      state: 'warning',
      warnings: 1,
    })
  })

  it('U-81 a hidden check with errors leaves the warnings of the others', () => {
    const report = printReport([
      cube({ bed_fit: [TOO_LARGE], overhangs: [OVERHANG_LARGE, OVERHANG_SMALL] }),
    ])
    expect(healthBadge(printingState({ report, hiddenChecks: ['bed_fit'] }))).toEqual({
      state: 'warning',
      errors: 0,
      warnings: 2,
      stale: false,
    })
  })

  it('U-81 is ok when every check with problems is hidden', () => {
    const report = printReport([
      cube({ bed_fit: [TOO_LARGE], overhangs: [OVERHANG_LARGE] }, { overhangs: 2 }),
    ])
    const hiddenChecks = ['bed_fit', 'overhangs']
    expect(healthBadge(printingState({ report, hiddenChecks })).state).toBe('ok')
  })

  it('U-81 a report for an older revision is stale and keeps its state', () => {
    const report = printReport([cube({ overhangs: [OVERHANG_LARGE, OVERHANG_SMALL] })])
    expect(
      healthBadge(printingState({ report, reportRevision: 3, documentRevision: 4 })),
    ).toEqual({ state: 'warning', errors: 0, warnings: 2, stale: true })
  })

  it('U-81 a report for the current revision is not stale', () => {
    const report = printReport([cube({ overhangs: [OVERHANG_LARGE] })])
    expect(
      healthBadge(printingState({ report, reportRevision: 4, documentRevision: 4 })).stale,
    ).toBe(false)
  })
})

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

  it('is empty without a report', () => {
    expect(healthRows(printingState({ report: null }))).toEqual([])
  })
})
