import { describe, expect, it } from 'vitest'
import type { Diagnostic, Edit } from '../../generated/protocol'
import { MESSAGES } from '../../messages'
import {
  actionLabels,
  notificationWith,
  runAction,
  startWithDocument,
  type FakeApp,
} from '../../test/fakes/app'
import { CUBE_FEATURES_DOCUMENT, CUBE_TOP_FRONT_EDGE, featuresOf } from '../../test/fakes/documents'
import { elementItem } from '../../test/fakes/selection'
import type { FakeFeatureStatus } from '../../test/fakes/core'
import { lostEdgeRepair } from '../../test/fakes/repairs'
import { settle } from '../../test/fakes/settle'
import { capturePick, editFeature, undoCommandChange } from './command'
import { applyEdits, undo } from './document'

// The failure notification of ui.md 4.2: a user change turns features from not failing to failing.

const TOO_LARGE: Diagnostic = {
  severity: 'error',
  code: 'fillet_too_large',
  message: 'Fillet radius 15 mm is too large for the selected edges.',
  field: '/radius',
  suggestion: '2.3 mm',
}

const RADIUS_15: readonly Edit[] = [
  {
    type: 'update_feature',
    feature: { id: 'f-fil1', type: 'fillet', body: 'b-cube', edges: [], radius: '15 mm' },
  },
]

/**
 * Starts the cube with features, all ok, and returns the statuses the fake core reports from its
 * next recompute on, so a test can make features fail as the result of a user change.
 */
async function cubeWithStatuses(): Promise<{
  readonly fake: FakeApp
  readonly statuses: Record<string, FakeFeatureStatus>
}> {
  const statuses: Record<string, FakeFeatureStatus> = {}
  const fake = await startWithDocument('cube-features.ptsm', CUBE_FEATURES_DOCUMENT, {
    core: { statuses },
  })
  return { fake, statuses }
}

const FAILED: FakeFeatureStatus = { status: 'error', messages: [TOO_LARGE] }

/** The first edge of Fillet 1, which no longer resolves. */
const LOST_EDGE = lostEdgeRepair()

const RENAME_BODY: readonly Edit[] = [{ type: 'update_body', body: { id: 'b-cube', name: 'Box' } }]

describe('failure notifications', () => {
  it('one feature that fails after a change: "{label} failed. {message}" with Edit and Undo', async () => {
    const { fake, statuses } = await cubeWithStatuses()
    statuses['f-fil1'] = FAILED
    await applyEdits(fake.app, 'Change Fillet 1', RADIUS_15)
    await settle()
    const notification = notificationWith(
      fake.app,
      MESSAGES.feature_failed('Fillet 1', TOO_LARGE.message),
    )
    expect(notification.severity).toBe('error')
    expect(actionLabels(notification)).toEqual(['Edit', 'Undo'])
    delete statuses['f-fil1']
    runAction(notification, 'Undo')
    await settle()
    expect(fake.transport.commandTypes()).toContain('undo')
  })

  it('U-110 a failed feature with a lost reference offers "Edit", which edits it with Edges active, and no "Re-pick"', async () => {
    const { fake, statuses } = await cubeWithStatuses()
    statuses['f-fil1'] = { ...FAILED, repairs: [LOST_EDGE] }
    await applyEdits(fake.app, 'Change Fillet 1', RADIUS_15)
    await settle()
    const notification = notificationWith(
      fake.app,
      MESSAGES.feature_failed('Fillet 1', TOO_LARGE.message),
    )
    expect(actionLabels(notification)).toEqual(['Edit', 'Undo'])
    runAction(notification, 'Edit')
    await settle()
    expect(fake.app.store.getState().command).toMatchObject({
      featureId: 'f-fil1',
      editing: true,
      activeField: 'edges',
    })
  })

  it('several failing features: a count with Show problems and Undo', async () => {
    const { fake, statuses } = await cubeWithStatuses()
    statuses['f-fil1'] = FAILED
    statuses['f-sh1'] = FAILED
    await applyEdits(fake.app, 'Change Fillet 1', RADIUS_15)
    await settle()
    const notification = notificationWith(fake.app, MESSAGES.features_failed(2))
    expect(actionLabels(notification)).toEqual(['Show problems', 'Undo'])
    runAction(notification, 'Show problems')
    expect(fake.app.store.getState().panels.side).toBe('problems')
  })

  it('names the failed features, with the message id as its code', async () => {
    const { fake, statuses } = await cubeWithStatuses()
    statuses['f-fil1'] = FAILED
    await applyEdits(fake.app, 'Change Fillet 1', RADIUS_15)
    await settle()
    const notification = notificationWith(
      fake.app,
      MESSAGES.feature_failed('Fillet 1', TOO_LARGE.message),
    )
    expect(notification.features).toEqual(['f-fil1'])
    expect(notification.code).toBe('feature_failed')
  })

  it('is dismissed once the feature it names is not in error anymore, after an undo (3.22)', async () => {
    const { fake, statuses } = await cubeWithStatuses()
    statuses['f-fil1'] = FAILED
    await applyEdits(fake.app, 'Change Fillet 1', RADIUS_15)
    await settle()
    expect(fake.app.store.getState().notifications).toHaveLength(1)
    delete statuses['f-fil1']
    await undo(fake.app)
    await settle()
    expect(fake.app.store.getState().notifications).toEqual([])
  })

  it('stays while any feature it names is still in error', async () => {
    const { fake, statuses } = await cubeWithStatuses()
    statuses['f-fil1'] = FAILED
    statuses['f-sh1'] = FAILED
    await applyEdits(fake.app, 'Change Fillet 1', RADIUS_15)
    await settle()
    const message = MESSAGES.features_failed(2)
    expect(notificationWith(fake.app, message).features).toEqual(['f-fil1', 'f-sh1'])
    delete statuses['f-fil1']
    await applyEdits(fake.app, 'Rename body', RENAME_BODY)
    await settle()
    expect(notificationWith(fake.app, message)).toBeDefined()
    delete statuses['f-sh1']
    await applyEdits(fake.app, 'Rename body', RENAME_BODY)
    await settle()
    expect(fake.app.store.getState().notifications).toEqual([])
  })

  it('a feature that already failed before the change is not reported again', async () => {
    const fake = await startWithDocument('cube-features.ptsm', CUBE_FEATURES_DOCUMENT, {
      core: { statuses: { 'f-fil1': FAILED } },
    })
    await applyEdits(fake.app, 'Change Fillet 1', RADIUS_15)
    await settle()
    expect(fake.app.store.getState().notifications).toEqual([])
  })
})

