import { featureLabel } from '../core/labels'
import { activeFeatures, sketchRecord } from '../core/recipe'
import type { FeatureRecord, FixAction, Units } from '../generated/protocol'
import { pointsInto } from '../inspector/status'
import { MESSAGES } from '../messages'
import { isCheckHidden } from '../print/checks'
import { sameOrientation } from '../print/orientation'
import { groupedRows, type ProblemRows } from '../problems/problems'
import type { AppState, DocumentState, SelectionItem, SketchSession } from './store'

// Derived views of the state (ui.md 2.8). Selectors that build new values remember their last
// inputs and result, so `useAppState(selector)` gets the same value back until the inputs change.

/**
 * Memoizes a derivation on the identity of its inputs: while every input is the same object as
 * last time, the last result is returned again, so `useAppState` and the scene see a stable value
 * until the inputs change.
 */
export function memoized<Inputs extends unknown[], Result>(
  inputsOf: (state: AppState) => readonly [...Inputs],
  derive: (...inputs: Inputs) => Result,
): (state: AppState) => Result {
  let last: { readonly inputs: readonly [...Inputs]; readonly result: Result } | null = null
  return (state) => {
    const inputs = inputsOf(state)
    if (
      last !== null &&
      last.inputs.length === inputs.length &&
      last.inputs.every((input, index) => Object.is(input, inputs[index]))
    ) {
      return last.result
    }
    const result = derive(...inputs)
    last = { inputs, result }
    return result
  }
}

/** The document's units, which its bare numbers take and values show in, or mm without a document. */
export function selectDocumentUnits(state: AppState): Units {
  return state.document?.recipe.units ?? 'mm'
}

/** The display label of a feature (features.md 3.12), or its id when there is no document. */
export function selectFeatureLabel(state: AppState, id: string): string {
  return state.document === null ? id : featureLabel(state.document.recipe, id, state.catalog)
}

/** Whether the document has changes that are not in its file (core dirty or locally unsaved). */
export function isUnsaved(document: DocumentState): boolean {
  return document.dirty || document.localUnsaved
}

/** Whether an open document has changes that are not in its file (3.19). */
export function selectUnsaved(state: AppState): boolean {
  return state.document !== null && isUnsaved(state.document)
}

/** A sketch session editing a sketch, not picking a plane. */
export type EditingSession = SketchSession & { readonly sketchId: string }

/** The session of the sketch being edited, or null outside sketch mode and while picking a plane. */
export function editingSession(state: AppState): EditingSession | null {
  const session = state.sketch
  if (state.context !== 'sketch' || session === null || session.sketchId === null) {
    return null
  }
  return { ...session, sketchId: session.sketchId }
}

/** The edited sketch's id: undefined outside sketch mode, null while picking a plane. */
export function selectEditedSketch(state: AppState): string | null | undefined {
  return state.context === 'sketch' && state.sketch !== null ? state.sketch.sketchId : undefined
}

/** The record of the sketch with this id, or null without an id, a document or such a sketch. */
export function selectSketchRecord(state: AppState, sketchId: string | null): FeatureRecord | null {
  const recipe = state.document?.recipe
  return sketchId === null || recipe === undefined ? null : sketchRecord(recipe, sketchId)
}

/** The sketch whose plane is being picked again (3.16.1, re-attaching), or null. */
export function selectReattachingSketch(state: AppState): string | null {
  return state.context === 'sketch' ? (state.sketch?.reattaching ?? null) : null
}

/** The pointer of a sketch's plane, which has no inspector field, so re-attaching picks it (3.16.1). */
const SKETCH_PLANE_POINTER = '/plane'

/** Whether the feature is a sketch and the pointer lies in its plane, such as `/plane/face` (3.16.1). */
export function selectPointsIntoSketchPlane(
  state: AppState,
  feature: string,
  pointer: string,
): boolean {
  return selectSketchRecord(state, feature) !== null && pointsInto(pointer, SKETCH_PLANE_POINTER)
}

/** Whether a sketch's face is lost: its status has a repair in its plane (3.16.1). */
export function selectSketchFaceLost(state: AppState, sketchId: string): boolean {
  if (selectSketchRecord(state, sketchId) === null) {
    return false
  }
  const repairs = state.document?.featureStatus[sketchId]?.repairs ?? []
  return repairs.some((repair) => pointsInto(repair.path, SKETCH_PLANE_POINTER))
}

