import type {
  Diagnostic,
  FeatureStatus,
  FixAction,
  Ghost,
  GhostKind,
  MeshElement,
  Placement,
  Repair,
  SketchSolution,
} from '../generated/protocol'
import { fieldPointer, pointsInto } from '../inspector/status'
import { capitalized, MESSAGES } from '../messages'
import { sketchToWorld } from '../sketch/placement'
import { memoized, selectReattachingSketch } from '../state/selectors'
import type { AppState } from '../state/store'
import { pointAt, type Vector3 } from '../viewport/math'

// Repair mode's data (ui.md 3.7): the current repair of a feature and its numbered candidates, the
// message action that stores each candidate, found by its label (open issue 18), and the outline
// a ghost is drawn with; and the ghost of the cause a `show` action showed (4.2).

export type RepairCandidateView = {
  readonly number: number
  readonly description: string
  /** What it keeps and how far it is; empty for a region candidate, which has no second line. */
  readonly relation: string
  /** The candidate's element in a model-layer mesh; null for a region candidate. */
  readonly element: MeshElement | null
  /** The message action that stores the candidate, or null. */
  readonly action: FixAction | null
}

/** The current repair of a feature and its numbered candidates (3.7). */
export type RepairView = {
  readonly repair: Repair
  readonly candidates: readonly RepairCandidateView[]
}

/** Whether an action's label names the candidate `description` the way a Use or Attach action does (engine.md 3.22). */
function storesCandidate(label: string, description: string): boolean {
  if (
    label === `Use ${description}` ||
    label === `Use only ${description}` ||
    label === `Attach to ${description}`
  ) {
    return true
  }
  const attachSketch = "Attach '"
  const ending = `' to ${description}`
  return (
    label.startsWith(attachSketch) &&
    label.endsWith(ending) &&
    label.length > attachSketch.length + ending.length
  )
}

/**
 * The `apply` action of `messages` labeled `Use {description}`, `Use only {description}`,
 * `Attach to {description}` or `Attach '{sketch}' to {description}` (engine.md 3.22), or null: the
 * label is the only link between a candidate and its action on the wire (open issue 18).
 */
export function candidateAction(
  messages: readonly Diagnostic[],
  description: string,
): FixAction | null {
  for (const message of messages) {
    for (const action of message.actions ?? []) {
      if (action.run.kind === 'apply' && storesCandidate(action.label, description)) {
        return action
      }
    }
  }
  return null
}

/**
 * The current repair of a feature and its numbered candidates (3.7), or null when its status has
 * no repair: the first repair whose path maps to the active field, else the first repair.
 */
export function repairView(status: FeatureStatus, activePath: string | null): RepairView | null {
  const [first] = status.repairs
  if (first === undefined) {
    return null
  }
  const repair =
    (activePath === null
      ? undefined
      : status.repairs.find((candidate) => pointsInto(candidate.path, fieldPointer(activePath)))) ??
    first
  const view = (
    index: number,
    description: string,
    relation: string,
    element: MeshElement | null,
  ): RepairCandidateView => ({
    number: index + 1,
    description,
    relation,
    element,
    action: candidateAction(status.messages, description),
  })
  const candidates =
    repair.kind === 'element'
      ? repair.candidates.map((candidate, index) =>
          view(index, candidate.description, candidate.relation, candidate.element ?? null),
        )
      : repair.candidates.map((candidate, index) => view(index, candidate.description, '', null))
  return { repair, candidates }
}

/** The title of a repair's ghost or ring (4.3): "(missing)" for a loss, "(check it)" while it is still used. */
export function repairTitle(repair: Repair): string {
  const description = capitalized(repair.description)
  return repair.resolution === 'missing'
    ? MESSAGES.repair_missing(description)
    : MESSAGES.repair_check(description)
}

/**
 * The segments of a ghost's points as world points, two per segment: `lines` as given, the
 * boundary of `triangles` (the triangle sides that belong to one triangle only); empty for
 * `points`, which are drawn as rings.
 */
