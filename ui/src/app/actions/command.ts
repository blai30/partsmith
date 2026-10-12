import { CoreError } from '../../core/client'
import { generateId, usedIds } from '../../core/ids'
import { featureLabel, nextBodyName } from '../../core/labels'
import { createLatestOnly, type LatestOnly } from '../../core/queue'
import type {
  DocHandle,
  Edit,
  ElementDescriptor,
  FeatureRecord,
  FeatureStatus,
  Placement,
  SelectionKind,
  SketchEntity,
} from '../../generated/protocol'
import {
  bodyCreatingTarget,
  buildInspectorModel,
  defaultParams,
  filledByCommand,
  getParam,
  setParam,
  type InspectorField,
  type InspectorModel,
} from '../../inspector/model'
import { acceptsTarget, selectionValue } from '../../inspector/selection-values'
import { fieldForPointer, fieldPointer, repairField } from '../../inspector/status'
import { MESSAGES } from '../../messages'
import { TIMING_NAMES } from '../../performance/marks'
import { pointSelectionItem } from '../../sketch/hit'
import { memoized } from '../../state/selectors'
import { worldToSketch } from '../../sketch/placement'
import type {
  ActiveCommand,
  AppState,
  AppStore,
  CatalogFeature,
  CommandStep,
  DocumentState,
  HelperSketch,
  SelectionItem,
} from '../../state/store'
import { addScaled, dot, length, subtract } from '../../viewport/math'
import type { PointerRay } from '../../viewport/picking'
import type { App } from '../createApp'
import { measureToNextFrame } from '../timing'
import { setSessionVisible } from './bodies'
import { commitEdits, documentOf, reportNewFailures, showCoreError } from './edits'
import { notify, showPickHint } from './notifications'
import { enterSketch, finishSketch, sketchPlaneOf, startNewSketch } from './sketch'
import { setSuppressed } from './timeline'

// Feature commands (ui.md 3.7): an inspector over one feature record inside a transaction, with a
// live preview of every complete state of its params, a history of the user's changes, and the
// bodies and helper sketch the command creates.

type Params = Readonly<Record<string, unknown>>

/** A body the command creates, and the field that names it (2.12). */
type NewBody = NonNullable<ActiveCommand['newBody']>

/** One preview request: the params, and when the user changed the value they come from. */
type PreviewRequest = {
  readonly params: Params
  /** `performance.now()` of the inspector change, which starts `ps:preview` (2.18), or null. */
  readonly changedAt: number | null
}

/**
 * A reference of the edited feature the core could not resolve (4.2): the selection field it is in,
 * where in that field's value it sits (`['0']` for the first edge, `['regions', '1']` for a profile's
 * second region) and the stored value, which the next pick in that field replaces.
 */
type RepairSlot = {
  readonly field: string
  readonly segments: readonly string[]
  readonly lost: unknown
}

/** What a command keeps beside its store state. */
type CommandSession = {
  readonly featureId: string
  readonly feature: CatalogFeature
  /** The preview requests, one at a time with only the newest waiting. */
  readonly preview: LatestOnly<PreviewRequest>
  /** Record properties that are not params (`name`, `suppressed`), kept on every update. */
  readonly recordExtras: Readonly<Record<string, unknown>>
  /** Feature statuses before the command, to report a failure it causes. */
  readonly statusesBefore: Readonly<Record<string, FeatureStatus>>
  readonly lastTargetBody: string | null
  /** Fields whose value is still being checked (an expression awaiting evaluation). */
  readonly pendingFields: Set<string>
  /** Ids this command handed out (bodies, the helper sketch and its points), so none is reused. */
  readonly issued: Set<string>
  /** Every body this command made up, by id, so a restored change finds the body its value names. */
  readonly createdBodies: Map<string, NewBody>
  /** Body fields that the first pick of a face or edge field filled, by that field's path (2.12). */
  readonly filledBodies: Map<string, string>
  /** The lost references of an edited feature, in its status's order (3.7, 4.2). */
  repairs: readonly RepairSlot[]
  /** The params of the newest preview request, to tell whether the latest params were sent. */
  submitted: Params | null
  /** Whether the newest preview request failed in the core. */
  failed: boolean
  /** When the user last changed a value, until a preview request carries it. */
  changedAt: number | null
  /** The body this command has added inside the transaction, or null. */
  addedBody: string | null
  /** The helper sketch this command has added inside the transaction, or null. */
  appliedHelper: string | null
  /** The expression field whose text has changed since it gained focus, which is one change (3.7). */
  typingField: string | null
  /** Whether OK is on its way, so a second Enter or click while it runs commits nothing more. */
  confirming: boolean
}

const sessions = new WeakMap<AppStore, CommandSession>()

/** Record properties beside the params (model.md FeatureRecord). */
const RECORD_EXTRAS: ReadonlySet<string> = new Set(['name', 'suppressed'])

/** Plural nouns of the selection kinds, for the hint of a refused pick (3.7). */
const SELECTION_NOUNS: Readonly<Record<SelectionKind, string>> = {
  body: 'bodies',
  sketch: 'sketches',
  sketch_region: 'regions',
  sketch_point: 'points',
  face: 'faces',
  edge: 'edges',
  plane: 'planes',
  axis: 'axes',
}

/** A hole center snaps to the face's centroid within this many CSS pixels (3.7). */
const CENTROID_SNAP_PIXELS = 8

/** Sketch coordinates of hole centers are kept to this many decimals of a millimeter. */
const CENTER_DECIMALS = 6

function commandOf(app: App): ActiveCommand | null {
  return app.store.getState().command
}

function sessionOf(app: App): CommandSession | null {
  const command = commandOf(app)
  if (command === null) {
    return null
  }
  const session = sessions.get(app.store) ?? null
  return session !== null && command.featureId === session.featureId
    ? session
    : adoptSession(app, command)
}

/**
 * A session for a command the store holds without one, such as a store assembled directly: the
 * store's ActiveCommand is the command (2.8), so the session is rebuilt from it and the document.
 */
function adoptSession(app: App, command: ActiveCommand): CommandSession | null {
  const state = app.store.getState()
  const feature = state.catalog?.byType[command.featureType]
  const document = state.document
  if (feature === undefined || document === null) {
    return null
  }
  const record = document.recipe.features?.find((candidate) => candidate.id === command.featureId)
  const recordExtras = Object.fromEntries(
    Object.entries(record ?? {}).filter(([key]) => RECORD_EXTRAS.has(key)),
  )
  const session = createSession(app, {
    featureId: command.featureId,
    feature,
    recordExtras,
    statusesBefore: document.featureStatus,
    lastTargetBody: lastTargetBodyOf(document),
  })
  if (command.applied) {
    // The applied record is the focused feature, so these params need no preview.
    session.submitted = command.params
    session.addedBody =
      command.newBody !== null &&
      (document.recipe.bodies ?? []).some((body) => body.id === command.newBody?.id)
        ? command.newBody.id
        : null
    session.appliedHelper = command.helper?.applied === true ? command.helper.id : null
  }
  if (command.newBody !== null) {
    session.createdBodies.set(command.newBody.id, command.newBody)
  }
  return session
}

function updateCommand(app: App, change: (command: ActiveCommand) => Partial<ActiveCommand>): void {
  app.store.setState((state) =>
    state.command === null ? {} : { command: { ...state.command, ...change(state.command) } },
  )
}

function modelOf(session: CommandSession, command: ActiveCommand): InspectorModel {
  return buildInspectorModel({
    feature: session.feature,
    params: command.params,
    errors: command.fieldErrors,
  })
}

function fieldsOf(model: InspectorModel): readonly InspectorField[] {
  return [...model.essential, ...model.more]
}

function isEmptyValue(value: unknown): boolean {
  if (value === undefined || value === null || value === '') {
    return true
  }
  if (Array.isArray(value)) {
    return value.length === 0
  }
  return (
    typeof value === 'object' &&
    Array.isArray(Reflect.get(value, 'regions')) &&
    Reflect.get(value, 'regions').length === 0
  )
}

/** The path of the object holding a field (`''` at the top). */
function parentOf(path: string): string {
  const separator = path.lastIndexOf('.')
  return separator === -1 ? '' : path.slice(0, separator)
}

function siblingPath(path: string, name: string): string {
  const parent = parentOf(path)
  return parent === '' ? name : `${parent}.${name}`
}

/** Selection fields of the model, in manifest order. */
function selectionFields(model: InspectorModel, feature: CatalogFeature): InspectorField[] {
  const fields = fieldsOf(model)
  return feature.manifest.fields.flatMap((hint) => {
    const field = fields.find((candidate) => candidate.path === hint.path)
    return field?.control.type === 'selection' ? [field] : []
  })
}

/** Whether a body field fills itself from the first pick of a face or edge field beside it (2.12). */
function fillsFromSiblingPick(field: InspectorField, fields: readonly InspectorField[]): boolean {
  if (field.control.type !== 'selection' || field.control.accepts !== 'body') {
    return false
  }
  const parent = parentOf(field.path)
  return fields.some(
    (other) =>
      other.control.type === 'selection' &&
      (other.control.accepts === 'face' || other.control.accepts === 'edge') &&
      parentOf(other.path) === parent,
  )
}

