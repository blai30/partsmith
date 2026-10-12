import type { BufferView } from '../../core/frame'
import {
  PROTOCOL_VERSION,
  type BoundingBox,
  type CatalogEntry,
  type Command,
  type CommandResult,
  type Diagnostic,
  type Dimension,
  type Document,
  type Edit,
  type ElementDescriptor,
  type Event as ProtocolEvent,
  type FeatureState,
  type FeatureStatus,
  type ParameterValue,
  type PrinterProfile,
  type Repair,
  type Resolution,
  type SketchSolution,
} from '../../generated/protocol'
import { TEST_CATALOG_ENTRIES } from './catalog'
import {
  bodiesOf,
  documentText,
  emptyDocument,
  featuresOf,
  parametersOf,
  type FeatureRecord,
} from './documents'
import { TEST_PRINTERS } from './printers'
import { meshParts, type ErrorReply } from './protocol'

// An in-memory engine that answers protocol commands the way engine.md describes, closely enough for
// UI tests: documents with revisions, labeled edits, transactions with a focus, undo and redo, the dirty
// baseline of save and mark_saved, and the events every change emits. Geometry is not computed: a body
// has geometry when an active feature names it as its `body`, and its mesh is a single triangle.

type CommandOfType<Type extends Command['type']> = Extract<Command, { type: Type }>

/** What the fake core answers to one request. */
export type FakeReply =
  | {
      readonly result: CommandResult
      readonly events?: readonly ProtocolEvent[]
      readonly buffers?: readonly BufferView[]
    }
  | { readonly error: ErrorReply; readonly events?: readonly ProtocolEvent[] }

/** A feature status other than ok that the fake core reports. */
export type FakeFeatureStatus = {
  readonly status: FeatureState
  readonly messages: readonly Diagnostic[]
  /** References that did not resolve; none when absent. */
  readonly repairs?: readonly Repair[]
}

export type FakeCoreOptions = {
  /** `engine_info.engine_version`; default '0.1.0'. */
  readonly engineVersion?: string
  /** `engine_info.protocol_version`; default the generated `PROTOCOL_VERSION`. */
  readonly protocolVersion?: number
  /** The `feature_catalog` entries; default the test catalog. */
  readonly catalog?: readonly CatalogEntry[]
  /** The `list_printer_profiles` profiles; default the two built-in profiles. */
  readonly printers?: readonly PrinterProfile[]
  /** Solutions by sketch feature id, reported in `sketch_solved` and answered to `solve_sketch`. */
  readonly solutions?: Readonly<Record<string, SketchSolution>>
  /** Statuses by feature id for active, unsuppressed features; the others are ok. */
  readonly statuses?: Readonly<Record<string, FakeFeatureStatus>>
  /** Bounding boxes of bodies with geometry; default -10 to 10 in x and y and 0 to 20 in z. */
  readonly bounds?: Readonly<Record<string, BoundingBox>>
  /** `list_elements` answers by body id. */
  readonly elements?: Readonly<Record<string, readonly ElementDescriptor[]>>
}

/** The state of one open document in the fake core. */
export type FakeDocumentState = {
  readonly handle: number
  readonly recipe: Document
  readonly revision: number
  readonly dirty: boolean
  /** Labels on the undo stack, oldest first. */
  readonly undoLabels: readonly string[]
  readonly redoLabels: readonly string[]
  /** Label of the open transaction, or null. */
  readonly transaction: string | null
  readonly focus: string | null
  /** Model-layer mesh id by body id, for bodies with geometry. */
  readonly meshIds: Readonly<Record<string, number>>
}

export type FakeCore = {
  readonly options: FakeCoreOptions
  /** Handles of the open documents, in opening order. */
  readonly handles: readonly number[]
  /** An open document; throws for an unknown handle. */
  document(handle: number): FakeDocumentState
  /** The default answer to a command; changes the core's state like the real engine. */
  reply(command: Command): FakeReply
}

type HistoryEntry = { readonly label: string; readonly before: Document; readonly after: Document }

