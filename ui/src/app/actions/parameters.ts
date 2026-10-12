import { generateId, usedIds } from '../../core/ids'
import type { Parameter } from '../../generated/protocol'
import type { App } from '../createApp'
import { commitEdits, documentOf, mutate, reportNewFailures, type RefusalOptions } from './edits'

// Parameter edits (ui.md 3.11). Each resolves whether the core accepted the change; with
// `onRefused`, the core's refusal goes to the caller (for the field) instead of a notification.

function parameterOf(app: App, id: string): Parameter | undefined {
  return documentOf(app)?.recipe.parameters?.find((parameter) => parameter.id === id)
}

/** Appends a parameter with a new `p-` id, labeled "Add parameter {name}". */
export async function addParameter(
  app: App,
  name: string,
  expression: string,
  options: RefusalOptions = {},
): Promise<boolean> {
  const document = documentOf(app)
  if (document === null) {
    return false
  }
  const taken = usedIds(document.recipe)
  const parameter: Parameter = { id: generateId('p', (id) => taken.has(id)), name, expr: expression }
  return commitEdits(
    app,
    `Add parameter ${name}`,
    [{ type: 'add_parameter', parameter, index: document.recipe.parameters?.length ?? 0 }],
    options,
  )
}

/** Renames a parameter; the core rewrites every expression that uses the old name. */
export async function renameParameter(
  app: App,
  id: string,
  name: string,
  options: RefusalOptions = {},
): Promise<boolean> {
  const before = documentOf(app)?.featureStatus ?? {}
  const revision = await mutate(
    app,
    (document) => app.core.renameParameter(document.documentHandle, id, name),
    options,
  )
  if (revision === null) {
    return false
  }
  reportNewFailures(app, before)
  return true
}

/** Changes a parameter's expression ("Change {name}") or comment ("Change comment of {name}"; empty removes it). */
export async function updateParameter(
  app: App,
  id: string,
  change: { expression?: string; comment?: string | null },
  options: RefusalOptions = {},
): Promise<boolean> {
  const parameter = parameterOf(app, id)
  if (parameter === undefined) {
    return false
  }
  const updated: Parameter = { ...parameter }
  if (change.expression !== undefined) {
    updated.expr = change.expression
  }
  if (change.comment !== undefined) {
    const comment = change.comment?.trim() ?? ''
    if (comment === '') {
      Reflect.deleteProperty(updated, 'comment')
    } else {
      updated.comment = comment
    }
  }
  const label =
    change.expression === undefined
      ? `Change comment of ${parameter.name}`
      : `Change ${parameter.name}`
  return commitEdits(app, label, [{ type: 'update_parameter', parameter: updated }], options)
}

/** Deletes a parameter, labeled "Delete parameter {name}". */
export async function deleteParameter(app: App, id: string): Promise<boolean> {
  const parameter = parameterOf(app, id)
  if (parameter === undefined) {
    return false
  }
  return commitEdits(app, `Delete parameter ${parameter.name}`, [{ type: 'remove_parameter', id }])
}