/**
 * The first required, empty selection field that waits for a pick, or null: a body field that the
 * first face or edge pick fills, or that the command fills itself (op new), waits for none.
 */
function firstEmptySelectionField(
  model: InspectorModel,
  feature: CatalogFeature,
  params: Params,
): string | null {
  const fields = selectionFields(model, feature)
  const next = fields.find(
    (field) =>
      field.required &&
      isEmptyValue(field.value) &&
      !fillsFromSiblingPick(field, fields) &&
      !filledByCommand(field, params, feature),
  )
  return next?.path ?? null
}

/**
 * The field that takes picks when a command opens (3.7 step 5): the first required empty
 * selection field, else the first essential multiple selection field, else none.
 */
function openingActiveField(
  model: InspectorModel,
  feature: CatalogFeature,
  params: Params,
): string | null {
  const empty = firstEmptySelectionField(model, feature, params)
  if (empty !== null) {
    return empty
  }
  const multiple = model.essential.find(
    (field) => field.control.type === 'selection' && field.control.multiple,
  )
  return multiple?.path ?? null
}

/** The body of the latest active feature with a body that has model geometry (3.7 step 3). */
function lastTargetBodyOf(document: DocumentState): string | null {
  const features = document.recipe.features ?? []
  const rollback = document.recipe.rollback ?? null
  const end = features.findIndex((feature) => feature.id === rollback)
  const active = features.slice(0, end === -1 ? features.length : end)
  for (const feature of active.toReversed()) {
    const body = feature['body']
    if (
      feature.suppressed !== true &&
      typeof body === 'string' &&
      document.meshes[body] !== undefined
    ) {
      return body
    }
  }
  return null
}

/** The timeline index of a new feature: before the rollback marker, so it is active. */
function insertionIndex(document: DocumentState): number {
  const features = document.recipe.features ?? []
  const rollback = document.recipe.rollback ?? null
  const index = rollback === null ? -1 : features.findIndex((feature) => feature.id === rollback)
  return index === -1 ? features.length : index
}

function sameValue(first: unknown, second: unknown): boolean {
  return JSON.stringify(first) === JSON.stringify(second)
}

/** Whether the params can be previewed: complete, no field error and nothing pending. */
function previewable(app: App, session: CommandSession, command: ActiveCommand): boolean {
  return (
    modelOf(session, command).complete &&
    session.pendingFields.size === 0 &&
    app.store.getState().document !== null
  )
}

/** A fresh id for something this command creates, free in the document and in this command. */
function issueId(app: App, session: CommandSession, prefix: 'b' | 'f' | 'e'): string {
  const recipe = documentOf(app)?.recipe
  const taken = recipe === undefined ? new Set<string>() : usedIds(recipe)
  const id = generateId(
    prefix,
    (candidate) =>
      taken.has(candidate) || session.issued.has(candidate) || candidate === session.featureId,
  )
  session.issued.add(id)
  return id
}

/** Makes up a body for the command to create in the field at `path`. */
function allocateBody(app: App, session: CommandSession, path: NewBody['path']): NewBody {
  const recipe = documentOf(app)?.recipe
  const taken = new Set([...session.createdBodies.values()].map((body) => body.name))
  let name = recipe === undefined ? 'Body 1' : nextBodyName(recipe)
  if (recipe !== undefined && taken.has(name)) {
    // A name this command already gave out stays its own until that body is gone.
    const bodies = [
      ...(recipe.bodies ?? []),
      ...[...session.createdBodies.values()].map((body) => ({ id: body.id, name: body.name })),
    ]
    name = nextBodyName({ ...recipe, bodies })
  }
  const body: NewBody = { id: issueId(app, session, 'b'), name, path }
  session.createdBodies.set(body.id, body)
  return body
}

/** The body this command should have in the document for these params, and the params naming it. */
function wantedBody(
  app: App,
  session: CommandSession,
  command: ActiveCommand,
  params: Params,
): { readonly body: NewBody | null; readonly params: Params } {
  const rule = bodyCreatingTarget(session.feature)
  const current = command.newBody
  if (rule !== null) {
    // A body-creating target holds the created body until it is cleared or replaced (2.12).
    const target = getParam(params, 'target')
    return {
      body: current !== null && target === current.id ? current : null,
      params,
    }
  }
  const hasBodyField = session.feature.manifest.fields.some(
    (hint) => hint.path === 'body' && hint.selection === 'body',
  )
  if (!hasBodyField || params['op'] !== 'new') {
    return { body: null, params }
  }
  const body = params['body']
  if (current !== null && (isEmptyValue(body) || body === current.id)) {
    return { body: current, params: { ...params, body: current.id } }
  }
  if (!isEmptyValue(body)) {
    return { body: null, params }
  }
  const created = allocateBody(app, session, 'body')
  return { body: created, params: { ...params, body: created.id } }
}

/** The sketch record of a helper. */
function helperRecord(helper: HelperSketch): FeatureRecord {
  return { ...helper.params, id: helper.id, type: 'sketch' }
}

/** What one preview application sends and leaves behind. */
type PreviewPlan = {
  readonly edits: readonly Edit[]
  readonly params: Params
  /** The store's created body after the application (unchanged for body-creating targets). */
  readonly newBody: ActiveCommand['newBody']
  readonly addedBody: string | null
  readonly appliedHelper: string | null
}

/**
 * The edits of one preview (3.7): the created body, the helper sketch and the feature itself the
 * first time, updates later, and removals of a body or helper the params no longer use.
 */
function previewEdits(
  app: App,
  session: CommandSession,
  command: ActiveCommand,
  document: DocumentState,
  params: Params,
): PreviewPlan {
  const edits: Edit[] = []
  const wanted = wantedBody(app, session, command, params)
  const next = wanted.params
  const body = wanted.body
  if (body !== null && session.addedBody !== body.id) {
    edits.push({
      type: 'add_body',
      index: (document.recipe.bodies ?? []).length,
      body: { id: body.id, name: body.name },
    })
  }
  const features = document.recipe.features ?? []
  const appliedIndex = features.findIndex((feature) => feature.id === session.featureId)
  let featureIndex =
    command.applied && appliedIndex !== -1 ? appliedIndex : insertionIndex(document)
  const helper = command.helper
  if (helper !== null) {
    if (session.appliedHelper === helper.id) {
      edits.push({ type: 'update_feature', feature: helperRecord(helper) })
    } else {
      // The helper goes where the feature goes, right before it (3.7).
      edits.push({ type: 'add_feature', index: featureIndex, feature: helperRecord(helper) })
      featureIndex += 1
    }
  }
  const record: FeatureRecord = {
    ...session.recordExtras,
    ...next,
    id: session.featureId,
    type: session.feature.type,
  }
  edits.push(
    command.applied
      ? { type: 'update_feature', feature: record }
      : { type: 'add_feature', index: featureIndex, feature: record },
  )
  if (session.appliedHelper !== null && session.appliedHelper !== helper?.id) {
    edits.push({ type: 'remove_feature', id: session.appliedHelper })
  }
  if (session.addedBody !== null && session.addedBody !== body?.id) {
    edits.push({ type: 'remove_body', id: session.addedBody })
  }
  return {
    edits,
    params: next,
    newBody: bodyCreatingTarget(session.feature) === null ? body : command.newBody,
    addedBody: body?.id ?? null,
    appliedHelper: helper?.id ?? null,
  }
}

/** Applies the params as the focused feature inside the transaction. */
async function applyPreview(app: App, session: CommandSession, params: Params): Promise<void> {
  const command = commandOf(app)
  const document = documentOf(app)
  if (command === null || command.featureId !== session.featureId || document === null) {
    return
  }
  const plan = previewEdits(app, session, command, document, params)
  await app.core.applyEdits(document.documentHandle, null, plan.edits)
  session.addedBody = plan.addedBody
  session.appliedHelper = plan.appliedHelper
  if (!command.applied) {
    await app.core.setFocus(document.documentHandle, session.featureId)
  }
  const latest = sameValue(commandOf(app)?.params, params)
  if (latest) {
    // The record now names the body this command made; the params follow it.
    session.submitted = plan.params
  }
  updateCommand(app, (current) => ({
    applied: true,
    newBody: current.newBody === command.newBody ? plan.newBody : current.newBody,
    helper:
      current.helper === null
        ? null
        : { ...current.helper, applied: current.helper.id === session.appliedHelper },
    params: latest ? plan.params : current.params,
  }))
}

function previewStateAfter(app: App, featureId: string): ActiveCommand['preview'] {
  return documentOf(app)?.featureStatus[featureId]?.status === 'error' ? 'failed' : 'shown'
}

type SessionInput = Pick<
  CommandSession,
  'featureId' | 'feature' | 'recordExtras' | 'statusesBefore' | 'lastTargetBody'
>

