import { describe, expect, it } from 'vitest'
import { applySketchConstraints } from '../app/actions/sketch'
import type { App } from '../app/createApp'
import type { SketchConstraint, SketchParams, SketchSolution } from '../generated/protocol'
import { MESSAGES } from '../messages'
import { lineEntity, solutionFor } from './fixtures.testing'
import { settleCommits, sketchHarness } from './view.testing'

// A constraint tool's refusals (ui.md 3.16.6): the over-constrained warning with "Replace them"
// and "Cancel", the redundancy note, and a replacement the solver refuses too.

const LINE = lineEntity('e-1', [0, 0], [20, 0])
const HORIZONTAL: SketchConstraint = { id: 'c-1', type: 'horizontal', line: 'e-1' }
const LENGTH: SketchConstraint = { id: 'c-2', type: 'distance', line: 'e-1', expr: '40' }
const VERTICAL: SketchConstraint = { id: 'c-9', type: 'vertical', line: 'e-1' }

function has(candidate: SketchParams, id: string): boolean {
  return (candidate.constraints ?? []).some((constraint) => constraint.id === id)
}

/** The solver's verdicts: the new vertical conflicts with the horizontal, or else with the length. */
function conflicts(candidate: SketchParams): SketchSolution {
  const solution = solutionFor(candidate.entities ?? [], {
    constraints: [{ id: 'c-2', state: 'satisfied', value: 40 }],
  })
  if (!has(candidate, 'c-9')) {
    return solution
  }
  return { ...solution, conflicting: has(candidate, 'c-1') ? ['c-1', 'c-9'] : ['c-2', 'c-9'] }
}

function warnings(app: App) {
  return app.store.getState().notifications
}

function runAction(app: App, label: string): void {
  const action = warnings(app)
    .flatMap((notification) => notification.actions)
    .find((candidate) => candidate.label === label)
  if (action === undefined) {
    throw new Error(`No notification offers "${label}".`)
  }
  action.run()
}

describe('constraint tool refusals (3.16.6)', () => {
  it('warns when the new constraint conflicts, naming the others by title, with Replace them and Cancel', async () => {
    const harness = sketchHarness({
      entities: [LINE],
      constraints: [HORIZONTAL],
      solve: (candidate) =>
        has(candidate, 'c-9')
          ? { ...solutionFor(candidate.entities ?? []), conflicting: ['c-1', 'c-9'] }
          : solutionFor(candidate.entities ?? []),
    })
    const added = await applySketchConstraints(harness.app, 'vertical', [VERTICAL])
    expect(added).toBe(false)
    expect(harness.applied).toEqual([])
    const [warning] = warnings(harness.app)
    expect(warning).toMatchObject({
      severity: 'warning',
      message: MESSAGES.over_constrained('Vertical', 'Horizontal'),
    })
    expect(warning?.actions.map((action) => action.label)).toEqual(['Replace them', 'Cancel'])
  })

  it('Replace them commits the new constraint without the conflicting ones under its title', async () => {
    const harness = sketchHarness({
      entities: [LINE],
      constraints: [HORIZONTAL],
      solve: (candidate) =>
        has(candidate, 'c-9') && has(candidate, 'c-1')
          ? { ...solutionFor(candidate.entities ?? []), conflicting: ['c-1', 'c-9'] }
          : solutionFor(candidate.entities ?? []),
    })
    await applySketchConstraints(harness.app, 'vertical', [VERTICAL])
    runAction(harness.app, 'Replace them')
    await settleCommits()
    expect(warnings(harness.app)).toEqual([])
    expect(harness.applied).toHaveLength(1)
    const [applied] = harness.applied
    expect(applied?.label).toBe('Vertical')
    const [edit] = applied?.edits ?? []
    expect(edit?.type === 'update_feature' ? edit.feature['constraints'] : null).toEqual([VERTICAL])
  })

  it('Cancel dismisses the warning and commits nothing', async () => {
    const harness = sketchHarness({
      entities: [LINE],
      constraints: [HORIZONTAL],
      solve: (candidate) =>
        has(candidate, 'c-9')
          ? { ...solutionFor(candidate.entities ?? []), conflicting: ['c-1', 'c-9'] }
          : solutionFor(candidate.entities ?? []),
    })
    await applySketchConstraints(harness.app, 'vertical', [VERTICAL])
    const probes = harness.probes.length
    runAction(harness.app, 'Cancel')
    await settleCommits()
    expect(warnings(harness.app)).toEqual([])
    expect(harness.probes).toHaveLength(probes)
    expect(harness.applied).toEqual([])
  })

  it('a redundant constraint gives the info note and is not added', async () => {
    const harness = sketchHarness({
      entities: [LINE],
      constraints: [HORIZONTAL],
      solve: (candidate) =>
        has(candidate, 'c-9')
          ? { ...solutionFor(candidate.entities ?? []), redundant: ['c-9'] }
          : solutionFor(candidate.entities ?? []),
    })
    const added = await applySketchConstraints(harness.app, 'vertical', [VERTICAL])
    expect(added).toBe(false)
    expect(harness.applied).toEqual([])
    expect(warnings(harness.app)).toEqual([
      expect.objectContaining({
        severity: 'info',
        message: MESSAGES.redundant_constraint('Vertical'),
        actions: [],
      }),
    ])
  })

  it('a refused replacement names what it still conflicts with by title, not by id', async () => {
    const harness = sketchHarness({
      entities: [LINE],
      constraints: [HORIZONTAL, LENGTH],
      solution: solutionFor([LINE], {
        constraints: [{ id: 'c-2', state: 'satisfied', value: 40 }],
      }),
      solve: conflicts,
    })
    await applySketchConstraints(harness.app, 'vertical', [VERTICAL])
    runAction(harness.app, 'Replace them')
    await settleCommits()
    expect(harness.applied).toEqual([])
    expect(warnings(harness.app)).toEqual([
      expect.objectContaining({
        severity: 'warning',
        message: MESSAGES.over_constrained('Vertical', 'Distance 40'),
      }),
    ])
  })
})