type DocumentEntry = {
  readonly handle: number
  recipe: Document
  revision: number
  baseline: string
  saved: { readonly revision: number; readonly text: string } | null
  undo: HistoryEntry[]
  redo: HistoryEntry[]
  transaction: { readonly label: string; readonly before: Document } | null
  focus: string | null
  meshIds: Record<string, number>
}

class Rejection extends Error {
  readonly reply: ErrorReply

  constructor(reply: ErrorReply) {
    super(reply.message)
    this.reply = reply
  }
}

function reject(code: ErrorReply['code'], message: string): never {
  throw new Rejection({ code, message })
}

const DEFAULT_BOUNDS: BoundingBox = { min: [-10, -10, 0], max: [10, 10, 20] }

const LENGTH_UNITS: Readonly<Record<string, number>> = { mm: 1, cm: 10, m: 1000, in: 25.4 }
const ANGLE_UNITS: Readonly<Record<string, number>> = { deg: Math.PI / 180, rad: 1 }

type Quantity = { readonly value: number; readonly dimension: Dimension }

/** Evaluates products of numbers with optional units and parameter names, which is all the tests type. */
export function evaluateExpression(
  text: string,
  names: ReadonlyMap<string, Quantity>,
): Quantity | { readonly error: string } {
  const factors = text.split('*').map((factor) => factor.trim())
  let value = 1
  let length = 0
  let angle = 0
  for (const factor of factors) {
    const number = /^(-?\d+(?:\.\d+)?)\s*([a-z]*)$/.exec(factor)
    if (number !== null) {
      const unit = number[2] ?? ''
      const magnitude = Number(number[1])
      if (unit === '') {
        value *= magnitude
      } else if (unit in LENGTH_UNITS) {
        value *= magnitude * (LENGTH_UNITS[unit] ?? 1)
        length += 1
      } else if (unit in ANGLE_UNITS) {
        value *= magnitude * (ANGLE_UNITS[unit] ?? 1)
        angle += 1
      } else {
        return { error: `Unknown unit '${unit}'.` }
      }
      continue
    }
    const named = names.get(factor)
    if (named === undefined) {
      return { error: factor === '' ? 'The expression is empty.' : `Unknown name '${factor}'.` }
    }
    value *= named.value
    length += named.dimension.length
    angle += named.dimension.angle
  }
  return { value, dimension: { length, angle } }
}

function documentWith(
  recipe: Document,
  changes: { readonly rollback?: string | null; readonly printer?: string | null },
): Document {
  const rollback = changes.rollback === undefined ? (recipe.rollback ?? null) : changes.rollback
  const printer = changes.printer === undefined ? (recipe.printer ?? null) : changes.printer
  return {
    format: recipe.format,
    format_version: recipe.format_version,
    units: recipe.units,
    ...(printer === null ? {} : { printer }),
    parameters: parametersOf(recipe),
    bodies: bodiesOf(recipe),
    features: featuresOf(recipe),
    ...(rollback === null ? {} : { rollback }),
  }
}

function insertAt<Item>(items: readonly Item[], index: number, item: Item): Item[] {
  if (!Number.isInteger(index) || index < 0 || index > items.length) {
    reject('edit_rejected', `Index ${String(index)} is out of range.`)
  }
  return [...items.slice(0, index), item, ...items.slice(index)]
}

function indexOfId(items: readonly { readonly id: string }[], id: string, kind: string): number {
  const index = items.findIndex((item) => item.id === id)
  if (index === -1) {
    reject('edit_rejected', `There is no ${kind} with id ${id}.`)
  }
  return index
}

function replaceById<Item extends { readonly id: string }>(
  items: readonly Item[],
  item: Item,
  kind: string,
): Item[] {
  const index = indexOfId(items, item.id, kind)
  return items.map((existing, position) => (position === index ? item : existing))
}

function removeById<Item extends { readonly id: string }>(
  items: readonly Item[],
  id: string,
  kind: string,
): Item[] {
  indexOfId(items, id, kind)
  return items.filter((item) => item.id !== id)
}