function createSession(app: App, input: SessionInput): CommandSession {
  const session: CommandSession = {
    ...input,
    pendingFields: new Set(),
    issued: new Set(),
    createdBodies: new Map(),
    filledBodies: new Map(),
    repairs: [],
    submitted: null,
    failed: false,
    changedAt: null,
    addedBody: null,
    appliedHelper: null,
    typingField: null,
    confirming: false,
    preview: createLatestOnly<PreviewRequest, void>(
      (request) => app.mutations.run(() => applyPreview(app, session, request.params)),
      (_output, request) => {
        session.failed = false
        if (request.changedAt !== null) {
          // The preview meshes reached the scene with the response; the next frame draws them.
          measureToNextFrame(TIMING_NAMES.preview, request.changedAt)
        }
        const command = commandOf(app)
        if (sessionOf(app) === session && command !== null) {
          // Inputs that became incomplete or invalid while the request ran keep the preview hidden.
          updateCommand(app, () => ({
            preview: previewable(app, session, command)
              ? previewStateAfter(app, session.featureId)
              : 'none',
          }))
        }
      },
      (error) => {
        session.failed = true
        if (sessionOf(app) === session) {
          updateCommand(app, () => ({ preview: 'failed' }))
        }
        showCoreError(app, error)
      },
    ),
  }
  sessions.set(app.store, session)
  return session
}

/** Sends the params as the newest preview, carrying the time of the change they come from. */
function submitPreview(session: CommandSession, params: Params): void {
  session.submitted = params
  session.preview.submit({ params, changedAt: session.changedAt })
  session.changedAt = null
}

/** Whether the command's helper sketch differs from what the transaction holds. */
function helperChanged(session: CommandSession, command: ActiveCommand): boolean {
  return (
    session.appliedHelper !== (command.helper?.id ?? null) ||
    (command.helper !== null && !command.helper.applied)
  )
}

/** Submits a preview of the current params when they are complete; otherwise hides the preview. */
function refreshPreview(app: App): void {
  const session = sessionOf(app)
  const command = commandOf(app)
  if (session === null || command === null) {
    return
  }
  if (!previewable(app, session, command)) {
    updateCommand(app, () => ({ preview: 'none' }))
    return
  }
  if (
    sameValue(session.submitted, command.params) &&
    !session.failed &&
    !helperChanged(session, command)
  ) {
    // These params are applied or on their way.
    if (!session.preview.busy) {
      updateCommand(app, () => ({ preview: previewStateAfter(app, session.featureId) }))
    }
    return
  }
  updateCommand(app, () => ({ preview: 'pending' }))
  submitPreview(session, command.params)
}

function endSession(app: App): void {
  sessions.get(app.store)?.preview.cancel()
  sessions.delete(app.store)
  // A hint under a field of this command goes with it.
  app.store.setState((state) => ({
    command: null,
    pickHint: state.pickHint?.field === null ? state.pickHint : null,
  }))
}

/** The label of the field at a path, for the command history (3.20 details). */
function fieldLabel(session: CommandSession, path: string): string {
  return session.feature.manifest.fields.find((hint) => hint.path === path)?.label ?? path
}

/** The store change that records one user change of the field at `path` (3.7 command history). */
function recordedChange(
  command: ActiveCommand,
  path: string,
  label: string,
  record: boolean,
): Pick<ActiveCommand, 'history' | 'future' | 'changedPaths'> {
  const changedPaths = [...command.changedPaths.filter((changed) => changed !== path), path]
  if (!record) {
    return { history: command.history, future: command.future, changedPaths }
  }
  const step: CommandStep = {
    params: command.params,
    helper: command.helper,
    activeField: command.activeField,
    field: label,
  }
  return { history: [...command.history, step], future: [], changedPaths }
}

/** Whether a field takes the item; a feature row only counts as a sketch when it is one. */
function takes(state: AppState, accepts: SelectionKind, item: SelectionItem): boolean {
  if (!acceptsTarget(accepts, item)) {
    return false
  }
  if (accepts !== 'sketch' || item.kind !== 'feature') {
    return true
  }
  return sketchRecordOf(state, item.feature) !== null
}

/** The sketch record with this id, or null. */
function sketchRecordOf(state: AppState, sketchId: string | null): FeatureRecord | null {
  if (sketchId === null) {
    return null
  }
  const record = state.document?.recipe.features?.find((feature) => feature.id === sketchId)
  return record?.type === 'sketch' ? record : null
}

/** The one closed region of a sketch, as an item, when it has exactly one. */
function onlyRegionOf(state: AppState, sketchId: string): SelectionItem | null {
  const regions = state.document?.sketches[sketchId]?.solution.regions ?? []
  const [region] = regions
  if (regions.length !== 1 || region === undefined) {
    return null
  }
  return {
    key: `region:${sketchId}:${region.reference.key}`,
    kind: 'sketch_region',
    label: 'Region 1',
    body: null,
    feature: sketchId,
    element: null,
    sketchItem: { kind: 'region', sketch: sketchId, region },
    origin: null,
  }
}

/**
 * The hole centers a sketch offers (3.3): its non-construction point entities, or, when it has
 * none, the centers of its non-construction circles and arcs.
 */
function sketchPointsOf(state: AppState, sketchId: string): SelectionItem[] {
  const entities = state.document?.sketches[sketchId]?.solution.entities ?? []
  const drawn = entities.filter((entity) => entity.construction !== true)
  const points = drawn.filter((entity) => entity.type === 'point')
  if (points.length > 0) {
    return points.map((entity) => pointSelectionItem(sketchId, entity.id))
  }
  return drawn
    .filter((entity) => entity.type === 'circle' || entity.type === 'arc')
    .map((entity) => pointSelectionItem(sketchId, `${entity.id}.center`))
}

/** What a whole sketch stands for in a region or point field (3.3, 3.7 step 4), or null for other fields. */
function sketchStandIns(
  state: AppState,
  sketchId: string,
  accepts: SelectionKind,
): SelectionItem[] | null {
  if (accepts === 'sketch_region') {
    const region = onlyRegionOf(state, sketchId)
    return region === null ? [] : [region]
  }
  if (accepts === 'sketch_point') {
    return sketchPointsOf(state, sketchId)
  }
  return null
}

/**
 * Fills selection fields from the selection, in manifest order, each item used once (3.7 step 4).
 * A selected sketch, or the sketch just finished when nothing is selected, stands for its region
 * or its points in region and point fields.
 */
function preselected(
  state: AppState,
  feature: CatalogFeature,
  params: Params,
  items: readonly SelectionItem[],
  finishedSketch: string | null,
): Record<string, unknown> {
  let next: Record<string, unknown> = { ...params }
  const used = new Set<string>()
  const model = buildInspectorModel({ feature, params, errors: {} })
  for (const field of selectionFields(model, feature)) {
    if (field.control.type !== 'selection') {
      continue
    }
    const { accepts, multiple } = field.control
    const candidates = items.flatMap((item) => {
      const sketch = item.kind === 'feature' ? sketchRecordOf(state, item.feature) : null
      const standIns = sketch === null ? null : sketchStandIns(state, sketch.id, accepts)
      return standIns === null
        ? [{ source: item.key, item }]
        : standIns.map((standIn) => ({ source: item.key, item: standIn }))
    })
    if (items.length === 0 && finishedSketch !== null) {
      for (const standIn of sketchStandIns(state, finishedSketch, accepts) ?? []) {
        candidates.push({ source: `feature:${finishedSketch}`, item: standIn })
      }
    }
    const accepted = candidates.filter(
      (candidate) => !used.has(candidate.source) && takes(state, accepts, candidate.item),
    )
    const taken = multiple ? accepted : accepted.slice(0, 1)
    const value = selectionValue(
      accepts,
      taken.map((candidate) => candidate.item),
      multiple,
    )
    if (value === null || isEmptyValue(value)) {
      continue
    }
    for (const candidate of taken) {
      used.add(candidate.source)
    }
    next = setParam(feature, next, field.path, value)
    next = withSiblingBody(state, feature, next, field, taken[0]?.item ?? null)
  }
  return next
}

/**
 * Fills the fields beside a picked element (2.12): an empty body beside a face or edge field from
 * the item's body; the sketch beside a point field from the points' sketch, and the body beside
 * them from that sketch's plane when it lies on a face.
 */
function withSiblingBody(
  state: AppState,
  feature: CatalogFeature,
  params: Record<string, unknown>,
  field: InspectorField,
  item: SelectionItem | null,
): Record<string, unknown> {
  if (item === null || field.control.type !== 'selection') {
    return params
  }
  const accepts = field.control.accepts
  const hintOf = (path: string) => feature.manifest.fields.find((hint) => hint.path === path)
  const bodyPath = siblingPath(field.path, 'body')
  if (accepts === 'face' || accepts === 'edge') {
    if (
      item.body !== null &&
      hintOf(bodyPath)?.selection === 'body' &&
      isEmptyValue(getParam(params, bodyPath))
    ) {
      return setParam(feature, params, bodyPath, item.body)
    }
    return params
  }
  if (accepts !== 'sketch_point') {
    return params
  }
  const sketch = item.sketchItem?.sketch ?? null
  const sketchPath = siblingPath(field.path, 'sketch')
  if (sketch === null || hintOf(sketchPath)?.selection !== 'sketch') {
    return params
  }
  let next = setParam(feature, params, sketchPath, sketch)
  const plane: unknown = sketchRecordOf(state, sketch)?.['plane']
  const planeBody =
    typeof plane === 'object' && plane !== null && Reflect.get(plane, 'kind') === 'face'
      ? Reflect.get(plane, 'body')
      : undefined
  if (typeof planeBody === 'string' && hintOf(bodyPath)?.selection === 'body') {
    // A hole drills the body its sketch was drawn on.
    next = setParam(feature, next, bodyPath, planeBody)
  }
  return next
}

