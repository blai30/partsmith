import { CoreError } from '../../core/client'
import type { ElementDescriptor } from '../../generated/protocol'
import type { HoverTarget, SelectionItem } from '../../state/store'
import type { PickResult } from '../../viewport/picking'
import type { App } from '../createApp'
import { capturePick } from './command'
import { showCoreError } from './edits'

// The app selection and hover (ui.md 3.6).

const ELEMENT_NOUNS = { face: 'Face', edge: 'Edge', vertex: 'Vertex' } as const

/** Pick errors that mean the click hit geometry that is gone or not pickable: ignored silently. */
const IGNORED_PICK_ERRORS: ReadonlySet<string> = new Set(['stale_pick', 'preview_not_pickable'])

export function select(
  app: App,
  items: readonly SelectionItem[],
  mode: 'replace' | 'toggle',
): void {
  app.store.setState((state) => {
    if (mode === 'replace') {
      return { selection: [...items] }
    }
    let selection = [...state.selection]
    for (const item of items) {
      selection = selection.some((selected) => selected.key === item.key)
        ? selection.filter((selected) => selected.key !== item.key)
        : [...selection, item]
    }
    return { selection }
  })
}

export function clearSelection(app: App): void {
  app.store.setState((state) => (state.selection.length === 0 ? {} : { selection: [] }))
}

export function setHover(app: App, target: HoverTarget | null): void {
  app.store.setState((state) =>
    state.hover?.key === target?.key && state.hover?.kind === target?.kind ? {} : { hover: target },
  )
}

/** The selection item of a face, edge or vertex, keyed by its element name (3.6). */
export function elementSelectionItem(app: App, element: ElementDescriptor): SelectionItem {
  const body = app.store
    .getState()
    .document?.recipe.bodies?.find((candidate) => candidate.id === element.body)
  return {
    key: `element:${element.body}:${element.name}`,
    kind: element.kind,
    label: `${ELEMENT_NOUNS[element.kind]} of ${body?.name ?? element.body}`,
    body: element.body,
    feature: element.feature,
    element,
    sketchItem: null,
    origin: null,
  }
}

/** The selection item a pick means; mesh picks are converted by the core. Null when the pick is stale. */
async function itemFor(app: App, sample: PickResult): Promise<SelectionItem | null> {
  if (sample.kind === 'item') {
    return sample.item
  }
  const document = app.store.getState().document
  if (document === null) {
    return null
  }
  try {
    const element = await app.core.pickReference(document.documentHandle, {
      mesh_id: sample.meshId,
      kind: sample.element,
      index: sample.index,
    })
    return elementSelectionItem(app, element)
  } catch (error) {
    if (error instanceof CoreError && IGNORED_PICK_ERRORS.has(error.code)) {
      return null
    }
    showCoreError(app, error)
    return null
  }
}

/** A click on a pick target: selects it (toggles with a modifier), or feeds an active feature command. */
export async function pickAt(
  app: App,
  sample: PickResult,
  modifiers: { readonly toggle: boolean },
): Promise<void> {
  const item = await itemFor(app, sample)
  if (item === null) {
    return
  }
  if (app.store.getState().command !== null) {
    await capturePick(app, item)
    return
  }
  select(app, [item], modifiers.toggle ? 'toggle' : 'replace')
}
