import { describe, expect, it, vi } from 'vitest'
import { commitSketchChange, deleteSketchItems } from '../app/actions/sketch'
import { startSketchDrag } from '../app/actions/sketch-drag'
import type { App } from '../app/createApp'
import type { CoreApi, SolvedSketch } from '../core/api'
import type { Edit, SketchConstraint, SketchParams, SketchSolution } from '../generated/protocol'
import { createAppStore } from '../state/store'
import { coreErrorFor } from '../test/fakes/core-api'
import { createTestApp } from '../test/fakes/test-app'
import {
  DOCUMENT_HANDLE,
  SKETCH_ID,
  createFakeCore,
  curveItem,
  documentState,
  lineEntity,
  sketchRecord,
  sketchSession,
  solutionFor,
  solvedAs,
  solvedSketch,
  XY_PLANE,
} from './fixtures.testing'
import type { SketchChange } from './session'
import { sketchHarness } from './view.testing'

// sketch.md 4.2: the message of a solve that did not converge.
const NOT_CONVERGED =
  'The sketch could not be solved from its current shape. Undo the last change, or drag the geometry closer to the intended shape and try again.'

// The stored sketch: a horizontal line e-1.
const STORED_LINE = lineEntity('e-1', [0, 0], [10, 0])
const STORED_HORIZONTAL: SketchConstraint = { id: 'c-1', type: 'horizontal', line: 'e-1' }

// The change: a line e-2 from the end of e-1, with an inferred coincident and a requested vertical.
const NEW_LINE = lineEntity('e-2', [10, 0], [10.5, 10])
const INFERRED_COINCIDENT: SketchConstraint = {
  id: 'c-2',
  type: 'coincident',
  points: ['e-1.end', 'e-2.start'],
}
const REQUESTED_VERTICAL: SketchConstraint = { id: 'c-3', type: 'vertical', line: 'e-2' }
const LINE_CHANGE: SketchChange = {
  add: { entities: [NEW_LINE], constraints: [INFERRED_COINCIDENT, REQUESTED_VERTICAL] },
  requested: ['c-3'],
  inferred: ['c-2'],
}

// Where the solver puts e-2 once it is vertical.
const SOLVED_NEW_LINE = lineEntity('e-2', [10, 0], [10, 10.0125])

type SolveSketch = CoreApi['solveSketch']
type ApplyEdits = CoreApi['applyEdits']

function appWith(core: Partial<CoreApi>, plane: SketchParams['plane'] = XY_PLANE): App {
  return sketchHarness({
    entities: [STORED_LINE],
    constraints: [STORED_HORIZONTAL],
    plane,
    core,
  }).app
}

function answers(...solutions: SketchSolution[]): SolveSketch {
  const solve = vi.fn<SolveSketch>()
  for (const solution of solutions) {
    solve.mockResolvedValueOnce(solvedSketch(solution))
  }
  return solve
}

function revision(value: number) {
  return vi.fn<ApplyEdits>().mockResolvedValue(value)
}

/** The candidate params passed to the `index`th solveSketch call. */
function candidateOf(solve: SolveSketch, index: number): SketchParams | undefined {
  return vi.mocked(solve).mock.calls[index]?.[2]
}

