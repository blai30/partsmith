import type {
  Cause,
  FeatureRecord,
  FixAction,
  FixRef,
  Ghost,
  SolvedEntity,
} from '../../generated/protocol'
import { featureSelectionItem } from '../../inspector/selection-values'
import { selectionFieldFor } from '../../inspector/status'
import { MESSAGES } from '../../messages'
import { frameRepair } from '../../repair/frame'
import { causeGhost, keepCauseGhost, selectRepairMode } from '../../repair/repair'
import { entitySelectionItem } from '../../sketch/hit'
import {
  selectFeatureLabel,
  selectFixBlock,
  selectPointsIntoSketchPlane,
  selectReattachingSketch,
  selectSketchFaceLost,
  type FixPlace,
} from '../../state/selectors'
import type { Notification, SelectionItem } from '../../state/store'
import { DEFAULT_FIT_MARGIN } from '../../viewport/camera'
import { extendBounds, pointsOf } from '../../viewport/math'
import type { App } from '../createApp'
import { reportFailure } from '../report'
import { applyFixInCommand, cancelCommandImplicitly, editFeature, setActiveField } from './command'
import { undo } from './document'
import { documentOf, refusedDuringCommand, runChange } from './edits'
import { listedElementsOrNone } from './element-lists'
import { announce, notify } from './notifications'
import { elementSelectionItem } from './selection'
import { enterAttachedSketch, reattachSketch, recordSketchEdit } from './sketch'

// The actions of messages and cause lines (ui.md 4.2: `runFixAction`), Copy details (4.1) and
// "Show what '{label}' lost" (3.7: `showLost`, which opens repair mode and frames what was lost).

/**
 * An `apply` action (4.2): offered by the command's own banner it joins the transaction (3.7);
 * offered anywhere else during a feature command it waits with `finish_command_first`; at rest it
 * is one change through `apply_fix`, which the core labels with the action's label, and in a
 * sketch session a step of the session's history, so the next Mod+Z undoes it (3.20, U-145). One
 * that attaches the sketch being re-attached ends the pick in that sketch (3.16.1).
 */
async function applyFix(app: App, fix: FixRef, place: FixPlace): Promise<void> {
  if (app.store.getState().command !== null) {
    if (place === 'banner') {
      await applyFixInCommand(app, fix)
    } else {
      refusedDuringCommand(app)
    }
    return
  }
  const { sent } = await runChange(app, async (document) => {
    await app.core.applyFix(document.documentHandle, fix)
    return true
  })
  if (sent) {
    recordSketchEdit(app)
    await enterAttachedSketch(app)
  }
}

/**
 * A `pick` action (4.2): in the command editing its feature, the field its path maps to takes the
 * picks; otherwise the feature is edited with that field active (an implicit cancel of another
 * command first, 3.7). A sketch's plane is picked again by re-attaching the sketch (3.16.1).
 */
async function pickAgain(app: App, feature: string, path: string): Promise<void> {
  const state = app.store.getState()
  if (selectPointsIntoSketchPlane(state, feature, path)) {
    await reattachSketch(app, feature)
    return
  }
  const command = state.command
  if (command?.featureId === feature) {
    const fields = state.catalog?.byType[command.featureType]?.manifest.fields ?? []
    const field = selectionFieldFor(fields, path)
    if (field !== null) {
      setActiveField(app, field.path)
    }
    return
  }
  await editFeature(app, feature, { field: path })
}

/**
 * The items a `show` frames (4.2): the entity a sketch still has, else what the feature made: a
 * sketch's entities, a datum plane, or the faces it created in the current meshes.
 */
async function shownItems(
  app: App,
  record: FeatureRecord,
  drawn: SolvedEntity | undefined,
): Promise<readonly SelectionItem[]> {
  const document = documentOf(app)
  if (document === null) {
    return []
  }
  const feature = record.id
  if (record.type === 'sketch') {
    const entities =
      drawn === undefined ? (document.sketches[feature]?.solution.entities ?? []) : [drawn]
    return entities.map((entity) => entitySelectionItem(feature, entity))
  }
  if (record.type === 'datum_plane') {
    return [
      featureSelectionItem(
        feature,
        selectFeatureLabel(app.store.getState(), feature),
        'datum_plane',
      ),
    ]
  }
  const faces = await Promise.all(
    Object.entries(document.meshes).map(([body, mesh]) =>
      listedElementsOrNone(app, body, 'face', mesh.meshId),
    ),
  )
  return faces
    .flat()
    .filter((face) => face.feature === feature)
    .map((face) => elementSelectionItem(app, face))
}

/**
 * Frames selection items in the viewport, with the shown cause's ghost (4.2); nothing without a
 * viewport or anything drawn.
 */
function frameShown(app: App, items: readonly SelectionItem[]): void {
  const viewport = app.viewport()
  if (viewport === null) {
    return
  }
  const ghost = causeGhost(app.store.getState())
  const bounds = extendBounds(
    items.length === 0 ? null : viewport.itemBounds(items),
    ghost === undefined ? [] : pointsOf(ghost.positions),
  )
  if (bounds !== null) {
    viewport.zoomToBounds({ min: [...bounds.min], max: [...bounds.max] }, DEFAULT_FIT_MARGIN)
  }
}

/**
 * A `show` action (4.2): during a feature command, a show of the command's own feature frames its
 * preview and one of another feature first ends the command as an implicit cancel (3.7). Then the
 * feature is selected, or the entity it names while the sketch still has it, `AppState.shown`
 * keeps the cause's ghost, and what it made is framed with that ghost.
 */
