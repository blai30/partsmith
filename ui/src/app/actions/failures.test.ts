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
import { CUBE_FEATURES_DOCUMENT } from '../../test/fakes/documents'
import type { FakeFeatureStatus } from '../../test/fakes/core'
import { settle } from '../../test/fakes/settle'
import { applyEdits } from './document'

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

  it('a feature that already failed before the change is not reported again', async () => {
    const fake = await startWithDocument('cube-features.ptsm', CUBE_FEATURES_DOCUMENT, {
      core: { statuses: { 'f-fil1': FAILED } },
    })
    await applyEdits(fake.app, 'Change Fillet 1', RADIUS_15)
    await settle()
    expect(fake.app.store.getState().notifications).toEqual([])
  })
})