export function ghostSegments(kind: GhostKind, positions: Float32Array): Float32Array {
  if (kind === 'lines') {
    return positions
  }
  if (kind === 'points') {
    return new Float32Array(0)
  }
  const sides = new Map<string, { readonly start: number; readonly end: number; count: number }>()
  const triangles = Math.floor(positions.length / 9)
  for (let i = 0; i < triangles; i++) {
    for (const [start, end] of [
      [i * 3, i * 3 + 1],
      [i * 3 + 1, i * 3 + 2],
      [i * 3 + 2, i * 3],
    ] as const) {
      const ends = [pointAt(positions, start).join(','), pointAt(positions, end).join(',')]
      const key = ends.toSorted().join(' ')
      const side = sides.get(key)
      if (side === undefined) {
        sides.set(key, { start, end, count: 1 })
      } else {
        side.count += 1
      }
    }
  }
  const boundary: number[] = []
  for (const side of sides.values()) {
    if (side.count === 1) {
      boundary.push(...pointAt(positions, side.start), ...pointAt(positions, side.end))
    }
  }
  return Float32Array.from(boundary)
}

/** World points of sketch points on a placement, by sketch buffer: a frame converts each once. */
const worldBySketchPoints = new WeakMap<
  Float32Array,
  { readonly placement: Placement; readonly world: Float32Array }
>()

/** Sketch points (u, v pairs) as world points (x, y, z triples) on `placement`. */
function sketchPointsToWorld(placement: Placement, positions: Float32Array): Float32Array {
  const cached = worldBySketchPoints.get(positions)
  if (cached?.placement === placement) {
    return cached.world
  }
  const world = new Float32Array((positions.length >> 1) * 3)
  for (let i = 0; (i + 1) * 2 <= positions.length; i++) {
    world.set(sketchToWorld(placement, [positions[i * 2] ?? 0, positions[i * 2 + 1] ?? 0]), i * 3)
  }
  worldBySketchPoints.set(positions, { placement, world })
  return world
}

/**
 * A ghost's points as world points: `world` points as given, `sketch` points on the plane of the
 * sketch `sketch` at its latest solved placement; undefined without that sketch's solution.
 */
function worldGhostPoints(
  state: AppState,
  ghost: Ghost,
  positions: Float32Array,
  sketch: string | null,
): Float32Array | undefined {
  if (ghost.frame === 'world') {
    return positions
  }
  const placement =
    sketch === null ? undefined : state.document?.sketches[sketch]?.solution.placement
  return placement === undefined ? undefined : sketchPointsToWorld(placement, positions)
}

/**
 * A repair's ghost from the latest recompute as world points (3.7): an element's display geometry
 * as given, a region's old outline on the plane of its sketch; undefined without one.
 */
export function ghostBuffer(state: AppState, repair: Repair): Float32Array | undefined {
  const ghost = repair.ghost ?? null
  const positions = ghost === null ? undefined : state.document?.ghosts[ghost.positions]
  return ghost === null || positions === undefined
    ? undefined
    : worldGhostPoints(state, ghost, positions, repair.kind === 'region' ? repair.sketch : null)
}

/**
 * The points of each cause ghost a `show` action showed, from the recompute the action was offered
 * in: a later recompute, such as that of the implicit cancel a show during a feature command
 * starts with, numbers its ghost buffers anew, while the ghost stays until the selection changes
 * (4.2).
 */
const shownGhostPoints = new WeakMap<Ghost, Float32Array>()

/** Keeps the points of a cause's ghost as the latest recompute gives them, for a `show` of it (4.2). */
export function keepCauseGhost(state: AppState, ghost: Ghost): void {
  const positions = state.document?.ghosts[ghost.positions]
  if (positions !== undefined) {
    shownGhostPoints.set(ghost, positions)
  }
}

