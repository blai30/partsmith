import { afterEach, describe, expect, it, vi } from 'vitest'
import type { Cause, Edit, FeatureStatus, FixAction, Repair } from '../../generated/protocol'
import { MESSAGES } from '../../messages'
import { elementCentroids } from '../../repair/frame'
import { selectRepairMode } from '../../repair/repair'
import { createAppStore } from '../../state/store'
import {
  activeCommand,
  actionLabels,
  createTestApp,
  onlyNotification,
  runAction,
  selectionKeys,
} from '../../test/fakes/app'
import { coreErrorFor, createFakeCoreApi, type FakeCoreApi } from '../../test/fakes/core-api'
import {
  CUBE_DOCUMENT,
  CUBE_SKETCH_SOLUTION,
  CUBE_TOP_FACE,
  featuresOf,
} from '../../test/fakes/documents'
import {
  lostEdgeRepair,
  R16_DOCUMENT,
  R16_MESH_ID,
  R16_REPAIR,
  R16_STATUS,
  R16_STORE_CHANGES,
  r16Fix,
  r16ListElements,
  SPLIT_SOLUTION,
  SPLIT_STATUS,
} from '../../test/fakes/repairs'
import { settle } from '../../test/fakes/settle'
import { meshSummary, stateWithDocument } from '../../test/fakes/state'
import {
  createFakeViewport,
  expectBoundsContain,
  lastFramedBounds,
  type FakeViewport,
} from '../../test/fakes/viewport'
import type { App } from '../createApp'
import { editFeature, setActiveField } from './command'
import { applyEdits } from './document'
import { blurRepair, copyDetails, focusRepair, runFixAction, showLost } from './fixes'

// The actions of messages and cause lines (ui.md 4.2: runFixAction), Copy details (4.1) and
// "Show what '{label}' lost" (3.7: showLost).

afterEach(() => {
  Reflect.deleteProperty(navigator, 'clipboard')
  vi.restoreAllMocks()
})

/** Gives `navigator` a clipboard whose `writeText` does what `write` does. */
function stubClipboard(write: (text: string) => Promise<void>) {
  const writeText = vi.fn<(text: string) => Promise<void>>(write)
  Object.defineProperty(navigator, 'clipboard', { value: { writeText }, configurable: true })
  return writeText
}

type Harness = { readonly app: App; readonly core: FakeCoreApi; readonly viewport: FakeViewport }

/** The R16 document after its edit, with 'Fillet 1' in `status`, a fake viewport and the edges of `b-1`. */
function r16App(status: FeatureStatus = R16_STATUS): Harness {
  const core = createFakeCoreApi({ listElements: r16ListElements })
  const viewport = createFakeViewport()
  const app = createTestApp({
    core,
    viewport,
    store: createAppStore(
      stateWithDocument(
        R16_DOCUMENT,
        {},
        { featureStatus: { [status.id]: status }, ...R16_STORE_CHANGES },
      ),
    ),
  })
  return { app, core, viewport }
}

/** The cube with its sketch solved, and a fake viewport. */
function cubeApp(): Harness {
  const core = createFakeCoreApi()
  const viewport = createFakeViewport()
  const app = createTestApp({
    core,
    viewport,
    store: createAppStore(
      stateWithDocument(
        CUBE_DOCUMENT,
        {},
        {
          sketches: { 'f-sk1': { solution: CUBE_SKETCH_SOLUTION, display: null } },
          meshes: { 'b-cube': meshSummary(1, { min: [-10, -10, 0], max: [10, 10, 20] }) },
        },
      ),
    ),
  })
  return { app, core, viewport }
}

