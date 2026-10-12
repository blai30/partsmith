import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import type { BoundingBox, Health } from '../../generated/protocol'
import { healthBadge } from '../../print/health'
import {
  CUBE,
  DOCUMENT_HANDLE,
  bodyPrint,
  createTestApp,
  deferred,
  documentState,
  finding,
  meshSummary,
  nextTask,
  printReport,
  printState,
  setRevision,
  type TestApp,
} from '../../print/test-app.testing'
import { DEFAULT_SETTINGS } from '../../settings/settings'
import type { AppState, FindingKey } from '../../state/store'
import {
  focusFinding,
  installPrintAnalysis,
  runPrintAnalysis,
  setCheckHidden,
  setPrinter,
} from './print'

const AS_MODELED = { orientation: { kind: 'as_modeled' } }

// A report for the cube document at revision 3 with two overhang warnings.
const OVERHANG_REPORT = printReport([
  bodyPrint({
    body: CUBE.id,
    meshId: 1,
    findings: {
      overhangs: [
        finding({
          check: 'overhangs',
          code: 'overhang',
          severity: 'warning',
          message: 'Overhang steeper than 45 degrees over 80 mm². Add supports.',
          limit: 0.7853981633974483,
        }),
        finding({
          check: 'overhangs',
          code: 'overhang',
          severity: 'warning',
          message: 'Overhang steeper than 45 degrees over 12 mm². Add supports.',
          limit: 0.7853981633974483,
        }),
      ],
    },
  }),
])

function running(harness: TestApp): boolean {
  return harness.app.store.getState().print.running
}

describe('setPrinter', () => {
  it('commits set_printer with the label "Change printer"', async () => {
    const harness = createTestApp()
    await setPrinter(harness.app, 'generic-resin')
    expect(harness.core.applyEdits).toHaveBeenCalledTimes(1)
    expect(harness.core.applyEdits).toHaveBeenCalledWith(DOCUMENT_HANDLE, 'Change printer', [
      { type: 'set_printer', printer: 'generic-resin' },
    ])
  })

  it('clears the printer for "No printer"', async () => {
    const harness = createTestApp({ document: documentState({ printer: 'generic-fdm' }) })
    await setPrinter(harness.app, null)
    expect(harness.core.applyEdits).toHaveBeenCalledTimes(1)
    const [handle, label, edits] = harness.core.applyEdits.mock.calls[0] ?? []
    expect(handle).toBe(DOCUMENT_HANDLE)
    expect(label).toBe('Change printer')
    // model.md: `{"type": "set_printer"}` clears the printer, and an explicit null means the same.
    expect([[{ type: 'set_printer' }], [{ type: 'set_printer', printer: null }]]).toContainEqual(edits)
  })

  it('runs through the mutation queue after earlier mutations', async () => {
    const harness = createTestApp()
    const earlier = deferred<void>()
    const blocking = harness.app.mutations.run(() => earlier.promise)
    const changing = setPrinter(harness.app, 'generic-resin')
    await nextTask()
    expect(harness.core.applyEdits).not.toHaveBeenCalled()
    earlier.resolve()
    await blocking
    await changing
    expect(harness.core.applyEdits).toHaveBeenCalledTimes(1)
  })
})

describe('setCheckHidden', () => {
  it('hides a check by adding it to settings.hiddenChecks', () => {
    const harness = createTestApp()
    setCheckHidden(harness.app, 'overhangs', true)
    expect(harness.app.store.getState().settings.hiddenChecks).toEqual(['overhangs'])
  })

  it('shows a hidden check again by removing it from settings.hiddenChecks', () => {
    const harness = createTestApp({
      settings: { ...DEFAULT_SETTINGS, hiddenChecks: ['islands', 'overhangs'] },
    })
    setCheckHidden(harness.app, 'overhangs', false)
    expect(harness.app.store.getState().settings.hiddenChecks).toEqual(['islands'])
  })

  it('keeps one entry when a hidden check is hidden again', () => {
    const harness = createTestApp({ settings: { ...DEFAULT_SETTINGS, hiddenChecks: ['islands'] } })
    setCheckHidden(harness.app, 'islands', true)
    expect(harness.app.store.getState().settings.hiddenChecks).toEqual(['islands'])
  })

  it('keeps the other settings', () => {
    const settings = { ...DEFAULT_SETTINGS, theme: 'dark', uiScale: 1.5 } as const
    const harness = createTestApp({ settings })
    setCheckHidden(harness.app, 'thin_walls', true)
    expect(harness.app.store.getState().settings).toEqual({
      ...settings,
      hiddenChecks: ['thin_walls'],
    })
  })
})

