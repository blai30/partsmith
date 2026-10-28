import { CoreError } from '../../core/client'
import { generateId, usedIds } from '../../core/ids'
import { featureLabel, nextBodyName } from '../../core/labels'
import { createLatestOnly, createOneAtATime, type LatestOnly } from '../../core/queue'
import { CoreFatalError } from '../../core/transport'
import { activeFeatures, markerPosition } from '../../core/recipe'
import type {
  DocHandle,
  Edit,
  ElementDescriptor,
  FeatureRecord,
  FeatureStatus,
  FixRef,
  SelectionKind,
  SketchEntity,
} from '../../generated/protocol'
import {
  activeCommandFeature,
  activeCommandModel,
  bodyCreatingTarget,
  buildInspectorModel,
  defaultParams,
  fieldsOf,
  getParam,
  isEmptyValue,
  setParam,
  type InspectorField,
  type InspectorModel,
} from '../../inspector/model'
import { fieldForPointer, repairField, selectionFieldFor } from '../../inspector/status'
import { MESSAGES } from '../../messages'
import { TIMING_NAMES } from '../../performance/marks'
import { memoized } from '../../state/selectors'
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
import { referencedSketches } from '../../state/visibility'
import type { PointerRay } from '../../viewport/picking'
import type { App } from '../createApp'
import { reportFailure } from '../report'
import { measureToNextFrame } from '../timing'
import { setSessionVisible } from './bodies'
import {
  pendingRepair,
  repairedValue,
  repairSlots,
  replacedInList,
  type RepairSlot,
} from './command-repairs'
import {
  differingFields,
  sameValue,
  siblingPath,
  toggledValue,
  type Params,
} from './command-values'
import { commitEdits, documentOf, showCoreError } from './edits'
import { listedElements } from './element-lists'
import { helperFor, helperOnFace, holeCenter } from './hole-placement'
import { reportChange } from './loss-notifications'
import { notify, showPickHint } from './notifications'
import {
  firstEmptySelectionField,
  openingActiveField,
  preselected,
  takes,
  withSiblingBody,
} from './preselection'
import { enterSketch, finishSketch, sketchFaceBody, sketchPlaneOf, startNewSketch } from './sketch'
import { setSuppressed } from './timeline'

// Feature commands (ui.md 3.7): an inspector over one feature record inside a transaction, with a
// live preview of every complete state of its params, a history of the user's changes, and the
// bodies and helper sketch the command creates.

/** A body the command creates, and the field that names it (2.12). */
type NewBody = NonNullable<ActiveCommand['newBody']>