describe('runFixAction (4.2)', () => {
  it('runs an apply action through applyFix with its FixRef, outside a transaction at rest', async () => {
    const { app, core } = r16App()
    await runFixAction(
      app,
      {
        label: "Use the front edge of the cut made by 'Extrude 1'",
        run: { kind: 'apply', fix: r16Fix(0) },
      },
      'panel',
    )
    await settle()
    expect(core.callsTo('applyFix')).toEqual([[1, r16Fix(0)]])
    expect(core.callsTo('beginTransaction')).toEqual([])
  })

  it('runs a pick action as an edit of its feature with the field its path maps to active', async () => {
    const { app } = r16App()
    await runFixAction(
      app,
      {
        label: 'Pick an edge again',
        run: { kind: 'pick', feature: 'f-fil1', path: '/edges/0' },
      },
      'panel',
    )
    await settle()
    expect(activeCommand(app)).toMatchObject({
      featureId: 'f-fil1',
      editing: true,
      activeField: 'edges',
    })
  })

  it('runs a show action of a message by selecting the feature', async () => {
    const { app } = cubeApp()
    await runFixAction(
      app,
      { label: "Show 'Sketch 1'", run: { kind: 'show', feature: 'f-sk1' } },
      'panel',
    )
    await settle()
    expect(selectionKeys(app)).toEqual(['feature:f-sk1'])
    expect(app.store.getState().shown).toEqual({ feature: 'f-sk1', entity: null, ghost: null })
  })

  it('selects the entity a show names when the sketch still has it', async () => {
    const { app } = cubeApp()
    await runFixAction(
      app,
      {
        label: "Show the line in 'Sketch 1'",
        run: { kind: 'show', feature: 'f-sk1', entity: 'e-1' },
      },
      'panel',
    )
    await settle()
    expect(selectionKeys(app)).toEqual(['sketch:f-sk1:e-1'])
  })

  it('selects the sketch when the entity a show names is gone', async () => {
    const { app } = cubeApp()
    await runFixAction(
      app,
      {
        label: "Show the line in 'Sketch 1'",
        run: { kind: 'show', feature: 'f-sk1', entity: 'e-5' },
      },
      'panel',
    )
    await settle()
    expect(selectionKeys(app)).toEqual(['feature:f-sk1'])
  })

  it('runs an undo action as undo', async () => {
    const { app, core } = cubeApp()
    await runFixAction(app, { label: 'Undo', run: { kind: 'undo' } }, 'panel')
    await settle()
    expect(core.callsTo('undo')).toEqual([[1]])
  })

  it("refuses a notification's apply action during the command editing its feature, with finish_command_first (3.7)", async () => {
    const { app, core } = r16App()
    await editFeature(app, 'f-fil1')
    await settle()
    await runFixAction(
      app,
      {
        label: "Use the front edge of the cut made by 'Extrude 1'",
        run: { kind: 'apply', fix: r16Fix(0) },
      },
      'notification',
    )
    await settle()
    expect(core.callsTo('applyFix')).toEqual([])
    expect(app.store.getState().notifications).toMatchObject([
      { severity: 'info', code: 'finish_command_first' },
    ])
  })

  it("runs a cause line's apply action with the cause's FixRef", async () => {
    const { app, core } = r16App()
    const revert: FixAction = {
      label: "Set Distance of 'Extrude 1' back to 5 mm",
      run: { kind: 'apply', fix: { ...r16Fix(0), cause: true } },
    }
    const cause: Cause = {
      text: "This started after you changed Distance of 'Extrude 1'.",
      actions: [revert],
    }
    await runFixAction(app, revert, 'panel', cause)
    await settle()
    expect(core.callsTo('applyFix')).toEqual([[1, { ...r16Fix(0), cause: true }]])
  })

  it('U-128 shows a stale_fix refusal with "Show problems" only', async () => {
    const { app, core } = r16App()
    const message =
      'This fix no longer applies because the document changed after it was offered. Look at the problem again.'
    const stale = await coreErrorFor({ code: 'stale_fix', message })
    core.stub('applyFix', () => Promise.reject(stale))
    await runFixAction(
      app,
      {
        label: "Use the front edge of the cut made by 'Extrude 1'",
        run: { kind: 'apply', fix: r16Fix(0) },
      },
      'panel',
    )
    await settle()
    const notification = onlyNotification(app)
    expect(notification).toMatchObject({ severity: 'error', message, code: 'stale_fix' })
    expect(actionLabels(notification)).toEqual(['Show problems'])
  })
})

const KERNEL = {
  code: 'kernel',
  message:
    "Partsmith's geometry kernel could not build this fillet. Fillets fail like this when the edges meet other rounded edges at a corner; try a slightly different radius, or fillet those edges in two steps.",
  detail: 'fillet: StdFail_NotDone',
  cause: { text: 'This started after you changed Radius from 1 mm to 3 mm.', actions: [] },
}