/** Options for reopening a command the user can restore (3.7 implicit cancel). */
type RestoreOptions = {
  readonly restore?: SavedCommand
}

/** A command ended implicitly, kept so Restore can reopen it. */
type SavedCommand = {
  readonly featureType: string
  readonly featureId: string
  readonly editing: boolean
  readonly title: string
  readonly params: Params
  readonly helper: HelperSketch | null
  readonly activeField: string | null
  readonly newBody: NewBody | null
}

/** The params of a restored new command with the bodies it had made up taken out, so it makes them again. */
function withoutCreatedBodies(
  app: App,
  session: CommandSession,
  saved: SavedCommand,
): { readonly params: Params; readonly newBody: NewBody | null } {
  const created = saved.newBody
  if (created === null || getParam(saved.params, created.path) !== created.id) {
    return { params: saved.params, newBody: null }
  }
  if (created.path === 'body') {
    return { params: setParam(session.feature, saved.params, 'body', undefined), newBody: null }
  }
  const body = allocateBody(app, session, 'target')
  return { params: setParam(session.feature, saved.params, 'target', body.id), newBody: body }
}

/** Feature command starts in progress, per store: a start waits for the one before it to finish. */
const startsInProgress = new WeakMap<AppStore, Promise<void>>()

/**
 * Runs one start of a feature command after any start still in progress (a double-click, a held
 * key), so the second never begins a transaction while the first is opening one. With nothing in
 * progress it starts at once.
 */
function oneStartAtATime(app: App, start: () => Promise<void>): Promise<void> {
  const previous = startsInProgress.get(app.store)
  const started = previous === undefined ? start() : previous.then(start)
  const done = started.catch(() => undefined)
  startsInProgress.set(app.store, done)
  void done.then(() => {
    if (startsInProgress.get(app.store) === done) {
      startsInProgress.delete(app.store)
    }
  })
  return started
}

/**
 * Whether the active command is this one with no change made in it yet, so starting it again (a
 * double-click, a held key) keeps it rather than reopening it without its preselection.
 */
function alreadyOpen(app: App, featureType: string, editedFeature: string | null): boolean {
  const command = commandOf(app)
  return (
    command !== null &&
    command.history.length === 0 &&
    command.featureType === featureType &&
    (editedFeature === null ? !command.editing : command.featureId === editedFeature)
  )
}

/** Starts a command that creates a feature of this type (3.7). */
export function startFeatureCommand(
  app: App,
  featureType: string,
  options: RestoreOptions = {},
): Promise<void> {
  if (featureType === 'sketch') {
    return startNewSketch(app)
  }
  return oneStartAtATime(app, () => openFeatureCommand(app, featureType, options))
}

async function openFeatureCommand(
  app: App,
  featureType: string,
  options: RestoreOptions,
): Promise<void> {
  if (options.restore === undefined && alreadyOpen(app, featureType, null)) {
    return
  }
  if (commandOf(app) !== null) {
    await cancelCommandImplicitly(app)
  }
  const before = app.store.getState()
  const finishedSketch = before.context === 'sketch' ? (before.sketch?.sketchId ?? null) : null
  if (before.context === 'sketch') {
    await finishSketch(app)
  }
  app.store.setState({ context: 'model' })
  const state = app.store.getState()
  const feature = state.catalog?.byType[featureType]
  const document = state.document
  if (feature === undefined || document === null) {
    return
  }
  try {
    await app.mutations.run(() =>
      app.core.beginTransaction(document.documentHandle, feature.manifest.label),
    )
  } catch (error) {
    showCoreError(app, error)
    return
  }
  const taken = usedIds(document.recipe)
  const featureId = generateId('f', (id) => taken.has(id))
  const lastTargetBody = lastTargetBodyOf(document)
  const session = createSession(app, {
    featureId,
    feature,
    recordExtras: {},
    statusesBefore: document.featureStatus,
    lastTargetBody,
  })
  const saved = options.restore ?? null
  const opening =
    saved === null
      ? openingParams(app, session, state, finishedSketch)
      : restoredOpening(app, session, saved)
  const model = buildInspectorModel({ feature, params: opening.params, errors: {} })
  app.store.setState({
    selection: [],
    // Repeat starts this command again (3.5).
    lastFeatureType: featureType,
    command: {
      featureType,
      featureId,
      editing: false,
      title: feature.manifest.label,
      params: opening.params,
      activeField:
        saved === null ? openingActiveField(model, feature, opening.params) : saved.activeField,
      fieldErrors: {},
      applied: false,
      newBody: opening.newBody,
      helper: saved === null ? null : unappliedHelper(saved.helper),
      changedPaths: [],
      history: [],
      future: [],
      preview: 'none',
    },
  })
  refreshPreview(app)
}

/** The params a new command opens with (3.7 steps 3 and 4), and the body a target starts set to (2.12). */
function openingParams(
  app: App,
  session: CommandSession,
  state: AppState,
  finishedSketch: string | null,
): { readonly params: Params; readonly newBody: NewBody | null } {
  const feature = session.feature
  const document = state.document
  const defaults = defaultParams(feature, {
    bodies: document?.recipe.bodies ?? [],
    bodiesWithGeometry: Object.keys(document?.meshes ?? {}),
    lastTargetBody: session.lastTargetBody,
    document: document?.recipe ?? null,
    featureStatus: document?.featureStatus ?? {},
  })
  const params = preselected(state, feature, defaults, state.selection, finishedSketch)
  const rule = bodyCreatingTarget(feature)
  if (rule === 'always' || (rule === 'op' && params['op'] === 'new')) {
    // Both parts are kept, or the copies go to a new body, until the user says otherwise (2.12).
    const newBody = allocateBody(app, session, 'target')
    return { params: setParam(feature, params, 'target', newBody.id), newBody }
  }
  return { params, newBody: null }
}

/** The params of a restored command, with its helper's ids kept for it (3.7 Restore). */
function restoredOpening(
  app: App,
  session: CommandSession,
  saved: SavedCommand,
): { readonly params: Params; readonly newBody: NewBody | null } {
  if (saved.helper !== null) {
    for (const id of [saved.helper.id, ...pointIdsOf(saved.helper)]) {
      session.issued.add(id)
    }
  }
  return withoutCreatedBodies(app, session, saved)
}

/** A saved helper sketch as the command holds it before the transaction has it again. */
function unappliedHelper(helper: HelperSketch | null): HelperSketch | null {
  return helper === null ? null : { ...helper, applied: false }
}

/** The point entity ids of a helper sketch. */
function pointIdsOf(helper: HelperSketch | null): string[] {
  return (helper?.params.entities ?? []).map((entity) => entity.id)
}

/** The fix a `focus_inactive` refusal offers: unsuppress the feature, or move the marker past it. */
function focusFix(
  app: App,
  record: FeatureRecord,
): { readonly label: string; readonly run: () => void } {
  if (record.suppressed === true) {
    return {
      label: 'Unsuppress',
      run: () => {
        void setSuppressed(app, [record.id], false).then(() => editFeature(app, record.id))
      },
    }
  }
  return {
    label: 'Move marker to the end',
    run: () => {
      void commitEdits(app, 'Roll back', [{ type: 'set_rollback' }]).then((moved) =>
        moved ? editFeature(app, record.id) : undefined,
      )
    },
  }
}

/** Opens the inspector on an existing feature inside a transaction focused on it (3.7). */
export function editFeature(
  app: App,
  featureId: string,
  options: RestoreOptions = {},
): Promise<void> {
  return oneStartAtATime(app, () => openFeatureEdit(app, featureId, options))
}

