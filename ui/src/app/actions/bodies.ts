import type { Body } from '../../generated/protocol'
import type { App } from '../createApp'
import { commitEdits, documentOf } from './edits'

// Body edits and session-only body view state (ui.md 3.8, 3.10).

type OptionalBodyProperty = 'material' | 'color' | 'visible'

/** The body with one optional property set, or removed for null. */
function withProperty<Key extends OptionalBodyProperty>(
  body: Body,
  key: Key,
  value: Body[Key] | null,
): Body {
  const copy: Body = { ...body }
  if (value === null) {
    Reflect.deleteProperty(copy, key)
  } else {
    copy[key] = value
  }
  return copy
}

/** Commits one update_body made from the current body. */
async function updateBody(
  app: App,
  bodyId: string,
  change: (body: Body) => { readonly label: string; readonly body: Body },
): Promise<void> {
  const body = documentOf(app)?.recipe.bodies?.find((candidate) => candidate.id === bodyId)
  if (body === undefined) {
    return
  }
  const update = change(body)
  await commitEdits(app, update.label, [{ type: 'update_body', body: update.body }])
}

/** Shows or hides a body (undoable; `visible` is omitted when true). */
export async function setBodyVisible(app: App, bodyId: string, visible: boolean): Promise<void> {
  await updateBody(app, bodyId, (body) => ({
    label: `${visible ? 'Show' : 'Hide'} ${body.name}`,
    // Visible is omitted when true (model.md).
    body: withProperty(body, 'visible', visible ? null : false),
  }))
}

/** Sets a body's display color, or the default color for null. */
export async function setBodyColor(app: App, bodyId: string, color: string | null): Promise<void> {
  await updateBody(app, bodyId, (body) => ({
    label: 'Change color',
    body: withProperty(body, 'color', color),
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
    body: withProperty(body, 'material', trimmed === '' ? null : trimmed),
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