describe('runPrintAnalysis', () => {
  it('analyzes the document as modeled', async () => {
    const harness = createTestApp()
    await runPrintAnalysis(harness.app)
    expect(harness.core.analyzePrint).toHaveBeenCalledTimes(1)
    expect(harness.core.analyzePrint).toHaveBeenCalledWith(DOCUMENT_HANDLE, AS_MODELED)
  })

  it('sets print.running while the analysis is in flight', async () => {
    const harness = createTestApp()
    const answer = deferred<Health | null>()
    harness.core.analyzePrint.mockReturnValue(answer.promise)
    const analysis = runPrintAnalysis(harness.app)
    await vi.waitFor(() => {
      expect(harness.core.analyzePrint).toHaveBeenCalledTimes(1)
    })
    expect(running(harness)).toBe(true)
    answer.resolve('warning')
    await analysis
    await vi.waitFor(() => {
      expect(running(harness)).toBe(false)
    })
  })

  it('ignores a superseded (null) result', async () => {
    const harness = createTestApp({
      document: documentState({ revision: 4 }),
      print: printState({ report: OVERHANG_REPORT, reportRevision: 3 }),
    })
    harness.core.analyzePrint.mockResolvedValue(null)
    await runPrintAnalysis(harness.app)
    const print = harness.app.store.getState().print
    expect(print.report).toBe(OVERHANG_REPORT)
    expect(print.reportRevision).toBe(3)
    expect(print.running).toBe(false)
  })

  it('does not wait for the mutation queue', async () => {
    const harness = createTestApp()
    const earlier = deferred<void>()
    const blocking = harness.app.mutations.run(() => earlier.promise)
    const analysis = runPrintAnalysis(harness.app)
    await vi.waitFor(() => {
      expect(harness.core.analyzePrint).toHaveBeenCalledTimes(1)
    })
    earlier.resolve()
    await blocking
    await analysis
  })

  it('sends at most one analysis at a time and only the latest waiting one', async () => {
    const harness = createTestApp()
    const first = deferred<Health | null>()
    const second = deferred<Health | null>()
    harness.core.analyzePrint
      .mockReturnValueOnce(first.promise)
      .mockReturnValueOnce(second.promise)
    const analyses = [runPrintAnalysis(harness.app)]
    await vi.waitFor(() => {
      expect(harness.core.analyzePrint).toHaveBeenCalledTimes(1)
    })
    analyses.push(runPrintAnalysis(harness.app), runPrintAnalysis(harness.app))
    await nextTask()
    expect(harness.core.analyzePrint).toHaveBeenCalledTimes(1)
    first.resolve('ok')
    await vi.waitFor(() => {
      expect(harness.core.analyzePrint).toHaveBeenCalledTimes(2)
    })
    second.resolve('warning')
    await Promise.all(analyses)
    expect(harness.core.analyzePrint).toHaveBeenCalledTimes(2)
    expect(running(harness)).toBe(false)
  })

  it('does nothing without a document', async () => {
    const harness = createTestApp({ document: null })
    await runPrintAnalysis(harness.app)
    expect(harness.core.analyzePrint).not.toHaveBeenCalled()
  })
})