/** One preview request: the params, and when the user changed the value they come from. */
type PreviewRequest = {
  readonly params: Params
  /** `performance.now()` of the inspector change, which starts `ps:preview` (2.18), or null. */
  readonly changedAt: number | null
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

/** A feature record split into its params and the record properties beside them. */
function splitRecord(record: Readonly<Record<string, unknown>>): {
  readonly recordExtras: Readonly<Record<string, unknown>>
  readonly params: Params
} {
  const recordExtras: Record<string, unknown> = {}
  const params: Record<string, unknown> = {}
  for (const [key, value] of Object.entries(record)) {
    if (RECORD_EXTRAS.has(key)) {
      recordExtras[key] = value
    } else if (key !== 'id' && key !== 'type') {
      params[key] = value
    }
  }
  return { recordExtras, params }
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
  const { recordExtras } = splitRecord(record ?? {})
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

/** The body of the latest active feature with a body that has model geometry (3.7 step 3). */
function lastTargetBodyOf(document: DocumentState): string | null {
  for (const feature of activeFeatures(document.recipe).toReversed()) {
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
  return markerPosition(document.recipe.features ?? [], document.recipe.rollback ?? null)
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

/** Whether the feature has a selection field of this kind at this path. */
function hasSelectionField(feature: CatalogFeature, path: string, kind: SelectionKind): boolean {
  return feature.manifest.fields.some((hint) => hint.path === path && hint.selection === kind)
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
  if (!hasSelectionField(session.feature, 'body', 'body') || params['op'] !== 'new') {
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
  // A hint under a field of this command, and its repair mode, go with it (3.7).
  app.store.setState((state) => ({
    command: null,
    repair: null,
    pickHint: state.pickHint?.field === null ? state.pickHint : null,
  }))
}

/** The label of the field at a path, for the command history (3.20 details). */
function fieldLabel(session: CommandSession, path: string): string {
  return session.feature.manifest.fields.find((hint) => hint.path === path)?.label ?? path
}

/** The command's state as one step of its history, labeled with the field a change touches. */
function stepOf(command: ActiveCommand, field: string): CommandStep {
  return {
    params: command.params,
    helper: command.helper,
    activeField: command.activeField,
    field,
  }
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
  return { history: [...command.history, stepOf(command, label)], future: [], changedPaths }
}

/** How a command opens: reopening one the user restores (3.7 implicit cancel), or for a `pick` action (4.2). */
type OpenOptions = {
  /** The command to reopen with its params, helper sketch and active field. */
  readonly restore?: SavedCommand
  /** A JSON pointer into the edited record whose selection field takes the picks (a `pick` action, 4.2). */
  readonly field?: string
}

/** A command ended implicitly, kept so Restore can reopen it. */
type SavedCommand = Pick<
  ActiveCommand,
  | 'featureType'
  | 'featureId'
  | 'editing'
  | 'title'
  | 'params'
  | 'helper'
  | 'activeField'
  | 'newBody'
>

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

/** Feature command starts, per store: a start waits for the one before it to finish. */
const startsOneAtATime = createOneAtATime<AppStore>()

/**
 * Runs one start of a feature command after any start still in progress (a double-click, a held
 * key), so the second never begins a transaction while the first is opening one. With nothing in
 * progress it starts at once.
 */
function oneStartAtATime(app: App, start: () => Promise<void>): Promise<void> {
  return startsOneAtATime(app.store, start)
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
  options: OpenOptions = {},
): Promise<void> {
  if (featureType === 'sketch') {
    return startNewSketch(app)
  }
  return oneStartAtATime(app, () => openFeatureCommand(app, featureType, options))
}

async function openFeatureCommand(
  app: App,
  featureType: string,
  options: OpenOptions,
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
  // Steps 3 and 4 read the app state once the transaction has begun, so a click the core answered
  // meanwhile (one made right before the key) is preselected rather than cleared.
  const begun = app.store.getState()
  const current = begun.document ?? document
  const taken = usedIds(current.recipe)
  const featureId = generateId('f', (id) => taken.has(id))
  const lastTargetBody = lastTargetBodyOf(current)
  const session = createSession(app, {
    featureId,
    feature,
    recordExtras: {},
    statusesBefore: current.featureStatus,
    lastTargetBody,
  })
  const saved = options.restore ?? null
  const opening =
    saved === null
      ? openingParams(app, session, begun, finishedSketch)
      : restoredOpening(app, session, saved)
  const model = buildInspectorModel({ feature, params: opening.params, errors: {} })
  app.store.setState({
    selection: [],
    // Repeat starts this command again (3.5).
    lastFeatureType: featureType,
    command: openedCommand({
      featureType,
      featureId,
      editing: false,
      title: feature.manifest.label,
      params: opening.params,
      activeField:
        saved === null ? openingActiveField(model, feature, opening.params) : saved.activeField,
      applied: false,
      newBody: opening.newBody,
      helper: saved === null ? null : unappliedHelper(saved.helper),
      preview: 'none',
    }),
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

/** A command as it opens: no field errors and no changes yet. */
function openedCommand(
  command: Omit<ActiveCommand, 'fieldErrors' | 'changedPaths' | 'history' | 'future'>,
): ActiveCommand {
  return { ...command, fieldErrors: {}, changedPaths: [], history: [], future: [] }
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
export function editFeature(app: App, featureId: string, options: OpenOptions = {}): Promise<void> {
  return oneStartAtATime(app, () => openFeatureEdit(app, featureId, options))
}

async function openFeatureEdit(app: App, featureId: string, options: OpenOptions): Promise<void> {
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
    // A core that stopped took its transaction with it; a restarted one never had it (4.4).
    if (error instanceof CoreFatalError) {
      return
    }
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
  const { recordExtras, params } = splitRecord(record)
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
  const status = document.featureStatus[featureId]
  const activeField =
    saved === null ? openingEditField(feature, status, options.field) : saved.activeField
  app.store.setState({
    context: 'model',
    command: openedCommand({
      featureType: record.type,
      featureId,
      editing: true,
      title: label,
      params: restored?.params ?? params,
      activeField,
      applied: true,
      newBody: restored?.newBody ?? null,
      helper: saved === null ? null : unappliedHelper(saved.helper),
      preview: previewStateAfter(app, featureId),
    }),
  })
  if (saved !== null) {
    refreshPreview(app)
  }
}

/**
 * The field an edit opens with as the active field (3.7): the selection field a `pick` action's
 * pointer maps to, else the field of the first repair, so the next pick replaces the lost
 * reference (4.2); else none.
 */
function openingEditField(
  feature: CatalogFeature,
  status: FeatureStatus | undefined,
  pointer: string | undefined,
): string | null {
  const fields = feature.manifest.fields
  const picked = pointer === undefined ? null : selectionFieldFor(fields, pointer)
  return (picked ?? repairField(fields, status))?.path ?? null
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
  const hasBodyField = hasSelectionField(feature, 'body', 'body')
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
  const planeBody = sketchFaceBody(app.store.getState(), typeof sketch === 'string' ? sketch : null)
  if (planeBody === null) {
    return params
  }
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
   * change of the command history, and a changed text is pending until its field reports it (3.7).
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
  if (typing && typeof value === 'string' && value !== getParam(command.params, path)) {
    // A changed text is pending until its field reports it; an unchanged one renders nothing new,
    // so its field would never report it.
    session.pendingFields.add(path)
  }
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

/**
 * Makes a selection field receive picks, or none. In repair mode, a field that holds a repair
 * makes that one the current repair (3.7, `selectRepairMode`).
 */
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
  const placement = element?.placement ?? null
  if (element === null || plane?.kind !== 'face' || placement === null) {
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
    // The ray meets the face where it is now, in world coordinates.
    position: holeCenter(element, placement, pointer),
  }
  const helper: HelperSketch = helperInUse
    ? {
        ...current,
        params: { ...current.params, entities: [...(current.params.entities ?? []), entity] },
      }
    : {
        id: issueId(app, session, 'f'),
        params: { plane, entities: [entity], constraints: [] },
        placement,
        applied: false,
      }
  const value = helperInUse && Array.isArray(points) ? [...points, entity.id] : [entity.id]
  let params: Params = setParam(session.feature, command.params, field.path, value)
  params = setParam(session.feature, params, sketchPath, helper.id)
  const bodyPath = siblingPath(field.path, 'body')
  if (hasSelectionField(session.feature, bodyPath, 'body')) {
    // The hole drills the body of the face.
    params = setParam(session.feature, params, bodyPath, element.body)
  }
  commitPick(app, session, command, field, params, helper)
}

/** The edges of a body by name, listed once per model-layer mesh (3.7, 3.6). */
async function edgesOf(
  app: App,
  body: string,
): Promise<ReadonlyMap<string, ElementDescriptor> | null> {
  const meshId = documentOf(app)?.meshes[body]?.meshId
  const listed = meshId === undefined ? null : listedElements(app, body, 'edge', meshId)
  if (listed === null) {
    return null
  }
  return new Map((await listed).map((edge) => [edge.name, edge]))
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
    // The pick waits on the edge list (4.4).
    edges = await app.waitOn(edgesOf(app, item.body))
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
  const repair = pendingRepair(session.repairs, field.path, current)
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
  const repaired = repairedValue(session.repairs, accepts, multiple, field.path, field.value, item)
  const value =
    repaired === null
      ? toggledValue(accepts, multiple, field.value, item, currentSketch)
      : repaired.value
  const params = pickedParams(app, session, command, field, value, item)
  commitPick(app, session, command, field, params, command.helper)
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
  reportChange(app, session.statusesBefore)
}

/** Whether the active feature command can be confirmed: every required field set, no field error. */
export function commandComplete(state: AppState): boolean {
  return activeCommandModel(state, state.command?.fieldErrors)?.model.complete === true
}

/** The Enter ladder's last step (3.5): confirms the active feature command when it is complete. Returns whether it did. */
export function confirmIfComplete(app: App): boolean {
  if (!commandComplete(app.store.getState())) {
    return false
  }
  confirmCommand(app).catch(reportFailure('Confirming the command'))
  return true
}

/** Cancel: reverts the transaction; nothing is recorded (3.7). */
export async function cancelCommand(app: App): Promise<void> {
  const command = commandOf(app)
  const document = documentOf(app)
  if (command === null) {
    return
  }
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
          restoreCommand(app, saved).catch(reportFailure('Restoring the command'))
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
  restoreStep(app, session, step, {
    history: command.history.slice(0, -1),
    future: [...command.future, stepOf(command, step.field)],
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
  restoreStep(app, session, step, {
    history: [...command.history, stepOf(command, step.field)],
    future: command.future.slice(0, -1),
  })
}

/**
 * The command's value field (3.7 typing values): the expression field the user changed last, else
 * the first essential expression field; null without one.
 */
export function commandValueField(state: AppState): string | null {
  const active = activeCommandModel(state)
  if (active === null) {
    return null
  }
  const { command, model } = active
  const expressions = fieldsOf(model).filter((field) => field.control.type === 'expression')
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
    [activeCommandFeature(state), state.document?.recipe.bodies, state.document?.meshes] as const,
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

/** The paths of the command's fields whose values differ between two params, in display order. */
function changedFields(session: CommandSession, before: Params, after: Params): string[] {
  const model = buildInspectorModel({ feature: session.feature, params: after, errors: {} })
  return differingFields(fieldsOf(model), before, after).map((field) => field.path)
}

/**
 * An `apply` action of the command's own banner (3.7): `apply_fix` joins the transaction, with the
 * preview pending until its events arrive, then the command takes the feature's record from the
 * new recipe as its params (the helper sketch and the active field stay) as one history step named
 * after the field the message is about. Edits the fix made to other records are not history steps:
 * OK records them and Cancel reverts them. Nothing is sent while a preview is pending, because the
 * statuses that offered the fix are about to be replaced. Resolves whether the fix was applied.
 */
export async function applyFixInCommand(app: App, fix: FixRef): Promise<boolean> {
  const session = sessionOf(app)
  const command = commandOf(app)
  if (session === null || command === null || command.preview === 'pending') {
    return false
  }
  if (!(await runInCommand(app, (documentHandle) => app.core.applyFix(documentHandle, fix)))) {
    return false
  }
  const document = documentOf(app)
  const record = document?.recipe.features?.find((feature) => feature.id === session.featureId)
  if (record === undefined || commandOf(app) === null || sessionOf(app) !== session) {
    return true
  }
  const { params } = splitRecord(record)
  const pointer = fix.field ?? null
  const messageField =
    pointer === null
      ? null
      : fieldForPointer(
          session.feature.manifest.fields.map((hint) => hint.path),
          pointer,
        )
  // The record is what the transaction holds now.
  session.submitted = params
  updateCommand(app, (current) => {
    const changed = changedFields(session, current.params, params)
    const named = messageField ?? changed[0] ?? null
    return {
      params,
      history: [
        ...current.history,
        stepOf(current, named === null ? current.title : fieldLabel(session, named)),
      ],
      future: [],
      changedPaths: [...current.changedPaths.filter((path) => !changed.includes(path)), ...changed],
    }
  })
  refreshPreview(app)
  return true
}
