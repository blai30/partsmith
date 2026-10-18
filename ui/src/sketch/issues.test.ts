import { describe, expect, it } from 'vitest'
import type { Diagnostic, ProfileFinding } from '../generated/protocol'
import { lineEntity, solutionFor } from './fixtures.testing'
import {
  findingMessage,
  findingPoints,
  issueRings,
  openEnds,
  outlineCount,
  outlineProblems,
} from './issues'

// The profile findings of a solution: their messages, which the core renders (sketch.md 4.3,
// ui.md 2.14), the open ends they count and the rings of 3.16.2.

const OPEN: ProfileFinding = { type: 'open_endpoint', entity: 'e-1', point: [10, 0.1234567] }
const OVERLAP: ProfileFinding = {
  type: 'overlap',
  entities: ['e-1', 'e-2'],
  start: [0, 0],
  end: [4, 2],
}
const CROSSING: ProfileFinding = {
  type: 'self_intersection',
  entities: ['e-3', 'e-4'],
  point: [1, 1],
}
const EMPTY: ProfileFinding = { type: 'zero_length', entity: 'e-5' }

/** A finding message as the core renders it for the finding at `index` (engine.md 3.6). */
function findingDiagnostic(index: number, code: string, message: string): Diagnostic {
  return { severity: 'warning', code, message, field: `/profile_findings/${String(index)}` }
}

describe('profile findings', () => {
  it('rings every finding but zero-length curves, where the finding is', () => {
    expect(
      issueRings(solutionFor([], { profile_findings: [OPEN, OVERLAP, CROSSING, EMPTY] })),
    ).toEqual([
      { at: [10, 0.1234567], title: 'Open end' },
      { at: [2, 1], title: 'Overlapping curves' },
      { at: [1, 1], title: 'Curves cross' },
    ])
  })

  it('names the points a finding is about', () => {
    expect(findingPoints(OVERLAP)).toEqual([
      [0, 0],
      [4, 2],
    ])
    expect(findingPoints(EMPTY)).toEqual([])
  })
})

describe('findingMessage (2.14)', () => {
  const SOLVE_ISSUE: Diagnostic = {
    severity: 'error',
    code: 'sketch_not_solved',
    message:
      "'Sketch 1' could not be solved from its current shape. Undo the last change, or drag the geometry closer to the intended shape.",
  }
  const FIRST = findingDiagnostic(
    0,
    'open_endpoint',
    'The end of the line from (0, 0) to (10, 0.123457) at (10, 0.123457) is not connected to other geometry.',
  )
  const SECOND = findingDiagnostic(
    1,
    'overlap',
    'The line from (0, 0) to (4, 2) and the line from (4, 2) to (0, 0) overlap between (0, 0) and (4, 2).',
  )

  it("returns the core's message whose field is the finding's position, after the solve issues", () => {
    const solution = solutionFor([], {
      profile_findings: [OPEN, OVERLAP],
      messages: [SOLVE_ISSUE, FIRST, SECOND],
    })
    expect(findingMessage(solution, 0)).toEqual(FIRST)
    expect(findingMessage(solution, 1)).toEqual(SECOND)
  })

  it('is null for a finding the solution has no message for', () => {
    const solution = solutionFor([], { profile_findings: [OPEN], messages: [SOLVE_ISSUE] })
    expect(findingMessage(solution, 0)).toBeNull()
    expect(findingMessage(solution, 3)).toBeNull()
  })
})

function open(point: [number, number]): ProfileFinding {
  return { type: 'open_endpoint', entity: 'e-1', point }
}

// The 0.3 mm gap of newcomer error corpus NC01: one finding for the end of e-4 at (0, 0.3), paired
// with the start of e-1 at (0, 0) (sketch.md 4.3).
const GAP_ENTITIES = [lineEntity('e-1', [0, 0], [80, 0]), lineEntity('e-4', [0, 50], [0, 0.3])]
const GAP: ProfileFinding = {
  type: 'open_endpoint',
  entity: 'e-4',
  point: [0, 0.3],
  connect: { first: 'e-4.end', second: 'e-1.start', gap: 0.3 },
}

describe('paired open ends (2.14, 3.16.2, U-140)', () => {
  const solution = solutionFor(GAP_ENTITIES, { profile_findings: [GAP] })

  it('U-140 counts a finding with connect as two open ends', () => {
    expect(openEnds(solution)).toBe(2)
    expect(outlineCount(solution)).toBe(2)
    expect(outlineProblems(solution)).toBe('2 open ends')
  })

  it('U-140 rings both ends of a finding with connect', () => {
    expect(issueRings(solution)).toEqual([
      { at: [0, 0.3], title: 'Open end' },
      { at: [0, 0], title: 'Open end' },
    ])
  })

  it('counts a finding with connect once among other outline problems', () => {
    const crossing: ProfileFinding = {
      type: 'self_intersection',
      entities: ['e-1', 'e-4'],
      point: [1, 1],
    }
    const mixed = solutionFor(GAP_ENTITIES, { profile_findings: [GAP, crossing] })

    expect(outlineCount(mixed)).toBe(2)
    expect(outlineProblems(mixed)).toBe('2 outline problems')
  })
})

describe('outlineProblems (2.14)', () => {
  it('is null without findings', () => {
    expect(outlineProblems(solutionFor([]))).toBeNull()
  })

  it('counts open ends when every finding is one', () => {
    expect(outlineProblems(solutionFor([], { profile_findings: [open([0, 0])] }))).toBe(
      '1 open end',
    )
    expect(
      outlineProblems(solutionFor([], { profile_findings: [open([0, 0]), open([1, 0])] })),
    ).toBe('2 open ends')
  })

  it('counts outline problems otherwise', () => {
    const crossing: ProfileFinding = {
      type: 'self_intersection',
      entities: ['e-1', 'e-2'],
      point: [1, 1],
    }
    expect(outlineProblems(solutionFor([], { profile_findings: [crossing] }))).toBe(
      '1 outline problem',
    )
    expect(outlineProblems(solutionFor([], { profile_findings: [open([0, 0]), crossing] }))).toBe(
      '2 outline problems',
    )
  })
})