function moveById<Item extends { readonly id: string }>(
  items: readonly Item[],
  id: string,
  index: number,
  kind: string,
): Item[] {
  const current = indexOfId(items, id, kind)
  const item = items[current]
  if (item === undefined || !Number.isInteger(index) || index < 0 || index >= items.length) {
    reject('edit_rejected', `Index ${String(index)} is out of range.`)
  }
  const rest = items.filter((existing) => existing.id !== id)
  return [...rest.slice(0, index), item, ...rest.slice(index)]
}

function usedIds(recipe: Document): Set<string> {
  return new Set([
    ...parametersOf(recipe).map((parameter) => parameter.id),
    ...bodiesOf(recipe).map((body) => body.id),
    ...featuresOf(recipe).map((feature) => feature.id),
  ])
}

function requireFreeId(recipe: Document, id: string): void {
  if (usedIds(recipe).has(id)) {
    reject('edit_rejected', `The id ${id} is already used.`)
  }
}

/** Applies one model edit (model.md 2.9) or throws a Rejection. */
function applyEdit(recipe: Document, edit: Edit): Document {
  switch (edit.type) {
    case 'add_parameter':
      requireFreeId(recipe, edit.parameter.id)
      return { ...recipe, parameters: insertAt(parametersOf(recipe), edit.index, edit.parameter) }
    case 'update_parameter':
      return {
        ...recipe,
        parameters: replaceById(parametersOf(recipe), edit.parameter, 'parameter'),
      }
    case 'remove_parameter':
      return { ...recipe, parameters: removeById(parametersOf(recipe), edit.id, 'parameter') }
    case 'move_parameter':
      return {
        ...recipe,
        parameters: moveById(parametersOf(recipe), edit.id, edit.index, 'parameter'),
      }
    case 'add_body':
      requireFreeId(recipe, edit.body.id)
      return { ...recipe, bodies: insertAt(bodiesOf(recipe), edit.index, edit.body) }
    case 'update_body':
      return { ...recipe, bodies: replaceById(bodiesOf(recipe), edit.body, 'body') }
    case 'remove_body':
      return { ...recipe, bodies: removeById(bodiesOf(recipe), edit.id, 'body') }
    case 'move_body':
      return { ...recipe, bodies: moveById(bodiesOf(recipe), edit.id, edit.index, 'body') }
    case 'add_feature':
      requireFreeId(recipe, edit.feature.id)
      return { ...recipe, features: insertAt(featuresOf(recipe), edit.index, edit.feature) }
    case 'update_feature':
      return { ...recipe, features: replaceById(featuresOf(recipe), edit.feature, 'feature') }
    case 'remove_feature': {
      if (recipe.rollback === edit.id) {
        reject('edit_rejected', `Feature ${edit.id} is the rollback marker; move the marker first.`)
      }
      return { ...recipe, features: removeById(featuresOf(recipe), edit.id, 'feature') }
    }
    case 'move_feature':
      return { ...recipe, features: moveById(featuresOf(recipe), edit.id, edit.index, 'feature') }
    case 'set_rollback': {
      const rollback = edit.rollback ?? null
      if (rollback !== null) {
        indexOfId(featuresOf(recipe), rollback, 'feature')
      }
      return documentWith(recipe, { rollback })
    }
    case 'set_units':
      return { ...recipe, units: edit.units }
    case 'set_printer':
      return documentWith(recipe, { printer: edit.printer ?? null })
  }
}

function rollbackIndex(recipe: Document): number {
  const rollback = recipe.rollback ?? null
  const index =
    rollback === null ? -1 : featuresOf(recipe).findIndex((feature) => feature.id === rollback)
  return index === -1 ? featuresOf(recipe).length : index
}

/** Features that run at the display state: before the rollback marker and, with a focus, before the focused feature. */
function modelFeatures(entry: DocumentEntry): readonly FeatureRecord[] {
  const active = featuresOf(entry.recipe).slice(0, rollbackIndex(entry.recipe))
  const focusIndex =
    entry.focus === null ? -1 : active.findIndex((feature) => feature.id === entry.focus)
  return (focusIndex === -1 ? active : active.slice(0, focusIndex)).filter(
    (feature) => feature.suppressed !== true,
  )
}

