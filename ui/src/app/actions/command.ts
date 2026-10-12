import { CoreError } from '../../core/client'
import { generateId, usedIds } from '../../core/ids'
import { featureLabel, nextBodyName } from '../../core/labels'
import { createLatestOnly, type LatestOnly } from '../../core/queue'
import type { Edit, FeatureRecord, FeatureStatus, SelectionKind } from '../../generated/protocol'
import {
  buildInspectorModel,
  defaultParams,
  getParam,
  setParam,
  type InspectorField,
  type InspectorModel,
} from '../../inspector/model'
import { acceptsTarget, selectionValue } from '../../inspector/selection-values'
import { MESSAGES } from '../../messages'
import type {
  ActiveCommand,
  AppState,
  AppStore,
  CatalogFeature,
  DocumentState,
  SelectionItem,
} from '../../state/store'
import type { App } from '../createApp'
import { commitEdits, documentOf, reportNewFailures, showCoreError } from './edits'
import { notify } from './notifications'
import { enterSketch, finishSketch, startNewSketch } from './sketch'
import { setSessionVisible } from './bodies'
import { setSuppressed } from './timeline'

// Feature commands (ui.md 3.7): an inspector over one feature record inside a transaction, with a
// live preview of every complete state of its params.

type Params = Readonly<Record<string, unknown>>

