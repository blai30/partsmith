import type { ProfileFinding, SketchSolution } from '../generated/protocol'
import type { Vector2 } from './geometry'

// The solver's and profile check's messages (sketch.md 4.2, 4.3). A solution carries ids and
// findings but no text, so the UI words them exactly as the sketch crate does.

/** A number in sketch.md's message form: rounded to 6 decimals, a rounded zero written 0. */
export function messageNumber(value: number): string {
  const rounded = Math.round(value * 1e6) / 1e6
  return rounded === 0 ? '0' : String(rounded)
}

function messagePoint(point: Vector2): string {
  return `(${messageNumber(point[0])}, ${messageNumber(point[1])})`
}

/** sketch.md 4.2 `NotConverged`. */
const NOT_CONVERGED_MESSAGE =
  'The sketch could not be solved from its current shape. Undo the last change, or drag the geometry closer to the intended shape and try again.'

/**
 * Why a solution with `failed` set cannot be committed: its conflict (sketch.md 4.2 `Conflicting`)
 * when it has one, else that it did not converge.
 */
export function failedSolveMessage(solution: SketchSolution): string {
  const [only] = solution.conflicting
  if (solution.conflicting.length === 1 && only !== undefined) {
    return `Constraint ${only} cannot be satisfied. Delete it or change a dimension.`
  }
  if (solution.conflicting.length > 1) {
    return `Constraints ${solution.conflicting.join(', ')} conflict: they cannot all be satisfied at once. Delete one of them or change a dimension.`
  }
  return NOT_CONVERGED_MESSAGE
}

/** The full message of a profile finding (sketch.md 4.3). */
export function profileFindingMessage(finding: ProfileFinding): string {
  switch (finding.type) {
    case 'open_endpoint':
      return `The end of ${finding.entity} at ${messagePoint(finding.point)} is not connected to other geometry. Connect it with a coincident constraint, or delete the curve if it is not needed.`
    case 'overlap':
      return `${finding.entities[0]} and ${finding.entities[1]} overlap between ${messagePoint(finding.start)} and ${messagePoint(finding.end)}. Delete one of them, or move it so they meet only at their ends.`
    case 'self_intersection':
      return `${finding.entities[0]} and ${finding.entities[1]} cross at ${messagePoint(finding.point)}. A closed outline that crosses itself splits into several regions; check that this is intended.`
    case 'zero_length':
      return `${finding.entity} has no length and is ignored for profiles. Delete it.`
  }
}

/** The short name of a profile issue ring's title (ui.md 3.16.2), or null for findings without a ring. */
function profileIssueTitle(finding: ProfileFinding): string | null {
  switch (finding.type) {
    case 'open_endpoint':
      return 'Open end'
    case 'overlap':
      return 'Overlapping curves'
    case 'self_intersection':
      return 'Curves cross'
    case 'zero_length':
      return null
  }
}

/** Where a profile issue's ring goes (ui.md 3.16.2), or null for zero-length curves. */
function profileIssuePosition(finding: ProfileFinding): Vector2 | null {
  switch (finding.type) {
    case 'open_endpoint':
    case 'self_intersection':
      return finding.point
    case 'overlap':
      return [(finding.start[0] + finding.end[0]) / 2, (finding.start[1] + finding.end[1]) / 2]
    case 'zero_length':
      return null
  }
}

/** The sketch points a finding is about, for framing it. */
export function findingPoints(finding: ProfileFinding): readonly Vector2[] {
  switch (finding.type) {
    case 'open_endpoint':
    case 'self_intersection':
      return [finding.point]
    case 'overlap':
      return [finding.start, finding.end]
    case 'zero_length':
      return []
  }
}

/** A profile finding's ring on the sketch (3.16.2). */
export type IssueRing = {
  /** Where the ring goes, in sketch coordinates. */
  readonly at: Vector2
  /** The ring's SVG title: "Open end", "Overlapping curves" or "Curves cross". */
  readonly title: string
}

/** The rings of a solution's profile findings, in finding order; zero-length curves get none. */
export function issueRings(solution: SketchSolution): readonly IssueRing[] {
  return solution.profile_findings.flatMap((finding) => {
    const at = profileIssuePosition(finding)
    const title = profileIssueTitle(finding)
    return at === null || title === null ? [] : [{ at, title }]
  })
}

/**
 * The outline summary of the header (3.16.8): "1 open end", "{n} open ends", "1 outline problem" or
 * "{n} outline problems"; null without profile findings.
 */
export function outlineProblems(solution: SketchSolution): string | null {
  const findings = solution.profile_findings
  const count = findings.length
  if (count === 0) {
    return null
  }
  if (findings.every((finding) => finding.type === 'open_endpoint')) {
    return count === 1 ? '1 open end' : `${String(count)} open ends`
  }
  return count === 1 ? '1 outline problem' : `${String(count)} outline problems`
}
