import { generateId, usedIds } from '../../core/ids'
import type { Parameter } from '../../generated/protocol'
import type { DocumentState } from '../../state/store'
import type { App } from '../createApp'
import { runInCommand } from './command'
import {
  commitBuiltEdits,
  documentOf,
  runChange,
  type EditsBuilder,
  type RefusalOptions,
} from './edits'
import { withOptional } from './records'

// Parameter edits (ui.md 3.11). Each resolves whether the core accepted the change; with
// `onRefused`, the core's refusal goes to the caller (for the field) instead of a notification.
// During a feature command they join its transaction (3.7): no label of their own, OK records
// them with the feature and Cancel reverts them.

/** Whether a feature command's transaction is open, which parameter edits join. */
function inCommand(app: App): boolean {
  return app.store.getState().command !== null
}

/**
 * Applies parameter edits, built from the document when their turn in the mutation queue comes:
 * inside the open command's transaction, else as one labeled undo entry.
 */
function commitParameterEdits(
  app: App,
  build: EditsBuilder,
  options: RefusalOptions,
): Promise<boolean> {
  if (!inCommand(app)) {
    return commitBuiltEdits(app, build, options)
  }
  return runInCommand(
    app,
    async (documentHandle) => {
      const document = documentOf(app)
      const built = document === null ? null : build(document)
      if (built !== null) {
        await app.core.applyEdits(documentHandle, null, built.edits)
      }
    },
    options.onRefused,
  )
}

function parameterIn(document: DocumentState, id: string): Parameter | undefined {
  return document.recipe.parameters?.find((parameter) => parameter.id === id)
}

function parameterOf(app: App, id: string): Parameter | undefined {
  const document = documentOf(app)
  return document === null ? undefined : parameterIn(document, id)
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
  const parameter: Parameter = {
    id: generateId('p', (id) => taken.has(id)),
    name,
    expr: expression,
  }
  return commitParameterEdits(
    app,
    (current) => ({
      label: `Add parameter ${name}`,
      // At the end of the list as it is then, behind parameters added just before.
      edits: [{ type: 'add_parameter', parameter, index: current.recipe.parameters?.length ?? 0 }],
    }),
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
  if (inCommand(app)) {
    return runInCommand(
      app,
      (documentHandle) => app.core.renameParameter(documentHandle, id, name),
      options.onRefused,
    )
  }
  const { sent } = await runChange(
    app,
    async (document) => {
      await app.core.renameParameter(document.documentHandle, id, name)
      return true
    },
    options,
  )
  return sent
}

/** Changes a parameter's expression ("Change {name}") or comment ("Change comment of {name}"; empty removes it). */
export async function updateParameter(
  app: App,
  id: string,
  change: { expression?: string; comment?: string | null },
  options: RefusalOptions = {},
): Promise<boolean> {
  if (parameterOf(app, id) === undefined) {
    return false
  }
  return commitParameterEdits(
    app,
    (document) => {
      // Only the changed property is set on the parameter as it is then, so a rename or another
      // change queued before this one stays.
      const parameter = parameterIn(document, id)
      if (parameter === undefined) {
        return null
      }
      let updated: Parameter = { ...parameter }
      if (change.expression !== undefined) {
        updated.expr = change.expression
      }
      if (change.comment !== undefined) {
        const comment = change.comment?.trim() ?? ''
        updated = withOptional(updated, 'comment', comment === '' ? undefined : comment)
      }
      return {
        label:
          change.expression === undefined
            ? `Change comment of ${parameter.name}`
            : `Change ${parameter.name}`,
        edits: [{ type: 'update_parameter', parameter: updated }],
      }
    },
    options,
  )
}

/** Deletes a parameter, labeled "Delete parameter {name}". */
export async function deleteParameter(app: App, id: string): Promise<boolean> {
  const parameter = parameterOf(app, id)
  if (parameter === undefined) {
    return false
  }
  return commitParameterEdits(
    app,
    () => ({
      label: `Delete parameter ${parameter.name}`,
      edits: [{ type: 'remove_parameter', id }],
    }),
    {},
  )
}
