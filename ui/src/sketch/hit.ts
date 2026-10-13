import type { PointRef, RegionInfo, SketchSolution } from '../generated/protocol'
import type { SelectionItem } from '../state/store'
import { distance, length, type Vector2 } from './geometry'
import { closestOnEntity, ownerOf, solvedEntity, type SolvedEntity } from './operands'

// What the pointer is over in sketch mode (ui.md 3.6): the active sketch's points and curves and
// the sketch's origin and axes, found from the solved geometry, and their selection items.

/** Points are hit within half their 11-pixel pick square plus the 4-pixel pick radius. */
const POINT_HIT_PIXELS = 9.5
/** Curves are hit within half their 7-pixel pick width plus the 4-pixel pick radius. */
const CURVE_HIT_PIXELS = 7.5

export type OriginPart = 'point' | 'x_axis' | 'y_axis'

export type SketchHit =
  | { readonly kind: 'point'; readonly point: PointRef; readonly at: Vector2 }
  | { readonly kind: 'curve'; readonly entity: SolvedEntity }
  | { readonly kind: 'origin'; readonly part: OriginPart }

type Candidate = { readonly hit: SketchHit; readonly pixels: number; readonly origin: boolean }

/** Distances closer than this many pixels are a tie, so rounding never decides between them. */
const TIE_PIXELS = 1e-6

/** Whether a candidate beats the best so far: nearer, or a sketch item tied with an origin item. */
function beats(candidate: Candidate, best: Candidate): boolean {
  if (Math.abs(candidate.pixels - best.pixels) <= TIE_PIXELS) {
    return best.origin && !candidate.origin
  }
  return candidate.pixels < best.pixels
}

/** The nearest candidate within reach; at equal distance sketch items win over origin items. */
function nearest(candidates: readonly Candidate[], reach: number): SketchHit | null {
  let best: Candidate | null = null
  for (const candidate of candidates) {
    if (candidate.pixels <= reach && (best === null || beats(candidate, best))) {
      best = candidate
    }
  }
  return best?.hit ?? null
}

export type HitOptions = {
  /** Sketch millimeters per CSS pixel at the cursor. */
  readonly millimetersPerPixel: number
  /** The UI scale, which scales every pick distance. */
  readonly uiScale: number
  /** Whether the origin and axes count; the drag and box selection only want sketch geometry. */
  readonly includeOrigin: boolean
}

/** What is under the cursor: points first, then curves (ui.md 3.6). */
export function hitTest(
  solution: SketchSolution,
  cursor: Vector2,
  options: HitOptions,
): SketchHit | null {
  const toPixels = (millimeters: number): number => millimeters / options.millimetersPerPixel
  const points: Candidate[] = solution.points.map((solved) => ({
    hit: { kind: 'point', point: solved.point, at: solved.at },
    pixels: toPixels(distance(cursor, solved.at)),
    origin: false,
  }))
  const curves: Candidate[] = solution.entities.flatMap((entity) => {
    const closest = closestOnEntity(entity, cursor)
    return closest === null
      ? []
      : [
          {
            hit: { kind: 'curve', entity },
            pixels: toPixels(distance(cursor, closest)),
            origin: false,
          },
        ]
  })
  if (options.includeOrigin) {
    points.push({
      hit: { kind: 'origin', part: 'point' },
      pixels: toPixels(length(cursor)),
      origin: true,
    })
    curves.push(
      {
        hit: { kind: 'origin', part: 'x_axis' },
        pixels: toPixels(Math.abs(cursor[1])),
        origin: true,
      },
      {
        hit: { kind: 'origin', part: 'y_axis' },
        pixels: toPixels(Math.abs(cursor[0])),
        origin: true,
      },
    )
  }
  return (
    nearest(points, POINT_HIT_PIXELS * options.uiScale) ??
    nearest(curves, CURVE_HIT_PIXELS * options.uiScale)
  )
}

// Keys, kinds and labels match the items the scene gives the same targets (ui.md 3.6), so hover and
// selection mean the same thing whichever picked them.

const ENTITY_NOUNS = { line: 'Line', circle: 'Circle', arc: 'Arc' } as const