async function openFeatureEdit(
  app: App,
  featureId: string,
  options: RestoreOptions,
): Promise<void> {
  const document = documentOf(app)
  const record = document?.recipe.features?.find((feature) => feature.id === featureId)
  if (document === null || record === undefined) {
    return
  }
  if (record.type === 'sketch') {
    await enterSketch(app, featureId)
    return
  }
  if (record.type === 'import') {
    notify(app, {
      severity: 'info',
      message: MESSAGES.import_not_editable(),
      actions: [],
      code: 'import_not_editable',
    })
    return
  }
  const state = app.store.getState()
  const feature = state.catalog?.byType[record.type]
  if (feature === undefined) {
    return
  }
  if (options.restore === undefined && alreadyOpen(app, record.type, featureId)) {
    return
  }
  if (commandOf(app) !== null) {
    await cancelCommandImplicitly(app)
  }
  if (app.store.getState().context === 'sketch') {
    await finishSketch(app)
  }
  const label = featureLabel(document.recipe, featureId, state.catalog)
  const handle = document.documentHandle
  try {
    await app.mutations.run(() => app.core.beginTransaction(handle, `Edit ${label}`))
  } catch (error) {
    showCoreError(app, error)
    return
  }
  try {
    await app.mutations.run(() => app.core.setFocus(handle, featureId))
  } catch (error) {
    await app.mutations
      .run(() => app.core.cancelTransaction(handle))
      .catch((cancelError: unknown) => {
        showCoreError(app, cancelError)
      })
    if (error instanceof CoreError && error.code === 'focus_inactive') {
      notify(app, {
        severity: 'error',
        message: error.message,
        actions: [focusFix(app, record)],
        code: error.code,
      })
      return
    }
    showCoreError(app, error)
    return
  }
  const recordExtras: Record<string, unknown> = {}
  const params: Record<string, unknown> = {}
  for (const [key, value] of Object.entries(record)) {
    if (RECORD_EXTRAS.has(key)) {
      recordExtras[key] = value
    } else if (key !== 'id' && key !== 'type') {
      params[key] = value
    }
  }
  const session = createSession(app, {
    featureId,
    feature,
    recordExtras,
    statusesBefore: documentOf(app)?.featureStatus ?? {},
    lastTargetBody: lastTargetBodyOf(document),
  })
  // The record is applied already: it is the focused feature.
  session.submitted = params
  session.repairs = repairSlots(feature, params, document.featureStatus[featureId])
  const saved = options.restore ?? null
  const restored = saved === null ? null : restoredOpening(app, session, saved)
  app.store.setState({
    context: 'model',
    command: {
      featureType: record.type,
      featureId,
      editing: true,
      title: label,
      params: restored?.params ?? params,
      // A lost reference is picked again in its field (4.2), so that field takes the picks.
      activeField:
        saved === null
          ? (repairField(feature.manifest.fields, document.featureStatus[featureId])?.path ?? null)
          : saved.activeField,
      fieldErrors: {},
      applied: true,
      newBody: restored?.newBody ?? null,
      helper: saved === null ? null : unappliedHelper(saved.helper),
      changedPaths: [],
      history: [],
      future: [],
      preview: previewStateAfter(app, featureId),
    },
  })
  if (saved !== null) {
    refreshPreview(app)
  }
}

/** The helper after a change of the params: dropped once the point field holds none of its points. */
function helperFor(
  feature: CatalogFeature,
  params: Params,
  helper: HelperSketch | null,
): { readonly params: Params; readonly helper: HelperSketch | null } {
  if (helper === null) {
    return { params, helper }
  }
  const pointField = feature.manifest.fields.find((hint) => hint.selection === 'sketch_point')
  if (pointField === undefined) {
    return { params, helper: null }
  }
  const sketchPath = siblingPath(pointField.path, 'sketch')
  const points = getParam(params, pointField.path)
  const held = new Set(
    getParam(params, sketchPath) === helper.id && Array.isArray(points) ? points : [],
  )
  const all = helper.params.entities ?? []
  const entities = all.filter((entity) => held.has(entity.id))
  if (entities.length === 0) {
    // The helper goes; a sketch field still naming it is cleared with it.
    const cleared =
      getParam(params, sketchPath) === helper.id
        ? setParam(feature, params, sketchPath, undefined)
        : params
    return { params: cleared, helper: null }
  }
  return entities.length === all.length
    ? { params, helper }
    : { params, helper: { ...helper, params: { ...helper.params, entities } } }
}

/** Clears a body field that the first pick of a face or edge field filled once that field is empty (2.12). */
function withoutFilledBody(session: CommandSession, params: Params, path: string): Params {
  const bodyPath = session.filledBodies.get(path)
  if (bodyPath === undefined || !isEmptyValue(getParam(params, path))) {
    return params
  }
  session.filledBodies.delete(path)
  return setParam(session.feature, params, bodyPath, undefined)
}

/** The params after Operation changed, with the rules it triggers (3.7, 2.12). */
function afterOperationChange(
  app: App,
  session: CommandSession,
  command: ActiveCommand,
  params: Params,
  value: unknown,
): { readonly params: Params; readonly newBody: NewBody | null } {
  const feature = session.feature
  const previous = command.params['op']
  let next = params
  let newBody = command.newBody
  const rule = bodyCreatingTarget(feature)
  const hasBodyField = feature.manifest.fields.some(
    (hint) => hint.path === 'body' && hint.selection === 'body',
  )
  if (rule === 'op') {
    // Op new puts the copies in a new body; leaving it lets them join again (2.12).
    if (value === 'new' && previous !== 'new') {
      newBody = allocateBody(app, session, 'target')
      next = setParam(feature, next, 'target', newBody.id)
    } else if (value !== 'new' && previous === 'new') {
      next = setParam(feature, next, 'target', undefined)
      newBody = null
    }
  } else if (rule === null && hasBodyField) {
    // Leaving op new targets the last body again; returning to it makes a fresh body (3.7).
    if (value === 'new') {
      next = setParam(feature, next, 'body', undefined)
    } else if (previous === 'new') {
      next = setParam(feature, next, 'body', session.lastTargetBody ?? undefined)
    }
  }
  return { params: withCutDirection(app, session, command, next, value), newBody }
}

/**
 * The cut direction (3.7): in an Extrude whose profile is sketched on a face of its body, Cut and
 * Intersect point into the part and Join and New body out of it, until the user sets Reverse.
 */
function withCutDirection(
  app: App,
  session: CommandSession,
  command: ActiveCommand,
  params: Params,
  op: unknown,
): Params {
  if (session.feature.type !== 'extrude' || command.changedPaths.includes('reverse')) {
    return params
  }
  const sketch = getParam(params, 'profile.sketch')
  const plane: unknown = sketchRecordOf(
    app.store.getState(),
    typeof sketch === 'string' ? sketch : null,
  )?.['plane']
  if (typeof plane !== 'object' || plane === null || Reflect.get(plane, 'kind') !== 'face') {
    return params
  }
  const planeBody: unknown = Reflect.get(plane, 'body')
  if (planeBody !== command.params['body'] && planeBody !== params['body']) {
    return params
  }
  if (op === 'cut' || op === 'intersect') {
    return setParam(session.feature, params, 'reverse', true)
  }
  if (op === 'join' || op === 'new') {
    return setParam(session.feature, params, 'reverse', false)
  }
  return params
}

export type CommandFieldOptions = {
  /**
   * A text edit of an expression field: only the first one since the field gained focus is a
   * change of the command history (3.7).
   */
  readonly typing?: boolean
}

/** Changes one field of the active command, records the change and previews the result. */
export function setCommandField(
  app: App,
  path: string,
  value: unknown,
  options: CommandFieldOptions = {},
): void {
  const session = sessionOf(app)
  const command = commandOf(app)
  if (session === null || command === null) {
    return
  }
  session.changedAt = performance.now()
  const typing = options.typing === true
  const record = !typing || session.typingField !== path
  session.typingField = typing ? path : null
  const written = setParam(session.feature, command.params, path, value)
  const operation =
    path === 'op' && value !== command.params['op']
      ? afterOperationChange(app, session, command, written, value)
      : { params: written, newBody: command.newBody }
  let params = operation.params
  let newBody = operation.newBody
  if (path === 'target' && newBody !== null && getParam(params, 'target') !== newBody.id) {
    // A cleared or replaced target no longer holds the body this command made (2.12).
    newBody = null
  }
  params = withoutFilledBody(session, params, path)
  const helped = helperFor(session.feature, params, command.helper)
  updateCommand(app, () => ({
    params: helped.params,
    newBody,
    helper: helped.helper,
    ...recordedChange(command, path, fieldLabel(session, path), record),
  }))
  refreshPreview(app)
}

/** Notes that an expression field gained focus, so its next text edit is a new change (3.7). */
export function noteFieldFocused(app: App, path: string): void {
  const session = sessionOf(app)
  if (session !== null && session.typingField === path) {
    session.typingField = null
  }
}

/** Sets a body-creating target to a new body, the Choose menu's "New body" (2.12, 3.7). */
export function chooseNewBody(app: App, path: string): void {
  const session = sessionOf(app)
  const command = commandOf(app)
  if (session === null || command === null || bodyCreatingTarget(session.feature) === null) {
    return
  }
  const body = allocateBody(app, session, 'target')
  session.changedAt = performance.now()
  updateCommand(app, () => ({
    params: setParam(session.feature, command.params, path, body.id),
    newBody: body,
    ...recordedChange(command, path, fieldLabel(session, path), true),
  }))
  refreshPreview(app)
}

/**
 * The fix "Reverse and use {text}" (4.2): toggles Reverse and sets the field it is attached to, as
 * one change.
 */
