import { cleanup, render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { AppProvider } from '../app/AppProvider'
import type { App } from '../app/createApp'
import type { CoreApi } from '../core/api'
import type { Edit, SketchConstraint, SketchEntity } from '../generated/protocol'
import {
  arcEntity,
  circleEntity,
  createFakeCore,
  curveItem,
  documentState,
  lineEntity,
  sequentialIds,
  sketchRecord,
  sketchSession,
  solutionFor,
  solvedSketch,
  SKETCH_ID,
} from '../sketch/fixtures.testing'
import { createAppStore, type SelectionItem } from '../state/store'
import { createTestApp } from '../test/fakes/test-app'
import { entitySize, entitySizeDimension } from './entity-size'
import { SelectionInspector } from './SelectionInspector'

// The size field of the selection inspector (ui.md 3.8).

afterEach(() => {
  cleanup()
})

const LINE = lineEntity('e-1', [0, 0], [30, 0])
const CIRCLE = circleEntity('e-5', [55, 0], 5)
const ARC = arcEntity('e-6', [0, 40], 4, 0, Math.PI / 2)
const ENTITIES: SketchEntity[] = [LINE, CIRCLE, ARC]
const WIDTH: SketchConstraint = {
  id: 'c-1',
  type: 'distance',
  points: ['e-1.start', 'e-1.end'],
  expr: 'width',
}

const SOLUTION = solutionFor(ENTITIES)

function params(constraints: SketchConstraint[] = []) {
  return { plane: { kind: 'origin', plane: 'XY' } as const, entities: ENTITIES, constraints }
}

describe('entitySize (3.8)', () => {
  it('U-115 shows the expression of the driving distance on the two end points of a line', () => {
    expect(entitySize([curveItem('e-1', 'line')], params([WIDTH]), SOLUTION, 'mm')).toMatchObject({
      label: 'Length',
      text: 'width',
      dimension: WIDTH,
    })
  })

  it('finds a distance on the line itself, but not a reference distance', () => {
    const onLine: SketchConstraint = { id: 'c-2', type: 'distance', line: 'e-1', expr: '30 mm' }
    expect(entitySize([curveItem('e-1', 'line')], params([onLine]), SOLUTION, 'mm')?.text).toBe(
      '30 mm',
    )
    const reference: SketchConstraint = { ...WIDTH, driving: false }
    expect(
      entitySize([curveItem('e-1', 'line')], params([reference]), SOLUTION, 'mm'),
    ).toMatchObject({ text: '30', dimension: null })
  })

  it('U-115 shows formatNumber of the diameter of a circle without a dimension', () => {
    expect(entitySize([curveItem('e-5', 'circle')], params(), SOLUTION, 'mm')).toMatchObject({
      label: 'Diameter',
      type: 'diameter',
      text: '10',
      dimension: null,
    })
  })

  it('shows the radius of an arc, and nothing for points or several items', () => {
    expect(entitySize([curveItem('e-6', 'arc')], params(), SOLUTION, 'mm')).toMatchObject({
      label: 'Radius',
      type: 'radius',
      text: '4',
    })
    expect(
      entitySize([curveItem('e-1', 'line'), curveItem('e-5', 'circle')], params(), SOLUTION, 'mm'),
    ).toBeNull()
  })

  it('U-115 a new size for a dimensioned line replaces that dimension with the new expression', () => {
    const size = entitySize([curveItem('e-1', 'line')], params([WIDTH]), SOLUTION, 'mm')
    if (size === null) {
      throw new Error('The line has a size field.')
    }
    expect(
      entitySizeDimension(size, curveItem('e-1', 'line'), 'width + 5', SOLUTION, sequentialIds()),
    ).toEqual({ ...WIDTH, expr: 'width + 5' })
  })

  it('U-115 a new size for a circle is a new driving diameter dimension', () => {
    const size = entitySize([curveItem('e-5', 'circle')], params(), SOLUTION, 'mm')
    if (size === null) {
      throw new Error('The circle has a size field.')
    }
    expect(
      entitySizeDimension(size, curveItem('e-5', 'circle'), '12', SOLUTION, sequentialIds()),
    ).toEqual({ id: 'c-1', type: 'diameter', curve: 'e-5', expr: '12' })
  })
})

function sketchApp(constraints: SketchConstraint[], core: Partial<CoreApi>): App {
  return createTestApp({
    core: createFakeCore({
      evaluate: () => Promise.resolve({ value: 12, dimension: { length: 1, angle: 0 } }),
      ...core,
    }),
    store: createAppStore({
      core: { status: 'ready', engineVersion: '0.1.0', fatal: null, pendingMutations: 0 },
      context: 'sketch',
      sketch: sketchSession(),
      document: documentState({
        features: [sketchRecord({ entities: ENTITIES, constraints })],
        sketches: { [SKETCH_ID]: solvedSketch(SOLUTION) },
      }),
    }),
  })
}

function renderField(app: App, item: SelectionItem) {
  render(
    <AppProvider app={app}>
      <SelectionInspector items={[item]} />
    </AppProvider>,
  )
  return screen.getByTestId('entity-size')
}

/** The constraints of the sketch record the `index`th applyEdits call wrote. */
function writtenConstraints(applyEdits: CoreApi['applyEdits'], index = 0) {
  const edits = vi.mocked(applyEdits).mock.calls[index]?.[2] as readonly Edit[] | undefined
  const update = edits?.find((edit) => edit.type === 'update_feature')
  return update?.type === 'update_feature' ? update.feature['constraints'] : undefined
}

describe('the size field (3.8)', () => {
  it('U-115 committing "width + 5" for a dimensioned line replaces the dimension, labeled "Change dimension"', async () => {
    const applyEdits = vi.fn<CoreApi['applyEdits']>().mockResolvedValue(2)
    const app = sketchApp([WIDTH], {
      solveSketch: () => Promise.resolve(solvedSketch(SOLUTION)),
      applyEdits,
    })
    const field = renderField(app, curveItem('e-1', 'line'))
    expect(field).toHaveAccessibleName('Length')
    expect(field).toHaveValue('width')
    const user = userEvent.setup()
    await user.clear(field)
    await user.type(field, 'width + 5{Enter}')
    await waitFor(() => {
      expect(applyEdits).toHaveBeenCalledTimes(1)
    })
    expect(vi.mocked(applyEdits).mock.calls[0]?.[1]).toBe('Change dimension')
    expect(writtenConstraints(applyEdits)).toEqual([{ ...WIDTH, expr: 'width + 5' }])
  })

  it('U-115 committing "12" for a circle adds a requested diameter dimension, labeled "Dimension"', async () => {
    const applyEdits = vi.fn<CoreApi['applyEdits']>().mockResolvedValue(2)
    const solveSketch = vi.fn<CoreApi['solveSketch']>(() => Promise.resolve(solvedSketch(SOLUTION)))
    const app = sketchApp([], { solveSketch, applyEdits })
    const field = renderField(app, curveItem('e-5', 'circle'))
    expect(field).toHaveAccessibleName('Diameter')
    expect(field).toHaveValue('10')
    const user = userEvent.setup()
    await user.clear(field)
    await user.type(field, '12{Enter}')
    await waitFor(() => {
      expect(applyEdits).toHaveBeenCalledTimes(1)
    })
    expect(vi.mocked(applyEdits).mock.calls[0]?.[1]).toBe('Dimension')
    expect(writtenConstraints(applyEdits)).toEqual([
      expect.objectContaining({ type: 'diameter', curve: 'e-5', expr: '12' }),
    ])
  })

  it('offers "Add reference" and "Cancel" when the new dimension would over-constrain the sketch', async () => {
    const applyEdits = vi.fn<CoreApi['applyEdits']>().mockResolvedValue(2)
    const solveSketch = vi.fn<CoreApi['solveSketch']>((_document, _feature, candidate) => {
      const added = candidate?.constraints?.at(-1)
      const driving = added !== undefined && !('driving' in added && added.driving === false)
      return Promise.resolve(
        solvedSketch(
          driving && added !== undefined
            ? { ...SOLUTION, status: 'redundant', redundant: [added.id] }
            : SOLUTION,
        ),
      )
    })
    const app = sketchApp([], { solveSketch, applyEdits })
    const field = renderField(app, curveItem('e-5', 'circle'))
    const user = userEvent.setup()
    await user.clear(field)
    await user.type(field, '12{Enter}')
    const alert = await screen.findByRole('alert')
    expect(alert).toHaveTextContent(
      'Other dimensions and constraints already set this size. Add it as a reference dimension that only shows the measurement?',
    )
    expect(applyEdits).not.toHaveBeenCalled()
    await user.click(screen.getByRole('button', { name: 'Add reference' }))
    await waitFor(() => {
      expect(applyEdits).toHaveBeenCalledTimes(1)
    })
    expect(writtenConstraints(applyEdits)).toEqual([
      expect.objectContaining({ type: 'diameter', expr: '12', driving: false }),
    ])
  })
})

/** The selected dimension `c-1` of `f-sk1`. */
const DIMENSION_ITEM: SelectionItem = {
  key: `constraint:${SKETCH_ID}:c-1`,
  kind: 'sketch_constraint',
  label: 'Distance',
  body: null,
  feature: SKETCH_ID,
  element: null,
  sketchItem: { kind: 'constraint', sketch: SKETCH_ID, constraint: 'c-1' },
  origin: null,
}

/** A solver that reports every driving dimension it is asked about as conflicting. */
function conflictingSolver(): CoreApi['solveSketch'] {
  return (_document, _feature, candidate) =>
    Promise.resolve(
      solvedSketch(
        candidate === undefined
          ? SOLUTION
          : { ...SOLUTION, status: 'conflicting', conflicting: ['c-1', 'c-9'] },
      ),
    )
}

describe('a dimension in the selection inspector (3.8, 3.16.7)', () => {
  it('a value the sketch cannot take keeps the text, shows dimension_unsolvable and commits nothing', async () => {
    const applyEdits = vi.fn<CoreApi['applyEdits']>().mockResolvedValue(2)
    const app = sketchApp([WIDTH], { solveSketch: conflictingSolver(), applyEdits })
    render(
      <AppProvider app={app}>
        <SelectionInspector items={[DIMENSION_ITEM]} />
      </AppProvider>,
    )
    const field = screen.getByLabelText('Dimension value')
    const user = userEvent.setup()
    await user.clear(field)
    await user.type(field, '45{Enter}')
    await waitFor(() => {
      expect(screen.getByTestId('field-error')).toHaveTextContent(
        'The sketch cannot be solved with this value. Check for dimensions that cannot all be true at once.',
      )
    })
    expect(field).toHaveValue('45')
    expect(applyEdits).not.toHaveBeenCalled()
  })

  it('turning Reference off on a dimension the sketch cannot take says so and commits nothing', async () => {
    const applyEdits = vi.fn<CoreApi['applyEdits']>().mockResolvedValue(2)
    const app = sketchApp([{ ...WIDTH, driving: false }], {
      solveSketch: conflictingSolver(),
      applyEdits,
    })
    render(
      <AppProvider app={app}>
        <SelectionInspector items={[DIMENSION_ITEM]} />
      </AppProvider>,
    )
    await userEvent.setup().click(screen.getByRole('switch', { name: 'Reference' }))
    await waitFor(() => {
      expect(screen.getByTestId('field-error')).toHaveTextContent(
        'The sketch cannot be solved with this value.',
      )
    })
    expect(applyEdits).not.toHaveBeenCalled()
  })

  it('turning Reference on commits it as "Change dimension", since a reference dimension never conflicts', async () => {
    const applyEdits = vi.fn<CoreApi['applyEdits']>().mockResolvedValue(2)
    const app = sketchApp([WIDTH], { solveSketch: conflictingSolver(), applyEdits })
    render(
      <AppProvider app={app}>
        <SelectionInspector items={[DIMENSION_ITEM]} />
      </AppProvider>,
    )
    await userEvent.setup().click(screen.getByRole('switch', { name: 'Reference' }))
    await waitFor(() => {
      expect(applyEdits).toHaveBeenCalledTimes(1)
    })
    expect(vi.mocked(applyEdits).mock.calls[0]?.[1]).toBe('Change dimension')
  })
})
