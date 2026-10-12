import { featureLabel } from '../../core/labels'
import type { Edit, FeatureRecord } from '../../generated/protocol'
import { MESSAGES } from '../../messages'
import type { App } from '../createApp'
import { coreUndo } from './document'
import { commitEdits, documentOf } from './edits'
import { notify } from './notifications'

// Timeline edits (ui.md 3.9): rollback marker, reorder, suppress, delete and rename.

function recordsOf(app: App): readonly FeatureRecord[] {
  return documentOf(app)?.recipe.features ?? []
}

function labelOf(app: App, featureId: string): string {
  const document = documentOf(app)
  return document === null
    ? featureId
    : featureLabel(document.recipe, featureId, app.store.getState().catalog)
}

/** The record with one property set, or removed when `value` is undefined. */
function withProperty(record: FeatureRecord, key: string, value: unknown): FeatureRecord {
  const copy: FeatureRecord = { ...record }
  if (value === undefined) {
    Reflect.deleteProperty(copy, key)
  } else {
    copy[key] = value
  }
  return copy
}

/** The set_rollback edit for marker position p: features 1 to p are active (3.9). */
function rollbackEdit(records: readonly FeatureRecord[], position: number): Edit {
  const first = records[position]
  return first === undefined
    ? { type: 'set_rollback' }
    : { type: 'set_rollback', rollback: first.id }
}

/** Moves the rollback marker to position 0 to N, labeled "Roll back". */
export async function setRollback(app: App, position: number): Promise<void> {
  await commitEdits(app, 'Roll back', [rollbackEdit(recordsOf(app), position)])
}

/** Moves a feature to a timeline index, labeled "Reorder"; its own index changes nothing. */
export async function moveFeature(app: App, featureId: string, toIndex: number): Promise<void> {
  const index = recordsOf(app).findIndex((record) => record.id === featureId)
  if (index === -1 || index === toIndex) {
    return
  }
  await commitEdits(app, 'Reorder', [{ type: 'move_feature', id: featureId, index: toIndex }])
}

/** Suppresses or unsuppresses features in one edit. */
export async function setSuppressed(
  app: App,
  featureIds: readonly string[],
  suppressed: boolean,
): Promise<void> {
  const chosen = new Set(featureIds)
  const records = recordsOf(app).filter((record) => chosen.has(record.id))
  const [first] = records
  if (first === undefined) {
    return
  }
  const verb = suppressed ? 'Suppress' : 'Unsuppress'
  const subject =
    records.length === 1 ? labelOf(app, first.id) : `${String(records.length)} features`
  await commitEdits(
    app,
    `${verb} ${subject}`,
    records.map((record) => ({
      type: 'update_feature',
      // Suppressed is omitted when false (model.md).
      feature: withProperty(record, 'suppressed', suppressed ? true : undefined),
    })),
  )
}

/**
 * Deletes features in one edit. When the rollback feature goes, the marker first moves to the
 * nearest later feature that stays, or to the end; "Deleted {label}." then offers Undo.
 */
export async function deleteFeatures(app: App, featureIds: readonly string[]): Promise<void> {
  const document = documentOf(app)
  if (document === null) {
    return
  }
  const doomed = new Set(featureIds)
  const records = document.recipe.features ?? []
  const deleted = records.filter((record) => doomed.has(record.id))
  const [first] = deleted
  if (first === undefined) {
    return
  }
  const edits: Edit[] = []
  const rollback = document.recipe.rollback ?? null
  if (rollback !== null && doomed.has(rollback)) {
    const rollbackIndex = records.findIndex((record) => record.id === rollback)
    const next = records.slice(rollbackIndex + 1).find((record) => !doomed.has(record.id))
    edits.push(
      next === undefined ? { type: 'set_rollback' } : { type: 'set_rollback', rollback: next.id },
    )
  }
  for (const record of deleted) {
    edits.push({ type: 'remove_feature', id: record.id })
  }
  const subject =
    deleted.length === 1 ? labelOf(app, first.id) : `${String(deleted.length)} features`
  if (!(await commitEdits(app, `Delete ${subject}`, edits))) {
    return
  }
  notify(app, {
    severity: 'info',
    message: MESSAGES.deleted(subject),
    actions: [
      {
        label: 'Undo',
        run: () => {
          void coreUndo(app)
        },
      },
    ],
  })
}

/** Renames a feature, labeled "Rename"; an empty name brings back the default label. */
export async function renameFeature(app: App, featureId: string, name: string): Promise<void> {
  const record = recordsOf(app).find((candidate) => candidate.id === featureId)
  if (record === undefined) {
    return
  }
  const trimmed = name.trim()
  await commitEdits(app, 'Rename', [
    {
      type: 'update_feature',
      feature: withProperty(record, 'name', trimmed === '' ? undefined : trimmed),
    },
  ])
}