export function reverseAndUse(app: App, path: string, text: string): void {
  const session = sessionOf(app)
  const command = commandOf(app)
  if (session === null || command === null) {
    return
  }
  session.changedAt = performance.now()
  const reversed = getParam(command.params, 'reverse') === true
  let params = setParam(session.feature, command.params, 'reverse', !reversed)
  params = setParam(session.feature, params, path, text)
  const change = recordedChange(command, path, fieldLabel(session, path), true)
  updateCommand(app, () => ({
    params,
    ...change,
    changedPaths: [...change.changedPaths.filter((changed) => changed !== 'reverse'), 'reverse'],
  }))
  refreshPreview(app)
}

/**
 * Records the inspector's check of a field: an error to show (null when valid) and whether a
 * result is still pending. Errors and pending fields keep the params from being previewed.
 */
export function setFieldStatus(
  app: App,
  path: string,
  status: { readonly error: string | null; readonly pending: boolean },
): void {
  const session = sessionOf(app)
  if (session === null) {
    return
  }
  if (status.pending) {
    session.pendingFields.add(path)
  } else {
    session.pendingFields.delete(path)
  }
  updateCommand(app, (command) => {
    const others = Object.fromEntries(
      Object.entries(command.fieldErrors).filter(([fieldPath]) => fieldPath !== path),
    )
    return { fieldErrors: status.error === null ? others : { ...others, [path]: status.error } }
  })
  refreshPreview(app)
}

/** Makes a selection field receive picks, or none. */
export function setActiveField(app: App, path: string | null): void {
  updateCommand(app, () => ({ activeField: path }))
}

/** Notes the selection field whose box the pointer is over, so the viewport highlights its items (3.7). */
export function setHoveredField(app: App, path: string | null): void {
  updateCommand(app, (command) =>
    (command.hoveredField ?? null) === path ? {} : { hoveredField: path },
  )
}

/** The hint for a pick the active field refuses (3.7). */
function refusalHint(accepts: SelectionKind): string {
  switch (accepts) {
    case 'plane':
      return MESSAGES.pick_plane()
    case 'axis':
      return MESSAGES.pick_axis()
    case 'sketch_point':
      return MESSAGES.pick_points()
    default:
      return MESSAGES.pick_kind(SELECTION_NOUNS[accepts])
  }
}

/** A property of an object value, or undefined. */
function propertyOf(value: unknown, key: string): unknown {
  return typeof value === 'object' && value !== null ? Reflect.get(value, key) : undefined
}

/** The lost references of a feature's status as slots of its selection fields (4.2). */
function repairSlots(
  feature: CatalogFeature,
  params: Params,
  status: FeatureStatus | undefined,
): readonly RepairSlot[] {
  const paths = feature.manifest.fields
    .filter((hint) => (hint.selection ?? null) !== null)
    .map((hint) => hint.path)
  return (status?.repairs ?? []).flatMap((repair): RepairSlot[] => {
    const field = fieldForPointer(paths, repair.path)
    if (field === null) {
      return []
    }
    const segments = repair.path
      .slice(fieldPointer(field).length)
      .split('/')
      .filter((segment) => segment !== '')
    // A whole single value is replaced by any pick already; only an entry of a list needs a slot.
    return segments.length === 0
      ? []
      : [{ field, segments, lost: valueAt(getParam(params, field), segments) }]
  })
}

/** The value at a path of segments inside a value, or undefined. */
function valueAt(value: unknown, segments: readonly string[]): unknown {
  let current = value
  for (const segment of segments) {
    if (Array.isArray(current)) {
      current = current[Number(segment)]
    } else {
      current = propertyOf(current, segment)
    }
  }
  return current
}

/** The repair slot of a field whose lost reference is still in its value, or null. */
function pendingRepair(session: CommandSession, field: string, value: unknown): RepairSlot | null {
  return (
    session.repairs.find(
      (slot) =>
        slot.field === field &&
        slot.lost !== undefined &&
        sameValue(valueAt(value, slot.segments), slot.lost),
    ) ?? null
  )
}

/**
 * The list a repair slot sits in with its lost entry replaced by `entries` (4.2): entries the list
 * already holds elsewhere are not added twice. Null when the slot's list cannot be found.
 */
function replacedInList(
  value: unknown,
  slot: RepairSlot,
  entries: readonly unknown[],
): unknown[] | null {
  const list = valueAt(value, slot.segments.slice(0, -1))
  const index = Number(slot.segments.at(-1))
  if (!Array.isArray(list) || !Number.isInteger(index) || index < 0 || index >= list.length) {
    return null
  }
  const others = list.filter((_entry, position) => position !== index)
  const added = entries.filter((entry) => !others.some((other) => sameValue(other, entry)))
  return [...others.slice(0, index), ...added, ...others.slice(index)]
}

/** A value with the list a repair slot sits in replaced. */
function withList(value: unknown, slot: RepairSlot, list: readonly unknown[]): unknown {
  const parents = slot.segments.slice(0, -1)
  if (parents.length === 0) {
    return list
  }
  // The lists of selection values sit one level down at most (a profile's `regions`).
  const [key] = parents
  return typeof value === 'object' && value !== null && key !== undefined
    ? { ...value, [key]: list }
    : value
}

/**
 * The field's value after a pick that replaces a lost reference (3.7, 4.2): the picked item takes
 * the lost entry's place. Null when no lost reference of this field is waiting, or when the pick
 * cannot go in its list (a region of another sketch replaces the whole profile instead).
 */
function repairedValue(
  session: CommandSession,
  accepts: SelectionKind,
  multiple: boolean,
  field: string,
  current: unknown,
  item: SelectionItem,
): { readonly value: unknown } | null {
  const slot = pendingRepair(session, field, current)
  if (slot === null) {
    return null
  }
  const picked = selectionValue(accepts, [item], multiple)
  if (
    accepts === 'sketch_region' &&
    propertyOf(picked, 'sketch') !== propertyOf(current, 'sketch')
  ) {
    return null
  }
  const entries = valueAt(picked, slot.segments.slice(0, -1))
  if (!Array.isArray(entries)) {
    return null
  }
  const list = replacedInList(current, slot, entries)
  return list === null ? null : { value: withList(current, slot, list) }
}

/** The field's value with the picked item toggled (added, replacing a single value, or removed). */
function toggledValue(
  accepts: SelectionKind,
  multiple: boolean,
  current: unknown,
  item: SelectionItem,
  currentSketch: unknown,
): unknown {
  const picked = selectionValue(accepts, [item], multiple)
  if (accepts === 'sketch_region') {
    // A region of another sketch replaces the profile (2.12).
    const sketch = propertyOf(current, 'sketch')
    const regions = propertyOf(current, 'regions')
    const pickedRegions = propertyOf(picked, 'regions')
    if (
      sketch !== propertyOf(picked, 'sketch') ||
      !Array.isArray(regions) ||
      !Array.isArray(pickedRegions)
    ) {
      return picked
    }
    const toggled = toggledList(regions, pickedRegions)
    return toggled.length === 0 ? undefined : { sketch, regions: toggled }
  }
  if (accepts === 'sketch_point' && item.sketchItem?.sketch !== currentSketch) {
    // A point of another sketch replaces the list (2.12).
    return picked
  }
  if (multiple) {
    const list = Array.isArray(current) ? current : []
    const additions = Array.isArray(picked) ? picked : [picked]
    const toggled = toggledList(list, additions)
    return toggled.length === 0 ? undefined : toggled
  }
  return sameValue(current, picked) ? undefined : picked
}

function toggledList(list: readonly unknown[], additions: readonly unknown[]): unknown[] {
  let toggled = [...list]
  for (const addition of additions) {
    toggled = toggled.some((entry) => sameValue(entry, addition))
      ? toggled.filter((entry) => !sameValue(entry, addition))
      : [...toggled, addition]
  }
  return toggled
}

/** Shows `pick_same_body` when a face or edge pick lies on another body than the field's (2.12). */
function refusedForBody(
  app: App,
  field: InspectorField,
  accepts: SelectionKind,
  params: Params,
  body: string | null,
): boolean {
  if ((accepts !== 'face' && accepts !== 'edge') || body === null) {
    return false
  }
  const current = getParam(params, siblingPath(field.path, 'body'))
  if (typeof current !== 'string' || current === '' || current === body) {
    return false
  }
  const name =
    documentOf(app)?.recipe.bodies?.find((candidate) => candidate.id === current)?.name ?? current
  showPickHint(app, MESSAGES.pick_same_body(SELECTION_NOUNS[accepts], name), field.path)
  return true
}

/**
 * The params with a pick's new value in the field and the sibling rules of 2.12 applied; a body
 * the first pick fills is remembered so emptying the field clears it again.
 */
