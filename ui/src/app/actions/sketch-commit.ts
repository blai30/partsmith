import type { SolvedSketch } from '../../core/api'
import { CoreError } from '../../core/client'
import { CoreFatalError } from '../../core/transport'
import type {
  DocHandle,
  Edit,
  FeatureRecord,
  SketchEntity,
  SketchParams,
  SketchPlane,
  SketchSolution,
} from '../../generated/protocol'
import { MESSAGES } from '../../messages'
import { referencedEntities } from '../../sketch/constraints'
import { failedSolveMessage } from '../../sketch/issues'
import type { SketchChange, SketchCommitResult } from '../../sketch/session'
import type { App } from '../createApp'
import { documentOf } from './edits'
import { recordSketchEdit, sketchParamsOf, sketchRecordOf } from './sketch'

// Committing a change of the edited sketch (ui.md 3.16.3): the candidate params are probed with the
// solver, then applied as one undo entry.

/** The candidate params: the stored ones with the change applied (3.16.3 step 1). */
function applySketchChange(params: SketchParams, change: SketchChange): SketchParams {
  const removedEntities = new Set(change.removeEntities ?? [])
  const removedConstraints = new Set(change.removeConstraints ?? [])
  const toggled = new Set(change.toggleConstruction ?? [])
  const entities = (params.entities ?? [])
    .filter((entity) => !removedEntities.has(entity.id))
    .map((entity) => (toggled.has(entity.id) ? toggledConstruction(entity) : entity))
  const replacement = change.replaceConstraint
  const constraints = (params.constraints ?? [])
    .filter(
      (constraint) =>
        !removedConstraints.has(constraint.id) &&
        !referencedEntities(constraint).some((entity) => removedEntities.has(entity)),
    )
    .map((constraint) =>
      replacement !== undefined && constraint.id === replacement.id ? replacement : constraint,
    )
  return {
    plane: params.plane,
    entities: [...entities, ...(change.add?.entities ?? [])],
    constraints: [...constraints, ...(change.add?.constraints ?? [])],
  }
}

/** The entity with its construction flag flipped; sketch.md omits the flag when false. */
function toggledConstruction(entity: SketchEntity): SketchEntity {
  if (entity.construction === true) {
    const { construction: _construction, ...rest } = entity
    return rest
  }
  return { ...entity, construction: true }
}

/** The solved geometry of each candidate entity, without the solver's `constrained` state. */
function solvedEntities(
  candidate: readonly SketchEntity[],
  solution: SketchSolution,
): SketchEntity[] {
  const solved = new Map(solution.entities.map((entity) => [entity.id, entity]))
  return candidate.map((entity) => {
    const match = solved.get(entity.id)
    if (match === undefined) {
      return entity
    }
    const { constrained: _constrained, ...geometry } = match
    return geometry
  })
}

/** How many times a probe superseded by another solve of the same sketch is sent again. */
const SUPERSEDED_RETRIES = 3

/** The core's refusal of a probe or commit as a failed result; anything else is a bug. */
function failure(error: unknown): SketchCommitResult {
  if (error instanceof CoreError || error instanceof CoreFatalError) {
    return { outcome: 'failed', message: error.message }
  }
  throw error
}

/**
 * Commits one sketch change as one undo entry (3.16.3): the candidate is probed with the solver
 * first; a requested constraint that is redundant or conflicting refuses the change, inferred ones
 * are dropped silently, and a failed solve or a refused probe applies nothing. The committed
 * entities carry the solved geometry, and a sketch on a face records the solved placement as its
 * fallback. Reading the params, probing and applying are one task of the mutation queue, so no
 * drag end, undo or other commit lands between them.
 */
export function commitSketchChange(
  app: App,
  label: string,
  change: SketchChange,
): Promise<SketchCommitResult> {
  // The change belongs to the sketch edited when the user made it, even when the session has
  // finished or moved to another sketch by the time its turn in the queue comes.
  const sketchId = app.store.getState().sketch?.sketchId ?? null
  return app.mutations.run(() => commitNow(app, sketchId, label, change))
}

