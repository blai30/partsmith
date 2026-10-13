import type { Body } from '../../generated/protocol'
import type { App } from '../createApp'
import { commitBuiltEdits, documentOf, refusedDuringCommand } from './edits'
import { withOptional } from './records'

// Body edits and session-only body view state (ui.md 3.8, 3.10).

/**
 * Commits one update_body made from the body as it is when the edit's turn in the mutation queue
 * comes, so a change queued before it (a rename, then a color) stays.
 */
async function updateBody(
  app: App,
  bodyId: string,
  change: (body: Body) => { readonly label: string; readonly body: Body },
): Promise<void> {
  const exists = documentOf(app)?.recipe.bodies?.some((candidate) => candidate.id === bodyId)
  if (exists !== true || refusedDuringCommand(app)) {
    return
  }
  await commitBuiltEdits(app, (document) => {
    const body = document.recipe.bodies?.find((candidate) => candidate.id === bodyId)
    if (body === undefined) {
      return null
    }
    const update = change(body)
    return { label: update.label, edits: [{ type: 'update_body', body: update.body }] }
  })
}

/** Shows or hides a body (undoable; `visible` is omitted when true). */
export async function setBodyVisible(app: App, bodyId: string, visible: boolean): Promise<void> {
  await updateBody(app, bodyId, (body) => ({
    label: `${visible ? 'Show' : 'Hide'} ${body.name}`,
    // Visible is omitted when true (model.md).
    body: withOptional(body, 'visible', visible ? undefined : false),
  }))
}

/** Sets a body's display color, or the default color for null. */
export async function setBodyColor(app: App, bodyId: string, color: string | null): Promise<void> {
  await updateBody(app, bodyId, (body) => ({
    label: 'Change color',
    body: withOptional(body, 'color', color ?? undefined),
  }))
}

/** Sets a body's material, or none for null or an empty name. */
export async function setBodyMaterial(
  app: App,
  bodyId: string,
  material: string | null,
): Promise<void> {
  const trimmed = material?.trim() ?? ''
  await updateBody(app, bodyId, (body) => ({
    label: 'Change material',
    body: withOptional(body, 'material', trimmed === '' ? undefined : trimmed),
  }))
}

export async function renameBody(app: App, bodyId: string, name: string): Promise<void> {
  const trimmed = name.trim()
  if (trimmed === '') {
    return
  }
  await updateBody(app, bodyId, (body) => ({ label: 'Rename', body: { ...body, name: trimmed } }))
}

/** Locks or unlocks a body for this session: locked bodies are drawn but not pickable. */
export function setLocked(app: App, bodyId: string, locked: boolean): void {
  app.store.setState((state) => {
    const others = state.view.locked.filter((id) => id !== bodyId)
    return { view: { ...state.view, locked: locked ? [...others, bodyId] : others } }
  })
}

/** Shows or hides a sketch, datum plane or origin item for this session, by selection key. */
export function setSessionVisible(app: App, key: string, visible: boolean): void {
  app.store.setState((state) => ({
    view: { ...state.view, sessionVisible: { ...state.view.sessionVisible, [key]: visible } },
  }))
}