describe('copyDetails (4.1, U-130)', () => {
  it('U-130 writes the code, the message, the cause and the detail, one per line', async () => {
    const { app } = cubeApp()
    const writeText = stubClipboard(() => Promise.resolve())
    await copyDetails(app, KERNEL)
    expect(writeText).toHaveBeenCalledTimes(1)
    expect(writeText).toHaveBeenCalledWith(
      ['kernel', KERNEL.message, KERNEL.cause.text, 'fillet: StdFail_NotDone'].join('\n'),
    )
  })

  it('U-130 announces "Details copied." in the polite live region', async () => {
    const { app } = cubeApp()
    stubClipboard(() => Promise.resolve())
    await copyDetails(app, KERNEL)
    expect(app.store.getState().announcement?.message).toBe(MESSAGES.details_copied())
  })

  it('U-130 writes three lines for a message without a cause', async () => {
    const { app } = cubeApp()
    const writeText = stubClipboard(() => Promise.resolve())
    await copyDetails(app, { code: KERNEL.code, message: KERNEL.message, detail: KERNEL.detail })
    expect(writeText).toHaveBeenCalledWith(
      ['kernel', KERNEL.message, 'fillet: StdFail_NotDone'].join('\n'),
    )
  })

  it('U-130 a clipboard that refuses shows details_not_copied as a warning', async () => {
    const { app } = cubeApp()
    stubClipboard(() => Promise.reject(new DOMException('Denied', 'NotAllowedError')))
    await copyDetails(app, KERNEL)
    expect(app.store.getState().notifications).toMatchObject([
      {
        severity: 'warning',
        code: 'details_not_copied',
        message: MESSAGES.details_not_copied(),
      },
    ])
  })

  it('U-130 an internal_error with a detail shows its error notification with "Copy details"', async () => {
    const { app, core } = cubeApp()
    const message =
      'Partsmith hit an internal problem and did not apply the change. Copy the details and report it.'
    const internal = await coreErrorFor({
      code: 'internal_error',
      message,
      detail: 'panic: index out of range',
    })
    core.stub('applyEdits', () => Promise.reject(internal))
    const rename: readonly Edit[] = [{ type: 'update_body', body: { id: 'b-cube', name: 'Box' } }]
    await applyEdits(app, 'Rename', rename)
    await settle()
    const [notification] = app.store.getState().notifications
    expect(notification).toMatchObject({ severity: 'error', message, code: 'internal_error' })
    expect(notification === undefined ? [] : actionLabels(notification)).toContain('Copy details')
    const writeText = stubClipboard(() => Promise.resolve())
    if (notification !== undefined) {
      runAction(notification, 'Copy details')
    }
    await settle()
    expect(writeText).toHaveBeenCalledWith(
      ['internal_error', message, 'panic: index out of range'].join('\n'),
    )
  })
})

describe('showLost (3.7)', () => {
  it("opens repair mode on the feature's first repair", async () => {
    const { app } = r16App()
    await showLost(app, 'f-fil1')
    await settle()
    expect(activeCommand(app)).toMatchObject({ featureId: 'f-fil1', activeField: 'edges' })
    expect(selectRepairMode(app.store.getState())).toMatchObject({
      feature: 'f-fil1',
      view: { repair: { path: '/edges/0' } },
    })
  })

  it('frames the ghost and the candidates, at least a third of the body on each axis, with margin 0.1', async () => {
    const { app, viewport } = r16App()
    await showLost(app, 'f-fil1')
    await settle()
    const { bounds, margin } = lastFramedBounds(viewport)
    expect(margin).toBe(0.1)
    expectBoundsContain(bounds, [
      [-10, -10, 20],
      [10, -10, 20],
      [0, -10, 15],
      [0, -5, 20],
    ])
    for (const axis of [0, 1, 2] as const) {
      expect(bounds.max[axis] - bounds.min[axis]).toBeGreaterThanOrEqual(20 / 3 - 1e-9)
    }
  })

  it('frames the stored centroid when there is no ghost', async () => {
    const { app, viewport } = r16App({
      ...R16_STATUS,
      repairs: [{ ...R16_REPAIR, ghost: null, candidates: [] }],
    })
    await showLost(app, 'f-fil1')
    await settle()
    const { bounds } = lastFramedBounds(viewport)
    const centroid = [0, -10, 20] as const
    for (const axis of [0, 1, 2] as const) {
      expect(bounds.min[axis]).toBeLessThanOrEqual(centroid[axis])
      expect(bounds.max[axis]).toBeGreaterThanOrEqual(centroid[axis])
    }
  })

  it("frames a lost region's candidate numbers, at least a third of the sketch's drawing on each axis", async () => {
    const viewport = createFakeViewport()
    const app = createTestApp({
      core: createFakeCoreApi(),
      viewport,
      store: createAppStore(
        stateWithDocument(
          CUBE_DOCUMENT,
          {},
          {
            sketches: { 'f-sk1': { solution: SPLIT_SOLUTION, display: null } },
            featureStatus: { 'f-ext1': SPLIT_STATUS },
          },
        ),
      ),
    })
    await showLost(app, 'f-ext1')
    await settle()
    expect(activeCommand(app)).toMatchObject({ featureId: 'f-ext1', activeField: 'profile' })
    const { bounds, margin } = lastFramedBounds(viewport)
    // The numbers sit at (5, 0, 0) and (-5, 0, 0); the drawing is 20 mm across.
    const third = 20 / 3
    expect(bounds.min[0]).toBeCloseTo(-5, 9)
    expect(bounds.max[0]).toBeCloseTo(5, 9)
    expect(bounds.min[1]).toBeCloseTo(-third / 2, 9)
    expect(bounds.max[1]).toBeCloseTo(third / 2, 9)
    expect(margin).toBe(0.1)
  })
})

