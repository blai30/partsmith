import { act, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { installPopoverPolyfill } from '../../components/popover-polyfill.testing'
import { CoreFatalError } from '../../core/transport'
import type {
  Diagnostic,
  Document,
  ElementDescriptor,
  FeatureStatus,
  FixAction,
  Reference,
  RepairCandidate,
  SketchPlane,
} from '../../generated/protocol'
import { inspectorField } from '../../inspector/inspector.testing'
import { documentWithStatuses, readyState } from '../../layout/fixtures.testing'
import { escapeStep } from '../../layout/ladders'
import { pickRow } from '../../layout/pick-row'
import {
  createModelApp,
  renderWithApp,
  stubAnimationFrames,
  stubElementSize,
  stubMatchMedia,
  type FrameQueue,
} from '../../layout/test-app.testing'
import type { ElementRepair } from '../../repair/repair'
import type { DocumentState } from '../../state/store'
import { coreErrorFor, type FakeCoreApi } from '../../test/fakes/core-api'
import { deferred } from '../../test/fakes/deferred'
import { CUBE_DOCUMENT, type FeatureRecord, withFeatureChanges } from '../../test/fakes/documents'
import { datumItem, meshPick, originItem } from '../../test/fakes/selection'
import { settle } from '../../test/fakes/settle'
import { meshSummary } from '../../test/fakes/state'
import type { App } from '../createApp'
import { confirmAfterClicks, editFeature, setActiveField, startFeatureCommand } from './command'
import { pickAt } from './selection'

// A datum plane's new base asks the core (ui.md 3.7, engine.md 3.22, U-251): while a command edits
// a datum plane, each pick of its Base plane field asks `attachPlane` before it lands, from the
// view, a browser row or the Choose menu. A pick the core gives no plane for is refused with a
// hint under the field. A pick of the edited datum itself is refused without asking.

// The WebGL2 backend cannot run in jsdom; the viewport gets a FakeBackend instead.
vi.mock('../../viewport/webgl2/backend', async () => {
  const fakes = await import('../../test/fakes/backend')
  return { createWebGl2Backend: () => fakes.createFakeBackend() }
})

let restoreSize: () => void
let frames: FrameQueue

beforeEach(installPopoverPolyfill)

beforeEach(() => {
  restoreSize = stubElementSize(HTMLCanvasElement.prototype, { width: 800, height: 600 })
  stubMatchMedia({ reducedMotion: true })
  frames = stubAnimationFrames()
})

afterEach(() => {
  vi.useRealTimers()
  restoreSize()
  vi.unstubAllGlobals()
  vi.restoreAllMocks()
})

/** The top of 'Box 1' as the datums of datum-base-lifted.ptsm store it, before the lift. */
const TOP_OF_BOX_1 = { q: 'f-box1/top', fp: 'plane|n=0,0,1|c=0,0,10|a=100' }

/** A datum plane's base on the top of 'Box 1'. */
const ON_THE_TOP = { kind: 'face', body: 'b-1', face: TOP_OF_BOX_1 }

/** The records of datum-base-lifted.ptsm (ui.md 6.2), named, then 'Datum plane 3' on XY. */
const FEATURES: readonly FeatureRecord[] = [
  {
    id: 'f-box1',
    type: 'box',
    name: 'Box 1',
    body: 'b-1',
    size: ['10 mm', '10 mm', '10 mm'],
    op: 'new',
  },
  {
    id: 'f-mv1',
    type: 'move_body',
    name: 'Move body 1',
    body: 'b-1',
    translation: ['0 mm', '0 mm', '20 mm'],
  },
  { id: 'f-dp1', type: 'datum_plane', name: 'Datum plane 1', base: ON_THE_TOP },
  { id: 'f-dp2', type: 'datum_plane', name: 'Datum plane 2', base: ON_THE_TOP },
  {
    id: 'f-sk1',
    type: 'sketch',
    name: 'Sketch 1',
    plane: { kind: 'datum', feature: 'f-dp2' },
    entities: [{ id: 'e-1', type: 'circle', center: [0, 0], radius: 2 }],
    constraints: [],
  },
  {
    id: 'f-ext1',
    type: 'extrude',
    name: 'Extrude 1',
    body: 'b-1',
    profile: { sketch: 'f-sk1', regions: [{ key: 'e-1', fp: 'c=0,0|a=12.566371' }] },
    extent: { kind: 'distance', expr: '5 mm' },
    op: 'join',
  },
  {
    id: 'f-dp3',
    type: 'datum_plane',
    name: 'Datum plane 3',
    base: { kind: 'origin', plane: 'XY' },
  },
]

/** U-251's document: datum-base-lifted.ptsm with 'Datum plane 3' on XY after 'Extrude 1'. */
const LIFTED: Document = {
  ...CUBE_DOCUMENT,
  format_version: 3,
  bodies: [{ id: 'b-1', name: 'Body 1' }],
  features: [...FEATURES],
}

/** The front side of 'Box 1' (y = -5) on the lifted cube, as `pick_reference` describes it. */
const BOX_1_FRONT: ElementDescriptor = {
  name: 'f-box1/front',
  kind: 'face',
  body: 'b-1',
  feature: 'f-box1',
  reference: { q: 'f-box1/front', fp: 'plane|n=0,-1,0|c=0,-5,5|a=100|adj=4,4' },
  surface: 'plane',
  centroid: [0, -5, 25],
  area: 100,
  normal: [0, -1, 0],
  placement: { origin: [0, -5, 25], x_axis: [1, 0, 0], normal: [0, -1, 0] },
  // The lift undone: the fallback in body coordinates (engine.md 3.9).
  fallback: { origin: [0, -5, 5], x_axis: [1, 0, 0], normal: [0, -1, 0] },
  adjacent: [],
}

/** The plane a host builds from the front side of 'Box 1', as for a new sketch (3.16.1). */
const BOX_1_FRONT_PLANE: SketchPlane = {
  kind: 'face',
  body: 'b-1',
  face: BOX_1_FRONT.reference,
  fallback: { origin: [0, -5, 5], x_axis: [1, 0, 0], normal: [0, -1, 0] },
  fallback_frame: 'body',
}

/** The bottom of 'Box 1' in body coordinates. */
const BOX_1_BOTTOM: Reference = { q: 'f-box1/bottom', fp: 'plane|n=0,0,-1|c=0,0,0|a=100|adj=4,4' }

/** The base the Base plane field holds for the front side of 'Box 1': no fallback, no frame. */
const BOX_1_FRONT_BASE = { kind: 'face', body: 'b-1', face: BOX_1_FRONT.reference }

const XY: SketchPlane = { kind: 'origin', plane: 'XY' }
const XZ: SketchPlane = { kind: 'origin', plane: 'XZ' }

/** The text of `datum_base_moved` (4.3) for 'Datum plane 2' on `target`. */
function datumBaseMoved(target: string): string {
  return `'Datum plane 2' cannot be placed on ${target}: the two do not move together, and what later features place on the faces made from the sketches that build on 'Datum plane 2' could shift with no message. Pick a plane or face that moves the same way as 'Datum plane 2'.`
}

/** The store's document state of U-251's document with these statuses in place of ok ones. */
function documentFor(
  statuses: readonly FeatureStatus[] = [],
  document: Document = LIFTED,
): DocumentState {
  return documentWithStatuses(document, statuses, {
    name: 'datum-base-lifted',
    meshes: { 'b-1': meshSummary(1, { min: [-5, -5, 20], max: [5, 5, 35] }) },
  })
}

type Harness = { readonly app: App; readonly core: FakeCoreApi }

/** Edits 'Datum plane 2' with its Base plane field made active. */
async function editingDatum2(document: DocumentState = documentFor()): Promise<Harness> {
  const { app, core } = createModelApp({ state: readyState(document) })
  await editFeature(app, 'f-dp2')
  await settle()
  setActiveField(app, 'base')
  return { app, core }
}

/** The whole shell, editing 'Datum plane 2' with its Base plane field made active. */
async function renderedEditingDatum2(document: DocumentState = documentFor()): Promise<Harness> {
  const { app, core } = createModelApp({ state: readyState(document) })
  renderWithApp(app)
  flush()
  await act(async () => {
    await editFeature(app, 'f-dp2')
    await settle()
  })
  setActiveField(app, 'base')
  flush()
  return { app, core }
}

function flush(): void {
  act(() => {
    frames.flush()
  })
}

/** Clicks a browser row and lets its turn land. */
async function clickRow(app: App, item: Parameters<typeof pickRow>[1]): Promise<void> {
  pickRow(app, item, false)
  await settle()
}

/** Clicks the front side of 'Box 1' in the view and lets the click land. */
async function clickBoxFront({ app, core }: Harness): Promise<void> {
  core.stub('pickReference', () => Promise.resolve(BOX_1_FRONT))
  await pickAt(app, meshPick(BOX_1_FRONT, 1), { toggle: false })
  await settle()
}

/** The value of the Base plane field. */
function baseOf(app: App): unknown {
  return app.store.getState().command?.params['base']
}

/** The labels of the command history's steps. */
function historyOf(app: App): readonly string[] {
  return (app.store.getState().command?.history ?? []).map((step) => step.field)
}

/** The `applyEdits` calls without a label: the previews and the edits joining the transaction. */
function previews(core: FakeCoreApi): readonly (readonly unknown[])[] {
  return core.callsTo('applyEdits').filter((call) => call[1] === null)
}

/** The `base` of each `update_feature` of 'Datum plane 2' among `calls`, in order. */
function basesSent(calls: readonly (readonly unknown[])[]): unknown[] {
  return calls.flatMap((call) =>
    (call[2] as readonly { type: string; feature?: { id: string; base?: unknown } }[])
      .filter((edit) => edit.type === 'update_feature' && edit.feature?.id === 'f-dp2')
      .map((edit) => edit.feature?.base),
  )
}

/** The `base` of the last `update_feature` of 'Datum plane 2' among `calls`. */
function lastBaseSent(calls: readonly (readonly unknown[])[]): unknown {
  return basesSent(calls).at(-1)
}

/** The hint under the Base plane field. */
function baseHint(app: App): string | null {
  const hint = app.store.getState().pickHint
  return hint?.field === 'base' ? hint.message : null
}

/** Expects a refused pick: the field, the history and the previews as they were, the field active. */
function expectRefused(app: App, core: FakeCoreApi, previewsBefore: number): void {
  expect(baseOf(app)).toEqual(ON_THE_TOP)
  expect(historyOf(app)).toEqual([])
  expect(previews(core)).toHaveLength(previewsBefore)
  expect(app.store.getState().command?.activeField).toBe('base')
}

/** A `reference_guessed` warning of 'Datum plane 2' at `/base/face` with these actions. */
function guessedBase(actions: readonly FixAction[]): Diagnostic {
  return {
    severity: 'warning',
    code: 'reference_guessed',
    message:
      "The top of 'Box 1' no longer exists as named, so the face that fits its shape best, the top of 'Box 1', is used. Check it, then keep it or pick another.",
    field: '/base/face',
    actions: [...actions],
  }
}

/** The status of 'Datum plane 2' with `guessedBase` and its element repair at `/base/face`. */
function guessedBaseStatus(
  actions: readonly FixAction[],
  candidates: readonly RepairCandidate[],
): FeatureStatus {
  const repair: ElementRepair = {
    kind: 'element',
    path: '/base/face',
    body: 'b-1',
    reference: TOP_OF_BOX_1,
    resolution: 'ambiguous',
    description: "the top of 'Box 1'",
    candidates: [...candidates],
  }
  return { id: 'f-dp2', status: 'warning', messages: [guessedBase(actions)], repairs: [repair] }
}

/** A candidate of the repair of 'Datum plane 2''s base. */
function baseCandidate(face: Reference, description: string): RepairCandidate {
  return {
    reference: face,
    description,
    relation: "on 'Box 1'",
    meaning: false,
    distance: 0,
    score: 0.5,
  }
}

/** The core's `Use` action of the bottom of 'Box 1' at `/base/face` of 'Datum plane 2'. */
const USE_THE_BOTTOM: FixAction = {
  label: "Use the bottom of 'Box 1'",
  run: {
    kind: 'apply',
    fix: {
      feature: 'f-dp2',
      code: 'reference_guessed',
      field: '/base/face',
      cause: false,
      index: 1,
      revision: 1,
    },
  },
}

/** The core's `Pick a face again` action at `/base/face` of 'Datum plane 2'. */
const PICK_A_FACE_AGAIN: FixAction = {
  label: 'Pick a face again',
  run: { kind: 'pick', feature: 'f-dp2', path: '/base/face' },
}

/** Edits 'Datum plane 2', picks XY into its Base plane field, and cancels the edit with Restore. */
async function canceledWithXy(): Promise<Harness> {
  const { app, core } = await editingDatum2()
  await clickRow(app, originItem('XY'))
  await startFeatureCommand(app, 'extrude')
  await settle()
  return { app, core }
}

/** Runs the Restore action of the `command_canceled` notification. */
async function restore(app: App): Promise<void> {
  const canceled = app.store
    .getState()
    .notifications.find((notification) => notification.code === 'command_canceled')
  if (canceled === undefined) {
    throw new Error('The edit was not canceled with Restore.')
  }
  canceled.actions.find((action) => action.label === 'Restore')?.run()
  await settle()
}

describe('a datum plane new base asks the core (3.7, U-251)', () => {
  it('U-251 (a) the browser row Top plane (XY) asks attachPlane once and lands the plane the core returns', async () => {
    const { app, core } = await editingDatum2()
    await clickRow(app, originItem('XY'))
    expect(core.callsTo('attachPlane')).toEqual([[1, 'f-dp2', XY]])
    expect(baseOf(app)).toEqual(XY)
    expect(historyOf(app)).toEqual(['Base plane'])
    expect(lastBaseSent(previews(core))).toEqual(XY)
  })

  it('U-251 (b) a flat face of Box 1 picked in the view asks with the plane of a new sketch and stores the face without its fallback', async () => {
    const harness = await editingDatum2()
    await clickBoxFront(harness)
    expect(harness.core.callsTo('attachPlane')).toEqual([[1, 'f-dp2', BOX_1_FRONT_PLANE]])
    expect(baseOf(harness.app)).toEqual(BOX_1_FRONT_BASE)
    expect(Object.keys(baseOf(harness.app) as object)).not.toContain('fallback')
    expect(Object.keys(baseOf(harness.app) as object)).not.toContain('fallback_frame')
  })

  it('U-251 (c) a pick the core gives no plane for is refused with datum_base_moved under the field for 3 s', async () => {
    const { app, core } = await editingDatum2()
    core.stub('attachPlane', () => Promise.resolve(null))
    const previewsBefore = previews(core).length
    vi.useFakeTimers()
    await clickRow(app, originItem('XY'))
    expectRefused(app, core, previewsBefore)
    expect(baseHint(app)).toBe(datumBaseMoved('the Top plane (XY)'))
    expect(app.store.getState().notifications).toEqual([])
    vi.advanceTimersByTime(2_999)
    expect(baseHint(app)).not.toBeNull()
    vi.advanceTimersByTime(1)
    expect(baseHint(app)).toBeNull()
  })

  it('U-251 (c) a refused face names it by its feature, and a refused datum plane by its label', async () => {
    const harness = await editingDatum2()
    harness.core.stub('attachPlane', () => Promise.resolve(null))
    const previewsBefore = previews(harness.core).length
    await clickBoxFront(harness)
    expectRefused(harness.app, harness.core, previewsBefore)
    expect(baseHint(harness.app)).toBe(datumBaseMoved("this face of 'Box 1'"))
    await clickRow(harness.app, datumItem('f-dp1', 'Datum plane 1'))
    expectRefused(harness.app, harness.core, previewsBefore)
    expect(baseHint(harness.app)).toBe(datumBaseMoved("'Datum plane 1'"))
  })

  it('U-251 (d) a refused datum plane after the edited one shows attach_after', async () => {
    const { app, core } = await editingDatum2()
    core.stub('attachPlane', () => Promise.resolve(null))
    await clickRow(app, datumItem('f-dp3', 'Datum plane 3'))
    expect(core.callsTo('attachPlane')).toEqual([[1, 'f-dp2', { kind: 'datum', feature: 'f-dp3' }]])
    expect(baseHint(app)).toBe(
      "'Datum plane 2' cannot be placed on 'Datum plane 3', which comes after it. Pick an origin plane, or a flat face or datum plane made before 'Datum plane 2'.",
    )
  })

  it('U-251 (d) a refused suppressed datum plane shows attach_datum_unbuilt for a suppressed datum', async () => {
    const suppressed: FeatureStatus = {
      id: 'f-dp1',
      status: 'suppressed',
      messages: [],
      repairs: [],
    }
    const { app, core } = await editingDatum2(
      documentFor([suppressed], withFeatureChanges(LIFTED, 'f-dp1', { suppressed: true })),
    )
    core.stub('attachPlane', () => Promise.resolve(null))
    await clickRow(app, datumItem('f-dp1', 'Datum plane 1'))
    expect(baseHint(app)).toBe(
      "'Datum plane 2' cannot be placed on 'Datum plane 1', which is suppressed. Unsuppress 'Datum plane 1', or pick another plane or face.",
    )
  })

  it('U-251 (d) a refused datum plane with an error shows attach_datum_unbuilt for an error', async () => {
    const failed: FeatureStatus = {
      id: 'f-dp1',
      status: 'error',
      messages: [
        {
          severity: 'error',
          code: 'references_not_found',
          message: "The face 'Datum plane 1' is placed on can no longer be found.",
        },
      ],
      repairs: [],
    }
    const { app, core } = await editingDatum2(documentFor([failed]))
    core.stub('attachPlane', () => Promise.resolve(null))
    await clickRow(app, datumItem('f-dp1', 'Datum plane 1'))
    expect(baseHint(app)).toBe(
      "'Datum plane 2' cannot be placed on 'Datum plane 1', which has an error. Fix 'Datum plane 1' first, or pick another plane or face.",
    )
  })

  it('U-251 (e) the Choose menu entry Front plane (XZ) asks attachPlane and follows its answer as a row does', async () => {
    const { app, core } = await renderedEditingDatum2()
    const user = userEvent.setup()
    core.stub('attachPlane', () => Promise.resolve(null))
    await user.click(within(inspectorField('base')).getByRole('button', { name: /Choose/ }))
    await user.click(screen.getByRole('menuitem', { name: /Front plane \(XZ\)/ }))
    await act(() => settle())
    expect(core.callsTo('attachPlane')).toEqual([[1, 'f-dp2', XZ]])
    expect(baseOf(app)).toEqual(ON_THE_TOP)
    expect(baseHint(app)).toBe(datumBaseMoved('the Front plane (XZ)'))
    core.stub('attachPlane', (_documentHandle, _feature, plane) => Promise.resolve(plane))
    await user.click(within(inspectorField('base')).getByRole('button', { name: /Choose/ }))
    await user.click(screen.getByRole('menuitem', { name: /Front plane \(XZ\)/ }))
    await act(() => settle())
    expect(baseOf(app)).toEqual(XZ)
  })

  it("U-251 (f) a rejection of attachPlane keeps the field and shows the core's message as for any refused change", async () => {
    const { app, core } = await editingDatum2()
    const refusal = await coreErrorFor({
      code: 'unknown_body',
      message: 'There is no body with id b-9.',
    })
    core.stub('attachPlane', () => Promise.reject(refusal))
    const previewsBefore = previews(core).length
    await clickRow(app, originItem('XY'))
    expectRefused(app, core, previewsBefore)
    const notifications = app.store.getState().notifications
    expect(notifications.map((notification) => notification.code)).toEqual(['unknown_body'])
    expect(notifications[0]?.message).toBe('There is no body with id b-9.')
  })

  it('U-251 (g) a cancel ends the command at once, and the answer that comes after it lands nothing', async () => {
    const harness = await editingDatum2()
    const answer = deferred<SketchPlane | null>()
    harness.core.stub('attachPlane', () => answer.promise)
    harness.core.stub('pickReference', () => Promise.resolve(BOX_1_FRONT))
    const picked = pickAt(harness.app, meshPick(BOX_1_FRONT, 1), { toggle: false })
    await settle()
    expect(harness.core.callsTo('attachPlane')).toHaveLength(1)
    expect(escapeStep(harness.app)).toBe(true)
    await settle()
    expect(harness.app.store.getState().command).toBeNull()
    const sentBefore = harness.core.callsTo('applyEdits').length
    answer.resolve(BOX_1_FRONT_PLANE)
    await picked
    await settle()
    expect(harness.core.callsTo('applyEdits')).toHaveLength(sentBefore)
    expect(harness.app.store.getState().command).toBeNull()
    expect(harness.app.store.getState().pickHint).toBeNull()
  })

  it('U-251 (g) Enter waits for the answer to a pick in the view and commits the face as the base', async () => {
    const harness = await editingDatum2()
    const answer = deferred<SketchPlane | null>()
    harness.core.stub('attachPlane', () => answer.promise)
    harness.core.stub('pickReference', () => Promise.resolve(BOX_1_FRONT))
    const picked = pickAt(harness.app, meshPick(BOX_1_FRONT, 1), { toggle: false })
    await settle()
    expect(confirmAfterClicks(harness.app)).toBe(true)
    await settle()
    expect(harness.core.callsTo('commitTransaction')).toEqual([])
    answer.resolve(BOX_1_FRONT_PLANE)
    await picked
    await settle()
    expect(harness.core.callsTo('commitTransaction')).toEqual([[1]])
    expect(lastBaseSent(harness.core.callsTo('applyEdits'))).toEqual(BOX_1_FRONT_BASE)
  })

  it('U-251 (h) a new datum plane asks nothing', async () => {
    const { app, core } = createModelApp({ state: readyState(documentFor()) })
    await startFeatureCommand(app, 'datum_plane')
    await settle()
    setActiveField(app, 'base')
    await clickRow(app, originItem('XY'))
    expect(core.callsTo('attachPlane')).toEqual([])
    expect(baseOf(app)).toEqual(XY)
  })

  it('U-251 (i) a pick of the edited datum itself is refused with datum_base_self without asking the core', async () => {
    const { app, core } = await editingDatum2()
    const previewsBefore = previews(core).length
    await clickRow(app, datumItem('f-dp2', 'Datum plane 2'))
    expect(core.callsTo('attachPlane')).toEqual([])
    expectRefused(app, core, previewsBefore)
    expect(baseHint(app)).toBe(
      "'Datum plane 2' cannot be placed on itself. Pick an origin plane, or a flat face or datum plane made before 'Datum plane 2'.",
    )
  })

  it('U-251 (j) in repair mode a refused pick keeps the stored reference in the repair field', async () => {
    const { app, core } = await editingDatum2(documentFor([guessedBaseStatus([], [])]))
    expect(app.store.getState().command?.activeField).toBe('base')
    core.stub('attachPlane', () => Promise.resolve(null))
    const previewsBefore = previews(core).length
    await clickRow(app, originItem('XY'))
    expectRefused(app, core, previewsBefore)
    expect(baseHint(app)).toBe(datumBaseMoved('the Top plane (XY)'))
  })

  it('U-251 (k) Enter waits for the answer to the browser row Datum plane 1 and commits it as the base', async () => {
    const { app, core } = await editingDatum2()
    const answer = deferred<SketchPlane | null>()
    core.stub('attachPlane', () => answer.promise)
    pickRow(app, datumItem('f-dp1', 'Datum plane 1'), false)
    await settle()
    expect(confirmAfterClicks(app)).toBe(true)
    await settle()
    expect(core.callsTo('commitTransaction')).toEqual([])
    answer.resolve({ kind: 'datum', feature: 'f-dp1' })
    await settle()
    expect(core.callsTo('commitTransaction')).toEqual([[1]])
    expect(lastBaseSent(core.callsTo('applyEdits'))).toEqual({ kind: 'datum', feature: 'f-dp1' })
  })

  it('U-251 (k) Enter waits for the answer to the Choose menu entry Top plane (XY) and commits it as the base', async () => {
    const { app, core } = await renderedEditingDatum2()
    const user = userEvent.setup()
    const answer = deferred<SketchPlane | null>()
    core.stub('attachPlane', () => answer.promise)
    await user.click(within(inspectorField('base')).getByRole('button', { name: /Choose/ }))
    await user.click(screen.getByRole('menuitem', { name: /Top plane \(XY\)/ }))
    await act(async () => {
      confirmAfterClicks(app)
      await settle()
    })
    expect(core.callsTo('commitTransaction')).toEqual([])
    await act(async () => {
      answer.resolve(XY)
      await settle()
    })
    expect(core.callsTo('commitTransaction')).toEqual([[1]])
    expect(lastBaseSent(core.callsTo('applyEdits'))).toEqual(XY)
  })

  it('U-251 (l) the Restore of a canceled edit asks for its saved base and then for the next row pick', async () => {
    const { app, core } = await canceledWithXy()
    await restore(app)
    expect(core.callsTo('attachPlane')).toEqual([
      [1, 'f-dp2', XY],
      [1, 'f-dp2', XY],
    ])
    expect(app.store.getState().command?.featureId).toBe('f-dp2')
    expect(baseOf(app)).toEqual(XY)
    setActiveField(app, 'base')
    await clickRow(app, originItem('XZ'))
    expect(core.callsTo('attachPlane').at(-1)).toEqual([1, 'f-dp2', XZ])
  })

  it("U-251 (m) a Restore whose saved base the core refuses reopens the edit with the record's base and the hint", async () => {
    const { app, core } = await canceledWithXy()
    core.stub('attachPlane', () => Promise.resolve(null))
    const sentBeforeRestore = core.callsTo('applyEdits').length
    await restore(app)
    expect(core.callsTo('attachPlane').at(-1)).toEqual([1, 'f-dp2', XY])
    expect(app.store.getState().command?.featureId).toBe('f-dp2')
    expect(baseOf(app)).toEqual(ON_THE_TOP)
    expect(app.store.getState().command?.activeField).toBe('base')
    const basesAfterRestore = basesSent(core.callsTo('applyEdits').slice(sentBeforeRestore))
    expect(basesAfterRestore).not.toContainEqual(XY)
    expect(baseHint(app)).toBe(datumBaseMoved('the Top plane (XY)'))
  })

  it('U-251 (m) a refused saved face asks with the Top plane as its fallback and is named this face', async () => {
    const harness = await editingDatum2()
    await clickBoxFront(harness)
    await startFeatureCommand(harness.app, 'extrude')
    await settle()
    harness.core.stub('attachPlane', () => Promise.resolve(null))
    await restore(harness.app)
    expect(harness.core.callsTo('attachPlane').at(-1)).toEqual([
      1,
      'f-dp2',
      {
        kind: 'face',
        body: 'b-1',
        face: BOX_1_FRONT.reference,
        fallback: { origin: [0, 0, 0], x_axis: [1, 0, 0], normal: [0, 0, 1] },
      },
    ])
    expect(baseOf(harness.app)).toEqual(ON_THE_TOP)
    expect(baseHint(harness.app)).toBe(datumBaseMoved('this face'))
  })

  it('U-251 (n) a core that stops during the Restore request leaves the edit closed', async () => {
    const { app, core } = await canceledWithXy()
    core.stub('attachPlane', () =>
      Promise.reject(new CoreFatalError('The geometry core was stopped.')),
    )
    const sentBeforeRestore = core.callsTo('applyEdits').length
    const notificationsBefore = app.store.getState().notifications
    await restore(app)
    expect(core.callsTo('attachPlane').at(-1)).toEqual([1, 'f-dp2', XY])
    expect(app.store.getState().command).toBeNull()
    expect(core.callsTo('applyEdits')).toHaveLength(sentBeforeRestore)
    expect(app.store.getState().notifications).toEqual(notificationsBefore)
  })

  it('U-251 (o) a base candidate the core leaves without an action says why in its tooltip', async () => {
    const status = guessedBaseStatus(
      [USE_THE_BOTTOM, PICK_A_FACE_AGAIN],
      [
        baseCandidate(BOX_1_FRONT.reference, "the front side of 'Box 1'"),
        baseCandidate(BOX_1_BOTTOM, "the bottom of 'Box 1'"),
      ],
    )
    await renderedEditingDatum2(documentFor([status]))
    const [front, bottom] = screen.getAllByTestId('repair-candidate-button')
    expect(front).toHaveAttribute('aria-disabled', 'true')
    expect(front).toHaveAccessibleDescription(datumBaseMoved("the front side of 'Box 1'"))
    expect(bottom).not.toHaveAttribute('aria-disabled')
    expect(bottom).toHaveAccessibleDescription('')
  })
})
