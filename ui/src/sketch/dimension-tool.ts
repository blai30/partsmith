import type { SketchConstraint, SketchSolution } from '../generated/protocol'
import type { SelectionItem } from '../state/store'
import type { DimensionType } from './constraints'
import { dimensionFor, dimensionTypeFor, measuredValue } from './dimensions'
import type { Vector2 } from './geometry'
import { operandOf } from './operands'

// The dimension tool's clicks (ui.md 3.16.7): which items it has picked, where the label goes, and
// the dimension that would be added.

/**
 * Where the tool is: waiting for the first item; after a point (or an axis), which needs a point
 * or line next; after a line, whose length a click on empty space places; or placing a label.
 */
export type DimensionPicking =
  | { readonly stage: 'first' }
  | { readonly stage: 'point'; readonly first: SelectionItem }
  | { readonly stage: 'line'; readonly first: SelectionItem }
  | { readonly stage: 'place'; readonly picks: readonly SelectionItem[] }

export const DIMENSION_START: DimensionPicking = { stage: 'first' }

/** A dimension about to be added: its operands, label position and type. */
export type DimensionDraft = {
  readonly picks: readonly SelectionItem[]
  readonly labelAt: Vector2
  readonly type: DimensionType
}

export type DimensionClick = {
  readonly picking: DimensionPicking
  /** The dimension whose editor the click opens, or null. */
  readonly draft: DimensionDraft | null
  /** Why the click was refused, for the sketch hint, or null. */
  readonly refused: string | null
}

/** The items the tool has picked so far. */
export function pickedItems(picking: DimensionPicking): readonly SelectionItem[] {
  switch (picking.stage) {
    case 'first':
      return []
    case 'point':
    case 'line':
      return [picking.first]
    case 'place':
      return picking.picks
  }
}

/** The draft for picks with the label at `labelAt`, or why they cannot be dimensioned. */
export function dimensionDraft(
  picks: readonly SelectionItem[],
  labelAt: Vector2,
  solution: SketchSolution,
): DimensionDraft | { readonly unsupported: string } {
  const type = dimensionTypeFor(picks, labelAt, solution)
  return typeof type === 'string' ? { picks, labelAt, type } : type
}

function unchanged(picking: DimensionPicking): DimensionClick {
  return { picking, draft: null, refused: null }
}

/** Picks a second operand, or refuses it with the reason when the pair cannot be dimensioned. */
function second(
  picking: Extract<DimensionPicking, { stage: 'point' | 'line' }>,
  item: SelectionItem,
  at: Vector2,
  solution: SketchSolution,
): DimensionClick {
  if (item.key === picking.first.key) {
    return unchanged(picking)
  }
  const picks = [picking.first, item]
  const draft = dimensionDraft(picks, at, solution)
  return 'unsupported' in draft
    ? { picking, draft: null, refused: draft.unsupported }
    : unchanged({ stage: 'place', picks })
}

/** The tool's next state for a click at `at` on `item` (null for empty space). */
export function dimensionClick(
  picking: DimensionPicking,
  item: SelectionItem | null,
  at: Vector2,
  solution: SketchSolution,
): DimensionClick {
  switch (picking.stage) {
    case 'first': {
      const operand = item === null ? null : operandOf(item)
      if (item === null || operand === null) {
        return unchanged(picking)
      }
      switch (operand.kind) {
        case 'point':
          return unchanged({ stage: 'point', first: item })
        case 'line':
          // An axis has no length of its own, so it needs a second operand like a point.
          return unchanged(
            operand.axis ? { stage: 'point', first: item } : { stage: 'line', first: item },
          )
        case 'circle':
        case 'arc':
          return unchanged({ stage: 'place', picks: [item] })
      }
    }
    case 'point':
      return item === null ? unchanged(picking) : second(picking, item, at, solution)
    case 'line': {
      if (item !== null) {
        return second(picking, item, at, solution)
      }
      const draft = dimensionDraft([picking.first], at, solution)
      return 'unsupported' in draft
        ? { picking: DIMENSION_START, draft: null, refused: draft.unsupported }
        : { picking: DIMENSION_START, draft, refused: null }
    }
    case 'place': {
      const draft = dimensionDraft(picking.picks, at, solution)
      return 'unsupported' in draft
        ? { picking: DIMENSION_START, draft: null, refused: draft.unsupported }
        : { picking: DIMENSION_START, draft, refused: null }
    }
  }
}

/** A dimension being placed, drawn with its measured value while the label follows the cursor. */
export type DimensionPreview = {
  readonly constraint: SketchConstraint
  readonly value: number
}

const PREVIEW_IDS = { next: () => 'dimension-preview' }

/**
 * The dimension the tool would place with its label at `cursor`: while placing, and after a line
 * (its length), or null when the picks cannot be dimensioned there.
 */
export function dimensionPreview(
  picking: DimensionPicking,
  cursor: Vector2,
  solution: SketchSolution,
): DimensionPreview | null {
  const picks =
    picking.stage === 'place' ? picking.picks : picking.stage === 'line' ? [picking.first] : null
  if (picks === null) {
    return null
  }
  const draft = dimensionDraft(picks, cursor, solution)
  if ('unsupported' in draft) {
    return null
  }
  return draftPreview(draft, solution)
}

/** The constraint and measured value of a draft. */
export function draftPreview(draft: DimensionDraft, solution: SketchSolution): DimensionPreview {
  const value = measuredValue(draft.type, draft.picks, draft.labelAt, solution)
  const constraint = dimensionFor(
    draft.type,
    draft.picks,
    '0',
    true,
    draft.labelAt,
    PREVIEW_IDS,
    solution,
  )
  return { constraint, value }
}
