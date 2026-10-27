import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import type { BoundingBox, Finding, Health, Orientation } from '../../generated/protocol'
import {
  CUBE,
  DOCUMENT_HANDLE,
  HOLLOW_DRAIN_KEY,
  SLAB_SPLIT,
  SLAB_SPLIT_KEY,
  bodyPrint,
  createPrintApp,
  documentState,
  finding,
  printReport,
  printState,
  setRevision,
  suggestionReport,
  suggestionState,
  type PrintApp,
} from '../../print/fixtures.testing'
import { focusedFindingPoint } from '../../print/health'
import { focusedSplitOutlines } from '../../print/suggestion'
import { DEFAULT_SETTINGS } from '../../settings/settings'
import { selectHealthBadge } from '../../state/selectors'
import {
  INITIAL_STATE,
  type ActiveCommand,
  type AppState,
  type FindingKey,
} from '../../state/store'
import { coreErrorFor } from '../../test/fakes/core-api'
import { deferred } from '../../test/fakes/deferred'
import { CUBE_TOP_FACE } from '../../test/fakes/documents'
import { settle } from '../../test/fakes/settle'
import { meshSummary } from '../../test/fakes/state'
import {
  focusFinding,
  installPrintAnalysis,
  openPrintHealth,
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

// A matcher for the axis-aligned cube of side `side` centered at `center`.
function cube(center: readonly [number, number, number], side: number) {
  return {
    min: center.map((value) => expect.closeTo(value - side / 2, 9)),
    max: center.map((value) => expect.closeTo(value + side / 2, 9)),
  }
}

// An Extrude command with its transaction open.
const EXTRUDE: ActiveCommand = {
  featureType: 'extrude',
  featureId: 'f-extrude',
  editing: false,
  title: 'Extrude',
  params: {},
  activeField: null,
  fieldErrors: {},
  applied: true,
  newBody: null,
  helper: null,
  changedPaths: [],
  history: [],
  future: [],
  preview: 'shown',
}

function analyses(harness: PrintApp): readonly (readonly unknown[])[] {
  return harness.core.callsTo('analyzePrint')
}

function running(harness: PrintApp): boolean {
  return harness.app.store.getState().print.running
}

describe('setPrinter', () => {
  it('commits set_printer with the label "Change printer"', async () => {
    const harness = createPrintApp()
    await setPrinter(harness.app, 'generic-resin')
    expect(harness.core.callsTo('applyEdits')).toEqual([
      [DOCUMENT_HANDLE, 'Change printer', [{ type: 'set_printer', printer: 'generic-resin' }]],
    ])
  })

  it('clears the printer for "No printer"', async () => {
    const harness = createPrintApp({ document: documentState({ printer: 'generic-fdm' }) })
    await setPrinter(harness.app, null)
    const calls = harness.core.callsTo('applyEdits')
    expect(calls).toHaveLength(1)
    expect(calls[0]?.[0]).toBe(DOCUMENT_HANDLE)
    expect(calls[0]?.[1]).toBe('Change printer')
    // model.md: `{"type": "set_printer"}` clears the printer, and an explicit null means the same.
    expect([[{ type: 'set_printer' }], [{ type: 'set_printer', printer: null }]]).toContainEqual(
      calls[0]?.[2],
    )
  })

  it('runs through the mutation queue after earlier mutations', async () => {
    const harness = createPrintApp()
    const earlier = deferred<void>()
    const blocking = harness.app.mutations.run(() => earlier.promise)
    const changing = setPrinter(harness.app, 'generic-resin')
    await settle()
    expect(harness.core.callsTo('applyEdits')).toEqual([])
    earlier.resolve()
    await blocking
    await changing
    expect(harness.core.callsTo('applyEdits')).toHaveLength(1)
  })

  it("changes nothing when the choice is the document's printer already", async () => {
    const harness = createPrintApp({ document: documentState({ printer: 'generic-fdm' }) })
    await setPrinter(harness.app, 'generic-fdm')
    expect(harness.core.callsTo('applyEdits')).toEqual([])
  })

  it('changes nothing when the default is chosen for a document without a printer', async () => {
    const harness = createPrintApp({ document: documentState({ printer: null }) })
    await setPrinter(harness.app, null)
    expect(harness.core.callsTo('applyEdits')).toEqual([])
  })

  it('sets the default profile by id on a document without a printer', async () => {
    const harness = createPrintApp({ document: documentState({ printer: null }) })
    await setPrinter(harness.app, 'generic-fdm')
    expect(harness.core.callsTo('applyEdits')).toEqual([
      [DOCUMENT_HANDLE, 'Change printer', [{ type: 'set_printer', printer: 'generic-fdm' }]],
    ])
  })

  it('is refused while a feature command is active, with finish_command_first as an info notification (3.7)', async () => {
    const harness = createPrintApp({
      command: EXTRUDE,
      document: documentState({ transactionOpen: true }),
    })
    await setPrinter(harness.app, 'generic-resin')
    expect(harness.core.callsTo('applyEdits')).toEqual([])
    expect(harness.app.store.getState().notifications).toMatchObject([
      {
        severity: 'info',
        message: `Finish or cancel ${EXTRUDE.title} first.`,
        code: 'finish_command_first',
      },
    ])
  })

  it('changes nothing while a transaction is open', async () => {
    const harness = createPrintApp({ document: documentState({ transactionOpen: true }) })
    await setPrinter(harness.app, 'generic-resin')
    expect(harness.core.callsTo('applyEdits')).toEqual([])
  })

  it('does nothing without a document', async () => {
    const harness = createPrintApp({ document: null })
    await setPrinter(harness.app, 'generic-resin')
    expect(harness.core.callsTo('applyEdits')).toEqual([])
  })
})

describe('setCheckHidden', () => {
  it('hides a check by adding it to settings.hiddenChecks', () => {
    const harness = createPrintApp()
    setCheckHidden(harness.app, 'overhangs', true)
    expect(harness.app.store.getState().settings.hiddenChecks).toEqual(['overhangs'])
  })

  it('shows a hidden check again by removing it from settings.hiddenChecks', () => {
    const harness = createPrintApp({
      settings: { ...DEFAULT_SETTINGS, hiddenChecks: ['islands', 'overhangs'] },
    })
    setCheckHidden(harness.app, 'overhangs', false)
    expect(harness.app.store.getState().settings.hiddenChecks).toEqual(['islands'])
  })

  it('keeps one entry when a hidden check is hidden again', () => {
    const harness = createPrintApp({
      settings: { ...DEFAULT_SETTINGS, hiddenChecks: ['islands'] },
    })
    setCheckHidden(harness.app, 'islands', true)
    expect(harness.app.store.getState().settings.hiddenChecks).toEqual(['islands'])
  })

  it('keeps the other settings', () => {
    const settings = { ...DEFAULT_SETTINGS, theme: 'dark', uiScale: 1.5 } as const
    const harness = createPrintApp({ settings })
    setCheckHidden(harness.app, 'thin_walls', true)
    expect(harness.app.store.getState().settings).toEqual({
      ...settings,
      hiddenChecks: ['thin_walls'],
    })
  })
})

describe('runPrintAnalysis', () => {
  it('analyzes the document as modeled', async () => {
    const harness = createPrintApp()
    await runPrintAnalysis(harness.app)
    expect(analyses(harness)).toEqual([[DOCUMENT_HANDLE, AS_MODELED]])
  })

  it('sets print.running while the analysis is in flight', async () => {
    const harness = createPrintApp()
    const answer = deferred<Health | null>()
    harness.core.stub('analyzePrint', () => answer.promise)
    const analysis = runPrintAnalysis(harness.app)
    await vi.waitFor(() => {
      expect(analyses(harness)).toHaveLength(1)
    })
    expect(running(harness)).toBe(true)
    answer.resolve('warning')
    await analysis
    await vi.waitFor(() => {
      expect(running(harness)).toBe(false)
    })
  })

  it('ignores a superseded (null) result', async () => {
    const harness = createPrintApp({
      document: documentState({ revision: 4 }),
      print: printState({ report: OVERHANG_REPORT, reportRevision: 3 }),
    })
    harness.core.stub('analyzePrint', () => Promise.resolve(null))
    await runPrintAnalysis(harness.app)
    await settle()
    const print = harness.app.store.getState().print
    expect(print.report).toBe(OVERHANG_REPORT)
    expect(print.reportRevision).toBe(3)
    expect(print.running).toBe(false)
  })

  it('does not wait for the mutation queue', async () => {
    const harness = createPrintApp()
    const earlier = deferred<void>()
    const blocking = harness.app.mutations.run(() => earlier.promise)
    const analysis = runPrintAnalysis(harness.app)
    await vi.waitFor(() => {
      expect(analyses(harness)).toHaveLength(1)
    })
    earlier.resolve()
    await blocking
    await analysis
  })

  it('sends at most one analysis at a time and only the latest waiting one', async () => {
    const harness = createPrintApp()
    const first = deferred<Health | null>()
    const second = deferred<Health | null>()
    const answers = [first, second]
    harness.core.stub('analyzePrint', () => answers.shift()?.promise ?? Promise.resolve('ok'))
    const pending = [runPrintAnalysis(harness.app)]
    await vi.waitFor(() => {
      expect(analyses(harness)).toHaveLength(1)
    })
    // Two edits land while the first analysis runs; only the newest revision is analyzed next.
    setRevision(harness.app, 4)
    pending.push(runPrintAnalysis(harness.app))
    setRevision(harness.app, 5)
    pending.push(runPrintAnalysis(harness.app))
    await settle()
    expect(analyses(harness)).toHaveLength(1)
    first.resolve('ok')
    await vi.waitFor(() => {
      expect(analyses(harness)).toHaveLength(2)
    })
    second.resolve('warning')
    await Promise.all(pending)
    expect(analyses(harness)).toHaveLength(2)
    expect(running(harness)).toBe(false)
  })

  it('does nothing without a document', async () => {
    const harness = createPrintApp({ document: null })
    await runPrintAnalysis(harness.app)
    expect(analyses(harness)).toEqual([])
  })

  it('sends nothing more while an analysis of the same revision is in flight', async () => {
    const harness = createPrintApp()
    const answer = deferred<Health | null>()
    harness.core.stub('analyzePrint', () => answer.promise)
    const first = runPrintAnalysis(harness.app)
    await vi.waitFor(() => {
      expect(analyses(harness)).toHaveLength(1)
    })
    const second = runPrintAnalysis(harness.app)
    answer.resolve('ok')
    await Promise.all([first, second])
    expect(analyses(harness)).toHaveLength(1)
    expect(running(harness)).toBe(false)
  })

  it('sends nothing more while an analysis of the same revision is waiting', async () => {
    const harness = createPrintApp({ document: documentState({ revision: 3 }) })
    const first = deferred<Health | null>()
    const answers = [first]
    harness.core.stub('analyzePrint', () => answers.shift()?.promise ?? Promise.resolve('ok'))
    const pending = [runPrintAnalysis(harness.app)]
    await vi.waitFor(() => {
      expect(analyses(harness)).toHaveLength(1)
    })
    setRevision(harness.app, 4)
    pending.push(runPrintAnalysis(harness.app), runPrintAnalysis(harness.app))
    first.resolve('ok')
    await Promise.all(pending)
    expect(analyses(harness)).toHaveLength(2)
  })

  it('analyzes the same revision again once the earlier analysis has finished ("Check again")', async () => {
    const harness = createPrintApp()
    await runPrintAnalysis(harness.app)
    await runPrintAnalysis(harness.app)
    expect(analyses(harness)).toHaveLength(2)
  })

  it('analyzes the same revision of another document', async () => {
    const harness = createPrintApp()
    const answer = deferred<Health | null>()
    harness.core.stub('analyzePrint', () => answer.promise)
    const first = runPrintAnalysis(harness.app)
    await vi.waitFor(() => {
      expect(analyses(harness)).toHaveLength(1)
    })
    harness.app.store.setState((state) =>
      state.document === null
        ? {}
        : { document: { ...state.document, documentHandle: DOCUMENT_HANDLE + 1 } },
    )
    const second = runPrintAnalysis(harness.app)
    harness.core.stub('analyzePrint', () => Promise.resolve('ok'))
    answer.resolve('ok')
    await Promise.all([first, second])
    expect(analyses(harness).map(([documentHandle]) => documentHandle)).toEqual([
      DOCUMENT_HANDLE,
      DOCUMENT_HANDLE + 1,
    ])
  })

  it('shows a CoreError of the analysis as an error notification and stops running', async () => {
    const harness = createPrintApp()
    const error = await coreErrorFor({
      code: 'internal_error',
      message: 'The print check could not run.',
    })
    harness.core.stub('analyzePrint', () => Promise.reject(error))
    await runPrintAnalysis(harness.app)
    const state = harness.app.store.getState()
    expect(state.notifications.map(({ severity, message }) => ({ severity, message }))).toEqual([
      { severity: 'error', message: 'The print check could not run.' },
    ])
    expect(state.print.running).toBe(false)
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

  const OVERHANG: FindingKey = { body: MUSHROOM.id, check: 'overhangs', index: 0 }
  const BED_FIT: FindingKey = { body: SLAB.id, check: 'bed_fit', index: 0 }

  function focusApp(viewportMounted = true): PrintApp {
    return createPrintApp(
      {
        context: 'print',
        document: documentState({
          bodies: [MUSHROOM, SLAB],
          meshes: {
            [MUSHROOM.id]: meshSummary(5, MUSHROOM_BOUNDS),
            [SLAB.id]: meshSummary(6, SLAB_BOUNDS),
          },
        }),
        print: printState({ report: REPORT, reportRevision: 3 }),
      },
      { viewportMounted },
    )
  }

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
    const harness = focusApp(false)
    focusFinding(harness.app, OVERHANG)
    expect(harness.app.store.getState().print.focusedFinding).toEqual(OVERHANG)
  })

  it('does not move the camera for a finding the report does not have', () => {
    const harness = focusApp()
    focusFinding(harness.app, { body: MUSHROOM.id, check: 'overhangs', index: 1 })
    focusFinding(harness.app, { body: SLAB.id, check: 'islands', index: 0 })
    focusFinding(harness.app, { body: 'b-gone', check: 'bed_fit', index: 0 })
    expect(harness.viewport.zoomToBounds).not.toHaveBeenCalled()
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

  // The cube document at revision 3 with a report of that revision, in Print context.
  function analyzingApp(overrides: Partial<AppState> = {}): PrintApp {
    const harness = createPrintApp({
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
    await settle()
    expect(analyses(harness)).toEqual([])
    await vi.advanceTimersByTimeAsync(1)
    await settle()
    expect(analyses(harness)).toEqual([[DOCUMENT_HANDLE, AS_MODELED]])
  })

  it('U-81 each revision change restarts the 300 ms delay', async () => {
    const harness = analyzingApp()
    setRevision(harness.app, 4)
    await vi.advanceTimersByTimeAsync(200)
    setRevision(harness.app, 5)
    await vi.advanceTimersByTimeAsync(299)
    await settle()
    expect(analyses(harness)).toEqual([])
    await vi.advanceTimersByTimeAsync(1)
    await settle()
    expect(analyses(harness)).toHaveLength(1)
  })

  it('U-81 a null result changes nothing', async () => {
    const harness = analyzingApp()
    harness.core.stub('analyzePrint', () => Promise.resolve(null))
    setRevision(harness.app, 4)
    await vi.advanceTimersByTimeAsync(300)
    await settle()
    expect(analyses(harness)).toHaveLength(1)
    const state = harness.app.store.getState()
    expect(state.print.report).toBe(OVERHANG_REPORT)
    expect(state.print.reportRevision).toBe(3)
    expect(state.print.running).toBe(false)
    expect(selectHealthBadge(state)).toMatchObject({ state: 'warning', warnings: 2, stale: true })
  })

  it('U-81 in Model context no analysis is scheduled', async () => {
    const harness = analyzingApp({ context: 'model' })
    setRevision(harness.app, 4)
    await vi.advanceTimersByTimeAsync(1000)
    await settle()
    expect(analyses(harness)).toEqual([])
  })

  it('U-81 in Sketch context no analysis is scheduled', async () => {
    const harness = analyzingApp({ context: 'sketch' })
    setRevision(harness.app, 4)
    await vi.advanceTimersByTimeAsync(1000)
    await settle()
    expect(analyses(harness)).toEqual([])
  })

  it('schedules nothing while a transaction is open', async () => {
    const harness = analyzingApp({
      document: documentState({ revision: 3, transactionOpen: true }),
    })
    setRevision(harness.app, 4)
    await vi.advanceTimersByTimeAsync(1000)
    await settle()
    expect(analyses(harness)).toEqual([])
  })

  it('runs the analysis on entering Print with a stale report', async () => {
    const harness = analyzingApp({
      context: 'model',
      document: documentState({ revision: 5 }),
      print: printState({ report: OVERHANG_REPORT, reportRevision: 3 }),
    })
    harness.app.store.setState({ context: 'print' })
    await vi.advanceTimersByTimeAsync(0)
    await settle()
    expect(analyses(harness)).toEqual([[DOCUMENT_HANDLE, AS_MODELED]])
  })

  it('runs the analysis on entering Print without a report', async () => {
    const harness = analyzingApp({ context: 'model', print: printState() })
    harness.app.store.setState({ context: 'print' })
    await vi.advanceTimersByTimeAsync(0)
    await settle()
    expect(analyses(harness)).toHaveLength(1)
  })

  it('runs nothing on entering Print with an up-to-date report', async () => {
    const harness = analyzingApp({ context: 'model' })
    harness.app.store.setState({ context: 'print' })
    await vi.advanceTimersByTimeAsync(1000)
    await settle()
    expect(analyses(harness)).toEqual([])
  })

  it('stops scheduling once the returned function is called', async () => {
    const harness = createPrintApp({
      context: 'print',
      document: documentState({ revision: 3 }),
      print: printState({ report: OVERHANG_REPORT, reportRevision: 3 }),
    })
    const stop = installPrintAnalysis(harness.app)
    setRevision(harness.app, 4)
    await vi.advanceTimersByTimeAsync(100)
    stop()
    await vi.advanceTimersByTimeAsync(1000)
    await settle()
    expect(analyses(harness)).toEqual([])
  })
})

describe('one analysis per revision (openPrintHealth and installPrintAnalysis)', () => {
  const stops: (() => void)[] = []

  afterEach(() => {
    for (const stop of stops.splice(0)) {
      stop()
    }
  })

  // The cube document at revision 4 with a report of revision 3, in Model context.
  function staleApp(): PrintApp {
    const harness = createPrintApp({
      context: 'model',
      document: documentState({ revision: 4 }),
      print: printState({ report: OVERHANG_REPORT, reportRevision: 3 }),
    })
    stops.push(installPrintAnalysis(harness.app))
    return harness
  }

  it('a badge click in Model, then entering Print, sends one analyzePrint', async () => {
    const harness = staleApp()
    const answer = deferred<Health | null>()
    harness.core.stub('analyzePrint', () => answer.promise)
    const clicked = openPrintHealth(harness.app)
    await vi.waitFor(() => {
      expect(analyses(harness)).toHaveLength(1)
    })
    harness.app.store.setState({ context: 'print' })
    await settle()
    answer.resolve('warning')
    await clicked
    await settle()
    expect(analyses(harness)).toHaveLength(1)
  })

  it('a badge click while the automatic analysis runs sends nothing more', async () => {
    const harness = staleApp()
    const answer = deferred<Health | null>()
    harness.core.stub('analyzePrint', () => answer.promise)
    harness.app.store.setState({ context: 'print' })
    await vi.waitFor(() => {
      expect(analyses(harness)).toHaveLength(1)
    })
    const clicked = openPrintHealth(harness.app)
    await settle()
    answer.resolve('warning')
    await clicked
    expect(analyses(harness)).toHaveLength(1)
    expect(harness.app.store.getState().panels.side).toBe('health')
  })
})

describe('setCheckHidden for mesh validity', () => {
  it('never hides mesh validity, which is always checked', () => {
    const harness = createPrintApp()
    setCheckHidden(harness.app, 'mesh_validity', true)
    expect(harness.app.store.getState().settings.hiddenChecks).toEqual([])
  })

  it('shows mesh validity again when stored settings hid it', () => {
    const harness = createPrintApp({
      settings: { ...DEFAULT_SETTINGS, hiddenChecks: ['mesh_validity', 'islands'] },
    })
    setCheckHidden(harness.app, 'mesh_validity', false)
    expect(harness.app.store.getState().settings.hiddenChecks).toEqual(['islands'])
  })
})

describe('openPrintHealth (the health badge)', () => {
  it('opens the health panel', async () => {
    const harness = createPrintApp({
      print: printState({ report: OVERHANG_REPORT, reportRevision: 3 }),
    })
    await openPrintHealth(harness.app)
    expect(harness.app.store.getState().panels.side).toBe('health')
  })

  it('runs the analysis when the report describes an older revision', async () => {
    const harness = createPrintApp({
      document: documentState({ revision: 4 }),
      print: printState({ report: OVERHANG_REPORT, reportRevision: 3 }),
    })
    await openPrintHealth(harness.app)
    expect(analyses(harness)).toEqual([[DOCUMENT_HANDLE, AS_MODELED]])
  })

  it('runs the analysis when there is no report yet', async () => {
    const harness = createPrintApp({ print: printState() })
    await openPrintHealth(harness.app)
    expect(analyses(harness)).toHaveLength(1)
  })

  it('runs nothing when the report is up to date', async () => {
    const harness = createPrintApp({
      print: printState({ report: OVERHANG_REPORT, reportRevision: 3 }),
    })
    await openPrintHealth(harness.app)
    expect(analyses(harness)).toEqual([])
  })
})

function focused(harness: PrintApp): FindingKey | null {
  return harness.app.store.getState().print.focusedFinding
}

describe('the focused finding marker (installPrintAnalysis)', () => {
  const OVERHANG: FindingKey = { body: CUBE.id, check: 'overhangs', index: 1 }
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

  // The cube document in Print context with the health panel open and a finding focused.
  function focusedApp(): PrintApp {
    const harness = createPrintApp({
      context: 'print',
      document: documentState({ revision: 3 }),
      print: printState({ report: OVERHANG_REPORT, reportRevision: 3 }),
      panels: { ...INITIAL_STATE.panels, side: 'health' },
    })
    stops.push(installPrintAnalysis(harness.app))
    focusFinding(harness.app, OVERHANG)
    return harness
  }

  it('stays while the panel is open and the document is unchanged', () => {
    const harness = focusedApp()
    harness.app.store.setState({ selection: [] })
    harness.app.store.setState((state) => ({ print: { ...state.print, running: true } }))
    expect(focused(harness)).toEqual(OVERHANG)
  })

  it('goes when the health panel closes', () => {
    const harness = focusedApp()
    harness.app.store.setState((state) => ({ panels: { ...state.panels, side: null } }))
    expect(focused(harness)).toBeNull()
  })

  it('goes when another side panel replaces the health panel', () => {
    const harness = focusedApp()
    harness.app.store.setState((state) => ({ panels: { ...state.panels, side: 'problems' } }))
    expect(focused(harness)).toBeNull()
  })

  it('goes when the document changes', () => {
    const harness = focusedApp()
    setRevision(harness.app, 4)
    expect(focused(harness)).toBeNull()
  })

  it('goes with the revision change that also schedules the analysis', async () => {
    const harness = focusedApp()
    setRevision(harness.app, 4)
    await vi.advanceTimersByTimeAsync(300)
    await settle()
    expect(analyses(harness)).toEqual([[DOCUMENT_HANDLE, AS_MODELED]])
  })
})

function setTransactionOpen(harness: PrintApp, transactionOpen: boolean): void {
  harness.app.store.setState((state) =>
    state.document === null ? {} : { document: { ...state.document, transactionOpen } },
  )
}

describe('analysis when a transaction closes (installPrintAnalysis)', () => {
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

  it('runs 300 ms after the transaction closes when edits made the report stale', async () => {
    const harness = createPrintApp({
      context: 'print',
      document: documentState({ revision: 3 }),
      print: printState({ report: OVERHANG_REPORT, reportRevision: 3 }),
    })
    stops.push(installPrintAnalysis(harness.app))
    setTransactionOpen(harness, true)
    setRevision(harness.app, 4)
    await vi.advanceTimersByTimeAsync(1000)
    await settle()
    expect(analyses(harness)).toEqual([])
    setTransactionOpen(harness, false)
    await vi.advanceTimersByTimeAsync(299)
    await settle()
    expect(analyses(harness)).toEqual([])
    await vi.advanceTimersByTimeAsync(1)
    await settle()
    expect(analyses(harness)).toEqual([[DOCUMENT_HANDLE, AS_MODELED]])
  })

  it('runs nothing when a transaction closes without changing the document', async () => {
    const harness = createPrintApp({
      context: 'print',
      document: documentState({ revision: 3, transactionOpen: true }),
      print: printState({ report: OVERHANG_REPORT, reportRevision: 3 }),
    })
    stops.push(installPrintAnalysis(harness.app))
    setTransactionOpen(harness, false)
    await vi.advanceTimersByTimeAsync(1000)
    await settle()
    expect(analyses(harness)).toEqual([])
  })
})

// A matcher for a box within 1e-9 of `bounds`.
function boxCloseTo(bounds: BoundingBox) {
  return {
    min: bounds.min.map((value) => expect.closeTo(value, 9)),
    max: bounds.max.map((value) => expect.closeTo(value, 9)),
  }
}

/** Expects every corner of `outline` to have `value` as its coordinate on `axis`. */
function expectInPlane(
  outline: readonly (readonly number[])[] | undefined,
  axis: number,
  value: number,
) {
  expect(outline).toHaveLength(4)
  for (const corner of outline ?? []) {
    expect(corner[axis]).toBeCloseTo(value, 9)
  }
}

describe('U-236 focusing a suggestion', () => {
  it('U-236 frames the box of a split finding once, with margin 0.3', () => {
    const harness = createPrintApp(suggestionState())
    focusFinding(harness.app, SLAB_SPLIT_KEY)
    expect(harness.viewport.zoomToBounds).toHaveBeenCalledTimes(1)
    expect(harness.viewport.zoomToBounds).toHaveBeenCalledWith(
      boxCloseTo({ min: [0, 0, 0], max: [400, 400, 10] }),
      0.3,
    )
  })

  it("U-236 marks a split finding at its box's center and outlines each plane in the box", () => {
    const harness = createPrintApp(suggestionState())
    focusFinding(harness.app, SLAB_SPLIT_KEY)
    const state = harness.app.store.getState()
    expect(focusedFindingPoint(state)).toEqual([200, 200, 5])
    const outlines = focusedSplitOutlines(state)
    expect(outlines).toHaveLength(2)
    expectInPlane(outlines[0], 0, 200)
    expectInPlane(outlines[1], 1, 200)
  })

  it('U-236 frames the cube around a split without planes, and outlines nothing', () => {
    const empty = { ...SLAB_SPLIT, suggestion: { kind: 'split' as const, planes: [] } }
    const harness = createPrintApp(
      suggestionState(null, {
        print: printState({ report: suggestionReport(empty), reportRevision: 3 }),
      }),
    )
    focusFinding(harness.app, SLAB_SPLIT_KEY)
    const side = 0.25 * Math.hypot(400, 400, 10)
    expect(harness.viewport.zoomToBounds).toHaveBeenCalledWith(cube([200, 200, 5], side), 0.3)
    expect(focusedSplitOutlines(harness.app.store.getState())).toEqual([])
  })

  it("U-236 frames a 20 mm cube around a drain hole and marks the hole's point", () => {
    const harness = createPrintApp(suggestionState())
    focusFinding(harness.app, HOLLOW_DRAIN_KEY)
    expect(harness.viewport.zoomToBounds).toHaveBeenCalledTimes(1)
    expect(harness.viewport.zoomToBounds).toHaveBeenCalledWith(cube([-8, -8, 2], 20), 0.3)
    const state = harness.app.store.getState()
    expect(focusedFindingPoint(state)).toEqual([-8, -8, 2])
    expect(focusedSplitOutlines(state)).toEqual([])
  })

  it('outlines nothing without a focused finding', () => {
    const harness = createPrintApp(suggestionState())
    expect(focusedSplitOutlines(harness.app.store.getState())).toEqual([])
  })

  it('returns the same outlines while the report and the focused finding stay', () => {
    const harness = createPrintApp(suggestionState(SLAB_SPLIT_KEY))
    const state = harness.app.store.getState()
    const outlines = focusedSplitOutlines(state)
    expect(focusedSplitOutlines({ ...state, selection: [] })).toBe(outlines)
  })
})

/** The number of split planes `focusedSplitOutlines` holds and the marker's point. */
function shownSuggestion(harness: PrintApp): {
  readonly planes: number
  readonly point: readonly number[] | null
} {
  const state = harness.app.store.getState()
  return { planes: focusedSplitOutlines(state).length, point: focusedFindingPoint(state) }
}

function setPrintOrientation(harness: PrintApp, orientation: Orientation | null): void {
  harness.app.store.setState((state) => ({ print: { ...state.print, orientation } }))
}

const SHOWN = { planes: 2, point: [200, 200, 5] }
const GONE = { planes: 0, point: null }

describe('U-236 the split planes and the marker go (installPrintAnalysis)', () => {
  const TOP_DOWN: Orientation = {
    kind: 'face_down',
    body: SLAB_SPLIT_KEY.body,
    face: CUBE_TOP_FACE.reference,
  }
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

  // Print context with the slab's split finding focused and the analysis installed.
  function focusedApp(orientation: Orientation | null = null): PrintApp {
    const harness = createPrintApp(
      suggestionState(null, {
        print: printState({
          report: suggestionReport(),
          reportRevision: 3,
          orientation,
          reportOrientation: orientation,
        }),
      }),
    )
    stops.push(installPrintAnalysis(harness.app))
    focusFinding(harness.app, SLAB_SPLIT_KEY)
    // Every test below starts with the planes and the marker shown.
    expect(shownSuggestion(harness)).toEqual(SHOWN)
    return harness
  }

  it('shows the planes and the marker of the focused split', () => {
    expect(shownSuggestion(focusedApp())).toEqual(SHOWN)
  })

  it('U-236 they go when the print check panel closes', () => {
    const harness = focusedApp()
    harness.app.store.setState((state) => ({ panels: { ...state.panels, side: null } }))
    expect(shownSuggestion(harness)).toEqual(GONE)
  })

  it('U-236 they go with a new document revision', () => {
    const harness = focusedApp()
    setRevision(harness.app, 4)
    expect(shownSuggestion(harness)).toEqual(GONE)
  })

  it('U-236 they go when a face is laid on the bed', () => {
    const harness = focusedApp()
    setPrintOrientation(harness, TOP_DOWN)
    expect(shownSuggestion(harness)).toEqual(GONE)
  })

  it('U-236 they go when the part prints as modeled again', () => {
    const harness = focusedApp(TOP_DOWN)
    setPrintOrientation(harness, null)
    expect(shownSuggestion(harness)).toEqual(GONE)
  })

  it("U-236 they go when the finding's check is hidden", () => {
    const harness = focusedApp()
    setCheckHidden(harness.app, 'bed_fit', true)
    expect(shownSuggestion(harness)).toEqual(GONE)
  })

  it('U-236 they go when a new report replaces the one they were taken from', () => {
    const harness = focusedApp()
    setRevision(harness.app, 4)
    // The stale report's rows stay listed, so its finding can be focused again.
    focusFinding(harness.app, SLAB_SPLIT_KEY)
    expect(shownSuggestion(harness)).toEqual(SHOWN)
    // In the report of revision 4, the same key names a finding split along z only.
    const tooTall: Finding = {
      ...SLAB_SPLIT,
      suggestion: { kind: 'split', planes: [{ origin: [200, 200, 5], normal: [0, 0, 1] }] },
    }
    harness.app.store.setState((state) => ({
      print: { ...state.print, report: suggestionReport(tooTall), reportRevision: 4 },
    }))
    expect(shownSuggestion(harness)).toEqual(GONE)
  })

  it('they stay when another check is hidden', () => {
    const harness = focusedApp()
    setCheckHidden(harness.app, 'overhangs', true)
    expect(shownSuggestion(harness)).toEqual(SHOWN)
  })

  it('they stay when the same orientation is set again', () => {
    const harness = focusedApp(TOP_DOWN)
    setPrintOrientation(harness, { ...TOP_DOWN })
    expect(shownSuggestion(harness)).toEqual(SHOWN)
  })
})
