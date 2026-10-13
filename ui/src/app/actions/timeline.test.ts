import { describe, expect, it } from 'vitest'
import type { Document } from '../../generated/protocol'
import { MESSAGES } from '../../messages'
import {
  actionLabels,
  currentDocument,
  type FakeApp,
  labeledEdits,
  notificationWith,
  runAction,
  startWithDocument,
} from '../../test/fakes/app'
import {
  CUBE_DOCUMENT,
  CUBE_FEATURES_DOCUMENT,
  featuresOf,
  withFeatureChanges,
} from '../../test/fakes/documents'
import { editsOf } from '../../test/fakes/protocol'
import { settle } from '../../test/fakes/settle'
import {
  deleteFeatures,
  moveFeature,
  renameFeature,
  setRollback,
  setSuppressed,
  startRollbackDrag,
  type RollbackDrag,
} from './timeline'

async function cubeFeatures(recipe: Partial<Document> = {}): Promise<FakeApp> {
  return startWithDocument('cube-features.ptsm', { ...CUBE_FEATURES_DOCUMENT, ...recipe })
}

describe('deleteFeatures (3.9)', () => {
  it('U-68 deleting the rollback feature first moves the marker to the nearest later remaining feature', async () => {
    const fake = await cubeFeatures({ rollback: 'f-fil1' })
    await deleteFeatures(fake.app, ['f-fil1'])
    await settle()
    expect(labeledEdits(fake)).toEqual([
      {
        label: 'Delete Fillet 1',
        edits: [
          { type: 'set_rollback', rollback: 'f-sh1' },
          { type: 'remove_feature', id: 'f-fil1' },
        ],
      },
    ])
  })

  it('U-68 with no later remaining feature, the marker moves to the end (no id)', async () => {
    const fake = await cubeFeatures({ rollback: 'f-fil1' })
    await deleteFeatures(fake.app, ['f-fil1', 'f-sh1'])
    await settle()
    expect(labeledEdits(fake)).toEqual([
      {
        label: 'Delete 2 features',
        edits: [
          { type: 'set_rollback' },
          { type: 'remove_feature', id: 'f-fil1' },
          { type: 'remove_feature', id: 'f-sh1' },
        ],
      },
    ])
  })

  it('deletes without touching the marker when the rollback feature stays', async () => {
    const fake = await cubeFeatures()
    await deleteFeatures(fake.app, ['f-sh1'])
    await settle()
    expect(labeledEdits(fake)).toEqual([
      { label: 'Delete Shell 1', edits: [{ type: 'remove_feature', id: 'f-sh1' }] },
    ])
    expect(featuresOf(currentDocument(fake.app).recipe).map((feature) => feature.id)).toEqual([
      'f-sk1',
      'f-ext1',
      'f-fil1',
    ])
  })

  it('shows "Deleted {label}." with Undo, which restores the feature', async () => {
    const fake = await cubeFeatures()
    await deleteFeatures(fake.app, ['f-sh1'])
    await settle()
    const notification = notificationWith(fake.app, MESSAGES.deleted('Shell 1'))
    expect(notification.severity).toBe('info')
    expect(actionLabels(notification)).toEqual(['Undo'])
    runAction(notification, 'Undo')
    await settle()
    expect(fake.transport.commandTypes()).toContain('undo')
    expect(featuresOf(currentDocument(fake.app).recipe).map((feature) => feature.id)).toContain(
      'f-sh1',
    )
  })
})

describe('moveFeature (3.9)', () => {
  it('U-68 moving the feature at index 3 to index 1 sends move_feature labeled "Reorder"', async () => {
    const fake = await cubeFeatures()
    await moveFeature(fake.app, 'f-sh1', 1)
    await settle()
    expect(labeledEdits(fake)).toEqual([
      { label: 'Reorder', edits: [{ type: 'move_feature', id: 'f-sh1', index: 1 }] },
    ])
  })

  it('U-68 moving a feature to its own index sends no edit', async () => {
    const fake = await cubeFeatures()
    await moveFeature(fake.app, 'f-sh1', 3)
    await settle()
    expect(fake.transport.commandTypes()).not.toContain('apply_edits')
  })
})