describe('focusFinding', () => {
  const MUSHROOM = { id: 'b-mushroom', name: 'Mushroom' }
  const SLAB = { id: 'b-slab', name: 'Slab' }
  // Diagonal about 49.2 mm: a quarter of it is below the 20 mm minimum.
  const MUSHROOM_BOUNDS: BoundingBox = { min: [-15, -15, 0], max: [15, 15, 25] }
  // Diagonal about 565.8 mm: a quarter of it is above the 20 mm minimum.
  const SLAB_BOUNDS: BoundingBox = { min: [0, 0, 0], max: [400, 400, 10] }

  const REPORT = printReport([
    bodyPrint({
      body: MUSHROOM.id,
      meshId: 5,
      findings: {
        overhangs: [
          finding({
            check: 'overhangs',
            code: 'overhang',
            severity: 'warning',
            message: 'Overhang steeper than 45 degrees over 500 mm². Add supports.',
            location: { kind: 'point', at: [5, 0, 20] },
            limit: 0.7853981633974483,
          }),
        ],
      },
    }),
    bodyPrint({
      body: SLAB.id,
      meshId: 6,
      findings: {
        bed_fit: [
          finding({
            check: 'bed_fit',
            code: 'footprint_too_large',
            severity: 'error',
            message: 'The part is 400 × 400 mm, but the bed is 256 × 256 mm. Split it.',
            location: { kind: 'bounds', min: [100, 120, 0], max: [300, 280, 10] },
          }),
        ],
      },
    }),
  ])

  function focusApp(options: { readonly viewportMounted?: boolean } = {}): TestApp {
    return createTestApp(
      {
        context: 'print',
        document: documentState({
          bodies: [MUSHROOM, SLAB],
          meshes: { [MUSHROOM.id]: meshSummary(5, MUSHROOM_BOUNDS), [SLAB.id]: meshSummary(6, SLAB_BOUNDS) },
        }),
        print: printState({ report: REPORT, reportRevision: 3 }),
      },
      options,
    )
  }

  function cube(center: readonly [number, number, number], side: number) {
    return {
      min: center.map((value) => expect.closeTo(value - side / 2, 9)),
      max: center.map((value) => expect.closeTo(value + side / 2, 9)),
    }
  }

  const OVERHANG: FindingKey = { body: MUSHROOM.id, check: 'overhangs', index: 0 }
  const BED_FIT: FindingKey = { body: SLAB.id, check: 'bed_fit', index: 0 }

  it('stores the focused finding', () => {
    const harness = focusApp()
    focusFinding(harness.app, OVERHANG)
    expect(harness.app.store.getState().print.focusedFinding).toEqual(OVERHANG)
  })

  it('frames a 20 mm cube around a point location of a small body, with margin 0.3', () => {
    const harness = focusApp()
    focusFinding(harness.app, OVERHANG)
    expect(harness.viewport.zoomToBounds).toHaveBeenCalledTimes(1)
    expect(harness.viewport.zoomToBounds).toHaveBeenCalledWith(cube([5, 0, 20], 20), 0.3)
  })

  it('frames a cube a quarter of the body diagonal around the center of a bounds location', () => {
    const harness = focusApp()
    focusFinding(harness.app, BED_FIT)
    const side = 0.25 * Math.hypot(400, 400, 10)
    expect(harness.viewport.zoomToBounds).toHaveBeenCalledWith(cube([200, 200, 5], side), 0.3)
  })

  it('replaces the focused finding when another one is chosen', () => {
    const harness = focusApp()
    focusFinding(harness.app, OVERHANG)
    focusFinding(harness.app, BED_FIT)
    expect(harness.app.store.getState().print.focusedFinding).toEqual(BED_FIT)
    expect(harness.viewport.zoomToBounds).toHaveBeenCalledTimes(2)
  })

  it('stores the finding without a mounted viewport', () => {
    const harness = focusApp({ viewportMounted: false })
    focusFinding(harness.app, OVERHANG)
    expect(harness.app.store.getState().print.focusedFinding).toEqual(OVERHANG)
  })
})