/** The stored edges of Fillet 1 in cube-features.ptsm. */
const STORED_EDGES = featuresOf(CUBE_FEATURES_DOCUMENT).find(
  (feature) => feature.id === 'f-fil1',
)?.['edges'] as readonly unknown[]

async function editingLostEdge(): Promise<FakeApp> {
  const fake = await startWithDocument('cube-features.ptsm', CUBE_FEATURES_DOCUMENT, {
    core: { statuses: { 'f-fil1': { ...FAILED, repairs: [LOST_EDGE] } } },
  })
  await editFeature(fake.app, 'f-fil1')
  await settle()
  return fake
}

function edges(fake: FakeApp): unknown {
  return fake.app.store.getState().command?.params['edges']
}

describe('picking a lost reference again (3.7, 4.2)', () => {
  it("U-110 the next pick in Edges takes the lost edge's place instead of adding to the list", async () => {
    const fake = await editingLostEdge()
    await capturePick(fake.app, elementItem(CUBE_TOP_FRONT_EDGE, 'Cube'))
    expect(edges(fake)).toEqual([CUBE_TOP_FRONT_EDGE.reference, ...STORED_EDGES.slice(1)])
  })

  it('U-110 once the lost edge is replaced, picks toggle as usual', async () => {
    const fake = await editingLostEdge()
    await capturePick(fake.app, elementItem(CUBE_TOP_FRONT_EDGE, 'Cube'))
    await capturePick(fake.app, elementItem(CUBE_TOP_FRONT_EDGE, 'Cube'))
    expect(edges(fake)).toEqual(STORED_EDGES.slice(1))
  })

  it('U-110 undoing the pick brings the lost edge back, and the next pick replaces it again', async () => {
    const fake = await editingLostEdge()
    await capturePick(fake.app, elementItem(CUBE_TOP_FRONT_EDGE, 'Cube'))
    await undoCommandChange(fake.app)
    expect(edges(fake)).toEqual(STORED_EDGES)
    await capturePick(fake.app, elementItem(CUBE_TOP_FRONT_EDGE, 'Cube'))
    expect(edges(fake)).toEqual([CUBE_TOP_FRONT_EDGE.reference, ...STORED_EDGES.slice(1)])
  })

  it('U-110 picking an edge the list already holds just removes the lost one', async () => {
    const fake = await editingLostEdge()
    const [, second] = STORED_EDGES
    const held = {
      ...CUBE_TOP_FRONT_EDGE,
      reference: second as typeof CUBE_TOP_FRONT_EDGE.reference,
    }
    await capturePick(fake.app, elementItem(held, 'Cube'))
    expect(edges(fake)).toEqual(STORED_EDGES.slice(1))
  })
})