describe('setRollback (3.9)', () => {
  it('U-68 position N (the end) sends set_rollback without an id', async () => {
    const fake = await cubeFeatures({ rollback: 'f-fil1' })
    await setRollback(fake.app, 4)
    await settle()
    expect(labeledEdits(fake)).toEqual([{ label: 'Roll back', edits: [{ type: 'set_rollback' }] }])
  })

  it('U-68 position 2 sends the id of the third feature', async () => {
    const fake = await cubeFeatures()
    await setRollback(fake.app, 2)
    await settle()
    expect(labeledEdits(fake)).toEqual([
      { label: 'Roll back', edits: [{ type: 'set_rollback', rollback: 'f-fil1' }] },
    ])
    expect(currentDocument(fake.app).featureStatus['f-fil1']?.status).toBe('rolled_back')
  })

  it('U-68 position 0 sends the id of the first feature', async () => {
    const fake = await cubeFeatures()
    await setRollback(fake.app, 0)
    await settle()
    expect(labeledEdits(fake)).toEqual([
      { label: 'Roll back', edits: [{ type: 'set_rollback', rollback: 'f-sk1' }] },
    ])
  })
})

describe('setSuppressed (3.9)', () => {
  it('suppresses with update_feature labeled "Suppress {label}"', async () => {
    const fake = await cubeFeatures()
    await setSuppressed(fake.app, ['f-fil1'], true)
    await settle()
    const [applied] = fake.transport.commandsOfType('apply_edits')
    expect(applied?.label).toBe('Suppress Fillet 1')
    expect(editsOf(applied)).toHaveLength(1)
    const [edit] = editsOf(applied)
    expect(edit?.type === 'update_feature' ? edit.feature : null).toMatchObject({
      id: 'f-fil1',
      type: 'fillet',
      suppressed: true,
      radius: '2 mm',
    })
    expect(currentDocument(fake.app).featureStatus['f-fil1']?.status).toBe('suppressed')
  })

  it('unsuppresses with update_feature labeled "Unsuppress {label}"', async () => {
    const fake = await startWithDocument(
      'cube-features.ptsm',
      withFeatureChanges(CUBE_FEATURES_DOCUMENT, 'f-fil1', { suppressed: true }),
    )
    await setSuppressed(fake.app, ['f-fil1'], false)
    await settle()
    const [applied] = fake.transport.commandsOfType('apply_edits')
    expect(applied?.label).toBe('Unsuppress Fillet 1')
    const [edit] = editsOf(applied)
    const feature = edit?.type === 'update_feature' ? edit.feature : null
    expect(feature?.id).toBe('f-fil1')
    expect(feature?.suppressed ?? false).toBe(false)
  })

  it('changes several features in one edit', async () => {
    const fake = await cubeFeatures()
    await setSuppressed(fake.app, ['f-fil1', 'f-sh1'], true)
    await settle()
    const applied = fake.transport.commandsOfType('apply_edits')
    expect(applied).toHaveLength(1)
    expect(
      editsOf(applied[0]).map((edit) =>
        edit.type === 'update_feature' ? edit.feature.id : edit.type,
      ),
    ).toEqual(['f-fil1', 'f-sh1'])
  })
})

describe('renameFeature (3.9)', () => {
  it('sets the name with update_feature labeled "Rename"', async () => {
    const fake = await startWithDocument('cube.ptsm', CUBE_DOCUMENT)
    await renameFeature(fake.app, 'f-ext1', 'Base')
    await settle()
    const [applied] = fake.transport.commandsOfType('apply_edits')
    expect(applied?.label).toBe('Rename')
    const [edit] = editsOf(applied)
    expect(edit?.type === 'update_feature' ? edit.feature : null).toMatchObject({
      id: 'f-ext1',
      type: 'extrude',
      name: 'Base',
      extent: { kind: 'distance', expr: '20 mm' },
    })
  })

  it('an empty name removes the name so the default label returns', async () => {
    const fake = await startWithDocument(
      'cube.ptsm',
      withFeatureChanges(CUBE_DOCUMENT, 'f-ext1', { name: 'Base' }),
    )
    await renameFeature(fake.app, 'f-ext1', '')
    await settle()
    const [applied] = fake.transport.commandsOfType('apply_edits')
    const [edit] = editsOf(applied)
    const feature = edit?.type === 'update_feature' ? edit.feature : null
    expect(feature?.id).toBe('f-ext1')
    expect(feature?.name ?? null).toBeNull()
  })
})

