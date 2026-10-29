import { CoreError } from '../../core/client'
import type { ElementDescriptor } from '../../generated/protocol'
import type { AppStore, HoverTarget, SelectionItem } from '../../state/store'
import type { PickResult, PointerRay } from '../../viewport/picking'
import type { App } from '../createApp'
import { capturePick, confirmRunning, confirmSettled } from './command'
import { showCoreError } from './edits'

// The app selection and hover (ui.md 3.6).

const ELEMENT_NOUNS = { face: 'Face', edge: 'Edge', vertex: 'Vertex' } as const

/** Pick errors that mean the click hit geometry that is gone or not pickable: ignored silently. */
const IGNORED_PICK_ERRORS: ReadonlySet<string> = new Set(['stale_pick', 'preview_not_pickable'])

/** Each store's clicks on the model, settled once every one has landed where it goes. */
const landingClicks = new WeakMap<AppStore, Promise<void>>()

/**
 * The selection with each item toggled by key: removed when it is selected, else added after the
 * items already selected. A selection holds each key once.
 */
export function toggledByKey(
  selection: readonly SelectionItem[],
  items: readonly SelectionItem[],
): SelectionItem[] {
  // Keys keep insertion order, so toggled-on items follow the ones already selected.
  const byKey = new Map(selection.map((item) => [item.key, item]))
  for (const item of items) {
    if (!byKey.delete(item.key)) {
      byKey.set(item.key, item)
    }
  }
  return [...byKey.values()]
}

export function select(
  app: App,
  items: readonly SelectionItem[],
  mode: 'replace' | 'toggle',
): void {
  app.store.setState((state) => ({
    selection: mode === 'replace' ? [...items] : toggledByKey(state.selection, items),
  }))
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

/**
 * Resolves once every click on the model made so far has landed in the selection or the command,
 * however late the core answered it, so what reads the selection next sees it (3.16.1).
 */
export function clicksLanded(app: App): Promise<void> {
  return landingClicks.get(app.store) ?? Promise.resolve()
}

/**
 * A click on a pick target: selects it (toggles with a modifier), or feeds an active feature
 * command. `pointer` is the click's ray, which a face picked for a hole center needs (3.7). A
 * click made before OK lands in the command first, and OK waits for it; one made while OK runs
 * goes where a click goes once OK has settled.
 */
export function pickAt(
  app: App,
  sample: PickResult,
  modifiers: { readonly toggle: boolean },
  pointer?: PointerRay,
): Promise<void> {
  const runningConfirm = confirmRunning(app) ? confirmSettled(app) : null
  const landing = landClick(app, sample, modifiers, runningConfirm, pointer)
  const earlier = clicksLanded(app)
  landingClicks.set(
    app.store,
    Promise.all([earlier, landing.catch(() => undefined)]).then(() => undefined),
  )
  return landing
}

async function landClick(
  app: App,
  sample: PickResult,
  modifiers: { readonly toggle: boolean },
  runningConfirm: Promise<void> | null,
  pointer?: PointerRay,
): Promise<void> {
  const item = await itemFor(app, sample)
  if (item === null) {
    return
  }
  if (runningConfirm !== null) {
    await runningConfirm
  }
  if (app.store.getState().command !== null) {
    await capturePick(app, item, pointer)
    return
  }
  select(app, [item], modifiers.toggle ? 'toggle' : 'replace')
}