/**
 * The items the selection inspector shows (3.8): the sketch selection in sketch mode while no tool
 * is active, the app selection elsewhere.
 */
export function selectInspectedItems(state: AppState): readonly SelectionItem[] {
  if (state.context !== 'sketch') {
    return state.selection
  }
  return state.sketch !== null && state.sketch.tool === null ? state.sketch.selection : []
}

/** Whether the inspector is open (3.7, 3.8): for a feature command, or for inspected items. */
export function selectInspectorOpen(state: AppState): boolean {
  return state.command !== null || selectInspectedItems(state).length > 0
}

/** Ids of the features before the rollback marker, in timeline order. */
export const selectActiveFeatureIds: (state: AppState) => readonly string[] = memoized(
  (state) => [state.document?.recipe] as const,
  (recipe) => (recipe === undefined ? [] : activeFeatures(recipe).map((feature) => feature.id)),
)

/**
 * Whether the empty-document first step shows (3.2): a ready core, a document without features,
 * no feature command, no sketch session and no Lay on face pick, whose print hint takes the same
 * slot. It follows the document, never a first-run flag.
 */
export function selectEmptyState(state: AppState): boolean {
  return (
    state.core.status === 'ready' &&
    state.document !== null &&
    (state.document.recipe.features ?? []).length === 0 &&
    state.command === null &&
    state.sketch === null &&
    !state.print.pickingFace
  )
}

/** The origin axes and point, shown while the document has no features (3.10). */
const ORIGIN_ANCHORS: ReadonlySet<string> = new Set([
  'origin:X',
  'origin:Y',
  'origin:Z',
  'origin:O',
])

/**
 * Whether an origin item (`origin:{name}`) is shown this session (3.10): an entry written by its
 * eye button or V always wins; otherwise the axes and the origin point are shown while the
 * document has no features, and the planes are hidden.
 */
export function selectOriginVisible(state: AppState, key: string): boolean {
  const stored = state.view.sessionVisible[key]
  if (stored !== undefined) {
    return stored
  }
  return ORIGIN_ANCHORS.has(key) && (state.document?.recipe.features ?? []).length === 0
}

/** Selection kinds that belong to one sketch, which the Sketch radio edits (3.3). */
const SKETCH_ITEM_KINDS: ReadonlySet<string> = new Set([
  'sketch_curve',
  'sketch_point',
  'sketch_region',
])

/**
 * The sketch the Sketch radio and `context.sketch` edit (3.3): the one selected sketch feature, or
 * the sketch whose curves, points and regions make up the whole selection; null otherwise, when
 * they start a new sketch.
 */
export function selectSketchToEdit(state: AppState): string | null {
  const { selection } = state
  const [first] = selection
  if (first === undefined) {
    return null
  }
  const sketchId = first.feature
  const sketchOnly =
    (selection.length === 1 && first.kind === 'feature') ||
    selection.every((item) => SKETCH_ITEM_KINDS.has(item.kind) && item.feature === sketchId)
  return sketchOnly && selectSketchRecord(state, sketchId) !== null ? sketchId : null
}

/**
 * The tooltip of the Sketch radio (3.3): "Edit {label}" for the sketch it edits, else "New sketch";
 * null in sketch mode, where choosing it changes nothing.
 */
export function selectSketchRadioTooltip(state: AppState): string | null {
  if (state.context === 'sketch') {
    return null
  }
  const sketchId = selectSketchToEdit(state)
  return sketchId === null ? 'New sketch' : `Edit ${selectFeatureLabel(state, sketchId)}`
}

/** The sketch status shown in the sketch header and the status bar (3.16.8). */
export type SketchDof = {
  readonly state: 'picking' | 'empty' | 'conflict' | 'failed' | 'full' | 'under'
  readonly dof: number
  readonly conflicting: number
  readonly redundant: number
}

/** The DOF state of the sketch being edited; null outside sketch mode or before it is solved. */
export const selectSketchDof: (state: AppState) => SketchDof | null = memoized(
  (state) => {
    const sketchId = state.context === 'sketch' ? (state.sketch?.sketchId ?? null) : undefined
    return [
      sketchId,
      sketchId === undefined || sketchId === null
        ? undefined
        : state.document?.sketches[sketchId]?.solution,
    ] as const
  },
  (sketchId, solution): SketchDof | null => {
    if (sketchId === undefined) {
      return null
    }
    if (sketchId === null) {
      return { state: 'picking', dof: 0, conflicting: 0, redundant: 0 }
    }
    if (solution === undefined) {
      return null
    }
    const counts = {
      dof: solution.dof,
      conflicting: solution.conflicting.length,
      redundant: solution.redundant.length,
    }
    if (solution.entities.length === 0) {
      return { state: 'empty', ...counts }
    }
    if (counts.conflicting > 0) {
      return { state: 'conflict', ...counts }
    }
    if (solution.failed) {
      return { state: 'failed', ...counts }
    }
    return { state: solution.dof === 0 ? 'full' : 'under', ...counts }
  },
)

