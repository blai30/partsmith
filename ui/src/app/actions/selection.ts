import { CoreError } from '../../core/client'
import type { ElementDescriptor } from '../../generated/protocol'
import type { AppStore, HoverTarget, SelectionItem } from '../../state/store'
import type { PickResult, PointerRay } from '../../viewport/picking'
import type { App } from '../createApp'
import { capturePick, commandLasts, confirmRunning, confirmSettled } from './command'
import { showCoreError } from './edits'

// The app selection and hover (ui.md 3.6). Clicks on the model and selection changes land in the
// order they were made: a click holds its place from the moment it is made until the core has
// answered its pick and it has landed, and what changes the selection meanwhile lands after it.

const ELEMENT_NOUNS = { face: 'Face', edge: 'Edge', vertex: 'Vertex' } as const

/** Pick errors that mean the click hit geometry that is gone or not pickable: ignored silently. */
const IGNORED_PICK_ERRORS: ReadonlySet<string> = new Set(['stale_pick', 'preview_not_pickable'])

/** The clicks on the model and selection changes of one store on their way (3.6). */
type InputOrder = {
  /** Settles once every place held so far has been left. */
  landed: Promise<void>
  /** The places held and not left yet. */
  held: number
}

const inputOrders = new WeakMap<AppStore, InputOrder>()

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

/**
 * Holds a place in the order input lands, from now until `leave` is called: input made meanwhile
 * lands after it. `turn` resolves once the input made before it has landed.
 */
function holdPlace(app: App): { readonly turn: Promise<void>; readonly leave: () => void } {
  let order = inputOrders.get(app.store)
  if (order === undefined) {
    order = { landed: Promise.resolve(), held: 0 }
    inputOrders.set(app.store, order)
  }
  const current = order
  const turn = current.landed
  let release: (() => void) | undefined
  const left = new Promise<void>((resolve) => {
    release = resolve
  })
  current.landed = turn.then(() => left)
  current.held += 1
  let held = true
  return {
    turn,
    leave: () => {
      if (held) {
        held = false
        current.held -= 1
        release?.()
      }
    },
  }
}

/** Whether a click on the model or a selection change is on its way, which what comes next waits for. */
export function inputOnItsWay(app: App): boolean {
  return (inputOrders.get(app.store)?.held ?? 0) > 0
}

/**
 * Resolves once every click on the model and selection change made so far has landed, however
 * late the core answered, so what reads the selection then reads what they left (3.6).
 */
export function inputLanded(app: App): Promise<void> {
  return inputOrders.get(app.store)?.landed ?? Promise.resolve()
}

/**
 * Lands input in the order it was made (3.6): `land` runs at once when no click on the model or
 * selection change is on its way and there is no `first` to wait for, else once those have landed
 * and `first` has settled. The input holds its place from now until `land` has settled, so what is
 * made meanwhile lands after it. `land` changes the selection with `setSelection` or `landPick`,
 * which act at once, since its turn has come.
 */
export function landInTurn(
  app: App,
  land: () => void | Promise<void>,
  first?: Promise<void>,
): Promise<void> {
  if (first === undefined && !inputOnItsWay(app)) {
    const landing = land()
    if (landing === undefined) {
      return Promise.resolve()
    }
    // What is made while this lands waits for it.
    const place = holdPlace(app)
    return landing.finally(place.leave)
  }
  const place = holdPlace(app)
  return Promise.all([place.turn, first]).then(land).finally(place.leave)
}

/**
 * Replaces or toggles the selection at once. Only a landing whose turn has come (`landInTurn`)
 * calls it; everything else selects with `select`, which waits for the input on its way.
 */
export function setSelection(
  app: App,
  items: readonly SelectionItem[],
  mode: 'replace' | 'toggle',
): void {
  app.store.setState((state) => {
    const selection = mode === 'replace' ? [...items] : toggledByKey(state.selection, items)
    return selection.length === 0 && state.selection.length === 0 ? {} : { selection }
  })
}

/** Replaces or toggles the selection once the clicks and selection changes on their way have landed (3.6). */
export function select(
  app: App,
  items: readonly SelectionItem[],
  mode: 'replace' | 'toggle',
): void {
  void landInTurn(app, () => {
    setSelection(app, items, mode)
  })
}

/** Empties the selection once the clicks and selection changes on their way have landed (3.6). */
export function clearSelection(app: App): void {
  select(app, [], 'replace')
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

/**
 * The selection item a pick means; mesh picks are converted by the core (3.6). Null when the core
 * refuses the pick (`stale_pick` and `preview_not_pickable` silently, any other error shown) or
 * the core stopped.
 */
export async function pickedItem(app: App, sample: PickResult): Promise<SelectionItem | null> {
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

/** Lands a picked item where a click goes now: the active feature command, else the selection. */
async function landItem(
  app: App,
  item: SelectionItem | null,
  modifiers: { readonly toggle: boolean },
  pointer: PointerRay | undefined,
): Promise<void> {
  if (item === null) {
    return
  }
  if (app.store.getState().command !== null) {
    await capturePick(app, item, pointer)
    return
  }
  setSelection(app, [item], modifiers.toggle ? 'toggle' : 'replace')
}

/**
 * A click on a pick target: selects it (toggles with a modifier), or feeds an active feature
 * command. `pointer` is the click's ray, which a face picked for a hole center needs (3.7). The
 * core is asked at once, and the click lands in the order it was made (3.6): a click made before
 * OK lands in the command first, and OK waits for it; one made into a command that a cancel ends
 * before the core answers lands nothing, as it is canceled with the command; one made while OK
 * runs goes where a click goes once OK has settled.
 */
export function pickAt(
  app: App,
  sample: PickResult,
  modifiers: { readonly toggle: boolean },
  pointer?: PointerRay,
): Promise<void> {
  const runningConfirm = confirmRunning(app) ? confirmSettled(app) : undefined
  const lasts = runningConfirm === undefined ? commandLasts(app) : () => true
  const picking = pickedItem(app, sample)
  return landInTurn(
    app,
    async () => {
      const item = await picking
      if (lasts()) {
        await landItem(app, item, modifiers, pointer)
      }
    },
    runningConfirm,
  )
}

/**
 * A click whose turn has come (`landInTurn`), such as one that waited for a command's end (3.6):
 * the core is asked now, and the click lands as `pickAt`'s does.
 */
export async function landPick(
  app: App,
  sample: PickResult,
  modifiers: { readonly toggle: boolean },
  pointer?: PointerRay,
): Promise<void> {
  await landItem(app, await pickedItem(app, sample), modifiers, pointer)
}
