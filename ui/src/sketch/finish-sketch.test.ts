import { describe, expect, it } from 'vitest'
import { commitSketchChange, enterSketch, finishSketch } from '../app/actions/sketch'
import type { App } from '../app/createApp'
import type { FeatureRecord, RegionInfo, SketchSolution } from '../generated/protocol'
import { MESSAGES } from '../messages'
import { TEST_CATALOG } from '../test/fakes/catalog'
import { lineEntity, pointEntity, SKETCH_ID, solutionFor, solvedSketch } from './fixtures.testing'
import { sketchHarness } from './view.testing'

// Finishing a sketch (ui.md 3.16.11): what it selects, the failures its edits caused (U-110) and
// the warning about its outline.

function region(key: string): RegionInfo {
  return { reference: { key, fp: `fp-${key}` }, area: 100, centroid: [5, 5] }
}

const SQUARE = [
  lineEntity('e-1', [0, 0], [10, 0]),
  lineEntity('e-2', [10, 0], [10, 10]),
  lineEntity('e-3', [10, 10], [0, 10]),
  lineEntity('e-4', [0, 10], [0, 0]),
]

function showSolution(app: App, solution: SketchSolution): void {
  app.store.setState((state) =>
    state.document === null
      ? {}
      : {
          document: {
            ...state.document,
            sketches: { ...state.document.sketches, [SKETCH_ID]: solvedSketch(solution) },
          },
        },
  )
}

function selectionKeys(app: App): string[] {
  return app.store.getState().selection.map((item) => item.key)
}

describe('finishSketch selection (3.16.11)', () => {
  it('U-99 a sketch with one region selects that region', async () => {
    const harness = sketchHarness({
      entities: SQUARE,
      solution: solutionFor(SQUARE, { regions: [region('r-1')] }),
    })
    await finishSketch(harness.app, { selectResult: true })
    expect(selectionKeys(harness.app)).toEqual([`region:${SKETCH_ID}:r-1`])
    const [item] = harness.app.store.getState().selection
    expect(item).toMatchObject({
      kind: 'sketch_region',
      label: 'Region 1',
      feature: SKETCH_ID,
      sketchItem: { kind: 'region', sketch: SKETCH_ID, region: region('r-1') },
    })
  })

  it('U-99 a sketch with two regions selects the sketch feature', async () => {
    const harness = sketchHarness({
      entities: SQUARE,
      solution: solutionFor(SQUARE, { regions: [region('r-1'), region('r-2')] }),
      state: { catalog: TEST_CATALOG },
    })
    await finishSketch(harness.app, { selectResult: true })
    expect(harness.app.store.getState().selection).toEqual([
      {
        key: `feature:${SKETCH_ID}`,
        kind: 'feature',
        label: 'Sketch 1',
        body: null,
        feature: SKETCH_ID,
        element: null,
        sketchItem: null,
        origin: null,
      },
    ])
  })

  it('U-99 a sketch without regions selects its points, leaving out construction points', async () => {
    const points = [pointEntity('e-p1', [0, 0]), pointEntity('e-p2', [5, 5], true)]
    const harness = sketchHarness({ entities: points })
    await finishSketch(harness.app, { selectResult: true })
    expect(selectionKeys(harness.app)).toEqual([`point:${SKETCH_ID}:e-p1`])
    expect(harness.app.store.getState().selection[0]?.sketchItem).toEqual({
      kind: 'point',
      sketch: SKETCH_ID,
      point: 'e-p1',
    })
  })

  it('selects the sketch feature for curves without a region or points', async () => {
    const open = [lineEntity('e-1', [0, 0], [10, 0])]
    const harness = sketchHarness({ entities: open })
    await finishSketch(harness.app, { selectResult: true })
    expect(selectionKeys(harness.app)).toEqual([`feature:${SKETCH_ID}`])
  })

  it('selects nothing for an empty sketch', async () => {
    const harness = sketchHarness()
    await finishSketch(harness.app, { selectResult: true })
    expect(selectionKeys(harness.app)).toEqual([])
  })

  it('keeps the selection when finishing for a feature command or the Print radio', async () => {
    const harness = sketchHarness({
      entities: SQUARE,
      solution: solutionFor(SQUARE, { regions: [region('r-1')] }),
    })
    await finishSketch(harness.app)
    expect(selectionKeys(harness.app)).toEqual([])
    expect(harness.app.store.getState().context).toBe('model')
  })
})