function resolvedWithoutMatches(): Resolution {
  return { status: 'resolved', matches: [] }
}

/** Just enough of a document check for the fake: the format tag and the three record lists. */
function isDocument(value: unknown): value is Document {
  if (typeof value !== 'object' || value === null) {
    return false
  }
  const field = (name: string): unknown => Reflect.get(value, name)
  return (
    field('format') === 'partsmith' &&
    Array.isArray(field('parameters')) &&
    Array.isArray(field('bodies')) &&
    Array.isArray(field('features'))
  )
}

function bodyOf(feature: FeatureRecord): string | null {
  const body: unknown = feature['body']
  return typeof body === 'string' ? body : null
}

function dirtyOf(entry: DocumentEntry): boolean {
  return documentText(entry.recipe) !== entry.baseline
}

function parameterValues(recipe: Document): ParameterValue[] {
  const names = new Map<string, Quantity>()
  return parametersOf(recipe).map((parameter) => {
    const result = evaluateExpression(parameter.expr, names)
    if ('error' in result) {
      return { id: parameter.id, error: result.error }
    }
    names.set(parameter.name, result)
    return { id: parameter.id, value: result.value, dimension: result.dimension }
  })
}

function defaultSolution(feature: FeatureRecord): SketchSolution {
  return {
    feature: feature.id,
    status: 'under_constrained',
    failed: false,
    dof: 0,
    redundant: [],
    conflicting: [],
    placement: { origin: [0, 0, 0], x_axis: [1, 0, 0], normal: [0, 0, 1] },
    entities: [],
    points: [],
    constraints: [],
    regions: [],
    profile_findings: [],
  }
}

function documentChanged(entry: DocumentEntry): ProtocolEvent {
  const undo = entry.undo.at(-1)?.label ?? null
  const redo = entry.redo.at(-1)?.label ?? null
  return {
    type: 'document_changed',
    doc: entry.handle,
    revision: entry.revision,
    document: entry.recipe,
    dirty: dirtyOf(entry),
    ...(undo === null ? {} : { undo }),
    ...(redo === null ? {} : { redo }),
    transaction_open: entry.transaction !== null,
    ...(entry.transaction === null ? {} : { transaction: entry.transaction.label }),
    ...(entry.focus === null ? {} : { focus: entry.focus }),
  }
}

function applyAll(recipe: Document, edits: readonly Edit[]): Document {
  let next = recipe
  edits.forEach((edit, index) => {
    try {
      next = applyEdit(next, edit)
    } catch (error) {
      if (error instanceof Rejection) {
        throw new Rejection({ ...error.reply, edit_index: index })
      }
      throw error
    }
  })
  return next
}

/** Records a change outside a transaction as one undo entry; inside one, the transaction keeps it. */
function commitChange(entry: DocumentEntry, label: string | null, after: Document): void {
  if (entry.transaction === null) {
    entry.undo.push({ label: label ?? 'Edit', before: entry.recipe, after })
    entry.redo = []
  }
  entry.recipe = after
  entry.revision += 1
}

function rejectDuringTransaction(entry: DocumentEntry): void {
  if (entry.transaction !== null) {
    reject('transaction_open', `Finish or cancel "${entry.transaction.label}" first.`)
  }
}

function requireTransaction(entry: DocumentEntry): {
  readonly label: string
  readonly before: Document
} {
  if (entry.transaction === null) {
    reject('no_transaction', 'There is no edit in progress.')
  }
  return entry.transaction
}

function featureLabel(entry: DocumentEntry, id: string): string {
  const feature = featuresOf(entry.recipe).find((record) => record.id === id)
  return feature?.name ?? id
}