describe('commitSketchChange', () => {
  it('U-61 solves the candidate: the stored entities and constraints plus the change', async () => {
    const solveSketch = answers(solutionFor([STORED_LINE, SOLVED_NEW_LINE]))
    const app = appWith({ solveSketch, applyEdits: revision(5) })

    await commitSketchChange(app, 'Line', LINE_CHANGE)

    expect(solveSketch).toHaveBeenCalledWith(DOCUMENT_HANDLE, SKETCH_ID, expect.anything())
    expect(candidateOf(solveSketch, 0)).toEqual({
      plane: { kind: 'origin', plane: 'XY' },
      entities: [STORED_LINE, NEW_LINE],
      constraints: [STORED_HORIZONTAL, INFERRED_COINCIDENT, REQUESTED_VERTICAL],
    })
  })

  it('U-61 a requested id reported conflicting returns over_constrained and calls no applyEdits', async () => {
    const conflicting = solutionFor([STORED_LINE, NEW_LINE], {
      status: 'conflicting',
      conflicting: ['c-1', 'c-3'],
    })
    const applyEdits = revision(5)
    const vertical: SketchConstraint = { id: 'c-3', type: 'vertical', line: 'e-1' }
    const app = appWith({ solveSketch: answers(conflicting), applyEdits })

    const result = await commitSketchChange(app, 'Vertical', {
      add: { entities: [], constraints: [vertical] },
      requested: ['c-3'],
      inferred: [],
    })

    expect(result).toMatchObject({
      outcome: 'over_constrained',
      constraint: 'c-3',
      redundant: false,
    })
    expect(result).toHaveProperty('conflicting', expect.arrayContaining(['c-1']))
    expect(applyEdits).not.toHaveBeenCalled()
  })

  it('a requested id reported redundant returns over_constrained with redundant true and calls no applyEdits (3.16.3)', async () => {
    const redundant = solutionFor([STORED_LINE], { status: 'redundant', redundant: ['c-4'] })
    const applyEdits = revision(5)
    const again: SketchConstraint = { id: 'c-4', type: 'horizontal', line: 'e-1' }
    const app = appWith({ solveSketch: answers(redundant), applyEdits })

    const result = await commitSketchChange(app, 'Horizontal', {
      add: { entities: [], constraints: [again] },
      requested: ['c-4'],
      inferred: [],
    })

    expect(result).toMatchObject({
      outcome: 'over_constrained',
      constraint: 'c-4',
      redundant: true,
    })
    expect(applyEdits).not.toHaveBeenCalled()
  })

  it('U-61 an inferred id reported redundant is removed and the sketch is solved again without it', async () => {
    const solveSketch = answers(
      solutionFor([STORED_LINE, NEW_LINE], { status: 'redundant', redundant: ['c-2'] }),
      solutionFor([STORED_LINE, SOLVED_NEW_LINE]),
    )
    const app = appWith({ solveSketch, applyEdits: revision(5) })

    await commitSketchChange(app, 'Line', LINE_CHANGE)

    expect(solveSketch).toHaveBeenCalledTimes(2)
    expect(candidateOf(solveSketch, 1)?.constraints).toEqual([
      STORED_HORIZONTAL,
      REQUESTED_VERTICAL,
    ])
  })

  it('U-61 applyEdits gets the params without the dropped inferred id and with the solved geometry', async () => {
    const solveSketch = answers(
      solutionFor([STORED_LINE, NEW_LINE], { status: 'redundant', redundant: ['c-2'] }),
      {
        ...solutionFor([STORED_LINE, SOLVED_NEW_LINE]),
        entities: [
          solvedAs(STORED_LINE, 'fully_constrained'),
          solvedAs(SOLVED_NEW_LINE, 'under_constrained'),
        ],
      },
    )
    const applyEdits = revision(5)
    const app = appWith({ solveSketch, applyEdits })

    const result = await commitSketchChange(app, 'Line', LINE_CHANGE)

    const update: Edit = {
      type: 'update_feature',
      feature: {
        id: SKETCH_ID,
        type: 'sketch',
        plane: { kind: 'origin', plane: 'XY' },
        entities: [STORED_LINE, SOLVED_NEW_LINE],
        constraints: [STORED_HORIZONTAL, REQUESTED_VERTICAL],
      },
    }
    expect(applyEdits).toHaveBeenCalledTimes(1)
    expect(applyEdits).toHaveBeenCalledWith(DOCUMENT_HANDLE, 'Line', [update])
    expect(result).toEqual({ outcome: 'committed', revision: 5 })
  })

  it('U-61 a solution with failed true returns failed with the solver message and calls no applyEdits', async () => {
    const failed = solutionFor([STORED_LINE, NEW_LINE], { failed: true })
    const applyEdits = revision(5)
    const app = appWith({ solveSketch: answers(failed), applyEdits })

    const result = await commitSketchChange(app, 'Line', LINE_CHANGE)

    expect(result).toEqual({ outcome: 'failed', message: NOT_CONVERGED })
    expect(applyEdits).not.toHaveBeenCalled()
  })

  it('U-61 an invalid_params CoreError returns failed with the core message and calls no applyEdits', async () => {
    const message = 'Line e-2: the start and end are the same point.'
    const solveSketch = vi
      .fn<SolveSketch>()
      .mockRejectedValue(await coreErrorFor({ code: 'invalid_params', message }))
    const applyEdits = revision(5)
    const app = appWith({ solveSketch, applyEdits })

    const result = await commitSketchChange(app, 'Line', LINE_CHANGE)

    expect(result).toEqual({ outcome: 'failed', message })
    expect(applyEdits).not.toHaveBeenCalled()
  })

  it('an expression_error CoreError returns failed with the core message (3.16.3)', async () => {
    const message = "Unknown name 'widht'. Did you mean 'width'?"
    const solveSketch = vi
      .fn<SolveSketch>()
      .mockRejectedValue(await coreErrorFor({ code: 'expression_error', message }))
    const applyEdits = revision(5)
    const app = appWith({ solveSketch, applyEdits })

    const result = await commitSketchChange(app, 'Dimension', LINE_CHANGE)

    expect(result).toEqual({ outcome: 'failed', message })
    expect(applyEdits).not.toHaveBeenCalled()
  })

  it('U-142 a sketch on a face sends its plane exactly as stored, whatever solution.placement is (3.16.3)', async () => {
    // A face moved by (100, 0, 0) with a Move body: the fallback is in body coordinates.
    const facePlane: SketchParams['plane'] = {
      kind: 'face',
      body: 'b-cube',
      face: { q: 'f-ext1/end-cap', fp: 'plane|n=0,0,1|c=0,0,30|a=400|adj=4,4' },
      fallback: { origin: [0, 0, 30], x_axis: [1, 0, 0], normal: [0, 0, 1] },
      fallback_frame: 'body',
    }
    const placement: SketchSolution['placement'] = {
      origin: [100, 0, 30],
      x_axis: [1, 0, 0],
      normal: [0, 0, 1],
    }
    const solveSketch = answers(solutionFor([STORED_LINE, SOLVED_NEW_LINE], { placement }))
    const applyEdits = revision(5)
    const app = appWith({ solveSketch, applyEdits }, facePlane)

    await commitSketchChange(app, 'Line', LINE_CHANGE)

    const edits = vi.mocked(applyEdits).mock.calls[0]?.[2]
    expect(edits).toHaveLength(1)
    const [edit] = edits ?? []
    expect(edit?.type).toBe('update_feature')
    expect(edit?.type === 'update_feature' ? edit.feature['plane'] : null).toEqual(facePlane)
  })

  it('U-142 a face sketch with a world fallback keeps it in world coordinates, with no fallback_frame added', async () => {
    const worldPlane: SketchParams['plane'] = {
      kind: 'face',
      body: 'b-cube',
      face: { q: 'f-ext1/end-cap', fp: 'plane|n=0,0,1|c=0,0,20|a=400|adj=4,4' },
      fallback: { origin: [0, 0, 19], x_axis: [1, 0, 0], normal: [0, 0, 1] },
    }
    const placement: SketchSolution['placement'] = {
      origin: [0, 0, 20],
      x_axis: [1, 0, 0],
      normal: [0, 0, 1],
    }
    const solveSketch = answers(solutionFor([STORED_LINE, SOLVED_NEW_LINE], { placement }))
    const applyEdits = revision(5)
    const app = appWith({ solveSketch, applyEdits }, worldPlane)

    await commitSketchChange(app, 'Line', LINE_CHANGE)

    const [edit] = vi.mocked(applyEdits).mock.calls[0]?.[2] ?? []
    const plane = edit?.type === 'update_feature' ? edit.feature['plane'] : null
    expect(plane).toEqual(worldPlane)
    expect(Object.keys(plane as object)).not.toContain('fallback_frame')
  })

  it('U-142 probes the candidate with the stored plane too', async () => {
    const facePlane: SketchParams['plane'] = {
      kind: 'face',
      body: 'b-cube',
      face: { q: 'f-ext1/end-cap', fp: 'plane|n=0,0,1|c=0,0,30|a=400|adj=4,4' },
      fallback: { origin: [0, 0, 30], x_axis: [1, 0, 0], normal: [0, 0, 1] },
      fallback_frame: 'body',
    }
    const solveSketch = answers(solutionFor([STORED_LINE, SOLVED_NEW_LINE]))
    const app = appWith({ solveSketch, applyEdits: revision(5) }, facePlane)

    await commitSketchChange(app, 'Line', LINE_CHANGE)

    expect(candidateOf(solveSketch, 0)?.plane).toEqual(facePlane)
  })
})

