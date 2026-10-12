import { cleanup, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it } from 'vitest'
import { AppProvider } from '../app/AppProvider'
import type { SketchEntity, SketchSolution } from '../generated/protocol'
import {
  SKETCH_ID,
  createFakeCore,
  createSketchTestApp,
  documentState,
  lineEntity,
  sketchRecord,
  sketchSession,
  solutionFor,
  solvedEntity,
  solvedSketch,
} from './fixtures.testing'
import { SketchHeader } from './SketchHeader'

const LINE = lineEntity('e-1', [0, 0], [10, 0])

afterEach(() => {
  cleanup()
})

/** Renders the header in sketch mode on f-sk1 with these entities and this solution. */
function renderHeader(entities: SketchEntity[], solution: SketchSolution): HTMLElement {
  const app = createSketchTestApp({
    core: createFakeCore({}),
    state: {
      context: 'sketch',
      sketch: sketchSession(),
      document: documentState({
        features: [sketchRecord({ entities })],
        sketches: { [SKETCH_ID]: solvedSketch(solution) },
      }),
    },
  })
  render(
    <AppProvider app={app}>
      <SketchHeader />
    </AppProvider>,
  )
  return screen.getByTestId('sketch-header')
}

describe('SketchHeader', () => {
  it('U-63 no entities is empty, "No geometry yet"', () => {
    const header = renderHeader([], solutionFor([], { dof: 0, status: 'fully_constrained' }))
    expect(header).toHaveAttribute('data-state', 'empty')
    expect(header).toHaveTextContent('No geometry yet')
  })

  it('U-63 under_constrained with dof 3 is under, "3 DOF"', () => {
    const header = renderHeader([LINE], solutionFor([LINE], { dof: 3 }))
    expect(header).toHaveAttribute('data-state', 'under')
    expect(header).toHaveAttribute('data-dof', '3')
    expect(header).toHaveTextContent('3 DOF')
  })

  it('U-63 fully_constrained is full, "Fully constrained"', () => {
    const header = renderHeader(
      [LINE],
      solutionFor([LINE], {
        status: 'fully_constrained',
        dof: 0,
        entities: [solvedEntity(LINE, 'fully_constrained')],
      }),
    )
    expect(header).toHaveAttribute('data-state', 'full')
    expect(header).toHaveAttribute('data-dof', '0')
    expect(header).toHaveTextContent('Fully constrained')
  })

  it('U-63 status conflicting with 2 conflicting ids is conflict, "2 conflicting constraints"', () => {
    const header = renderHeader(
      [LINE],
      solutionFor([LINE], {
        status: 'conflicting',
        dof: 0,
        conflicting: ['c-1', 'c-2'],
        entities: [solvedEntity(LINE, 'conflicting')],
        constraints: [
          { id: 'c-1', state: 'conflicting' },
          { id: 'c-2', state: 'conflicting' },
        ],
      }),
    )
    expect(header).toHaveAttribute('data-state', 'conflict')
    expect(header).toHaveTextContent('2 conflicting constraints')
  })

  it('U-63 failed true with no conflicting ids is failed, "Cannot solve"', () => {
    const header = renderHeader([LINE], solutionFor([LINE], { failed: true, dof: 2 }))
    expect(header).toHaveAttribute('data-state', 'failed')
    expect(header).toHaveTextContent('Cannot solve')
  })

  it('U-63 status redundant with dof 0 and one redundant id is full with " · 1 redundant" appended', () => {
    const header = renderHeader(
      [LINE],
      solutionFor([LINE], {
        status: 'redundant',
        dof: 0,
        redundant: ['c-2'],
        entities: [solvedEntity(LINE, 'fully_constrained')],
        constraints: [
          { id: 'c-1', state: 'satisfied' },
          { id: 'c-2', state: 'redundant' },
        ],
      }),
    )
    expect(header).toHaveAttribute('data-state', 'full')
    expect(header).toHaveTextContent('Fully constrained · 1 redundant')
  })

  it('reads "Pick a plane or a flat face" with data-state picking while picking a plane (3.16.8)', () => {
    const app = createSketchTestApp({
      core: createFakeCore({}),
      state: { context: 'sketch', sketch: sketchSession({ sketchId: null }) },
    })
    render(
      <AppProvider app={app}>
        <SketchHeader />
      </AppProvider>,
    )
    const header = screen.getByTestId('sketch-header')
    expect(header).toHaveAttribute('data-state', 'picking')
    expect(header).toHaveTextContent('Pick a plane or a flat face')
  })
})