function pickedParams(
  app: App,
  session: CommandSession,
  command: ActiveCommand,
  field: InspectorField,
  value: unknown,
  item: SelectionItem | null,
): Params {
  let params: Params = setParam(session.feature, command.params, field.path, value)
  if (value !== undefined && item !== null) {
    const bodyPath = siblingPath(field.path, 'body')
    const bodyBefore = getParam(params, bodyPath)
    params = withSiblingBody(app.store.getState(), session.feature, { ...params }, field, item)
    if (
      isEmptyValue(field.value) &&
      isEmptyValue(bodyBefore) &&
      !isEmptyValue(getParam(params, bodyPath))
    ) {
      session.filledBodies.set(field.path, bodyPath)
    }
  }
  return withoutFilledBody(session, params, field.path)
}

/**
 * Stores a pick's result, records it as one change and moves the active field on (3.7 selection
 * capture): a single field that received its item, or a multiple field its first, hands the
 * picks to the next required empty selection field.
 */
function commitPick(
  app: App,
  session: CommandSession,
  command: ActiveCommand,
  field: InspectorField,
  params: Params,
  helper: HelperSketch | null,
): void {
  if (field.control.type !== 'selection') {
    return
  }
  session.changedAt = performance.now()
  const helped = helperFor(session.feature, params, helper)
  const filled = !isEmptyValue(getParam(helped.params, field.path))
  const nextModel = buildInspectorModel({
    feature: session.feature,
    params: helped.params,
    errors: command.fieldErrors,
  })
  const next = firstEmptySelectionField(nextModel, session.feature, helped.params)
  const first = filled && isEmptyValue(field.value)
  const activeField = !filled
    ? command.activeField
    : !field.control.multiple
      ? next
      : first
        ? (next ?? command.activeField)
        : command.activeField
  const newBody = command.newBody
  updateCommand(app, () => ({
    params: helped.params,
    helper: helped.helper,
    activeField,
    // A target picked from the existing bodies no longer holds the body this command made (2.12).
    newBody:
      newBody !== null && getParam(helped.params, newBody.path) !== newBody.id ? null : newBody,
    ...recordedChange(command, field.path, field.label, true),
  }))
  refreshPreview(app)
}

/** A world point rounded into a placement's sketch coordinates. */
function sketchCoordinates(placement: Placement, point: readonly [number, number, number]) {
  const [x, y] = worldToSketch(placement, point)
  const factor = 10 ** CENTER_DECIMALS
  const rounded = (value: number) => Math.round(value * factor) / factor + 0
  return [rounded(x), rounded(y)] as [number, number]
}

/** Where a click's ray meets a face's plane, snapped to the centroid within 8 CSS pixels (3.7). */
function holeCenter(element: ElementDescriptor, placement: Placement, pointer?: PointerRay) {
  const centroid = element.centroid
  if (pointer === undefined) {
    return sketchCoordinates(placement, centroid)
  }
  const along = dot(pointer.direction, placement.normal)
  const hit =
    Math.abs(along) < 1e-12
      ? centroid
      : addScaled(
          pointer.origin,
          pointer.direction,
          dot(subtract(placement.origin, pointer.origin), placement.normal) / along,
        )
  const snapped =
    length(subtract(hit, centroid)) <= CENTROID_SNAP_PIXELS * pointer.millimetersPerPixel
      ? centroid
      : hit
  return sketchCoordinates(placement, snapped)
}

/** Whether a helper sketch lies on this face. */
function helperOnFace(helper: HelperSketch, element: ElementDescriptor): boolean {
  const plane = helper.params.plane
  return (
    plane.kind === 'face' && plane.body === element.body && sameValue(plane.face, element.reference)
  )
}

/**
 * Places a hole center where a click met a flat face (3.7): the first click gives the command a
 * helper sketch on that face, further clicks on it add points, and another face is refused.
 */
function placeHoleOnFace(
  app: App,
  session: CommandSession,
  command: ActiveCommand,
  field: InspectorField,
  item: SelectionItem,
  pointer: PointerRay | undefined,
): void {
  const element = item.element
  const plane = sketchPlaneOf(item)
  if (element === null || plane?.kind !== 'face') {
    showPickHint(app, MESSAGES.pick_points(), field.path)
    return
  }
  const current = command.helper
  const points = getParam(command.params, field.path)
  const sketchPath = siblingPath(field.path, 'sketch')
  const helperInUse =
    current !== null &&
    getParam(command.params, sketchPath) === current.id &&
    Array.isArray(points) &&
    points.length > 0
  if (helperInUse && !helperOnFace(current, element)) {
    showPickHint(app, MESSAGES.hole_one_face(), field.path)
    return
  }
  const entity: SketchEntity = {
    id: issueId(app, session, 'e'),
    type: 'point',
    position: holeCenter(element, plane.fallback, pointer),
  }
  const helper: HelperSketch = helperInUse
    ? {
        ...current,
        params: { ...current.params, entities: [...(current.params.entities ?? []), entity] },
      }
    : {
        id: issueId(app, session, 'f'),
        params: { plane, entities: [entity], constraints: [] },
        applied: false,
      }
  const value = helperInUse && Array.isArray(points) ? [...points, entity.id] : [entity.id]
  let params: Params = setParam(session.feature, command.params, field.path, value)
  params = setParam(session.feature, params, sketchPath, helper.id)
  const bodyPath = siblingPath(field.path, 'body')
  if (
    session.feature.manifest.fields.some(
      (hint) => hint.path === bodyPath && hint.selection === 'body',
    )
  ) {
    // The hole drills the body of the face.
    params = setParam(session.feature, params, bodyPath, element.body)
  }
  commitPick(app, session, command, field, params, helper)
}

/** Edge lists by body for faces picked as their edges, kept per model-layer mesh id (3.7, 3.6). */
const edgeLists = new WeakMap<
  AppStore,
  Map<string, { readonly meshId: number; readonly edges: ReadonlyMap<string, ElementDescriptor> }>
>()

/** The edges of a body by name, listed once per model-layer mesh. */
async function edgesOf(
  app: App,
  body: string,
): Promise<ReadonlyMap<string, ElementDescriptor> | null> {
  const document = documentOf(app)
  const meshId = document?.meshes[body]?.meshId
  if (document === null || meshId === undefined) {
    return null
  }
  let lists = edgeLists.get(app.store)
  if (lists === undefined) {
    lists = new Map()
    edgeLists.set(app.store, lists)
  }
  const cached = lists.get(body)
  if (cached?.meshId === meshId) {
    return cached.edges
  }
  const listed = await app.core.listElements(document.documentHandle, body, 'edge')
  const edges = new Map(listed.map((edge) => [edge.name, edge]))
  lists.set(body, { meshId, edges })
  return edges
}

/**
 * Faces for edges (3.7): a face picked into a multiple edge field adds every edge around it that
 * the field does not hold, or removes them all when it holds every one.
 */
async function pickFaceEdges(
  app: App,
  session: CommandSession,
  field: InspectorField,
  item: SelectionItem,
): Promise<void> {
  const element = item.element
  if (element === null || item.body === null) {
    return
  }
  let edges: ReadonlyMap<string, ElementDescriptor> | null
  try {
    edges = await edgesOf(app, item.body)
  } catch (error) {
    showCoreError(app, error)
    return
  }
  const command = commandOf(app)
  if (edges === null || command === null || sessionOf(app) !== session) {
    return
  }
  if (refusedForBody(app, field, 'edge', command.params, item.body)) {
    return
  }
  const around = element.adjacent.flatMap((name) => {
    const edge = edges.get(name)
    return edge === undefined ? [] : [edge.reference]
  })
  if (around.length === 0) {
    return
  }
  const current = getParam(command.params, field.path)
  const list: readonly unknown[] = Array.isArray(current) ? current : []
  const repair = pendingRepair(session, field.path, current)
  const holdsAll = around.every((reference) => list.some((entry) => sameValue(entry, reference)))
  // A face picked to replace a lost edge puts its edges in that edge's place (4.2).
  const replaced = repair === null ? null : replacedInList(current, repair, around)
  const toggled =
    replaced ??
    (holdsAll
      ? list.filter((entry) => !around.some((reference) => sameValue(entry, reference)))
      : [
          ...list,
          ...around.filter((reference) => !list.some((entry) => sameValue(entry, reference))),
        ])
  const latestField =
    fieldsOf(modelOf(session, command)).find((candidate) => candidate.path === field.path) ?? field
  const params = pickedParams(
    app,
    session,
    command,
    latestField,
    toggled.length === 0 ? undefined : toggled,
    item,
  )
  commitPick(app, session, command, latestField, params, command.helper)
}

