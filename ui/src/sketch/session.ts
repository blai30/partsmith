import type { SketchConstraint } from '../generated/protocol'
import type { ConstraintType } from './constraints'
import type { SketchAddition } from './recipes'

// Sketch mode's tools and the changes they commit (ui.md 2.14, 3.16).

/** A sketch tool: a drawing tool, the dimension tool, or a constraint tool in pick mode. */
export type SketchToolId =
  | 'line'
  | 'rectangle_2point'
  | 'rectangle_center'
  | 'circle'
  | 'arc_3point'
  | 'point'
  | 'dimension'
  | `constrain_${ConstraintType}`

/** One edit of the sketch being edited, committed as one undo entry by `commitSketchChange`. */
export type SketchChange = {
  readonly add?: SketchAddition
  readonly removeEntities?: readonly string[]
  readonly removeConstraints?: readonly string[]
  readonly toggleConstruction?: readonly string[]
  readonly replaceConstraint?: SketchConstraint
  /** Constraint ids the user asked for; refused when the probe finds them redundant or conflicting. */
  readonly requested: readonly string[]
  /** Constraint ids added by inference; dropped silently when redundant or conflicting. */
  readonly inferred: readonly string[]
}

/** What became of a sketch change. */
export type SketchCommitResult =
  | { readonly outcome: 'committed'; readonly revision: number }
  | {
      readonly outcome: 'over_constrained'
      readonly constraint: string
      readonly conflicting: readonly string[]
      readonly redundant: boolean
    }
  | { readonly outcome: 'failed'; readonly message: string }
