import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import type { Orientation } from '../../generated/protocol'
import { MESSAGES } from '../../messages'
import {
  CUBE,
  DOCUMENT_HANDLE,
  bodyPrint,
  createPrintApp,
  documentState,
  printReport,
  printState,
  type PrintApp,
} from '../../print/fixtures.testing'
import { selectHealthBadge } from '../../state/selectors'
import type { AppState } from '../../state/store'
import { coreErrorFor } from '../../test/fakes/core-api'
import { CUBE_FRONT_FACE, CUBE_TOP_FACE } from '../../test/fakes/documents'
import { elementItem } from '../../test/fakes/selection'
import { settle } from '../../test/fakes/settle'
import { CYLINDRICAL_FACE } from '../../layout/fixtures.testing'
import { exportModel } from './export'
import {
  cancelLayOnFace,
  installPrintAnalysis,
  layOnFace,
  runPrintAnalysis,
  setPrintOrientation,
  startLayOnFace,
} from './print'

// Lay on face (ui.md 3.17): a session print orientation for print checks and 3MF and STL export.

/** The cube's top face resting on the bed. */
const TOP_DOWN: Orientation = { kind: 'face_down', body: CUBE.id, face: CUBE_TOP_FACE.reference }

const REPORT = printReport([bodyPrint({ body: CUBE.id, meshId: 1 })])

function print(harness: PrintApp): AppState['print'] {
  return harness.app.store.getState().print
}

describe('the Lay on face pick (3.17)', () => {
  it('starts and ends without changing the orientation', () => {
    const harness = createPrintApp({ context: 'print' })
    startLayOnFace(harness.app)
    expect(print(harness).pickingFace).toBe(true)
    cancelLayOnFace(harness.app)
    expect(print(harness)).toMatchObject({ pickingFace: false, orientation: null })
  })

  it('lays a flat face of a visible part body on the bed and ends the pick', () => {
    const harness = createPrintApp({ context: 'print' })
    startLayOnFace(harness.app)
    expect(layOnFace(harness.app, elementItem(CUBE_TOP_FACE, 'Cube'))).toBe(true)
    expect(print(harness)).toMatchObject({ pickingFace: false, orientation: TOP_DOWN })
  })

  it('refuses a face that is not flat with pick_flat_face in the print hint, and keeps picking', () => {
    const harness = createPrintApp({
      context: 'print',
      document: documentState({ bodies: [CUBE, { id: 'b-peg', name: 'Peg' }] }),
    })
    startLayOnFace(harness.app)
    expect(layOnFace(harness.app, elementItem(CYLINDRICAL_FACE, 'Peg'))).toBe(false)
    const state = harness.app.store.getState()
    expect(state.pickHint).toEqual({ message: MESSAGES.pick_flat_face(), field: null })
    expect(state.print).toMatchObject({ pickingFace: true, orientation: null })
  })

  it('refuses a flat face of a modifier body or a hidden body', () => {
    const modifier = createPrintApp({
      context: 'print',
      document: documentState({ bodies: [{ ...CUBE, role: 'settings_modifier' }] }),
    })
    startLayOnFace(modifier.app)
    expect(layOnFace(modifier.app, elementItem(CUBE_FRONT_FACE, 'Cube'))).toBe(false)
    expect(print(modifier).orientation).toBeNull()
    const hidden = createPrintApp({
      context: 'print',
      document: documentState({ bodies: [{ ...CUBE, visible: false }] }),
    })
    startLayOnFace(hidden.app)
    expect(layOnFace(hidden.app, elementItem(CUBE_FRONT_FACE, 'Cube'))).toBe(false)
  })

  it('setPrintOrientation(null) prints as modeled again', () => {
    const harness = createPrintApp({
      context: 'print',
      print: printState({ orientation: TOP_DOWN }),
    })
    setPrintOrientation(harness.app, null)
    expect(print(harness).orientation).toBeNull()
  })
})

