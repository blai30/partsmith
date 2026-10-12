import { describe, expect, it } from 'vitest'
import type { Document } from '../../generated/protocol'
import { MESSAGES } from '../../messages'
import {
  actionLabels,
  currentDocument,
  notificationWith,
  runAction,
  startWithDocument,
  type FakeApp,
} from '../../test/fakes/app'
import { CUBE_DOCUMENT, CUBE_FEATURES_DOCUMENT } from '../../test/fakes/documents'
import { withoutNulls } from '../../test/fakes/protocol'
import { settle } from '../../test/fakes/settle'
import {
  deleteFeatures,
  moveFeature,
  renameFeature,
  setRollback,
  setSuppressed,
} from './timeline'

/** The apply_edits requests as { label, edits }, null fields removed. */
function appliedEdits(fake: FakeApp): unknown[] {
  return fake.transport.commandsOfType('apply_edits').map((command) =>
    withoutNulls({ label: command.label, edits: command.edits }),
  )
}

async function cubeFeatures(recipe: Partial<Document> = {}): Promise<FakeApp> {
  return startWithDocument('cube-features.ptsm', { ...CUBE_FEATURES_DOCUMENT, ...recipe })
}

describe('deleteFeatures (3.9)', () => {
  it('U-68 deleting the rollback feature first moves the marker to the nearest later remaining feature', async () => {
    const fake = await cubeFeatures({ rollback: 'f-fil1' })
    await deleteFeatures(fake.app, ['f-fil1'])
    await settle()
    expect(appliedEdits(fake)).toEqual([
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
    expect(appliedEdits(fake)).toEqual([
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
    expect(appliedEdits(fake)).toEqual([
      { label: 'Delete Shell 1', edits: [{ type: 'remove_feature', id: 'f-sh1' }] },
    ])
    expect(currentDocument(fake.app).recipe.features.map((feature) => feature.id)).toEqual([
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
    expect(currentDocument(fake.app).recipe.features.map((feature) => feature.id)).toContain('f-sh1')
  })
})

describe('moveFeature (3.9)', () => {
  it('U-68 moving the feature at index 3 to index 1 sends move_feature labeled "Reorder"', async () => {
    const fake = await cubeFeatures()
    await moveFeature(fake.app, 'f-sh1', 1)
    await settle()
    expect(appliedEdits(fake)).toEqual([
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
    expect(appliedEdits(fake)).toEqual([{ label: 'Roll back', edits: [{ type: 'set_rollback' }] }])
  })

  it('U-68 position 2 sends the id of the third feature', async () => {
    const fake = await cubeFeatures()
    await setRollback(fake.app, 2)
    await settle()
    expect(appliedEdits(fake)).toEqual([
      { label: 'Roll back', edits: [{ type: 'set_rollback', rollback: 'f-fil1' }] },
    ])
    expect(currentDocument(fake.app).featureStatus['f-fil1']?.status).toBe('rolled_back')
  })

  it('U-68 position 0 sends the id of the first feature', async () => {
    const fake = await cubeFeatures()
    await setRollback(fake.app, 0)
    await settle()
    expect(appliedEdits(fake)).toEqual([
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
    expect(applied?.edits).toHaveLength(1)
    const [edit] = applied?.edits ?? []
    expect(edit?.type === 'update_feature' ? edit.feature : null).toMatchObject({
      id: 'f-fil1',
      type: 'fillet',
      suppressed: true,
      radius: '2 mm',
    })
    expect(currentDocument(fake.app).featureStatus['f-fil1']?.status).toBe('suppressed')
  })

  it('unsuppresses with update_feature labeled "Unsuppress {label}"', async () => {
    const fake = await cubeFeatures({
      features: CUBE_FEATURES_DOCUMENT.features.map((feature) =>
        feature.id === 'f-fil1' ? { ...feature, suppressed: true } : feature,
      ),
    })
    await setSuppressed(fake.app, ['f-fil1'], false)
    await settle()
    const [applied] = fake.transport.commandsOfType('apply_edits')
    expect(applied?.label).toBe('Unsuppress Fillet 1')
    const [edit] = applied?.edits ?? []
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
    expect(applied[0]?.edits.map((edit) => (edit.type === 'update_feature' ? edit.feature.id : edit.type))).toEqual(
      ['f-fil1', 'f-sh1'],
    )
  })
})

describe('renameFeature (3.9)', () => {
  it('sets the name with update_feature labeled "Rename"', async () => {
    const fake = await startWithDocument('cube.ptsm', CUBE_DOCUMENT)
    await renameFeature(fake.app, 'f-ext1', 'Base')
    await settle()
    const [applied] = fake.transport.commandsOfType('apply_edits')
    expect(applied?.label).toBe('Rename')
    const [edit] = applied?.edits ?? []
    expect(edit?.type === 'update_feature' ? edit.feature : null).toMatchObject({
      id: 'f-ext1',
      type: 'extrude',
      name: 'Base',
      extent: { kind: 'distance', expr: '20 mm' },
    })
  })

  it('an empty name removes the name so the default label returns', async () => {
    const fake = await startWithDocument('cube.ptsm', {
      ...CUBE_DOCUMENT,
      features: CUBE_DOCUMENT.features.map((feature) =>
        feature.id === 'f-ext1' ? { ...feature, name: 'Base' } : feature,
      ),
    })
    await renameFeature(fake.app, 'f-ext1', '')
    await settle()
    const [applied] = fake.transport.commandsOfType('apply_edits')
    const [edit] = applied?.edits ?? []
    const feature = edit?.type === 'update_feature' ? edit.feature : null
    expect(feature?.id).toBe('f-ext1')
    expect(feature?.name ?? null).toBeNull()
  })
})