async function show(
  app: App,
  feature: string,
  entity: string | null,
  ghost: Ghost | null,
): Promise<void> {
  const command = app.store.getState().command
  if (command?.featureId === feature) {
    app.viewport()?.fit()
    return
  }
  if (command !== null) {
    await cancelCommandImplicitly(app)
  }
  const state = app.store.getState()
  const record = state.document?.recipe.features?.find((candidate) => candidate.id === feature)
  if (record === undefined) {
    return
  }
  const drawn = state.document?.sketches[feature]?.solution.entities.find(
    (candidate) => candidate.id === entity,
  )
  const item =
    drawn === undefined
      ? featureSelectionItem(
          feature,
          selectFeatureLabel(state, feature),
          record.type === 'datum_plane' ? 'datum_plane' : 'feature',
        )
      : entitySelectionItem(feature, drawn)
  if (ghost !== null) {
    keepCauseGhost(state, ghost)
  }
  // One change, so the selection change does not clear what is shown.
  app.store.setState({ selection: [item], shown: { feature, entity, ghost } })
  frameShown(app, await shownItems(app, record, drawn))
}

/**
 * Runs one action of a message or of its cause line (4.2), offered at `place`: `apply` through
 * `apply_fix`, `pick`, `show` and `undo` as host actions; `finish_transaction` is never offered,
 * since the UI refuses itself, during a command, the requests the core answers with it (3.7).
 * `cause` is the cause line the action belongs to, whose ghost a `show` draws.
 */
export async function runFixAction(
  app: App,
  action: FixAction,
  place: FixPlace,
  cause?: Cause,
): Promise<void> {
  const run = action.run
  switch (run.kind) {
    case 'apply':
      await applyFix(app, run.fix, place)
      return
    case 'pick':
      await pickAgain(app, run.feature, run.path)
      return
    case 'show':
      await show(app, run.feature, run.entity ?? null, cause?.ghost ?? null)
      return
    case 'undo':
      await undo(app)
      return
    case 'finish_transaction':
      return
  }
}

/**
 * Runs an action a button or a candidate marker offers at `place` (4.2) unless it waits now
 * (`selectFixBlock`, 3.7), then calls `onRan`, for the focus rule of the banner (3.7).
 */
export function runOfferedFix(
  app: App,
  action: FixAction,
  place: FixPlace,
  {
    cause,
    onRan,
    failure = `Running "${action.label}"`,
  }: {
    /** The cause line the action belongs to, whose ghost a `show` draws; absent for a message action. */
    readonly cause?: Cause | undefined
    readonly onRan?: (() => void) | undefined
    /** What the console names when the action fails. */
    readonly failure?: string | undefined
  } = {},
): void {
  if (selectFixBlock(app.store.getState(), action, place) !== null) {
    return
  }
  runFixAction(app, action, place, cause)
    .then(() => onRan?.())
    .catch(reportFailure(failure))
}

/** The actions a message or a cause line offers, in order: all but `finish_transaction` (4.2). */
export function offeredActions(actions: readonly FixAction[]): readonly FixAction[] {
  return actions.filter((action) => action.run.kind !== 'finish_transaction')
}

/**
 * An action of a core error or of a cause line as a notification offers it (4.2); an `apply`
 * edits the document, so it waits during a feature command (3.7).
 */
export function fixNotificationAction(
  app: App,
  action: FixAction,
  cause?: Cause,
): Notification['actions'][number] {
  return {
    label: action.label,
    run: () => {
      void runFixAction(app, action, 'notification', cause)
    },
    editsDocument: action.run.kind === 'apply',
  }
}

/**
 * Copies the code, the message, the cause and the detail of a message or core error for reports
 * (4.1), each on its own line, and announces it; a clipboard that refuses shows
 * `details_not_copied` as a warning.
 */
export async function copyDetails(
  app: App,
  message: {
    readonly code: string
    readonly message: string
    readonly cause?: Cause | null
    readonly detail?: string | null
  },
): Promise<void> {
  const lines = [message.code, message.message, message.cause?.text, message.detail].filter(
    (line): line is string => line !== null && line !== undefined,
  )
  try {
    await navigator.clipboard.writeText(lines.join('\n'))
  } catch (error) {
    console.info('The clipboard refused the details.', error)
    notify(app, {
      severity: 'warning',
      message: MESSAGES.details_not_copied(),
      actions: [],
      code: 'details_not_copied',
    })
    return
  }
  announce(app, MESSAGES.details_copied())
}

/**
 * Shows repair mode for a feature whose Problems row takes focus (3.7, 3.9), unless a feature
 * command is active or the feature has no repair.
 */
export function focusRepair(app: App, featureId: string): void {
  const state = app.store.getState()
  const repairs = state.document?.featureStatus[featureId]?.repairs ?? []
  if (state.command === null && repairs.length > 0) {
    app.store.setState({ repair: { feature: featureId } })
  }
}

/** Ends the repair mode a feature's Problems row showed once focus leaves it (3.7, 3.9). */
export function blurRepair(app: App, featureId: string): void {
  app.store.setState((state) =>
    state.command === null && state.repair?.feature === featureId ? { repair: null } : {},
  )
}

/**
 * "Show what '{label}' lost" (3.7): edits the feature, whose first repair's field takes the picks,
 * or re-attaches a sketch whose face is lost (3.16.1), either of which shows repair mode, and frames
 * what was lost with the candidates' markers.
 */
export async function showLost(app: App, featureId: string): Promise<void> {
  await (selectSketchFaceLost(app.store.getState(), featureId)
    ? reattachSketch(app, featureId)
    : editFeature(app, featureId))
  const state = app.store.getState()
  const opened =
    state.command?.featureId === featureId || selectReattachingSketch(state) === featureId
  const mode = opened ? selectRepairMode(state) : null
  if (mode !== null) {
    await frameRepair(app, mode)
  }
}
