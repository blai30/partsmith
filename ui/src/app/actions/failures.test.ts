import { describe, expect, it } from 'vitest'
import type { Diagnostic, Edit, Event as ProtocolEvent } from '../../generated/protocol'
import { MESSAGES } from '../../messages'
import {
  actionLabels,
  notificationWith,
  runAction,
  startWithDocument,
  type FakeApp,
} from '../../test/fakes/app'
import { CUBE_FEATURES_DOCUMENT } from '../../test/fakes/documents'
import type { FakeReply } from '../../test/fakes/core'
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

/** Answers apply_edits as the core does, with the given features failing in its recompute. */
function failing(fake: FakeApp, featureIds: readonly string[]): void {
  fake.transport.handle('apply_edits', (command, core): FakeReply => {
    const reply = core.reply(command)
    const events = (reply.events ?? []).map((event): ProtocolEvent =>
      event.type === 'recomputed'
        ? {
            ...event,
            features: event.features.map((status) =>
              featureIds.includes(status.id)
                ? { ...status, status: 'error', messages: [TOO_LARGE] }
                : status,
            ),
          }
        : event,
    )
    return { ...reply, events }
  })
}

describe('failure notifications', () => {
  it('one feature that fails after a change: "{label} failed. {message}" with Edit and Undo', async () => {
    const fake = await startWithDocument('cube-features.ptsm', CUBE_FEATURES_DOCUMENT)
    failing(fake, ['f-fil1'])
    await applyEdits(fake.app, 'Change Fillet 1', RADIUS_15)
    await settle()
    const notification = notificationWith(
      fake.app,
      MESSAGES.feature_failed('Fillet 1', TOO_LARGE.message),
    )
    expect(notification.severity).toBe('error')
    expect(actionLabels(notification)).toEqual(['Edit', 'Undo'])
    fake.transport.handle('apply_edits', null)
    runAction(notification, 'Undo')
    await settle()
    expect(fake.transport.commandTypes()).toContain('undo')
  })

  it('several failing features: a count with Show problems and Undo', async () => {
    const fake = await startWithDocument('cube-features.ptsm', CUBE_FEATURES_DOCUMENT)
    failing(fake, ['f-fil1', 'f-sh1'])
    await applyEdits(fake.app, 'Change Fillet 1', RADIUS_15)
    await settle()
    const notification = notificationWith(fake.app, MESSAGES.features_failed(2))
    expect(actionLabels(notification)).toEqual(['Show problems', 'Undo'])
    runAction(notification, 'Show problems')
    expect(fake.app.store.getState().panels.side).toBe('problems')
  })

  it('a feature that already failed before the change is not reported again', async () => {
    const fake = await startWithDocument('cube-features.ptsm', CUBE_FEATURES_DOCUMENT, {
      core: { statuses: { 'f-fil1': { status: 'error', messages: [TOO_LARGE] } } },
    })
    await applyEdits(fake.app, 'Change Fillet 1', RADIUS_15)
    await settle()
    expect(fake.app.store.getState().notifications).toEqual([])
  })
})