/** Feeds a pick to the active selection field (3.7 selection capture). */
export async function capturePick(
  app: App,
  item: SelectionItem,
  pointer?: PointerRay,
): Promise<void> {
  const session = sessionOf(app)
  const command = commandOf(app)
  if (session === null || command === null || command.activeField === null) {
    return
  }
  const model = modelOf(session, command)
  const field = fieldsOf(model).find((candidate) => candidate.path === command.activeField)
  if (field === undefined || field.control.type !== 'selection') {
    return
  }
  const { accepts, multiple } = field.control
  if (accepts === 'sketch_point' && item.kind === 'face') {
    if (item.element?.surface !== 'plane') {
      showPickHint(app, refusalHint(accepts), field.path)
      return
    }
    placeHoleOnFace(app, session, command, field, item, pointer)
    return
  }
  if (accepts === 'edge' && multiple && item.kind === 'face') {
    await pickFaceEdges(app, session, field, item)
    return
  }
  const state = app.store.getState()
  if (!takes(state, accepts, item)) {
    showPickHint(app, refusalHint(accepts), field.path)
    return
  }
  if (refusedForBody(app, field, accepts, command.params, item.body)) {
    return
  }
  const currentSketch = getParam(command.params, siblingPath(field.path, 'sketch'))
  const repaired = repairedValue(session, accepts, multiple, field.path, field.value, item)
  const value =
    repaired === null
      ? toggledValue(accepts, multiple, field.value, item, currentSketch)
      : repaired.value
  const params = pickedParams(app, session, command, field, value, item)
  commitPick(app, session, command, field, params, command.helper)
}

/** Every sketch a value refers to through a `sketch` key (a ProfileRef, an AxisRef, a hole's sketch). */
function referencedSketches(value: unknown): string[] {
  if (Array.isArray(value)) {
    return value.flatMap(referencedSketches)
  }
  if (typeof value !== 'object' || value === null) {
    return []
  }
  return Object.entries(value).flatMap(([key, entry]) =>
    key === 'sketch' && typeof entry === 'string' ? [entry] : referencedSketches(entry),
  )
}

/** OK: applies the latest params and commits the transaction as one undo entry (3.7). */
export async function confirmCommand(app: App): Promise<void> {
  const session = sessionOf(app)
  const command = commandOf(app)
  const document = documentOf(app)
  if (session === null || command === null || document === null) {
    return
  }
  if (session.confirming || !previewable(app, session, command)) {
    return
  }
  session.confirming = true
  try {
    if (!sameValue(session.submitted, command.params) || helperChanged(session, command)) {
      submitPreview(session, command.params)
    }
    await session.preview.flush()
    if (session.failed || sessionOf(app) !== session) {
      return
    }
    await app.mutations.run(() => app.core.commitTransaction(document.documentHandle))
  } catch (error) {
    showCoreError(app, error)
    return
  } finally {
    session.confirming = false
  }
  const params = commandOf(app)?.params ?? command.params
  endSession(app)
  for (const sketch of new Set(referencedSketches(params))) {
    setSessionVisible(app, `feature:${sketch}`, false)
  }
  reportNewFailures(app, session.statusesBefore)
}

/** Cancel: reverts the transaction; nothing is recorded (3.7). */
export async function cancelCommand(app: App): Promise<void> {
  const command = commandOf(app)
  const document = documentOf(app)
  if (command === null) {
    return
  }
  sessions.get(app.store)?.preview.cancel()
  endSession(app)
  if (document === null) {
    return
  }
  try {
    await app.mutations.run(() => app.core.cancelTransaction(document.documentHandle))
  } catch (error) {
    showCoreError(app, error)
  }
}

/** Reopens a command that ended implicitly, canceling whatever command is active (3.7 Restore). */
async function restoreCommand(app: App, saved: SavedCommand): Promise<void> {
  if (commandOf(app) !== null) {
    await cancelCommand(app)
  }
  if (saved.editing) {
    await editFeature(app, saved.featureId, { restore: saved })
  } else {
    await startFeatureCommand(app, saved.featureType, { restore: saved })
  }
}

/**
 * Cancels a command that ends because the user did something else (another command, an edit, a
 * sketch, a context change). A command with changes offers Restore in `command_canceled` (3.7).
 */
export async function cancelCommandImplicitly(app: App): Promise<void> {
  const command = commandOf(app)
  if (command === null) {
    return
  }
  const saved: SavedCommand | null =
    command.history.length === 0
      ? null
      : {
          featureType: command.featureType,
          featureId: command.featureId,
          editing: command.editing,
          title: command.title,
          params: command.params,
          helper: command.helper,
          activeField: command.activeField,
          newBody: command.newBody,
        }
  await cancelCommand(app)
  if (saved === null) {
    return
  }
  notify(app, {
    severity: 'info',
    message: MESSAGES.command_canceled(saved.title),
    code: 'command_canceled',
    actions: [
      {
        label: 'Restore',
        run: () => {
          restoreCommand(app, saved).catch((error: unknown) => {
            console.error('Restoring the command failed.', error)
          })
        },
      },
    ],
  })
}

/** The created body a restored value names, if this command made it up. */
function restoredBody(session: CommandSession, params: Params): NewBody | null {
  for (const body of session.createdBodies.values()) {
    if (getParam(params, body.path) === body.id) {
      return body
    }
  }
  return null
}

/** Puts a recorded command state back and previews it. */
function restoreStep(
  app: App,
  session: CommandSession,
  step: CommandStep,
  change: Pick<ActiveCommand, 'history' | 'future'>,
): void {
  session.changedAt = performance.now()
  session.typingField = null
  updateCommand(app, () => ({
    params: step.params,
    helper:
      step.helper === null
        ? null
        : { ...step.helper, applied: step.helper.id === session.appliedHelper },
    activeField: step.activeField,
    newBody: restoredBody(session, step.params),
    ...change,
  }))
  refreshPreview(app)
}

/** Mod+Z in a command (3.7, 3.20): undoes the last change, or cancels the command when there is none. */
export async function undoCommandChange(app: App): Promise<void> {
  const session = sessionOf(app)
  const command = commandOf(app)
  if (session === null || command === null) {
    return
  }
  const step = command.history.at(-1)
  if (step === undefined) {
    await cancelCommand(app)
    return
  }
  const current: CommandStep = {
    params: command.params,
    helper: command.helper,
    activeField: command.activeField,
    field: step.field,
  }
  restoreStep(app, session, step, {
    history: command.history.slice(0, -1),
    future: [...command.future, current],
  })
}

/** Mod+Shift+Z in a command (3.7, 3.20): redoes the last undone change. */
export async function redoCommandChange(app: App): Promise<void> {
  const session = sessionOf(app)
  const command = commandOf(app)
  const step = command?.future.at(-1)
  if (session === null || command === null || step === undefined) {
    return
  }
  const current: CommandStep = {
    params: command.params,
    helper: command.helper,
    activeField: command.activeField,
    field: step.field,
  }
  restoreStep(app, session, step, {
    history: [...command.history, current],
    future: command.future.slice(0, -1),
  })
}

/**
 * The command's value field (3.7 typing values): the expression field the user changed last, else
 * the first essential expression field; null without one.
 */
export function commandValueField(state: AppState): string | null {
  const command = state.command
  const feature = command === null ? undefined : state.catalog?.byType[command.featureType]
  if (command === null || feature === undefined) {
    return null
  }
  const model = buildInspectorModel({ feature, params: command.params, errors: {} })
  const expressions = [...model.essential, ...model.more].filter(
    (field) => field.control.type === 'expression',
  )
  const changed = command.changedPaths.findLast((path) =>
    expressions.some((field) => field.path === path),
  )
  return (
    changed ?? model.essential.find((field) => field.control.type === 'expression')?.path ?? null
  )
}

/** The params a command started from, without the reuse rule: what More options compares with (2.12). */
export const commandStartParams: (state: AppState) => Params | null = memoized(
  (state: AppState) =>
    [
      state.command === null ? undefined : state.catalog?.byType[state.command.featureType],
      state.document?.recipe.bodies,
      state.document?.meshes,
    ] as const,
  (feature, bodies, meshes) =>
    feature === undefined || meshes === undefined
      ? null
      : defaultParams(feature, {
          bodies: bodies ?? [],
          bodiesWithGeometry: Object.keys(meshes),
          lastTargetBody: null,
          document: null,
          featureStatus: {},
        }),
)

/**
 * Applies document edits that join the open command's transaction, such as parameter changes
 * (3.7): the preview shows `pending` until their events arrive. Resolves whether the core took them.
 */
export async function applyEditsInCommand(
  app: App,
  edits: readonly Edit[],
  onRefused?: (message: string) => void,
): Promise<boolean> {
  return runInCommand(app, (handle) => app.core.applyEdits(handle, null, edits), onRefused)
}

/** Runs a document change that joins the open command's transaction, with the preview pending meanwhile. */
export async function runInCommand(
  app: App,
  task: (documentHandle: DocHandle) => Promise<unknown>,
  onRefused?: (message: string) => void,
): Promise<boolean> {
  const document = documentOf(app)
  const command = commandOf(app)
  if (document === null || command === null) {
    return false
  }
  const before = command.preview
  updateCommand(app, () => ({ preview: 'pending' }))
  try {
    await app.mutations.run(() => task(document.documentHandle))
  } catch (error) {
    updateCommand(app, () => ({ preview: before }))
    showCoreError(app, error, onRefused === undefined ? {} : { onRefused })
    return false
  }
  const session = sessionOf(app)
  updateCommand(app, (current) => ({
    preview: !current.applied
      ? before
      : session !== null && !previewable(app, session, current)
        ? 'none'
        : previewStateAfter(app, current.featureId),
  }))
  return true
}