function sketchItemBase(
  sketchId: string,
): Pick<SelectionItem, 'body' | 'feature' | 'element' | 'origin'> {
  return { body: null, feature: sketchId, element: null, origin: null }
}

/** The selection item of a sketch point, keyed `point:{sketch}:{point ref}`. */
export function pointSelectionItem(sketchId: string, point: PointRef): SelectionItem {
  return {
    ...sketchItemBase(sketchId),
    key: `point:${sketchId}:${point}`,
    kind: 'sketch_point',
    label: `Point ${point}`,
    sketchItem: { kind: 'point', sketch: sketchId, point },
  }
}

/** The selection item of an entity: point entities as points, curves keyed `sketch:{sketch}:{entity}`. */
export function entitySelectionItem(sketchId: string, entity: SolvedEntity): SelectionItem {
  if (entity.type === 'point') {
    return {
      ...sketchItemBase(sketchId),
      key: `point:${sketchId}:${entity.id}`,
      kind: 'sketch_point',
      label: `Point ${entity.id}`,
      sketchItem: { kind: 'entity', sketch: sketchId, entity: entity.id, entityType: 'point' },
    }
  }
  return {
    ...sketchItemBase(sketchId),
    key: `sketch:${sketchId}:${entity.id}`,
    kind: 'sketch_curve',
    label: `${ENTITY_NOUNS[entity.type]} ${entity.id}`,
    sketchItem: { kind: 'entity', sketch: sketchId, entity: entity.id, entityType: entity.type },
  }
}

/** The selection item of the sketch's own origin (a point) or one of its axes (curves). */
export function originSelectionItem(sketchId: string, part: OriginPart): SelectionItem {
  const sketchItem = { kind: 'origin', sketch: sketchId, which: part } as const
  if (part === 'point') {
    return {
      ...sketchItemBase(sketchId),
      key: `point:${sketchId}:origin`,
      kind: 'sketch_point',
      label: 'Origin',
      sketchItem,
    }
  }
  return {
    ...sketchItemBase(sketchId),
    key: `sketch:${sketchId}:${part}`,
    kind: 'sketch_curve',
    label: part === 'x_axis' ? 'X axis' : 'Y axis',
    sketchItem,
  }
}

/** The selection item of a constraint glyph or dimension label, keyed `constraint:{sketch}:{id}`. */
export function constraintSelectionItem(
  sketchId: string,
  constraint: string,
  label: string,
): SelectionItem {
  return {
    ...sketchItemBase(sketchId),
    key: `constraint:${sketchId}:${constraint}`,
    kind: 'sketch_constraint',
    label,
    sketchItem: { kind: 'constraint', sketch: sketchId, constraint },
  }
}

/** The selection item of a sketch region, keyed `region:{sketch}:{region key}`. */
export function regionSelectionItem(
  sketchId: string,
  region: RegionInfo,
  label: string,
): SelectionItem {
  return {
    ...sketchItemBase(sketchId),
    key: `region:${sketchId}:${region.reference.key}`,
    kind: 'sketch_region',
    label,
    sketchItem: { kind: 'region', sketch: sketchId, region },
  }
}

/** The selection items of a solution's point entities that are not construction, in solution order. */
export function drawnPointItems(sketchId: string, solution: SketchSolution): SelectionItem[] {
  return solution.entities
    .filter((entity) => entity.type === 'point' && entity.construction !== true)
    .map((entity) => pointSelectionItem(sketchId, entity.id))
}

/** The selection item a hit stands for. */
export function hitSelectionItem(
  sketchId: string,
  hit: SketchHit,
  solution: SketchSolution,
): SelectionItem {
  switch (hit.kind) {
    case 'point': {
      // A point entity's only point is the entity itself.
      const entity = solvedEntity(solution, ownerOf(hit.point))
      return entity?.type === 'point'
        ? entitySelectionItem(sketchId, entity)
        : pointSelectionItem(sketchId, hit.point)
    }
    case 'curve':
      return entitySelectionItem(sketchId, hit.entity)
    case 'origin':
      return originSelectionItem(sketchId, hit.part)
  }
}
