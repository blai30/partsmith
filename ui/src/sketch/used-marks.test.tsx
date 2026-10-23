import { act, cleanup, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { deleteSketchItems, enterSketch } from '../app/actions/sketch'
import { AppProvider } from '../app/AppProvider'
import type { SketchEntity, Use } from '../generated/protocol'
import type { AppState } from '../state/store'
import { TEST_CATALOG } from '../test/fakes/catalog'
import { settle } from '../test/fakes/settle'
import {
  chamferRecord,
  edgeReference,
  extrudeRecord,
  filletRecord,
  USED_POINT_USES,
  USED_SKETCH_USES,
} from '../test/fakes/uses'
import {
  curveItem,
  documentState,
  lineEntity,
  pointEntity,
  SKETCH_ID,
  sketchRecord,
  solutionFor,
  solvedSketch,
} from './fixtures.testing'
import { SketchOverlay } from './SketchOverlay'
import {
  mountView,
  nextFrame,
  setTool,
  sketchHarness,
  type SketchHarness,
  type TestView,
} from './view.testing'

// Used marks and the hint on deleting a used entity in sketch mode (ui.md 3.16.10, U-137): the
// entities of the edited sketch that later features use, from `list_uses`, get a "used" mark, and
// deleting one says what uses it and how to keep it.

let view: TestView | null = null

afterEach(() => {
  cleanup()
  view?.remove()
  view = null
  vi.useRealTimers()
})

// U-137's 'Sketch 1' at a scale that fits the test view: a rectangle e-1 to e-4 and a point e-p1.
const ENTITIES: SketchEntity[] = [
  lineEntity('e-1', [0, 0], [40, 0]),
  lineEntity('e-2', [40, 0], [40, 25]),
  lineEntity('e-3', [40, 25], [0, 25]),
  lineEntity('e-4', [0, 25], [0, 0]),
  pointEntity('e-p1', [20, 12]),
]

const FILLET_HINT =
  "'Fillet 1' uses the edge made from this line. Draw its replacement in the same place and 'Fillet 1' will follow it."

/**
 * A session of 'Sketch 1' with 'Extrude 1', 'Fillet 1' and 'Chamfer 1' after it, whose uses are
 * known at the document's revision unless `known` is false; `listUses` answers the same uses.
 */
function usedHarness(
  uses: readonly Use[] = USED_SKETCH_USES,
  options: { readonly known?: boolean; readonly state?: Partial<AppState> } = {},
): SketchHarness {
  const document = documentState({
    features: [
      sketchRecord({ entities: ENTITIES }),
      extrudeRecord('f-ext1', SKETCH_ID, 'new'),
      filletRecord('f-fil1', [edgeReference('edge(f-ext1/end-cap|f-ext1/lateral/e-3)')]),
      chamferRecord('f-cha1', [edgeReference('edge(f-ext1/lateral/e-p1)')]),
    ],
    sketches: { [SKETCH_ID]: solvedSketch(solutionFor(ENTITIES)) },
  })
  return sketchHarness({
    entities: ENTITIES,
    state: {
      catalog: TEST_CATALOG,
      document:
        options.known === false
          ? document
          : { ...document, uses: { revision: document.revision, uses } },
      ...options.state,
    },
    core: { listUses: () => Promise.resolve(uses) },
  })
}

/** Renders the sketch overlay, with its header and hint, inside the test view's viewport region. */
function renderOverlay(harness: SketchHarness): void {
  view = mountView()
  const container = document.createElement('div')
  view.region.append(container)
  render(
    <AppProvider app={harness.app}>
      <SketchOverlay />
    </AppProvider>,
    { container },
  )
}

/** Deletes sketch entities of 'Sketch 1' as the Delete key does, and lets the commit land. */
async function deleteEntities(
  harness: SketchHarness,
  entities: readonly { readonly id: string; readonly type: 'point' | 'line' }[],
): Promise<void> {
  await act(async () => {
    await deleteSketchItems(
      harness.app,
      entities.map((entity) => curveItem(entity.id, entity.type)),
    )
    await settle()
  })
}

function hintText(): string {
  return screen.getByTestId('sketch-hint').textContent ?? ''
}

function usedMarks(): HTMLElement[] {
  return screen.queryAllByTestId('entity-used')
}

describe('refreshing the uses in a sketch session (3.16.10, U-137)', () => {
  it('U-137 asks for the uses when the sketch is entered', async () => {
    const harness = usedHarness(USED_SKETCH_USES, {
      known: false,
      state: { context: 'model', sketch: null },
    })
    const listUses = vi.spyOn(harness.core, 'listUses')
    await act(async () => {
      await enterSketch(harness.app, SKETCH_ID)
      await settle()
    })
    expect(listUses).toHaveBeenCalledTimes(1)
    expect(harness.app.store.getState().document?.uses?.uses).toEqual(USED_SKETCH_USES)
  })

  it('U-137 asks again after each committed sketch edit', async () => {
    const harness = usedHarness()
    const listUses = vi.spyOn(harness.core, 'listUses')
    await deleteEntities(harness, [{ id: 'e-1', type: 'line' }])
    expect(listUses).toHaveBeenCalledTimes(1)
    await deleteEntities(harness, [{ id: 'e-2', type: 'line' }])
    expect(listUses).toHaveBeenCalledTimes(2)
  })
})

describe('used marks (3.16.10, U-137)', () => {
  it("U-137 marks e-3, which made the edge 'Fillet 1' rounds, and no other entity", async () => {
    renderOverlay(usedHarness())
    await nextFrame()
    const marks = usedMarks()
    expect(marks.map((mark) => mark.getAttribute('data-entity'))).toEqual(['e-3'])
    const [mark] = marks
    expect(mark).toHaveAttribute('role', 'img')
    expect(mark).toHaveAccessibleName("Used by 'Fillet 1'")
  })

  it('U-137 marks a used point entity too', async () => {
    renderOverlay(usedHarness(USED_POINT_USES))
    await nextFrame()
    const entities = usedMarks().map((mark) => mark.getAttribute('data-entity') ?? '')
    expect(entities.toSorted()).toEqual(['e-3', 'e-p1'])
    expect(screen.getByRole('img', { name: "Used by 'Chamfer 1'" })).toHaveAttribute(
      'data-entity',
      'e-p1',
    )
  })

  it('places each mark at its entity, as constraint glyphs are (3.12)', async () => {
    renderOverlay(usedHarness())
    await nextFrame()
    const marks = usedMarks()
    expect(marks).toHaveLength(1)
    expect(marks[0]?.closest('[data-positioned]')).not.toBeNull()
  })

  it('U-137 hides the marks while "Show constraints" is off', async () => {
    const harness = usedHarness()
    renderOverlay(harness)
    await nextFrame()
    expect(usedMarks()).toHaveLength(1)
    act(() => {
      harness.app.store.setState((state) =>
        state.sketch === null ? {} : { sketch: { ...state.sketch, showConstraints: false } },
      )
    })
    await nextFrame()
    expect(usedMarks()).toEqual([])
  })

  it('shows no mark while nothing uses the sketch', async () => {
    renderOverlay(usedHarness([]))
    await nextFrame()
    expect(usedMarks()).toEqual([])
  })
})

describe('deleting a used entity (3.16.10, U-137)', () => {
  it('U-137 deleting e-3 commits "Delete" and says what uses it and how to keep it', async () => {
    const harness = usedHarness()
    renderOverlay(harness)
    await nextFrame()
    await deleteEntities(harness, [{ id: 'e-3', type: 'line' }])
    expect(harness.applied.map((edit) => edit.label)).toEqual(['Delete'])
    expect(hintText()).toBe(FILLET_HINT)
  })

  it('U-137 deleting e-1, which nothing uses, shows no such hint', async () => {
    const harness = usedHarness()
    renderOverlay(harness)
    await nextFrame()
    // The uses are known: e-3 is marked.
    expect(usedMarks()).toHaveLength(1)
    await deleteEntities(harness, [{ id: 'e-1', type: 'line' }])
    expect(harness.applied.map((edit) => edit.label)).toEqual(['Delete'])
    expect(hintText()).not.toContain('uses the')
  })

  it("U-137 deleting the used point e-p1 says that 'Chamfer 1' uses the edge made from it", async () => {
    const harness = usedHarness(USED_POINT_USES)
    renderOverlay(harness)
    await nextFrame()
    await deleteEntities(harness, [{ id: 'e-p1', type: 'point' }])
    expect(hintText()).toBe("'Chamfer 1' uses the edge made from this point.")
  })

  it('names the first deleted entity in stored order, whatever the selection order', async () => {
    const harness = usedHarness(USED_POINT_USES)
    renderOverlay(harness)
    await nextFrame()
    await deleteEntities(harness, [
      { id: 'e-p1', type: 'point' },
      { id: 'e-3', type: 'line' },
    ])
    expect(hintText()).toBe(FILLET_HINT)
  })

  it('names the first user in timeline order', async () => {
    const uses: readonly Use[] = [
      ...USED_SKETCH_USES,
      { user: 'f-cha1', field: '/edges/0', feature: 'f-ext1', entity: 'e-3' },
    ]
    const harness = usedHarness(uses)
    renderOverlay(harness)
    await nextFrame()
    await deleteEntities(harness, [{ id: 'e-3', type: 'line' }])
    expect(hintText()).toBe(FILLET_HINT)
  })

  it('the next step prompt replaces the hint', async () => {
    const harness = usedHarness()
    renderOverlay(harness)
    await nextFrame()
    await deleteEntities(harness, [{ id: 'e-3', type: 'line' }])
    expect(hintText()).toBe(FILLET_HINT)
    act(() => {
      setTool(harness.app, 'line')
    })
    await nextFrame()
    expect(hintText()).toBe('Click to start a line.')
  })

  it('the hint goes after 8 s', async () => {
    const harness = usedHarness()
    renderOverlay(harness)
    await nextFrame()
    vi.useFakeTimers({ toFake: ['setTimeout', 'clearTimeout'] })
    await deleteEntities(harness, [{ id: 'e-3', type: 'line' }])
    act(() => {
      vi.advanceTimersByTime(7_900)
    })
    expect(hintText()).toBe(FILLET_HINT)
    act(() => {
      vi.advanceTimersByTime(100)
    })
    expect(hintText()).not.toContain('uses the')
  })

  it('refuses and confirms nothing: the delete is one commit', async () => {
    const harness = usedHarness()
    renderOverlay(harness)
    await nextFrame()
    await deleteEntities(harness, [{ id: 'e-3', type: 'line' }])
    expect(harness.applied).toHaveLength(1)
    expect(screen.queryByRole('alertdialog')).toBeNull()
    expect(harness.app.store.getState().notifications).toEqual([])
  })
})