describe('deleteSketchItems', () => {
  it('removes the entities and every constraint that references them in one commit labeled Delete (3.16.10)', async () => {
    const otherLine = lineEntity('e-3', [0, 5], [10, 5])
    const coincident: SketchConstraint = {
      id: 'c-2',
      type: 'coincident',
      points: ['e-1.end', 'e-3.start'],
    }
    const parallel: SketchConstraint = { id: 'c-3', type: 'parallel', lines: ['e-1', 'e-3'] }
    const remaining: SketchConstraint = { id: 'c-4', type: 'horizontal', line: 'e-3' }
    const solveSketch = vi.fn<SolveSketch>((_documentHandle, _feature, candidate) =>
      Promise.resolve<SolvedSketch>(solvedSketch(solutionFor(candidate?.entities ?? []))),
    )
    const applyEdits = revision(6)
    const app = createTestApp({
      core: createFakeCore({ solveSketch, applyEdits }),
      store: createAppStore({
        context: 'sketch',
        sketch: sketchSession(),
        document: documentState({
          features: [
            sketchRecord({
              entities: [STORED_LINE, otherLine],
              constraints: [STORED_HORIZONTAL, coincident, parallel, remaining],
            }),
          ],
        }),
      }),
    })

    await deleteSketchItems(app, [curveItem('e-1', 'line')])

    expect(applyEdits).toHaveBeenCalledTimes(1)
    expect(applyEdits).toHaveBeenCalledWith(DOCUMENT_HANDLE, 'Delete', [
      {
        type: 'update_feature',
        feature: expect.objectContaining({ entities: [otherLine], constraints: [remaining] }),
      },
    ])
  })
})

