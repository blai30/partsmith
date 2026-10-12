import { describe, expect, it } from 'vitest'
import type { AppState } from '../state/store'
import { DEFAULT_SETTINGS } from '../settings/settings'
import { encodedFromHex, type ThemeName } from '../theme/roles'
import {
  overhangAnalysis,
  overhangFields,
  overhangLegendLimit,
  overhangLimit,
  overhangMapShown,
} from './overhang'
import {
  CUBE,
  CUBE_BOUNDS,
  PEG,
  PEG_BOUNDS,
  PRINTERS,
  bodyPrint,
  documentState,
  finding,
  meshSummary,
  printReport,
  printState,
  stateWith,
} from './fixtures.testing'

const GENERIC_FDM_LIMIT = 0.7853981633974483
const GENERIC_RESIN_LIMIT = 0.5235987755982988

function overhang(limit: number) {
  return finding({
    check: 'overhangs',
    code: 'overhang',
    severity: 'warning',
    message: 'Overhang steeper than the limit over 80 mm². Add supports.',
    limit,
  })
}

// The cube was analyzed on mesh 1 and is still on it; the peg was analyzed on mesh 2 and has
// mesh 3 now.
const REPORT = printReport([
  bodyPrint({ body: CUBE.id, meshId: 1, findings: { overhangs: [overhang(0.7)] } }),
  bodyPrint({ body: PEG.id, meshId: 2 }),
])

const CUBE_OVERHANG = new Float32Array([0, 0, 1.2, 1.2, 0.9, 0])
const PEG_OVERHANG = new Float32Array([0, 1.5, 1.5])
const OVERHANG = { [CUBE.id]: CUBE_OVERHANG, [PEG.id]: PEG_OVERHANG }

type MapOptions = {
  readonly context?: AppState['context']
  readonly overhangMap?: boolean
  readonly hiddenChecks?: readonly string[]
}

function mapState(options: MapOptions = {}): AppState {
  return stateWith({
    context: options.context ?? 'print',
    document: documentState({
      bodies: [CUBE, PEG],
      meshes: { [CUBE.id]: meshSummary(1, CUBE_BOUNDS), [PEG.id]: meshSummary(3, PEG_BOUNDS) },
    }),
    print: printState({
      report: REPORT,
      reportRevision: 3,
      overhangMap: options.overhangMap ?? true,
    }),
    settings: { ...DEFAULT_SETTINGS, hiddenChecks: options.hiddenChecks ?? [] },
  })
}

describe('overhangLimit', () => {
  it('is the limit of the first finding of an overhangs check in the report', () => {
    const report = printReport([
      bodyPrint({ body: CUBE.id, meshId: 1 }),
      bodyPrint({
        body: PEG.id,
        meshId: 2,
        findings: { overhangs: [overhang(0.6), overhang(0.65)] },
      }),
    ])
    expect(overhangLimit(report, PRINTERS)).toBe(0.6)
  })

  it("falls back to the max_overhang_angle of the report's profile", () => {
    expect(
      overhangLimit(
        printReport([bodyPrint({ body: CUBE.id, meshId: 1 })], 'generic-resin'),
        PRINTERS,
      ),
    ).toBe(GENERIC_RESIN_LIMIT)
    expect(overhangLimit(printReport([], 'generic-fdm'), PRINTERS)).toBe(GENERIC_FDM_LIMIT)
  })

  it('is null without an overhang finding and with a profile that is not installed', () => {
    expect(overhangLimit(printReport([], 'retired-printer'), PRINTERS)).toBeNull()
  })
})

describe('overhangMapShown', () => {
  it('is true in Print context with the Overhang map toggle on', () => {
    expect(overhangMapShown(mapState())).toBe(true)
  })

  it.each(['model', 'sketch'] as const)('is false in the %s context', (context) => {
    expect(overhangMapShown(mapState({ context }))).toBe(false)
  })

  it('is false while the Overhang map toggle is off', () => {
    expect(overhangMapShown(mapState({ overhangMap: false }))).toBe(false)
  })

  it('stays on while the overhangs check is hidden: the toggle alone shows or hides the map (3.17)', () => {
    expect(overhangMapShown(mapState({ hiddenChecks: ['overhangs'] }))).toBe(true)
    expect(overhangMapShown(mapState({ hiddenChecks: ['islands', 'thin_walls'] }))).toBe(true)
  })
})

describe('overhangFields', () => {
  it('gives each body whose model mesh is the analyzed one its overhang values, uncopied', () => {
    const fields = overhangFields(mapState(), OVERHANG)
    expect(Object.keys(fields)).toEqual([CUBE.id])
    expect(fields[CUBE.id]).toBe(CUBE_OVERHANG)
  })

  it('leaves out a body whose mesh changed since the report', () => {
    expect(overhangFields(mapState(), OVERHANG)).not.toHaveProperty(PEG.id)
  })

  it('leaves out a body without an overhang buffer', () => {
    expect(overhangFields(mapState(), { [PEG.id]: PEG_OVERHANG })).toEqual({})
  })

  it('is empty while the map is not shown', () => {
    expect(overhangFields(mapState({ context: 'model' }), OVERHANG)).toEqual({})
    expect(overhangFields(mapState({ overhangMap: false }), OVERHANG)).toEqual({})
  })
})

describe('overhangAnalysis', () => {
  it.each<ThemeName>(['light', 'dark'])(
    'ramps from analysisLow to analysisHigh between the limit and 90 degrees (%s theme)',
    (theme) => {
      expect(overhangAnalysis(mapState(), theme)).toEqual({
        low: encodedFromHex('#b8e6fe'),
        high: encodedFromHex('#7008e7'),
        threshold: 0.7,
        maximum: Math.PI / 2,
      })
    },
  )

  it("uses the profile's limit when the report has no overhang finding", () => {
    const state = stateWith({
      context: 'print',
      print: printState({
        report: printReport([bodyPrint({ body: CUBE.id, meshId: 1 })], 'generic-resin'),
        reportRevision: 3,
      }),
    })
    expect(overhangAnalysis(state, 'light')?.threshold).toBe(GENERIC_RESIN_LIMIT)
  })

  it('is null while the map is not shown', () => {
    expect(overhangAnalysis(mapState({ context: 'model' }), 'light')).toBeNull()
    expect(overhangAnalysis(mapState({ overhangMap: false }), 'dark')).toBeNull()
  })
})

describe('overhangLegendLimit', () => {
  it('is the overhang limit while the map is shown', () => {
    expect(overhangLegendLimit(mapState())).toBe(0.7)
  })

  it('is null while the map is not shown', () => {
    expect(overhangLegendLimit(mapState({ context: 'model' }))).toBeNull()
    expect(overhangLegendLimit(mapState({ overhangMap: false }))).toBeNull()
  })

  it('is null without a report', () => {
    expect(overhangLegendLimit(stateWith({ context: 'print', print: printState() }))).toBeNull()
  })
})

describe('overhangFields without a body mesh id', () => {
  it('leaves out a body analyzed without a display mesh', () => {
    const report = printReport([{ ...bodyPrint({ body: CUBE.id, meshId: 1 }), mesh_id: null }])
    const state = stateWith({
      context: 'print',
      print: printState({ report, reportRevision: 3 }),
    })
    expect(overhangFields(state, OVERHANG)).toEqual({})
  })
})