/** A drag of the marker of cube-features.ptsm (4 features, nothing rolled back) from the end. */
async function dragFromEnd(): Promise<{ fake: FakeApp; drag: RollbackDrag }> {
  const fake = await cubeFeatures()
  fake.transport.clearRequests()
  const drag = startRollbackDrag(fake.app, 4)
  if (drag === null) {
    throw new Error('The drag did not start.')
  }
  return { fake, drag }
}

describe('startRollbackDrag (3.9)', () => {
  it('begins "Roll back" and moves the model live with unlabeled set_rollback edits', async () => {
    const { fake, drag } = await dragFromEnd()
    drag.move(2)
    await settle()
    expect(fake.transport.commandsOfType('begin_transaction')).toEqual([
      expect.objectContaining({ type: 'begin_transaction', label: 'Roll back' }),
    ])
    expect(labeledEdits(fake)).toEqual([{ edits: [{ type: 'set_rollback', rollback: 'f-fil1' }] }])
    expect(currentDocument(fake.app).recipe.rollback).toBe('f-fil1')
    expect(currentDocument(fake.app).transactionOpen).toBe(true)
  })

  it('sends the newest gap after the one in flight, skipping the gaps in between', async () => {
    const { fake, drag } = await dragFromEnd()
    drag.move(3)
    drag.move(2)
    drag.move(1)
    await settle()
    expect(labeledEdits(fake)).toEqual([
      { edits: [{ type: 'set_rollback', rollback: 'f-sh1' }] },
      { edits: [{ type: 'set_rollback', rollback: 'f-ext1' }] },
    ])
  })

  it('dropping commits the transaction as one undo entry labeled "Roll back"', async () => {
    const { fake, drag } = await dragFromEnd()
    drag.move(1)
    await drag.drop()
    await settle()
    expect(fake.transport.commandTypes()).toEqual([
      'begin_transaction',
      'apply_edits',
      'commit_transaction',
    ])
    const document = currentDocument(fake.app)
    expect(document.recipe.rollback).toBe('f-ext1')
    expect(document.transactionOpen).toBe(false)
    expect(document.undoLabel).toBe('Roll back')
  })

  it('cancel (Esc) cancels the transaction and the marker returns', async () => {
    const { fake, drag } = await dragFromEnd()
    drag.move(1)
    await settle()
    await drag.cancel()
    await settle()
    expect(fake.transport.commandTypes()).toContain('cancel_transaction')
    expect(fake.transport.commandTypes()).not.toContain('commit_transaction')
    expect(currentDocument(fake.app).recipe.rollback ?? null).toBeNull()
    expect(currentDocument(fake.app).transactionOpen).toBe(false)
  })

  it('dropping at the starting gap cancels', async () => {
    const { fake, drag } = await dragFromEnd()
    drag.move(2)
    drag.move(4)
    await drag.drop()
    await settle()
    expect(fake.transport.commandTypes()).toContain('cancel_transaction')
    expect(fake.transport.commandTypes()).not.toContain('commit_transaction')
    expect(currentDocument(fake.app).recipe.rollback ?? null).toBeNull()
  })

  it('a move after the drop sends nothing', async () => {
    const { fake, drag } = await dragFromEnd()
    drag.move(1)
    await drag.drop()
    drag.move(2)
    await settle()
    expect(fake.transport.commandsOfType('apply_edits')).toHaveLength(1)
  })
})