/** Creates a fake core with its own documents; `options` apply to every answer. */
export function createFakeCore(options: FakeCoreOptions = {}): FakeCore {
  const documents = new Map<number, DocumentEntry>()
  let nextHandle = 1
  let nextMeshId = 1

  function entryFor(handle: number): DocumentEntry {
    const entry = documents.get(handle)
    if (entry === undefined) {
      reject('unknown_document', `Document ${String(handle)} is not open.`)
    }
    return entry
  }

  function featureStatuses(recipe: Document): FeatureStatus[] {
    const end = rollbackIndex(recipe)
    return featuresOf(recipe).map((feature, index) => {
      let status: FeatureState = 'ok'
      let messages: readonly Diagnostic[] = []
      let repairs: readonly Repair[] = []
      if (index >= end) {
        status = 'rolled_back'
      } else if (feature.suppressed === true) {
        status = 'suppressed'
      } else {
        const override = options.statuses?.[feature.id]
        if (override !== undefined) {
          status = override.status
          messages = override.messages
          repairs = override.repairs ?? []
        }
      }
      return { id: feature.id, status, messages: [...messages], repairs: [...repairs] }
    })
  }

  function solutionFor(feature: FeatureRecord): SketchSolution {
    return options.solutions?.[feature.id] ?? defaultSolution(feature)
  }

  /** The events of a recompute (engine.md 3.4 order) and the buffers their meshes reference. */
  function recomputeEvents(entry: DocumentEntry): {
    events: ProtocolEvent[]
    buffers: BufferView[]
  } {
    const events: ProtocolEvent[] = [documentChanged(entry)]
    const buffers: BufferView[] = []
    events.push({
      type: 'recomputed',
      doc: entry.handle,
      revision: entry.revision,
      features: featureStatuses(entry.recipe),
      parameters: parameterValues(entry.recipe),
      diagnostics: [],
    })
    for (const feature of featuresOf(entry.recipe)) {
      if (feature.type === 'sketch') {
        events.push({
          type: 'sketch_solved',
          doc: entry.handle,
          revision: entry.revision,
          solution: solutionFor(feature),
        })
      }
    }
    const withGeometry = new Set(modelFeatures(entry).map(bodyOf))
    const meshIds: Record<string, number> = {}
    for (const body of bodiesOf(entry.recipe)) {
      if (!withGeometry.has(body.id)) {
        events.push({
          type: 'body_mesh',
          doc: entry.handle,
          revision: entry.revision,
          body: body.id,
          layer: 'model',
          mesh_id: nextMeshId++,
        })
        continue
      }
      const meshId = nextMeshId++
      meshIds[body.id] = meshId
      const parts = meshParts(options.bounds?.[body.id] ?? DEFAULT_BOUNDS, buffers.length)
      buffers.push(...parts.buffers)
      events.push({
        type: 'body_mesh',
        doc: entry.handle,
        revision: entry.revision,
        body: body.id,
        layer: 'model',
        mesh_id: meshId,
        mesh: parts.mesh,
      })
    }
    entry.meshIds = meshIds
    const focused = featuresOf(entry.recipe).find((feature) => feature.id === entry.focus)
    const previewBody = focused === undefined ? null : bodyOf(focused)
    if (previewBody !== null) {
      const parts = meshParts(options.bounds?.[previewBody] ?? DEFAULT_BOUNDS, buffers.length)
      buffers.push(...parts.buffers)
      events.push({
        type: 'body_mesh',
        doc: entry.handle,
        revision: entry.revision,
        body: previewBody,
        layer: 'preview',
        mesh_id: nextMeshId++,
        mesh: parts.mesh,
      })
    }
    return { events, buffers }
  }

  function changed(entry: DocumentEntry, result: CommandResult): FakeReply {
    const { events, buffers } = recomputeEvents(entry)
    return { result, events, buffers }
  }

  function open(recipe: Document): DocumentEntry {
    const entry: DocumentEntry = {
      handle: nextHandle++,
      recipe,
      revision: 0,
      baseline: documentText(recipe),
      saved: null,
      undo: [],
      redo: [],
      transaction: null,
      focus: null,
      meshIds: {},
    }
    documents.set(entry.handle, entry)
    return entry
  }

  function applyEdits(command: CommandOfType<'apply_edits'>): FakeReply {
    const entry = entryFor(command.doc)
    const label = command.label ?? null
    if (entry.transaction === null && label === null) {
      reject('label_required', 'Edits outside a transaction need a label for the undo history.')
    }
    if (entry.transaction !== null && label !== null) {
      reject(
        'label_not_allowed',
        `Edits inside the transaction "${entry.transaction.label}" take the transaction's label.`,
      )
    }
    commitChange(entry, label, applyAll(entry.recipe, command.edits ?? []))
    return changed(entry, { type: 'apply_edits', revision: entry.revision })
  }

  function renameParameter(command: CommandOfType<'rename_parameter'>): FakeReply {
    const entry = entryFor(command.doc)
    const parameter = parametersOf(entry.recipe).find(
      (existing) => existing.id === command.parameter,
    )
    if (parameter === undefined) {
      reject('edit_rejected', `There is no parameter with id ${command.parameter}.`)
    }
    if (parametersOf(entry.recipe).some((existing) => existing.name === command.name)) {
      reject('edit_rejected', `The name ${command.name} is already used by another parameter.`)
    }
    const pattern = new RegExp(`\\b${parameter.name}\\b`, 'g')
    const rename = (expression: string): string => expression.replace(pattern, command.name)
    const parameters = parametersOf(entry.recipe).map((existing) =>
      Object.assign({}, existing, {
        name: existing.id === parameter.id ? command.name : existing.name,
        expr: rename(existing.expr),
      }),
    )
    const after: Document = { ...entry.recipe, parameters }
    commitChange(entry, `Rename parameter ${parameter.name} to ${command.name}`, after)
    return changed(entry, { type: 'rename_parameter', revision: entry.revision })
  }

  function setFocus(command: CommandOfType<'set_focus'>): FakeReply {
    const entry = entryFor(command.doc)
    requireTransaction(entry)
    const focus = command.feature ?? null
    if (focus !== null) {
      const index = indexOfId(featuresOf(entry.recipe), focus, 'feature')
      const feature = featuresOf(entry.recipe)[index]
      if (feature?.suppressed === true || index >= rollbackIndex(entry.recipe)) {
        reject(
          'focus_inactive',
          `${featureLabel(entry, focus)} is not active. Move the rollback marker past it or unsuppress it first.`,
        )
      }
    }
    entry.focus = focus
    entry.revision += 1
    return changed(entry, { type: 'set_focus' })
  }

  function undoOrRedo(command: CommandOfType<'undo'> | CommandOfType<'redo'>): FakeReply {
    const entry = entryFor(command.doc)
    rejectDuringTransaction(entry)
    const undoing = command.type === 'undo'
    const step = (undoing ? entry.undo : entry.redo).pop()
    if (step === undefined) {
      if (undoing) {
        reject('nothing_to_undo', 'There is nothing to undo.')
      }
      reject('nothing_to_redo', 'There is nothing to redo.')
    }
    const opposite = undoing ? entry.redo : entry.undo
    opposite.push(step)
    entry.recipe = undoing ? step.before : step.after
    entry.revision += 1
    return changed(entry, { type: command.type, revision: entry.revision, label: step.label })
  }

  function reply(command: Command): FakeReply {
    switch (command.type) {
      case 'engine_info':
        return {
          result: {
            type: 'engine_info',
            protocol_version: options.protocolVersion ?? PROTOCOL_VERSION,
            engine_version: options.engineVersion ?? '0.1.0',
            format_version: 1,
            kernel: 'Fake kernel',
          },
        }
      case 'feature_catalog':
        return {
          result: {
            type: 'feature_catalog',
            features: [...(options.catalog ?? TEST_CATALOG_ENTRIES)],
          },
        }
      case 'list_printer_profiles':
        return {
          result: {
            type: 'list_printer_profiles',
            profiles: [...(options.printers ?? TEST_PRINTERS)],
          },
        }
      case 'new_document': {
        const entry = open(emptyDocument(command.units ?? 'mm'))
        const recomputed = recomputeEvents(entry)
        return { result: { type: 'new_document', doc: entry.handle }, ...recomputed }
      }
      case 'open_document': {
        let parsed: unknown
        try {
          parsed = JSON.parse(command.text)
        } catch (error) {
          return reject(
            'parse_error',
            `The file is not a valid Partsmith document: ${error instanceof Error ? error.message : String(error)}`,
          )
        }
        if (!isDocument(parsed)) {
          return reject(
            'parse_error',
            'The file is not a valid Partsmith document: "format" must be "partsmith".',
          )
        }
        const entry = open(parsed)
        return changed(entry, { type: 'open_document', doc: entry.handle })
      }
      case 'save_document': {
        const entry = entryFor(command.doc)
        rejectDuringTransaction(entry)
        const text = documentText(entry.recipe)
        entry.saved = { revision: entry.revision, text }
        return { result: { type: 'save_document', text, assets: [], revision: entry.revision } }
      }
      case 'mark_saved': {
        const entry = entryFor(command.doc)
        const wasDirty = dirtyOf(entry)
        if (entry.saved !== null && entry.saved.revision === command.revision) {
          entry.baseline = entry.saved.text
        }
        const events = wasDirty === dirtyOf(entry) ? [] : [documentChanged(entry)]
        return { result: { type: 'mark_saved' }, events }
      }
      case 'close_document':
        entryFor(command.doc)
        documents.delete(command.doc)
        return { result: { type: 'close_document' } }
      case 'get_document': {
        const entry = entryFor(command.doc)
        return {
          result: { type: 'get_document', revision: entry.revision, document: entry.recipe },
        }
      }
      case 'apply_edits':
        return applyEdits(command)
      case 'rename_parameter':
        return renameParameter(command)
      case 'begin_transaction': {
        const entry = entryFor(command.doc)
        rejectDuringTransaction(entry)
        entry.transaction = { label: command.label, before: entry.recipe }
        entry.revision += 1
        return { result: { type: 'begin_transaction' }, events: [documentChanged(entry)] }
      }
      case 'set_focus':
        return setFocus(command)
      case 'commit_transaction': {
        const entry = entryFor(command.doc)
        const transaction = requireTransaction(entry)
        const recorded = documentText(transaction.before) !== documentText(entry.recipe)
        if (recorded) {
          entry.undo.push({
            label: transaction.label,
            before: transaction.before,
            after: entry.recipe,
          })
          entry.redo = []
        }
        const hadFocus = entry.focus !== null
        entry.transaction = null
        entry.focus = null
        entry.revision += 1
        const result: CommandResult = {
          type: 'commit_transaction',
          revision: entry.revision,
          recorded,
        }
        return hadFocus ? changed(entry, result) : { result, events: [documentChanged(entry)] }
      }
      case 'cancel_transaction': {
        const entry = entryFor(command.doc)
        const transaction = requireTransaction(entry)
        entry.recipe = transaction.before
        entry.transaction = null
        entry.focus = null
        entry.revision += 1
        return changed(entry, { type: 'cancel_transaction', revision: entry.revision })
      }
      case 'undo':
      case 'redo':
        return undoOrRedo(command)
      case 'solve_sketch': {
        const entry = entryFor(command.doc)
        const feature = featuresOf(entry.recipe)[
          indexOfId(featuresOf(entry.recipe), command.feature, 'feature')
        ]
        if (feature === undefined || feature.type !== 'sketch') {
          return reject('not_a_sketch', `${command.feature} is not a sketch.`)
        }
        return { result: { type: 'solve_sketch', solution: solutionFor(feature) } }
      }
      case 'drag_sketch': {
        const entry = entryFor(command.doc)
        const feature = featuresOf(entry.recipe)[
          indexOfId(featuresOf(entry.recipe), command.feature, 'feature')
        ]
        if (feature === undefined || feature.type !== 'sketch') {
          return reject('not_a_sketch', `${command.feature} is not a sketch.`)
        }
        return { result: { type: 'drag_sketch', solution: solutionFor(feature) } }
      }
      case 'end_sketch_drag': {
        const entry = entryFor(command.doc)
        if (!command.commit) {
          return { result: { type: 'end_sketch_drag', revision: entry.revision } }
        }
        commitChange(entry, 'Drag', entry.recipe)
        return changed(entry, { type: 'end_sketch_drag', revision: entry.revision })
      }
      case 'evaluate': {
        const entry = entryFor(command.doc)
        const names = new Map<string, Quantity>()
        for (const value of parameterValues(entry.recipe)) {
          const parameter = parametersOf(entry.recipe).find((existing) => existing.id === value.id)
          const number = value.value ?? null
          const dimension = value.dimension ?? null
          if (parameter !== undefined && number !== null && dimension !== null) {
            names.set(parameter.name, { value: number, dimension })
          }
        }
        const result = evaluateExpression(command.expr, names)
        if ('error' in result) {
          return reject('expression_error', result.error)
        }
        return { result: { type: 'evaluate', value: result.value, dimension: result.dimension } }
      }
      case 'list_elements': {
        entryFor(command.doc)
        const kind = command.kind ?? null
        const elements = (options.elements?.[command.body] ?? []).filter(
          (element) => kind === null || element.kind === kind,
        )
        return { result: { type: 'list_elements', elements } }
      }
      case 'pick_reference':
        entryFor(command.doc)
        return reject(
          'stale_pick',
          'The picked geometry changed since it was displayed. Pick again.',
        )
      case 'resolve_references':
        entryFor(command.doc)
        return {
          result: {
            type: 'resolve_references',
            results: (command.references ?? []).map(resolvedWithoutMatches),
          },
        }
      case 'measure': {
        const entry = entryFor(command.doc)
        const [target] = command.targets ?? []
        if (target === undefined || command.targets?.length !== 1) {
          return reject('invalid_measure', 'Measure takes exactly one target.')
        }
        if (target.kind !== 'body') {
          return reject('invalid_measure', 'The fake core measures bodies only.')
        }
        if (entry.meshIds[target.body] === undefined) {
          return reject('body_empty', `${target.body} has no geometry to measure.`)
        }
        // The volume of the body's bounding box: every fake body is a box.
        const box = options.bounds?.[target.body] ?? DEFAULT_BOUNDS
        const volume =
          (box.max[0] - box.min[0]) * (box.max[1] - box.min[1]) * (box.max[2] - box.min[2])
        return { result: { type: 'measure', volume, bounding_box: box } }
      }
      case 'analyze_print': {
        const entry = entryFor(command.doc)
        const profile = command.profile ?? entry.recipe.printer ?? 'generic-fdm'
        return {
          result: { type: 'analyze_print', health: 'ok' },
          events: [
            {
              type: 'print_health',
              doc: entry.handle,
              revision: entry.revision,
              report: {
                profile,
                orientation: command.orientation,
                summary: {
                  health: 'ok',
                  errors: 0,
                  warnings: 0,
                  infos: 0,
                  label: 'Ready to print',
                },
                bodies: [],
              },
            },
          ],
        }
      }
      case 'export': {
        entryFor(command.doc)
        const extension = command.format.kind
        return {
          result: {
            type: 'export',
            files: [{ name: `model.${extension}`, media_type: `model/${extension}`, data: 0 }],
            warnings: [],
          },
          buffers: [Uint8Array.of(0x50, 0x4b, 0x03, 0x04)],
        }
      }
      default:
        return reject('internal_error', `The fake core does not implement ${command.type}.`)
    }
  }

  return {
    options,
    get handles() {
      return [...documents.keys()]
    },
    document(handle) {
      const entry = documents.get(handle)
      if (entry === undefined) {
        throw new Error(`The fake core has no document ${String(handle)}.`)
      }
      return {
        handle: entry.handle,
        recipe: entry.recipe,
        revision: entry.revision,
        dirty: dirtyOf(entry),
        undoLabels: entry.undo.map((step) => step.label),
        redoLabels: entry.redo.map((step) => step.label),
        transaction: entry.transaction?.label ?? null,
        focus: entry.focus,
        meshIds: { ...entry.meshIds },
      }
    },
    reply(command) {
      try {
        return reply(command)
      } catch (error) {
        if (error instanceof Rejection) {
          return { error: error.reply }
        }
        throw error
      }
    },
  }
}
