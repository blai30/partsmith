import { cleanup, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { AppProvider } from '../app/AppProvider'
import type { ProfileFinding, SketchEntity, SketchSolution } from '../generated/protocol'
import type { AppState, SketchSession } from '../state/store'
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

  it('U-63 under_constrained with dof 3 is under, "Not fully constrained · 3 DOF"', () => {
    const header = renderHeader([LINE], solutionFor([LINE], { dof: 3 }))
    expect(header).toHaveAttribute('data-state', 'under')
    expect(header).toHaveAttribute('data-dof', '3')
    expect(header).toHaveTextContent('Not fully constrained · 3 DOF')
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

  it('U-63 status redundant with dof 0 and one redundant id is full with " · 1 extra constraint" appended', () => {
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
    expect(header).toHaveTextContent('Fully constrained · 1 extra constraint')
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

function fullWith(findings: ProfileFinding[]): SketchSolution {
  return solutionFor([LINE], {
    status: 'fully_constrained',
    dof: 0,
    entities: [solvedEntity(LINE, 'fully_constrained')],
    profile_findings: findings,
  })
}

describe('SketchHeader outline (3.16.8)', () => {
  const OPEN_START: ProfileFinding = { type: 'open_endpoint', entity: 'e-1', point: [0, 0] }
  const OPEN_END: ProfileFinding = { type: 'open_endpoint', entity: 'e-1', point: [10, 0] }
  const OVERLAP: ProfileFinding = {
    type: 'overlap',
    entities: ['e-1', 'e-2'],
    start: [0, 0],
    end: [5, 0],
  }

  it('U-63 two open ends keep full and append " · 2 open ends" with data-outline 2', () => {
    const header = renderHeader([LINE], fullWith([OPEN_START, OPEN_END]))
    expect(header).toHaveAttribute('data-state', 'full')
    expect(header).toHaveAttribute('data-outline', '2')
    expect(header).toHaveTextContent('Fully constrained · 2 open ends')
  })

  it('U-63 an open end and an overlap append " · 2 outline problems"', () => {
    const header = renderHeader([LINE], fullWith([OPEN_START, OVERLAP]))
    expect(header).toHaveTextContent('Fully constrained · 2 outline problems')
  })

  it('has data-outline 0 without findings', () => {
    expect(renderHeader([LINE], fullWith([]))).toHaveAttribute('data-outline', '0')
  })
})

describe('plane picking header (3.16.1)', () => {
  it('offers the three origin planes, and Top creates the sketch on XY', async () => {
    const applied: unknown[] = []
    const app = createSketchTestApp({
      core: createFakeCore({
        applyEdits: (_handle, label, edits) => {
          applied.push({ label, edits })
          return Promise.reject(new Error('stop after the edit'))
        },
      }),
      state: {
        context: 'sketch',
        sketch: sketchSession({ sketchId: null }),
        document: documentState({ features: [] }),
      },
    })
    render(
      <AppProvider app={app}>
        <SketchHeader />
      </AppProvider>,
    )
    const header = screen.getByTestId('sketch-header')
    expect(header).toHaveAttribute('data-state', 'picking')
    expect(header).toHaveTextContent('Pick a plane or a flat face to draw on')
    for (const name of [
      'Sketch on Top plane (XY)',
      'Sketch on Front plane (XZ)',
      'Sketch on Right plane (YZ)',
    ]) {
      expect(screen.getByRole('button', { name })).toBeInTheDocument()
    }
    const error = vi.spyOn(console, 'error').mockImplementation(() => undefined)
    await userEvent.click(screen.getByRole('button', { name: 'Sketch on Top plane (XY)' }))
    await vi.waitFor(() => {
      expect(applied).toHaveLength(1)
    })
    expect(applied[0]).toMatchObject({
      label: 'New sketch',
      edits: [
        {
          type: 'add_feature',
          feature: { type: 'sketch', plane: { kind: 'origin', plane: 'XY' } },
        },
      ],
    })
    error.mockRestore()
  })

  it('shows plane_pick_hint in the sketch hint', () => {
    const app = createSketchTestApp({
      core: createFakeCore({}),
      state: { context: 'sketch', sketch: sketchSession({ sketchId: null }) },
    })
    render(
      <AppProvider app={app}>
        <SketchHeader />
      </AppProvider>,
    )
    expect(screen.getByTestId('sketch-hint')).toHaveTextContent('Top plane (XY) is the print bed.')
  })
})

function renderWith(fields: Partial<AppState>): void {
  const app = createSketchTestApp({ core: createFakeCore({}), state: fields })
  render(
    <AppProvider app={app}>
      <SketchHeader />
    </AppProvider>,
  )
}

function editing(
  entities: SketchEntity[],
  session: Partial<SketchSession> = {},
): Partial<AppState> {
  return {
    context: 'sketch',
    sketch: sketchSession(session),
    document: documentState({
      features: [sketchRecord({ entities })],
      sketches: { [SKETCH_ID]: solvedSketch(solutionFor(entities)) },
    }),
  }
}

describe('sketch hint (3.16.3)', () => {
  it('prompts to start drawing in an empty sketch with no tool', () => {
    renderWith(editing([]))
    expect(screen.getByTestId('sketch-hint')).toHaveTextContent(
      'Choose Line (L), Rectangle (R) or Circle (C) to start drawing.',
    )
  })

  it('is empty in a sketch with entities and no tool, and stays a live region', () => {
    renderWith(editing([LINE]))
    const hint = screen.getByTestId('sketch-hint')
    expect(hint).toHaveTextContent('')
    expect(screen.getByRole('status')).toBe(hint)
  })

  it('shows a refused pick in place of the prompt', () => {
    renderWith({ ...editing([]), pickHint: { message: 'Pick a flat face.', field: null } })
    expect(screen.getByTestId('sketch-hint')).toHaveTextContent('Pick a flat face.')
  })

  it("ignores a command field's pick hint", () => {
    renderWith({ ...editing([LINE]), pickHint: { message: 'Pick edges.', field: 'edges' } })
    expect(screen.getByTestId('sketch-hint')).toHaveTextContent('')
  })
})
