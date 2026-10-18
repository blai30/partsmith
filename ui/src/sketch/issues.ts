import type { Connection, Diagnostic, ProfileFinding, SketchSolution } from '../generated/protocol'
import { counted } from '../messages'
import { midpoint, type Vector2 } from './geometry'
import { pointPosition } from './operands'

// The profile findings of a solution (sketch.md 4.3): their messages, which the core renders into
// the solution (engine.md 3.6), the open ends they count, and their rings and framing (ui.md 2.14,
// 3.16.2, 3.16.8).

/** The message of `solution.messages` for profile finding `index` (its `field` is `/profile_findings/{index}`), or null. */
export function findingMessage(solution: SketchSolution, index: number): Diagnostic | null {
  const field = `/profile_findings/${String(index)}`
  return solution.messages.find((message) => message.field === field) ?? null
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
      return midpoint(finding.start, finding.end)
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

/** The other open end an `open_endpoint` finding is paired with, or null for other findings. */
function connection(finding: ProfileFinding): Connection | null {
  return finding.type === 'open_endpoint' ? (finding.connect ?? null) : null
}

/** The solved position of the second open end a finding with `connect` stands for, else null. */
function pairedEnd(solution: SketchSolution, finding: ProfileFinding): Vector2 | null {
  const paired = connection(finding)
  return paired === null ? null : pointPosition(solution, paired.second)
}

/**
 * The points that frame a finding (3.16.8 "Show"): its own points and, for a finding with
 * `connect`, the solved position of its second end.
 */
export function framedPoints(solution: SketchSolution, finding: ProfileFinding): Vector2[] {
  const second = pairedEnd(solution, finding)
  return second === null ? [...findingPoints(finding)] : [...findingPoints(finding), second]
}

/**
 * The rings of a solution's profile findings, in finding order; zero-length curves get none. A
 * finding with `connect` stands for two open ends (sketch.md 4.3) and rings its second end too.
 */
export function issueRings(solution: SketchSolution): readonly IssueRing[] {
  return solution.profile_findings.flatMap((finding) => {
    const at = profileIssuePosition(finding)
    const title = profileIssueTitle(finding)
    if (at === null || title === null) {
      return []
    }
    const rings: IssueRing[] = [{ at, title }]
    const second = pairedEnd(solution, finding)
    if (second !== null) {
      rings.push({ at: second, title })
    }
    return rings
  })
}

/**
 * The open ends of the solution's outline: an `open_endpoint` finding counts 1, or 2 when it has
 * `connect` (it stands for two ends, sketch.md 4.3).
 */
export function openEnds(solution: SketchSolution): number {
  return solution.profile_findings.reduce((count, finding) => {
    if (finding.type !== 'open_endpoint') {
      return count
    }
    return count + (connection(finding) === null ? 1 : 2)
  }, 0)
}

function onlyOpenEnds(solution: SketchSolution): boolean {
  return solution.profile_findings.every((finding) => finding.type === 'open_endpoint')
}

/**
 * The number the outline summary counts, and the header's `data-outline` (3.16.8): the open ends
 * when every finding is `open_endpoint`, else the number of findings; 0 without findings.
 */
export function outlineCount(solution: SketchSolution): number {
  return onlyOpenEnds(solution) ? openEnds(solution) : solution.profile_findings.length
}

/**
 * The outline summary of the header (3.16.8): "1 open end", "{n} open ends", "1 outline problem" or
 * "{n} outline problems"; null without profile findings.
 */
export function outlineProblems(solution: SketchSolution): string | null {
  const count = outlineCount(solution)
  if (count === 0) {
    return null
  }
  return counted(count, onlyOpenEnds(solution) ? 'open end' : 'outline problem')
}
