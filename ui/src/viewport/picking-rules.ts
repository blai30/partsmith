import type { SelectionKind } from '../generated/protocol'
import { buildInspectorModel, getParam } from '../inspector/model'
import { itemsFromValue } from '../inspector/selection-values'
import type { AppState, SelectionItem } from '../state/store'
import type { SelectionItemKind } from './picking'

// What the viewport lets the pointer pick and which items it draws selected or hovered (ui.md 3.6,
// 3.7): the active selection field, the sketch session or the selection filter decide.

/** The item kinds each selection field kind takes (2.12 table); faces, edges and vertices of a body field pick their body. */
const FIELD_ACCEPTS: Readonly<Record<SelectionKind, ReadonlySet<SelectionItemKind>>> = {
  body: new Set(['body', 'face', 'edge', 'vertex']),
  sketch: new Set(['sketch_curve', 'sketch_region']),
  sketch_region: new Set(['sketch_region']),
  sketch_point: new Set(['sketch_point']),
  face: new Set(['face']),
  edge: new Set(['edge']),
  plane: new Set(['origin_plane', 'datum_plane', 'face']),
  axis: new Set(['origin_axis', 'sketch_curve', 'edge', 'face']),
}

const PLANE_PICKING: ReadonlySet<SelectionItemKind> = new Set([
  'origin_plane',
  'datum_plane',
  'face',
])

const ACTIVE_SKETCH_KINDS: ReadonlySet<SelectionItemKind> = new Set([
  'sketch_curve',
  'sketch_point',
  'sketch_region',
])

/** What may be picked now (3.6). */
export type Picking = {
  readonly accepts: (kind: SelectionItemKind) => boolean
  /** A face pick stands for its body (Bodies on, Faces off). */
  readonly facesPickBodies: boolean
}

/** The selection kind of the active command's active field, or null. */
export function activeFieldKind(state: AppState): SelectionKind | null {
  const command = state.command
  if (command === null || command.activeField === null) {
    return null
  }
  const hint = state.catalog?.byType[command.featureType]?.manifest.fields.find(
    (candidate) => candidate.path === command.activeField,
  )
  return hint?.selection ?? null
}

/**
 * Whether the active field also takes faces: a point field places holes on a flat face, and a
 * multiple edge field takes a face as all of its edges (3.7).
 */
function activeFieldTakesFaces(state: AppState): boolean {
  const command = state.command
  const kind = activeFieldKind(state)
  if (command === null || (kind !== 'sketch_point' && kind !== 'edge')) {
    return false
  }
  if (kind === 'sketch_point') {
    return true
  }
  return selectionFieldAt(state, command.activeField)?.multiple === true
}

/** What may be picked for the state (3.6). */
export function pickingFor(state: AppState): Picking {
  if (state.command !== null) {
    const field = activeFieldKind(state)
    const kinds = new Set<SelectionItemKind>(field === null ? [] : FIELD_ACCEPTS[field])
    if (activeFieldTakesFaces(state)) {
      kinds.add('face')
    }
    return { accepts: (kind) => kinds.has(kind), facesPickBodies: false }
  }
  if (state.context === 'sketch' && state.sketch !== null) {
    const kinds = state.sketch.sketchId === null ? PLANE_PICKING : ACTIVE_SKETCH_KINDS
    return { accepts: (kind) => kinds.has(kind), facesPickBodies: false }
  }
  // Lay on face takes faces only; whether one is flat is known once it is picked (3.6, 3.17).
  if (state.context === 'print' && state.print.pickingFace) {
    return { accepts: (kind) => kind === 'face', facesPickBodies: false }
  }
  const filter = state.view.filter
  const facesPickBodies = filter.body && !filter.face
  return {
    accepts: (kind) => {
      switch (kind) {
        case 'face':
          return filter.face || facesPickBodies
        case 'edge':
          return filter.edge
        case 'vertex':
          return filter.vertex
        case 'body':
          return filter.body
        default:
          return true
      }
    },
    facesPickBodies,
  }
}

/** A selection field of the active command with its value and the object that holds it. */
export type CommandSelectionField = {
  readonly path: string
  readonly accepts: SelectionKind
  readonly multiple: boolean
  readonly value: unknown
  /** The params object that holds the field, whose `body` or `sketch` qualifies its value. */
  readonly siblings: Readonly<Record<string, unknown>>
}

function isRecord(value: unknown): value is Readonly<Record<string, unknown>> {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
}

/** The selection fields of the active feature command, in manifest order. */
export function commandSelectionFields(state: AppState): readonly CommandSelectionField[] {
  const command = state.command
  const feature = command === null ? undefined : state.catalog?.byType[command.featureType]
  if (command === null || feature === undefined) {
    return []
  }
  const model = buildInspectorModel({ feature, params: command.params, errors: {} })
  return [...model.essential, ...model.more].flatMap((field) => {
    if (field.control.type !== 'selection') {
      return []
    }
    const lastDot = field.path.lastIndexOf('.')
    const parent =
      lastDot === -1 ? command.params : getParam(command.params, field.path.slice(0, lastDot))
    return [
      {
        path: field.path,
        accepts: field.control.accepts,
        multiple: field.control.multiple,
        value: field.value,
        siblings: isRecord(parent) ? parent : {},
      },
    ]
  })
}

/** The active command's selection field at a path, or undefined (always for a null path). */
export function selectionFieldAt(
  state: AppState,
  path: string | null,
): CommandSelectionField | undefined {
  return path === null
    ? undefined
    : commandSelectionFields(state).find((candidate) => candidate.path === path)
}

/** The selection items whose state the viewport shows now: the command's field values, the sketch selection or the app selection. */
export function shownSelection(state: AppState): readonly SelectionItem[] {
  if (state.command !== null) {
    return commandSelectionFields(state).flatMap((field) =>
      itemsFromValue(field.accepts, field.value, state, field.siblings),
    )
  }
  if (state.context === 'sketch' && state.sketch !== null) {
    return state.sketch.selection
  }
  return state.selection
}

/**
 * The keys drawn hovered: the pointer's hover target, and the items of the selection field whose
 * box the pointer is over (3.7).
 */
export function hoveredKeys(state: AppState): ReadonlySet<string> {
  const keys = new Set<string>()
  if (state.hover !== null) {
    keys.add(state.hover.key)
  }
  const field = selectionFieldAt(state, state.command?.hoveredField ?? null)
  if (field !== undefined) {
    for (const item of itemsFromValue(field.accepts, field.value, state, field.siblings)) {
      keys.add(item.key)
    }
  }
  return keys
}