async function commitNow(
  app: App,
  sketchId: string | null,
  label: string,
  change: SketchChange,
): Promise<SketchCommitResult> {
  const document = documentOf(app)
  const record = sketchRecordOf(app.store.getState(), sketchId)
  const params = record === null ? null : sketchParamsOf(record)
  if (document === null || sketchId === null || record === null || params === null) {
    return { outcome: 'failed', message: 'There is no sketch being edited.' }
  }
  return probe(app, label, {
    documentHandle: document.documentHandle,
    sketchId,
    record,
    candidate: applySketchChange(params, change),
    requested: change.requested,
    inferred: change.inferred,
    retries: 0,
  })
}

/** One solver probe of a commit and what it still has to check. */
type Probe = {
  readonly documentHandle: DocHandle
  readonly sketchId: string
  readonly record: FeatureRecord
  readonly candidate: SketchParams
  readonly requested: readonly string[]
  /** Inferred constraints still in the candidate. */
  readonly inferred: readonly string[]
  /** Probes sent again after another solve of the sketch superseded them. */
  readonly retries: number
}

/** Probes the candidate (3.16.3 step 2) and applies it, or says why not; each retry is one more probe. */
async function probe(app: App, label: string, current: Probe): Promise<SketchCommitResult> {
  let solved: SolvedSketch | null
  try {
    solved = await app.core.solveSketch(current.documentHandle, current.sketchId, current.candidate)
  } catch (error) {
    return failure(error)
  }
  if (solved === null) {
    return current.retries < SUPERSEDED_RETRIES
      ? probe(app, label, { ...current, retries: current.retries + 1 })
      : { outcome: 'failed', message: MESSAGES.core_not_running() }
  }
  const solution = solved.solution
  const refused =
    current.requested.find((id) => solution.conflicting.includes(id)) ??
    current.requested.find((id) => solution.redundant.includes(id))
  if (refused !== undefined) {
    const conflicting = solution.conflicting.includes(refused)
    return {
      outcome: 'over_constrained',
      constraint: refused,
      conflicting: conflicting ? solution.conflicting.filter((id) => id !== refused) : [],
      redundant: !conflicting,
    }
  }
  const dropped = new Set(
    current.inferred.filter(
      (id) => solution.conflicting.includes(id) || solution.redundant.includes(id),
    ),
  )
  if (dropped.size > 0) {
    // Inferred constraints that over-constrain the sketch are dropped and the rest solved again.
    return probe(app, label, {
      ...current,
      candidate: {
        ...current.candidate,
        constraints: (current.candidate.constraints ?? []).filter(
          (constraint) => !dropped.has(constraint.id),
        ),
      },
      inferred: current.inferred.filter((id) => !dropped.has(id)),
    })
  }
  if (solution.failed) {
    return { outcome: 'failed', message: failedSolveMessage(solution) }
  }
  return applyCandidate(app, label, current.record, current.candidate, solution)
}

/** Applies a probed candidate; it runs inside the commit's task of the mutation queue. */
async function applyCandidate(
  app: App,
  label: string,
  record: FeatureRecord,
  candidate: SketchParams,
  solution: SketchSolution,
): Promise<SketchCommitResult> {
  const document = documentOf(app)
  if (document === null) {
    return { outcome: 'failed', message: 'There is no document open.' }
  }
  const plane: SketchPlane =
    candidate.plane.kind === 'face'
      ? { ...candidate.plane, fallback: solution.placement }
      : candidate.plane
  const edit: Edit = {
    type: 'update_feature',
    feature: {
      ...record,
      plane,
      entities: solvedEntities(candidate.entities ?? [], solution),
      constraints: [...(candidate.constraints ?? [])],
    },
  }
  let revision: number
  try {
    revision = await app.core.applyEdits(document.documentHandle, label, [edit])
  } catch (error) {
    return failure(error)
  }
  // No failure notification here: features this edit made fail are reported on finishing (U-110).
  recordSketchEdit(app, record.id)
  return { outcome: 'committed', revision }
}