describe('the notification of a CoreError (4.2, U-147)', () => {
  const RENAME: readonly Edit[] = [{ type: 'update_body', body: { id: 'b-cube', name: 'Box' } }]

  it("U-147 shows no button for the error's finish_transaction action", async () => {
    const { app, core } = cubeApp()
    const refused = await coreErrorFor({
      code: 'transaction_open',
      message: 'Finish or cancel "Fillet" first.',
      detail: 'boom',
      actions: [{ label: 'Finish "Fillet" and save', run: { kind: 'finish_transaction' } }],
    })
    core.stub('applyEdits', () => Promise.reject(refused))
    await applyEdits(app, 'Rename', RENAME)
    await settle()
    const [notification] = app.store.getState().notifications
    expect(notification).toMatchObject({ severity: 'error', code: 'transaction_open' })
    expect(notification === undefined ? [] : actionLabels(notification)).not.toContain(
      'Finish "Fillet" and save',
    )
  })

  it("offers the error's other actions, which run as fix actions", async () => {
    const { app, core } = cubeApp()
    const fix = { feature: 'f-ext1', code: 'edit_rejected', cause: false, index: 0, revision: 1 }
    const refused = await coreErrorFor({
      code: 'edit_rejected',
      message: "Body 'Box' already exists.",
      actions: [{ label: "Use body 'Box'", run: { kind: 'apply', fix } }],
    })
    core.stub('applyEdits', () => Promise.reject(refused))
    await applyEdits(app, 'Rename', RENAME)
    await settle()
    const [notification] = app.store.getState().notifications
    expect(notification === undefined ? [] : actionLabels(notification)).toContain("Use body 'Box'")
    if (notification !== undefined) {
      runAction(notification, "Use body 'Box'")
    }
    await settle()
    expect(core.callsTo('applyFix')).toEqual([[1, fix]])
  })
})

describe('what a show action frames (4.2)', () => {
  const BOX = { min: [-10, -10, 0], max: [10, 10, 20] } as const

  it('frames the faces the feature made, from their meshes', async () => {
    const { app, core, viewport } = cubeApp()
    core.stub('listElements', (_documentHandle, _body, kind) =>
      Promise.resolve(
        kind === 'face' ? [CUBE_TOP_FACE, { ...CUBE_TOP_FACE, feature: 'f-box9' }] : [],
      ),
    )
    viewport.itemBounds.mockReturnValue({ min: [...BOX.min], max: [...BOX.max] })
    await runFixAction(
      app,
      { label: "Show 'Extrude 1'", run: { kind: 'show', feature: 'f-ext1' } },
      'panel',
    )
    await settle()
    const framed = viewport.itemBounds.mock.calls.at(-1)?.[0] ?? []
    expect(framed.map((item) => item.key)).toEqual([`element:b-cube:${CUBE_TOP_FACE.name}`])
    expect(viewport.zoomToBounds).toHaveBeenLastCalledWith(
      { min: [...BOX.min], max: [...BOX.max] },
      0.1,
    )
  })

  it("frames a sketch's entities, or the entity it names", async () => {
    const { app, viewport } = cubeApp()
    viewport.itemBounds.mockReturnValue({ min: [...BOX.min], max: [...BOX.max] })
    await runFixAction(
      app,
      { label: "Show 'Sketch 1'", run: { kind: 'show', feature: 'f-sk1' } },
      'panel',
    )
    await settle()
    const all = viewport.itemBounds.mock.calls.at(-1)?.[0] ?? []
    expect(all.map((item) => item.key)).toEqual(
      CUBE_SKETCH_SOLUTION.entities.map((entity) => `sketch:f-sk1:${entity.id}`),
    )
    await runFixAction(
      app,
      {
        label: "Show the line in 'Sketch 1'",
        run: { kind: 'show', feature: 'f-sk1', entity: 'e-1' },
      },
      'panel',
    )
    await settle()
    expect((viewport.itemBounds.mock.calls.at(-1)?.[0] ?? []).map((item) => item.key)).toEqual([
      'sketch:f-sk1:e-1',
    ])
  })

  it('frames nothing when nothing it made is drawn', async () => {
    const { app, viewport } = cubeApp()
    await runFixAction(
      app,
      { label: "Show 'Extrude 1'", run: { kind: 'show', feature: 'f-ext1' } },
      'panel',
    )
    await settle()
    expect(viewport.zoomToBounds).not.toHaveBeenCalled()
  })
})