/**
 * The ghost of the cause line whose `show` action ran (4.2) in world points, with how they are
 * joined: a `sketch` ghost lies on the plane of the shown sketch at its latest solved placement.
 * Undefined while nothing such is shown, or when its points or sketch are not there.
 */
export function causeGhost(
  state: AppState,
): { readonly kind: GhostKind; readonly positions: Float32Array } | undefined {
  const shown = state.shown
  const ghost = shown?.ghost ?? null
  const kept = ghost === null ? undefined : shownGhostPoints.get(ghost)
  const positions =
    shown === null || ghost === null || kept === undefined
      ? undefined
      : worldGhostPoints(state, ghost, kept, shown.feature)
  return ghost === null || positions === undefined ? undefined : { kind: ghost.kind, positions }
}

/** Repair mode as shown (3.7): the feature, its repairs, and its current repair with candidates. */
export type RepairMode = {
  readonly feature: string
  readonly repairs: readonly Repair[]
  readonly view: RepairView
}

/**
 * The repair mode shown (3.7): while a feature whose status has repairs is edited, that feature
 * with the repair of the active field; with no feature command, the sketch being re-attached
 * (3.16.1), else the feature `AppState.repair` names (a focused Problems row); else null.
 */
export const selectRepairMode: (state: AppState) => RepairMode | null = memoized(
  (state: AppState) => {
    const command = state.command
    const feature =
      command === null
        ? (selectReattachingSketch(state) ?? state.repair?.feature ?? null)
        : command.editing
          ? command.featureId
          : null
    return [
      feature,
      feature === null ? undefined : state.document?.featureStatus[feature],
      command?.activeField ?? null,
    ] as const
  },
  (feature, status, activePath): RepairMode | null => {
    if (feature === null || status === undefined) {
      return null
    }
    const view = repairView(status, activePath)
    return view === null ? null : { feature, repairs: status.repairs, view }
  },
)

/** The repair of a face, edge or vertex reference. */
export type ElementRepair = Extract<Repair, { kind: 'element' }>

/** The repair of a sketch region reference. */
export type RegionRepair = Extract<Repair, { kind: 'region' }>

/** Where the candidates of a region repair lie in their sketch's latest solution (3.7). */
export type RegionCandidatePlaces = {
  readonly sketch: string
  /** Each candidate's regions as indices into the solution's `regions`, in candidate order. */
  readonly regions: readonly (readonly number[])[]
  /** Each candidate's number: the centroid of its first region on the sketch's placement, or null. */
  readonly anchors: readonly (Vector3 | null)[]
}

/**
 * The regions each candidate of a region repair names, found by key in the sketch's latest
 * solution, and where its number goes (3.7); a region the sketch no longer has is left out.
 */
export function regionCandidatePlaces(
  repair: RegionRepair,
  solution: SketchSolution,
): RegionCandidatePlaces {
  const indexByKey = new Map(
    solution.regions.map((region, index) => [region.reference.key, index] as const),
  )
  const regions = repair.candidates.map((candidate) =>
    candidate.regions.flatMap((region) => {
      const index = indexByKey.get(region.key)
      return index === undefined ? [] : [index]
    }),
  )
  const anchors = regions.map((indices) => {
    const first = indices[0] === undefined ? undefined : solution.regions[indices[0]]
    return first === undefined ? null : sketchToWorld(solution.placement, first.centroid)
  })
  return { sketch: repair.sketch, regions, anchors }
}

/** The places of the current repair's candidates while repair mode shows a region repair, else null (3.7). */
export const selectRegionCandidates: (state: AppState) => RegionCandidatePlaces | null = memoized(
  (state: AppState) => {
    const repair = selectRepairMode(state)?.view.repair ?? null
    const region = repair?.kind === 'region' ? repair : null
    return [
      region,
      region === null ? undefined : state.document?.sketches[region.sketch]?.solution,
    ] as const
  },
  (repair, solution): RegionCandidatePlaces | null =>
    repair === null || solution === undefined ? null : regionCandidatePlaces(repair, solution),
)
