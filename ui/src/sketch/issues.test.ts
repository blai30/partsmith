import { describe, expect, it } from 'vitest'
import type { ProfileFinding } from '../generated/protocol'
import { solutionFor } from './fixtures.testing'
import {
  failedSolveMessage,
  findingEntities,
  findingPoints,
  issueRings,
  messageNumber,
  outlineProblems,
  profileFindingMessage,
} from './issues'

// The solver's and profile check's messages (sketch.md 4.2, 4.3) and the rings of 3.16.2.

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

describe('profile findings', () => {
  it('words each finding as the sketch crate does', () => {
    expect(profileFindingMessage(OPEN)).toBe(
      'The end of e-1 at (10, 0.123457) is not connected to other geometry. Connect it with a coincident constraint, or delete the curve if it is not needed.',
    )
    expect(profileFindingMessage(EMPTY)).toBe(
      'e-5 has no length and is ignored for profiles. Delete it.',
    )
  })

  it('rings every finding but zero-length curves, where the finding is', () => {
    expect(
      issueRings(solutionFor([], { profile_findings: [OPEN, OVERLAP, CROSSING, EMPTY] })),
    ).toEqual([
      { at: [10, 0.1234567], title: 'Open end' },
      { at: [2, 1], title: 'Overlapping curves' },
      { at: [1, 1], title: 'Curves cross' },
    ])
  })

  it('names the entities and points a finding is about', () => {
    expect(findingEntities(OPEN)).toEqual(['e-1'])
    expect(findingEntities(OVERLAP)).toEqual(['e-1', 'e-2'])
    expect(findingPoints(OVERLAP)).toEqual([
      [0, 0],
      [4, 2],
    ])
    expect(findingPoints(EMPTY)).toEqual([])
  })
})

describe('failed solves', () => {
  it('names one or more conflicting constraints, else did not converge', () => {
    expect(failedSolveMessage(solutionFor([], { failed: true, conflicting: ['c-1'] }))).toBe(
      'Constraint c-1 cannot be satisfied. Delete it or change a dimension.',
    )
    expect(failedSolveMessage(solutionFor([], { failed: true, conflicting: ['c-1', 'c-2'] }))).toBe(
      'Constraints c-1, c-2 conflict: they cannot all be satisfied at once. Delete one of them or change a dimension.',
    )
    expect(failedSolveMessage(solutionFor([], { failed: true }))).toMatch(
      /^The sketch could not be solved from its current shape\./,
    )
  })

  it('writes numbers rounded to 6 decimals with a rounded zero as 0', () => {
    expect(messageNumber(-0.0000001)).toBe('0')
    expect(messageNumber(2.5)).toBe('2.5')
  })
})

function open(point: [number, number]): ProfileFinding {
  return { type: 'open_endpoint', entity: 'e-1', point }
}

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