describe('finishSketch outline warning (3.16.11)', () => {
  it('names profile findings in a warning that leads back to the sketch', async () => {
    const chain = [lineEntity('e-1', [0, 0], [10, 0]), lineEntity('e-2', [10, 0], [10, -8])]
    const harness = sketchHarness({
      entities: chain,
      solution: solutionFor(chain, {
        profile_findings: [
          { type: 'open_endpoint', entity: 'e-1', point: [0, 0] },
          { type: 'open_endpoint', entity: 'e-2', point: [10, -8] },
        ],
      }),
      state: { catalog: TEST_CATALOG },
    })
    await finishSketch(harness.app)
    const notifications = harness.app.store.getState().notifications
    expect(notifications).toHaveLength(1)
    expect(notifications[0]).toMatchObject({
      severity: 'warning',
      message: MESSAGES.profile_problems('Sketch 1', 2),
    })
    expect(notifications[0]?.actions.map((action) => action.label)).toEqual(['Back to sketch'])
  })
})

const FILLET: FeatureRecord = { id: 'f-fil1', type: 'fillet', radius: '1 mm', edges: [] }

describe('failures during a sketch session (U-110)', () => {
  it('reports a feature the session broke when the sketch is finished, not after the edit', async () => {
    const line = lineEntity('e-1', [0, 0], [10, 0])
    const holder: { app: App | null } = { app: null }
    const harness = sketchHarness({
      state: { catalog: TEST_CATALOG },
      core: {
        applyEdits: () => {
          // The edit makes Fillet 1 fail, as the core's `recomputed` event would report.
          holder.app?.store.setState((state) =>
            state.document === null
              ? {}
              : {
                  document: {
                    ...state.document,
                    featureStatus: {
                      ...state.document.featureStatus,
                      'f-fil1': {
                        id: 'f-fil1',
                        status: 'error',
                        messages: [
                          {
                            severity: 'error',
                            code: 'fillet_failed',
                            message: 'The radius is too large for these edges.',
                          },
                        ],
                        repairs: [],
                      },
                    },
                  },
                },
          )
          return Promise.resolve(2)
        },
      },
    })
    holder.app = harness.app
    harness.app.store.setState((state) =>
      state.document === null
        ? {}
        : {
            document: {
              ...state.document,
              recipe: {
                ...state.document.recipe,
                features: [...(state.document.recipe.features ?? []), FILLET],
              },
              featureStatus: {
                'f-fil1': { id: 'f-fil1', status: 'ok', messages: [], repairs: [] },
              },
            },
          },
    )
    await enterSketch(harness.app, SKETCH_ID)
    const result = await commitSketchChange(harness.app, 'Line', {
      add: { entities: [line], constraints: [] },
      requested: [],
      inferred: [],
    })
    expect(result.outcome).toBe('committed')
    expect(harness.app.store.getState().notifications).toEqual([])

    showSolution(harness.app, solutionFor([line]))
    await finishSketch(harness.app)
    const notifications = harness.app.store.getState().notifications
    expect(notifications).toHaveLength(1)
    expect(notifications[0]).toMatchObject({
      severity: 'error',
      message: MESSAGES.feature_failed('Fillet 1', 'The radius is too large for these edges.'),
    })
    expect(notifications[0]?.actions.map((action) => action.label)).toEqual(['Edit', 'Undo'])
  })

  it('reports nothing on finishing when the session broke nothing', async () => {
    const harness = sketchHarness({ state: { catalog: TEST_CATALOG } })
    await enterSketch(harness.app, SKETCH_ID)
    await finishSketch(harness.app)
    expect(harness.app.store.getState().notifications).toEqual([])
  })
})