describe('a pick action of the command being edited (4.2)', () => {
  it('makes the field its path maps to the active field, without a new command', async () => {
    const { app, core } = r16App()
    await showLost(app, 'f-fil1')
    await settle()
    setActiveField(app, null)
    await runFixAction(
      app,
      {
        label: 'Pick an edge again',
        run: { kind: 'pick', feature: 'f-fil1', path: '/edges/0' },
      },
      'panel',
    )
    expect(activeCommand(app).activeField).toBe('edges')
    expect(core.callsTo('beginTransaction')).toHaveLength(1)
  })
})

describe('the current repair follows the active field (3.7)', () => {
  it('a field that holds a repair makes it the current repair', async () => {
    const extrude = featuresOf(CUBE_DOCUMENT).find((feature) => feature.type === 'extrude')
    if (extrude === undefined) {
      throw new Error('The cube has no extrude.')
    }
    const profileRepair: Repair = {
      kind: 'region',
      path: '/profile/regions/0',
      sketch: 'f-sk1',
      region: { key: 'e-1,e-2,e-3,e-4', fp: 'c=0,0|a=400' },
      resolution: 'missing',
      description: "the region of 'Sketch 1' around (0, 0)",
      candidates: [],
    }
    const status: FeatureStatus = {
      id: extrude.id,
      status: 'error',
      messages: [],
      repairs: [profileRepair, lostEdgeRepair({ path: '/body' })],
    }
    const core = createFakeCoreApi()
    const app = createTestApp({
      core,
      store: createAppStore(
        stateWithDocument(CUBE_DOCUMENT, {}, { featureStatus: { [extrude.id]: status } }),
      ),
    })
    await editFeature(app, extrude.id)
    expect(selectRepairMode(app.store.getState())?.view.repair.path).toBe('/profile/regions/0')
    setActiveField(app, 'body')
    expect(selectRepairMode(app.store.getState())?.view.repair.path).toBe('/body')
  })
})

describe('repair mode from a focused Problems row (3.7, 3.9)', () => {
  it("shows the feature's repairs while no command is active", () => {
    const { app } = r16App()
    focusRepair(app, 'f-fil1')
    expect(app.store.getState().repair).toEqual({ feature: 'f-fil1' })
  })

  it('shows nothing for a feature without repairs', () => {
    const { app } = r16App({ ...R16_STATUS, repairs: [] })
    focusRepair(app, 'f-fil1')
    expect(app.store.getState().repair).toBeNull()
  })

  it('ends only the repair mode of the feature that lost focus', () => {
    const { app } = r16App()
    focusRepair(app, 'f-fil1')
    blurRepair(app, 'f-other')
    expect(app.store.getState().repair).toEqual({ feature: 'f-fil1' })
    blurRepair(app, 'f-fil1')
    expect(app.store.getState().repair).toBeNull()
  })
})

describe('elementCentroids (3.7)', () => {
  it("gives each element's descriptor centroid in its body's current mesh", async () => {
    const { app } = r16App()
    expect(
      await elementCentroids(app, [
        { body: 'b-1', mesh_id: R16_MESH_ID, kind: 'edge', index: 3 },
        null,
      ]),
    ).toEqual([[0, -10, 15], null])
  })

  it('gives null for an element of a mesh the body no longer shows', async () => {
    const { app, core } = r16App()
    expect(
      await elementCentroids(app, [
        { body: 'b-1', mesh_id: R16_MESH_ID + 1, kind: 'edge', index: 3 },
      ]),
    ).toEqual([null])
    expect(core.callsTo('listElements')).toEqual([])
  })
})