describe('commit order (3.16.3, 3.16.9)', () => {
  it('a change asked for right after a drag release is built from the dragged geometry', async () => {
    const dragged = lineEntity('e-1', [0, 0], [10, 4])
    const calls: string[] = []
    const applied: Edit[][] = []
    const dragFrame: { answer: ((solved: SolvedSketch) => void) | null } = { answer: null }
    const store: { app: App | null } = { app: null }
    const app = appWith({
      dragSketch: () =>
        new Promise<SolvedSketch>((resolve) => {
          dragFrame.answer = resolve
        }),
      endSketchDrag: () => {
        calls.push('endSketchDrag')
        // The core stores the dragged geometry, as its document_changed event reports.
        store.app?.store.setState((state) =>
          state.document === null
            ? {}
            : {
                document: {
                  ...state.document,
                  recipe: {
                    ...state.document.recipe,
                    features: [sketchRecord({ entities: [dragged], constraints: [] })],
                  },
                },
              },
        )
        return Promise.resolve(2)
      },
      solveSketch: (_documentHandle, _feature, candidate) => {
        calls.push('solveSketch')
        return Promise.resolve(solvedSketch(solutionFor(candidate?.entities ?? [])))
      },
      applyEdits: (_documentHandle, _label, edits) => {
        calls.push('applyEdits')
        applied.push([...edits])
        return Promise.resolve(3)
      },
    })
    store.app = app
    const drag = startSketchDrag(app, SKETCH_ID, 'e-1.end')
    drag?.move([10, 4], 0)
    const releasing = drag?.finish()
    const committing = commitSketchChange(app, 'Fix', {
      add: { entities: [], constraints: [{ id: 'c-5', type: 'fix', target: 'e-1' }] },
      requested: ['c-5'],
      inferred: [],
    })
    dragFrame.answer?.(solvedSketch(solutionFor([dragged])))
    await releasing
    const result = await committing

    expect(result.outcome).toBe('committed')
    expect(calls).toEqual(['endSketchDrag', 'solveSketch', 'applyEdits'])
    expect(applied).toEqual([
      [
        {
          type: 'update_feature',
          feature: expect.objectContaining({ entities: [dragged] }),
        },
      ],
    ])
  })
})
