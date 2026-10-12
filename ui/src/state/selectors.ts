import { featureLabel } from '../core/labels'
import type { Diagnostic } from '../generated/protocol'
import type { AppState } from './store'

// Derived views of the state (ui.md 2.8). Selectors that build new values remember their last
// inputs and result, so `useAppState(selector)` gets the same value back until the inputs change.

/**
 * Memoizes a derivation on the identity of its inputs: while every input is the same object as
 * last time, the last result is returned again.
 */
function memoized<Inputs extends unknown[], Result>(
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

/** The display label of a feature (features.md 3.12), or its id when there is no document. */
export function selectFeatureLabel(state: AppState, id: string): string {
  return state.document === null ? id : featureLabel(state.document.recipe, id, state.catalog)
}

/** Ids of the features before the rollback marker, in timeline order. */
export const selectActiveFeatureIds: (state: AppState) => readonly string[] = memoized(
  (state) => [state.document?.recipe] as const,
  (recipe) => {
    const features = recipe?.features ?? []
    const rollback = recipe?.rollback ?? null
    const end = features.findIndex((feature) => feature.id === rollback)
    return features.slice(0, end === -1 ? features.length : end).map((feature) => feature.id)
  },
)

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
  /** Whether the report describes an older revision than the document's. */
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
    ] as const,
  (report, reportRevision, running, revision, hiddenChecks): HealthBadge => {
    let errors = 0
    let warnings = 0
    const hidden = new Set(hiddenChecks)
    for (const body of report?.bodies ?? []) {
      for (const check of body.report.checks) {
        if (hidden.has(check.check)) {
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
    const stale = report !== null && reportRevision !== revision
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

/** One row of the Problems panel (3.9). */
export type Problem =
  | {
      readonly kind: 'feature'
      readonly featureId: string
      readonly label: string
      readonly status: 'warning' | 'error'
      readonly messages: readonly Diagnostic[]
    }
  | { readonly kind: 'document'; readonly diagnostic: Diagnostic }

/** Features with warnings or errors in timeline order, then the document diagnostics. */
export const selectProblems: (state: AppState) => readonly Problem[] = memoized(
  (state) => [state.document, state.catalog] as const,
  (document, catalog): readonly Problem[] => {
    if (document === null) {
      return []
    }
    const features: Problem[] = (document.recipe.features ?? []).flatMap((feature) => {
      const status = document.featureStatus[feature.id]
      if (status === undefined || (status.status !== 'warning' && status.status !== 'error')) {
        return []
      }
      return [
        {
          kind: 'feature',
          featureId: feature.id,
          label: featureLabel(document.recipe, feature.id, catalog),
          status: status.status,
          messages: status.messages,
        },
      ]
    })
    const diagnostics: Problem[] = document.diagnostics.map((diagnostic) => ({
      kind: 'document',
      diagnostic,
    }))
    return [...features, ...diagnostics]
  },
)