/** The print health badge (3.17). */
export type HealthBadge = {
  readonly state: 'none' | 'running' | 'ok' | 'warning' | 'error'
  /** Errors of shown checks, omitted findings included. */
  readonly errors: number
  /** Warnings of shown checks, omitted findings included. */
  readonly warnings: number
  /** Whether the report describes an older revision than the document's, or another print orientation. */
  readonly stale: boolean
}

export const selectHealthBadge: (state: AppState) => HealthBadge = memoized(
  (state) =>
    [
      state.print.report,
      state.print.reportRevision,
      state.print.running,
      state.document?.revision ?? null,
      state.settings.hiddenChecks,
      state.print.reportOrientation,
      state.print.orientation,
    ] as const,
  (
    report,
    reportRevision,
    running,
    revision,
    hiddenChecks,
    reportOrientation,
    orientation,
  ): HealthBadge => {
    let errors = 0
    let warnings = 0
    for (const body of report?.bodies ?? []) {
      for (const check of body.report.checks) {
        if (isCheckHidden(hiddenChecks, check.check)) {
          continue
        }
        for (const finding of check.findings) {
          if (finding.severity === 'error') {
            errors += 1
          } else if (finding.severity === 'warning') {
            warnings += 1
          }
        }
        // Findings beyond the cap count with the severity of the check's listed findings.
        if (check.status === 'error') {
          errors += check.omitted
        } else if (check.status === 'warning') {
          warnings += check.omitted
        }
      }
    }
    const stale =
      report !== null &&
      (reportRevision !== revision || !sameOrientation(reportOrientation, orientation))
    if (running) {
      return { state: 'running', errors, warnings, stale }
    }
    if (report === null) {
      return { state: 'none', errors, warnings, stale }
    }
    const state = errors > 0 ? 'error' : warnings > 0 ? 'warning' : 'ok'
    return { state, errors, warnings, stale }
  },
)

/**
 * The rows of the Problems panel, grouped by root cause, and the notes (3.9). It depends on the
 * recipe, the statuses and the diagnostics only, so meshes and rebuild times arriving while the
 * user types in the inspector give the same rows back.
 */
export const selectProblems: (state: AppState) => ProblemRows = memoized(
  (state) =>
    [state.document?.recipe, state.document?.featureStatus, state.document?.diagnostics] as const,
  (recipe, featureStatus, diagnostics) =>
    groupedRows(recipe, featureStatus ?? {}, diagnostics ?? []),
)

/** The number of problems (3.9): one per top-level row, so per root and per document diagnostic; nested rows and notes are not counted. */
export function selectProblemCount(state: AppState): number {
  return selectProblems(state).rows.length
}

/**
 * Why document edits other than the active command's are disabled: `finish_command_first` while a
 * feature command is open (3.7), else null.
 */
export function selectCommandBlock(state: AppState): string | null {
  return state.command === null ? null : MESSAGES.finish_command_first(state.command.title)
}

/**
 * Where a message's action is offered: the banner of the feature command it belongs to (with the
 * candidate markers of its repair mode), whose `apply` actions join the command's transaction
 * (3.7), or a panel (Problems, the selection inspector) or a notification, whose `apply` actions
 * are document edits that wait during a feature command.
 */
export type FixPlace = 'banner' | 'panel' | 'notification'

/**
 * Why an action cannot run now (3.7): an `apply` action of the banner while the preview is pending,
 * since the statuses that carry it are about to be replaced, and one offered anywhere else during a
 * feature command; null when it can.
 */
export function selectFixBlock(state: AppState, action: FixAction, place: FixPlace): string | null {
  if (action.run.kind !== 'apply') {
    return null
  }
  if (place !== 'banner') {
    return selectCommandBlock(state)
  }
  return state.command?.preview === 'pending' ? MESSAGES.preview_pending() : null
}