describe('print checks in the Lay on face orientation (U-124)', () => {
  it('U-124 runPrintAnalysis sends a face-down orientation and records it in reportOrientation', async () => {
    const harness = createPrintApp({
      context: 'print',
      print: printState({ orientation: TOP_DOWN }),
    })
    await runPrintAnalysis(harness.app)
    expect(harness.core.callsTo('analyzePrint')).toEqual([
      [DOCUMENT_HANDLE, { orientation: TOP_DOWN }],
    ])
    expect(print(harness).reportOrientation).toEqual(TOP_DOWN)
  })

  it('U-124 resetting the orientation makes the badge stale', () => {
    const harness = createPrintApp({
      context: 'print',
      print: printState({
        report: REPORT,
        reportRevision: 3,
        reportOrientation: TOP_DOWN,
        orientation: TOP_DOWN,
      }),
    })
    expect(selectHealthBadge(harness.app.store.getState()).stale).toBe(false)
    setPrintOrientation(harness.app, null)
    expect(selectHealthBadge(harness.app.store.getState()).stale).toBe(true)
  })

  it('U-124 a report as modeled is current for no orientation and stale for a face-down one', () => {
    const harness = createPrintApp({
      context: 'print',
      print: printState({ report: REPORT, reportRevision: 3 }),
    })
    expect(selectHealthBadge(harness.app.store.getState()).stale).toBe(false)
    setPrintOrientation(harness.app, TOP_DOWN)
    expect(selectHealthBadge(harness.app.store.getState()).stale).toBe(true)
  })

  it('U-124 an invalid_orientation CoreError clears the orientation and shows the core message as a warning', async () => {
    const harness = createPrintApp({
      context: 'print',
      print: printState({ orientation: TOP_DOWN }),
    })
    const error = await coreErrorFor({
      code: 'invalid_orientation',
      message: 'That face cannot rest on the bed.',
    })
    harness.core.stub('analyzePrint', () => Promise.reject(error))
    await runPrintAnalysis(harness.app)
    await settle()
    const state = harness.app.store.getState()
    expect(state.print.orientation).toBeNull()
    expect(state.notifications).toEqual([
      expect.objectContaining({
        severity: 'warning',
        message: 'That face cannot rest on the bed.',
        code: 'invalid_orientation',
      }),
    ])
  })

  it('a reference_unresolved error of the picked face clears it the same way', async () => {
    const harness = createPrintApp({
      context: 'print',
      print: printState({ orientation: TOP_DOWN }),
    })
    const error = await coreErrorFor({
      code: 'reference_unresolved',
      message: 'The picked face no longer exists.',
    })
    harness.core.stub('analyzePrint', () => Promise.reject(error))
    await runPrintAnalysis(harness.app)
    await settle()
    expect(print(harness).orientation).toBeNull()
    expect(harness.app.store.getState().notifications[0]).toMatchObject({ severity: 'warning' })
  })

  it('a refusal while printing as modeled is an ordinary core error', async () => {
    const harness = createPrintApp({ context: 'print' })
    const error = await coreErrorFor({ code: 'internal_error', message: 'The analysis failed.' })
    harness.core.stub('analyzePrint', () => Promise.reject(error))
    await runPrintAnalysis(harness.app)
    await settle()
    expect(harness.app.store.getState().notifications[0]).toMatchObject({ severity: 'error' })
  })
})

describe('exports in the Lay on face orientation (U-124)', () => {
  it.each(['3mf', 'stl'] as const)(
    'U-124 exportModel passes the orientation for %s',
    async (format) => {
      const harness = createPrintApp({ print: printState({ orientation: TOP_DOWN }) })
      await exportModel(harness.app, { format, scope: 'visible' })
      const [call] = harness.core.callsTo('exportBodies')
      expect(call?.[3]).toEqual(TOP_DOWN)
    },
  )

  it.each(['obj', 'step'] as const)('U-124 exportModel keeps %s as modeled', async (format) => {
    const harness = createPrintApp({ print: printState({ orientation: TOP_DOWN }) })
    await exportModel(harness.app, { format, scope: 'visible' })
    const [call] = harness.core.callsTo('exportBodies')
    expect(call).toHaveLength(3)
  })

  it('U-124 an invalid_orientation export error clears the orientation with a warning', async () => {
    const harness = createPrintApp({ print: printState({ orientation: TOP_DOWN }) })
    const error = await coreErrorFor({
      code: 'invalid_orientation',
      message: 'That face cannot rest on the bed.',
    })
    harness.core.stub('exportBodies', () => Promise.reject(error))
    await exportModel(harness.app, { format: '3mf', scope: 'visible' })
    const state = harness.app.store.getState()
    expect(state.print.orientation).toBeNull()
    expect(state.notifications).toEqual([
      expect.objectContaining({
        severity: 'warning',
        message: 'That face cannot rest on the bed.',
      }),
    ])
    expect(harness.files.writes).toEqual([])
  })
})

describe('Lay on face in the automatic analysis (installPrintAnalysis)', () => {
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

  function installed(overrides: Partial<AppState>): PrintApp {
    const harness = createPrintApp(overrides)
    stops.push(installPrintAnalysis(harness.app))
    return harness
  }

  it('in Print context an orientation change schedules analyzePrint with it after 300 ms', async () => {
    const harness = installed({
      context: 'print',
      print: printState({ report: REPORT, reportRevision: 3 }),
    })
    setPrintOrientation(harness.app, TOP_DOWN)
    await vi.advanceTimersByTimeAsync(299)
    await settle()
    expect(harness.core.callsTo('analyzePrint')).toEqual([])
    await vi.advanceTimersByTimeAsync(1)
    await settle()
    expect(harness.core.callsTo('analyzePrint')).toEqual([
      [DOCUMENT_HANDLE, { orientation: TOP_DOWN }],
    ])
  })

  it('leaving Print ends the face pick and keeps the orientation', () => {
    const harness = installed({ context: 'print', print: printState({ orientation: TOP_DOWN }) })
    startLayOnFace(harness.app)
    harness.app.store.setState({ context: 'model' })
    expect(print(harness)).toMatchObject({ pickingFace: false, orientation: TOP_DOWN })
  })
})