/** What a command keeps beside its store state. */
type CommandSession = {
  readonly featureId: string
  readonly feature: CatalogFeature
  /** The preview requests, one at a time with only the newest waiting. */
  readonly preview: LatestOnly<Params>
  /** Record properties that are not params (`name`, `suppressed`), kept on every update. */
  readonly recordExtras: Readonly<Record<string, unknown>>
  /** Feature statuses before the command, to report a failure it causes. */
  readonly statusesBefore: Readonly<Record<string, FeatureStatus>>
  readonly lastTargetBody: string | null
  /** Fields whose value is still being checked (an expression awaiting evaluation). */
  readonly pendingFields: Set<string>
  /** The params of the newest preview request, to tell whether the latest params were sent. */
  submitted: Params | null
  /** Whether the newest preview request failed in the core. */
  failed: boolean
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
  const dot = path.lastIndexOf('.')
  return dot === -1 ? '' : path.slice(0, dot)
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

/** The first required, empty selection field, or null (3.7 step 5). */
function firstEmptySelectionField(model: InspectorModel, feature: CatalogFeature): string | null {
  const fields = selectionFields(model, feature)
  const next = fields.find(
    (field) => field.required && isEmptyValue(field.value) && !fillsFromSiblingPick(field, fields),
  )
  return next?.path ?? null
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

/**
 * The edits of one preview: on the first, a new body for op `new` and the feature itself; later,
 * the updated record, with the command's body created or removed when op changed (3.7).
 */
function previewEdits(
  session: CommandSession,
  command: ActiveCommand,
  document: DocumentState,
  params: Params,
): {
  readonly edits: readonly Edit[]
  readonly params: Params
  readonly newBody: ActiveCommand['newBody']
} {
  const edits: Edit[] = []
  let next: Record<string, unknown> = { ...params }
  let newBody = command.newBody
  const hasBodyField = session.feature.manifest.fields.some(
    (hint) => hint.path === 'body' && hint.selection === 'body',
  )
  const bodies = document.recipe.bodies ?? []
  if (hasBodyField && next['op'] === 'new') {
    if (newBody === null && isEmptyValue(next['body'])) {
      const taken = usedIds(document.recipe)
      newBody = { id: generateId('b', (id) => taken.has(id)), name: nextBodyName(document.recipe) }
      edits.push({
        type: 'add_body',
        index: bodies.length,
        body: { id: newBody.id, name: newBody.name },
      })
    }
    if (newBody !== null) {
      next = { ...next, body: newBody.id }
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
      : { type: 'add_feature', index: insertionIndex(document), feature: record },
  )
  if (next['op'] !== 'new' && newBody !== null) {
    // The body this command created is no longer used once op is not new.
    edits.push({ type: 'remove_body', id: newBody.id })
    newBody = null
  }
  return { edits, params: next, newBody }
}

/** Applies the params as the focused feature inside the transaction. */
async function applyPreview(app: App, session: CommandSession, params: Params): Promise<void> {
  const command = commandOf(app)
  const document = documentOf(app)
  if (command === null || command.featureId !== session.featureId || document === null) {
    return
  }
  const preview = previewEdits(session, command, document, params)
  await app.core.applyEdits(document.documentHandle, null, preview.edits)
  if (!command.applied) {
    await app.core.setFocus(document.documentHandle, session.featureId)
  }
  const latest = sameValue(commandOf(app)?.params, params)
  if (latest) {
    // The record now names the body this command made; the params follow it.
    session.submitted = preview.params
  }
  updateCommand(app, (current) => ({
    applied: true,
    newBody: preview.newBody,
    params: latest ? preview.params : current.params,
  }))
}

function previewStateAfter(app: App, featureId: string): ActiveCommand['preview'] {
  return documentOf(app)?.featureStatus[featureId]?.status === 'error' ? 'failed' : 'shown'
}

function createSession(
  app: App,
  input: Omit<CommandSession, 'preview' | 'pendingFields' | 'submitted' | 'failed'>,
): CommandSession {
  const session: CommandSession = {
    ...input,
    pendingFields: new Set(),
    submitted: null,
    failed: false,
    preview: createLatestOnly<Params, void>(
      (params) => app.mutations.run(() => applyPreview(app, session, params)),
      () => {
        session.failed = false
        if (sessionOf(app) === session) {
          updateCommand(app, () => ({ preview: previewStateAfter(app, session.featureId) }))
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
  if (sameValue(session.submitted, command.params) && !session.failed) {
    // These params are applied or on their way.
    if (!session.preview.busy) {
      updateCommand(app, () => ({ preview: previewStateAfter(app, session.featureId) }))
    }
    return
  }
  updateCommand(app, () => ({ preview: 'pending' }))
  session.submitted = command.params
  session.preview.submit(command.params)
}

function endSession(app: App): void {
  sessions.get(app.store)?.preview.cancel()
  sessions.delete(app.store)
  app.store.setState({ command: null })
}

/** Whether a field takes the item; a feature row only counts as a sketch when it is one. */
function takes(state: AppState, accepts: SelectionKind, item: SelectionItem): boolean {
  if (!acceptsTarget(accepts, item)) {
    return false
  }
  if (accepts !== 'sketch' || item.kind !== 'feature') {
    return true
  }
  return (state.document?.recipe.features ?? []).some(
    (feature) => feature.id === item.feature && feature.type === 'sketch',
  )
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

/** Fills selection fields from the selection, in manifest order, each item used once (3.7 step 4). */
function preselected(
  state: AppState,
  feature: CatalogFeature,
  params: Params,
  items: readonly SelectionItem[],
): Record<string, unknown> {
  let next: Record<string, unknown> = { ...params }
  const used = new Set<string>()
  const model = buildInspectorModel({ feature, params, errors: {} })
  for (const field of selectionFields(model, feature)) {
    if (field.control.type !== 'selection') {
      continue
    }
    const { accepts, multiple } = field.control
    const accepted = items.filter((item) => !used.has(item.key) && takes(state, accepts, item))
    const taken = multiple ? accepted : accepted.slice(0, 1)
    const value = selectionValue(accepts, taken, multiple)
    if (value === null || isEmptyValue(value)) {
      continue
    }
    for (const item of taken) {
      used.add(item.key)
    }
    next = setParam(feature, next, field.path, value)
    next = withSiblingBody(feature, next, field, taken[0] ?? null)
  }
  return next
}

/** Fills an empty body (or sketch) field beside a face, edge or point field from its first pick (2.12). */
function withSiblingBody(
  feature: CatalogFeature,
  params: Record<string, unknown>,
  field: InspectorField,
  item: SelectionItem | null,
): Record<string, unknown> {
  if (item === null || field.control.type !== 'selection') {
    return params
  }
  const accepts = field.control.accepts
  const sibling =
    accepts === 'face' || accepts === 'edge'
      ? { path: siblingPath(field.path, 'body'), kind: 'body', value: item.body }
      : accepts === 'sketch_point'
        ? {
            path: siblingPath(field.path, 'sketch'),
            kind: 'sketch',
            value: item.sketchItem?.sketch ?? null,
          }
        : null
  if (sibling === null || sibling.value === null) {
    return params
  }
  const hint = feature.manifest.fields.find((candidate) => candidate.path === sibling.path)
  if (hint?.selection !== sibling.kind) {
    return params
  }
  if (accepts === 'sketch_point' || isEmptyValue(getParam(params, sibling.path))) {
    return setParam(feature, params, sibling.path, sibling.value)
  }
  return params
}

/** Starts a command that creates a feature of this type (3.7). */
export async function startFeatureCommand(app: App, featureType: string): Promise<void> {
  if (featureType === 'sketch') {
    await startNewSketch(app)
    return
  }
  if (commandOf(app) !== null) {
    await cancelCommand(app)
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
  const defaults = defaultParams(feature, {
    bodies: document.recipe.bodies ?? [],
    bodiesWithGeometry: Object.keys(document.meshes),
    lastTargetBody,
  })
  const finishedRegion =
    state.selection.length === 0 && finishedSketch !== null
      ? onlyRegionOf(state, finishedSketch)
      : null
  const params = preselected(
    state,
    feature,
    defaults,
    finishedRegion === null ? state.selection : [finishedRegion],
  )
  createSession(app, {
    featureId,
    feature,
    recordExtras: {},
    statusesBefore: document.featureStatus,
    lastTargetBody,
  })
  const model = buildInspectorModel({ feature, params, errors: {} })
  app.store.setState({
    selection: [],
    command: {
      featureType,
      featureId,
      editing: false,
      title: feature.manifest.label,
      params,
      activeField: firstEmptySelectionField(model, feature),
      fieldErrors: {},
      applied: false,
      newBody: null,
      preview: 'none',
    },
  })
  refreshPreview(app)
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
export async function editFeature(app: App, featureId: string): Promise<void> {
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
    notify(app, { severity: 'info', message: MESSAGES.import_not_editable(), actions: [] })
    return
  }
  const state = app.store.getState()
  const feature = state.catalog?.byType[record.type]
  if (feature === undefined) {
    return
  }
  if (commandOf(app) !== null) {
    await cancelCommand(app)
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
      notify(app, { severity: 'error', message: error.message, actions: [focusFix(app, record)] })
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
  app.store.setState({
    context: 'model',
    command: {
      featureType: record.type,
      featureId,
      editing: true,
      title: label,
      params,
      activeField: null,
      fieldErrors: {},
      applied: true,
      newBody: null,
      preview: previewStateAfter(app, featureId),
    },
  })
}

/** Changes one field of the active command and previews the result. */
export function setCommandField(app: App, path: string, value: unknown): void {
  const session = sessionOf(app)
  const command = commandOf(app)
  if (session === null || command === null) {
    return
  }
  let params = setParam(session.feature, command.params, path, value)
  const previousOp = command.params['op']
  const hasBodyField = session.feature.manifest.fields.some(
    (hint) => hint.path === 'body' && hint.selection === 'body',
  )
  if (path === 'op' && value !== previousOp && hasBodyField) {
    // Leaving op new targets the last body again; returning to it makes a fresh body (3.7).
    if (value === 'new') {
      params = setParam(session.feature, params, 'body', undefined)
    } else if (previousOp === 'new') {
      params = setParam(session.feature, params, 'body', session.lastTargetBody ?? undefined)
    }
  }
  updateCommand(app, () => ({ params }))
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

/** The hint for a pick the active field refuses (3.7). */
function refusalHint(accepts: SelectionKind): string {
  return accepts === 'plane'
    ? MESSAGES.pick_plane()
    : accepts === 'axis'
      ? MESSAGES.pick_axis()
      : MESSAGES.pick_kind(SELECTION_NOUNS[accepts])
}

/** A property of an object value, or undefined. */
function propertyOf(value: unknown, key: string): unknown {
  return typeof value === 'object' && value !== null ? Reflect.get(value, key) : undefined
}

/** The field's value with the picked item toggled (added, replacing a single value, or removed). */
function toggledValue(
  accepts: SelectionKind,
  multiple: boolean,
  current: unknown,
  item: SelectionItem,
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

/** Feeds a pick to the active selection field (3.7 selection capture). */
export async function capturePick(app: App, item: SelectionItem): Promise<void> {
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
  if (!takes(app.store.getState(), accepts, item)) {
    notify(app, { severity: 'info', message: refusalHint(accepts), actions: [] })
    return
  }
  let params: Record<string, unknown> = { ...command.params }
  if ((accepts === 'face' || accepts === 'edge') && item.body !== null) {
    const bodyPath = siblingPath(field.path, 'body')
    const body = getParam(params, bodyPath)
    if (typeof body === 'string' && body !== '' && body !== item.body) {
      const bodyName =
        documentOf(app)?.recipe.bodies?.find((candidate) => candidate.id === body)?.name ?? body
      notify(app, {
        severity: 'info',
        message: MESSAGES.pick_same_body(accepts, bodyName),
        actions: [],
      })
      return
    }
  }
  const value = toggledValue(accepts, multiple, field.value, item)
  params = setParam(session.feature, params, field.path, value)
  if (value !== undefined) {
    params = withSiblingBody(session.feature, params, field, item)
  }
  const nextModel = buildInspectorModel({
    feature: session.feature,
    params,
    errors: command.fieldErrors,
  })
  const advance = !multiple && value !== undefined
  updateCommand(app, () => ({
    params,
    activeField: advance
      ? firstEmptySelectionField(nextModel, session.feature)
      : command.activeField,
  }))
  refreshPreview(app)
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
  if (!previewable(app, session, command)) {
    return
  }
  if (!sameValue(session.submitted, command.params)) {
    session.submitted = command.params
    session.preview.submit(command.params)
  }
  await session.preview.flush()
  if (session.failed || sessionOf(app) !== session) {
    return
  }
  try {
    await app.mutations.run(() => app.core.commitTransaction(document.documentHandle))
  } catch (error) {
    showCoreError(app, error)
    return
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