describe('automatic print analysis (installPrintAnalysis)', () => {
  const stops: (() => void)[] = []

  beforeEach(() => {
    vi.useFakeTimers()
  })

  afterEach(() => {
    for (const stop of stops.splice(0)) {
      stop()
    }
    vi.useRealTimers()
  })

  // The cube document at revision 3 with an up-to-date report, in `context`.
  function analyzingApp(overrides: Partial<AppState> = {}): TestApp {
    const harness = createTestApp({
      context: 'print',
      document: documentState({ revision: 3 }),
      print: printState({ report: OVERHANG_REPORT, reportRevision: 3 }),
      ...overrides,
    })
    stops.push(installPrintAnalysis(harness.app))
    return harness
  }

  it('U-81 in Print context a revision change schedules analyzePrint after 300 ms', async () => {
    const harness = analyzingApp()
    setRevision(harness.app, 4)
    await vi.advanceTimersByTimeAsync(299)
    expect(harness.core.analyzePrint).not.toHaveBeenCalled()
    await vi.advanceTimersByTimeAsync(1)
    expect(harness.core.analyzePrint).toHaveBeenCalledTimes(1)
    expect(harness.core.analyzePrint).toHaveBeenCalledWith(DOCUMENT_HANDLE, AS_MODELED)
  })

  it('U-81 each revision change restarts the 300 ms delay', async () => {
    const harness = analyzingApp()
    setRevision(harness.app, 4)
    await vi.advanceTimersByTimeAsync(200)
    setRevision(harness.app, 5)
    await vi.advanceTimersByTimeAsync(299)
    expect(harness.core.analyzePrint).not.toHaveBeenCalled()
    await vi.advanceTimersByTimeAsync(1)
    expect(harness.core.analyzePrint).toHaveBeenCalledTimes(1)
  })

  it('U-81 a null result changes nothing', async () => {
    const harness = analyzingApp()
    harness.core.analyzePrint.mockResolvedValue(null)
    setRevision(harness.app, 4)
    await vi.advanceTimersByTimeAsync(300)
    expect(harness.core.analyzePrint).toHaveBeenCalledTimes(1)
    await vi.advanceTimersByTimeAsync(0)
    const state = harness.app.store.getState()
    expect(state.print.report).toBe(OVERHANG_REPORT)
    expect(state.print.reportRevision).toBe(3)
    expect(state.print.running).toBe(false)
    expect(healthBadge(state)).toEqual({ state: 'warning', errors: 0, warnings: 2, stale: true })
  })

  it('U-81 in Model context no analysis is scheduled', async () => {
    const harness = analyzingApp({ context: 'model' })
    setRevision(harness.app, 4)
    await vi.advanceTimersByTimeAsync(1000)
    expect(harness.core.analyzePrint).not.toHaveBeenCalled()
  })

  it('U-81 in Sketch context no analysis is scheduled', async () => {
    const harness = analyzingApp({ context: 'sketch' })
    setRevision(harness.app, 4)
    await vi.advanceTimersByTimeAsync(1000)
    expect(harness.core.analyzePrint).not.toHaveBeenCalled()
  })

  it('schedules nothing while a transaction is open', async () => {
    const harness = analyzingApp({ document: documentState({ revision: 3, transactionOpen: true }) })
    setRevision(harness.app, 4)
    await vi.advanceTimersByTimeAsync(1000)
    expect(harness.core.analyzePrint).not.toHaveBeenCalled()
  })

  it('runs the analysis on entering Print with a stale report', async () => {
    const harness = analyzingApp({
      context: 'model',
      document: documentState({ revision: 5 }),
      print: printState({ report: OVERHANG_REPORT, reportRevision: 3 }),
    })
    harness.app.store.setState({ context: 'print' })
    await vi.advanceTimersByTimeAsync(0)
    expect(harness.core.analyzePrint).toHaveBeenCalledTimes(1)
    expect(harness.core.analyzePrint).toHaveBeenCalledWith(DOCUMENT_HANDLE, AS_MODELED)
  })

  it('runs the analysis on entering Print without a report', async () => {
    const harness = analyzingApp({ context: 'model', print: printState() })
    harness.app.store.setState({ context: 'print' })
    await vi.advanceTimersByTimeAsync(0)
    expect(harness.core.analyzePrint).toHaveBeenCalledTimes(1)
  })

  it('runs nothing on entering Print with an up-to-date report', async () => {
    const harness = analyzingApp({ context: 'model' })
    harness.app.store.setState({ context: 'print' })
    await vi.advanceTimersByTimeAsync(1000)
    expect(harness.core.analyzePrint).not.toHaveBeenCalled()
  })

  it('stops scheduling once the returned function is called', async () => {
    const harness = createTestApp({
      context: 'print',
      document: documentState({ revision: 3 }),
      print: printState({ report: OVERHANG_REPORT, reportRevision: 3 }),
    })
    const stop = installPrintAnalysis(harness.app)
    setRevision(harness.app, 4)
    await vi.advanceTimersByTimeAsync(100)
    stop()
    await vi.advanceTimersByTimeAsync(1000)
    expect(harness.core.analyzePrint).not.toHaveBeenCalled()
  })
})
