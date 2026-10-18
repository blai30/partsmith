import { act, render, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { AppProvider } from '../app/AppProvider'
import { installPopoverPolyfill } from '../components/popover-polyfill.testing'
import type { CoreApi } from '../core/api'
import type {
  Diagnostic,
  FixRef,
  ProfileFinding,
  SketchEntity,
  SketchSolution,
} from '../generated/protocol'
import { createAppStore, type AppState, type SketchSession } from '../state/store'
import { createTestApp } from '../test/fakes/test-app'
import {
  SKETCH_ID,
  circleEntity,
  createFakeCore,
  documentState,
  lineEntity,
  sketchRecord,
  sketchSession,
  solutionFor,
  solvedAs,
  solvedSketch,
} from './fixtures.testing'
import { COLOR_CLASSES } from '../theme/roles'
import { SketchHeader } from './SketchHeader'
import { settleCommits, sketchHarness } from './view.testing'

const LINE = lineEntity('e-1', [0, 0], [10, 0])

// The core's message of the open end of LINE (sketch.md 4.3, engine.md 3.6).
const OPEN_END_MESSAGE: Diagnostic = {
  severity: 'warning',
  code: 'open_endpoint',
  message:
    'The end of the line from (0, 0) to (10, 0) at (10, 0) is not connected to other geometry.',
  field: '/profile_findings/0',
}

// The 0.3 mm gap of newcomer error corpus NC01, its one finding, and the finding's message with
// its fix "Connect the ends" as the core offers it for the stored sketch.
const GAP_LINES = [
  lineEntity('e-1', [0, 0], [80, 0]),
  lineEntity('e-2', [80, 0], [80, 50]),
  lineEntity('e-3', [80, 50], [0, 50]),
  lineEntity('e-4', [0, 50], [0, 0.3]),
]
const GAP: ProfileFinding = {
  type: 'open_endpoint',
  entity: 'e-4',
  point: [0, 0.3],
  connect: { first: 'e-4.end', second: 'e-1.start', gap: 0.3 },
}
const CONNECT_FIX: FixRef = {
  feature: SKETCH_ID,
  code: 'open_endpoint',
  field: '/profile_findings/0',
  cause: false,
  index: 0,
  revision: 1,
}
const GAP_MESSAGE: Diagnostic = {
  severity: 'warning',
  code: 'open_endpoint',
  message:
    'The end of the line from (0, 50) to (0, 0.3) at (0, 0.3) is not connected to other geometry.',
  field: '/profile_findings/0',
  actions: [{ label: 'Connect the ends (0.3 mm apart)', run: { kind: 'apply', fix: CONNECT_FIX } }],
}

/** Renders the header in sketch mode on f-sk1 with these entities and this solution. */
function renderHeader(entities: SketchEntity[], solution: SketchSolution): HTMLElement {
  const app = createTestApp({
    core: createFakeCore({}),
    store: createAppStore({
      context: 'sketch',
      sketch: sketchSession(),
      document: documentState({
        features: [sketchRecord({ entities })],
        sketches: { [SKETCH_ID]: solvedSketch(solution) },
      }),
    }),
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

  it('keeps the sketch name and lets the status give way with an ellipsis in a narrow slot (3.24)', () => {
    const header = renderHeader([LINE], solutionFor([LINE], { dof: 3 }))
    const [name] = [...header.children]
    expect(name).toHaveClass('shrink-0', 'max-w-40', 'truncate')
    const status = within(header).getByText('Not fully constrained · 3 DOF').parentElement
    expect(status).toHaveClass('min-w-0', 'truncate')
  })

  it('U-209 is a surface chip with the opaque hover and no rule between its name and status', () => {
    const header = renderHeader([LINE], solutionFor([LINE], { dof: 3 }))
    expect(header).toHaveClass(COLOR_CLASSES.surface.background)
    expect(header).toHaveClass(COLOR_CLASSES.hover.opaque)
    expect(header).toHaveClass('border', COLOR_CLASSES.forcedEdge.border)
    const status = within(header).getByText('Not fully constrained · 3 DOF').parentElement
    expect(status).not.toHaveClass('border-l')
  })

  it('U-63 fully_constrained is full, "Fully constrained"', () => {
    const header = renderHeader(
      [LINE],
      solutionFor([LINE], {
        status: 'fully_constrained',
        dof: 0,
        entities: [solvedAs(LINE, 'fully_constrained')],
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
        entities: [solvedAs(LINE, 'conflicting')],
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
        entities: [solvedAs(LINE, 'fully_constrained')],
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
    const app = createTestApp({
      core: createFakeCore({}),
      store: createAppStore({ context: 'sketch', sketch: sketchSession({ sketchId: null }) }),
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
    entities: [solvedAs(LINE, 'fully_constrained')],
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

  it('counts the 0.3 mm gap of NC01, one finding with connect, as " · 2 open ends" and data-outline 2', () => {
    const lines = [lineEntity('e-1', [0, 0], [80, 0]), lineEntity('e-4', [0, 50], [0, 0.3])]
    const gap: ProfileFinding = {
      type: 'open_endpoint',
      entity: 'e-4',
      point: [0, 0.3],
      connect: { first: 'e-4.end', second: 'e-1.start', gap: 0.3 },
    }
    const header = renderHeader(lines, solutionFor(lines, { profile_findings: [gap] }))
    expect(header).toHaveAttribute('data-outline', '2')
    expect(header).toHaveTextContent('· 2 open ends')
  })
})

describe('plane picking header (3.16.1)', () => {
  it('gives the Top plane button the focus the Sketch button that started the pick had (3.24)', () => {
    const app = createTestApp({
      core: createFakeCore({}),
      store: createAppStore({
        context: 'sketch',
        sketch: sketchSession({ sketchId: null }),
        document: documentState({ features: [] }),
      }),
    })
    expect(document.activeElement).toBe(document.body)
    render(
      <AppProvider app={app}>
        <SketchHeader />
      </AppProvider>,
    )
    expect(screen.getByRole('button', { name: 'Sketch on Top plane (XY)' })).toHaveFocus()
  })

  it('offers the three origin planes, and Top creates the sketch on XY', async () => {
    const applied: unknown[] = []
    const app = createTestApp({
      core: createFakeCore({
        applyEdits: (_handle, label, edits) => {
          applied.push({ label, edits })
          return Promise.reject(new Error('stop after the edit'))
        },
      }),
      store: createAppStore({
        context: 'sketch',
        sketch: sketchSession({ sketchId: null }),
        document: documentState({ features: [] }),
      }),
    })
    render(
      <AppProvider app={app}>
        <SketchHeader />
      </AppProvider>,
    )
    const header = screen.getByTestId('sketch-header')
    expect(header).toHaveAttribute('data-state', 'picking')
    expect(header).toHaveTextContent('Pick a plane or a flat face to draw on')
    // It wraps inside the slot rather than overflowing the viewport at 200 percent UI scale.
    expect(header).toHaveClass('max-w-full', 'flex-wrap', 'min-h-10')
    expect(header).not.toHaveClass('whitespace-nowrap', 'h-10')
    const buttons = [
      'Sketch on Top plane (XY)',
      'Sketch on Front plane (XZ)',
      'Sketch on Right plane (YZ)',
    ].map((name) => screen.getByRole('button', { name }))
    // The three buttons are one flex item of the header, so they wrap onto a line together.
    const group = buttons[0]?.parentElement
    expect(group).toHaveClass('flex', 'gap-2')
    expect(group?.parentElement).toBe(header)
    for (const button of buttons) {
      expect(button.parentElement).toBe(group)
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
    const app = createTestApp({
      core: createFakeCore({}),
      store: createAppStore({ context: 'sketch', sketch: sketchSession({ sketchId: null }) }),
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
  const app = createTestApp({ core: createFakeCore({}), store: createAppStore(fields) })
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

describe('top-center slot (3.2)', () => {
  it('U-186 sits in the top-center slot of 3.2, clear of the view cube and passing picks through', () => {
    renderWith(editing([LINE]))
    const column = screen.getByTestId('sketch-header').parentElement
    expect(column).toBe(screen.getByTestId('top-center-slot'))
    expect(column).toHaveClass(
      'flex',
      'shrink-0',
      'flex-col',
      'items-center',
      'gap-2',
      '@xl/slot:mx-38',
    )
    expect(column).toContainElement(screen.getByTestId('sketch-hint'))
    const slot = column?.parentElement
    expect(slot).toHaveClass(
      'pointer-events-none',
      'absolute',
      'inset-4',
      'flex',
      'flex-col',
      '@container/slot',
    )
    const spacer = slot?.firstElementChild
    expect(spacer).toHaveAttribute('aria-hidden', 'true')
    expect(spacer).toHaveClass('@max-xl/slot:h-44')
  })

  it('U-186 shows the header and the hint as one-line chips', () => {
    renderWith({ context: 'sketch', sketch: sketchSession({ sketchId: null }) })
    for (const chip of [screen.getByTestId('sketch-header'), screen.getByTestId('sketch-hint')]) {
      expect(chip).toHaveClass('rounded-lg', 'border', 'shadow-md')
      expect(chip).toHaveClass(COLOR_CLASSES.elevation.shadow)
      expect(chip).toHaveClass(COLOR_CLASSES.surface.background)
      expect(chip).toHaveClass(COLOR_CLASSES.forcedEdge.border)
    }
  })

  it('shows the hint it is given in place of the step prompt', () => {
    const app = createTestApp({ core: createFakeCore({}), store: createAppStore(editing([])) })
    render(
      <AppProvider app={app}>
        <SketchHeader
          hint={{ text: 'Select a line or two points.', warning: false }}
          freePlacement
        />
      </AppProvider>,
    )
    expect(screen.getByTestId('sketch-hint')).toHaveTextContent(
      'Select a line or two points. Hold Ctrl to place freely.',
    )
  })
})

describe('sketch problems (3.16.8)', () => {
  it('lists the free geometry once opened, ending with the DOF help', async () => {
    const solution = solutionFor([LINE], {
      dof: 4,
      entities: [solvedAs(LINE, 'under_constrained')],
    })
    renderHeader([LINE], solution)
    const problems = screen.getByTestId('sketch-problems')
    expect(problems).toBeEmptyDOMElement()
    await userEvent.click(screen.getByTestId('sketch-header'))
    expect(problems).toHaveTextContent('Free geometry: 1 line')
    expect(problems).toHaveTextContent(
      'Blue geometry can still move or change size. Add dimensions (D) or constraints where the size matters; you can extrude at any time.',
    )
  })
})

describe('sketch problems actions (3.16.8)', () => {
  beforeEach(installPopoverPolyfill)

  const HORIZONTAL = { id: 'c-1', type: 'horizontal', line: 'e-1' } as const
  const VERTICAL = { id: 'c-2', type: 'vertical', line: 'e-1' } as const

  it('lists conflicting and extra constraints, and Delete removes one in a commit labeled "Delete"', async () => {
    const harness = sketchHarness({
      entities: [LINE],
      constraints: [HORIZONTAL, VERTICAL],
      solution: solutionFor([LINE], { conflicting: ['c-2'], redundant: ['c-1'] }),
    })
    render(
      <AppProvider app={harness.app}>
        <SketchHeader />
      </AppProvider>,
    )
    await userEvent.click(screen.getByTestId('sketch-header'))
    const problems = screen.getByTestId('sketch-problems')
    const constraints = within(problems).getByRole('region', { name: 'Constraints' })
    expect(constraints).toHaveTextContent('HorizontalExtra')
    expect(constraints).toHaveTextContent('VerticalConflicting')
    await userEvent.click(within(constraints).getByRole('button', { name: 'Delete Vertical' }))
    await act(settleCommits)
    expect(harness.applied.map((entry) => entry.label)).toEqual(['Delete'])
    const [edit] = harness.applied[0]?.edits ?? []
    const stored = edit?.type === 'update_feature' ? edit.feature['constraints'] : undefined
    expect(stored).toEqual([HORIZONTAL])
  })

  it('names each outline problem with a "Show" button that frames it', async () => {
    const harness = sketchHarness({
      entities: [LINE],
      solution: solutionFor([LINE], {
        profile_findings: [{ type: 'open_endpoint', entity: 'e-1', point: [10, 0] }],
        messages: [OPEN_END_MESSAGE],
      }),
    })
    render(
      <AppProvider app={harness.app}>
        <SketchHeader />
      </AppProvider>,
    )
    await userEvent.click(screen.getByTestId('sketch-header'))
    const outline = within(screen.getByTestId('sketch-problems')).getByRole('region', {
      name: 'Outline',
    })
    await userEvent.click(within(outline).getByRole('button', { name: /^Show: / }))
    expect(harness.app.viewport()?.zoomToBounds).toHaveBeenCalledTimes(1)
  })

  it("U-140 lists each finding with the core's message and its actions, and an action calls applyFix", async () => {
    const applyFix = vi.fn<CoreApi['applyFix']>().mockResolvedValue(2)
    const harness = sketchHarness({
      entities: GAP_LINES,
      solution: solutionFor(GAP_LINES, {
        profile_findings: [GAP],
        messages: [GAP_MESSAGE],
      }),
      core: { applyFix },
    })
    render(
      <AppProvider app={harness.app}>
        <SketchHeader />
      </AppProvider>,
    )
    await userEvent.click(screen.getByTestId('sketch-header'))
    const outline = within(screen.getByTestId('sketch-problems')).getByRole('region', {
      name: 'Outline',
    })
    expect(outline).toHaveTextContent(GAP_MESSAGE.message)
    await userEvent.click(
      within(outline).getByRole('button', { name: 'Connect the ends (0.3 mm apart)' }),
    )
    await act(settleCommits)
    expect(applyFix).toHaveBeenCalledExactlyOnceWith(1, CONNECT_FIX)
  })

  it('U-140 "Show" of a finding with connect frames both of its ends', async () => {
    // A 4 mm square whose last side stops 1.9 mm short of the first: the frame spans both ends
    // and grows to a fifth of the sketch's 4 mm only where it is smaller, across the gap.
    const square = [
      lineEntity('e-1', [0, 0], [4, 0]),
      lineEntity('e-2', [4, 0], [4, 4]),
      lineEntity('e-3', [4, 4], [0, 4]),
      lineEntity('e-4', [0, 4], [0, 1.9]),
    ]
    const gap: ProfileFinding = {
      type: 'open_endpoint',
      entity: 'e-4',
      point: [0, 1.9],
      connect: { first: 'e-4.end', second: 'e-1.start', gap: 1.9 },
    }
    const harness = sketchHarness({
      entities: square,
      solution: solutionFor(square, {
        profile_findings: [gap],
        messages: [{ ...GAP_MESSAGE, actions: [] }],
      }),
    })
    render(
      <AppProvider app={harness.app}>
        <SketchHeader />
      </AppProvider>,
    )
    await userEvent.click(screen.getByTestId('sketch-header'))
    const outline = within(screen.getByTestId('sketch-problems')).getByRole('region', {
      name: 'Outline',
    })
    await userEvent.click(within(outline).getByRole('button', { name: /^Show/ }))
    const zoom = vi.mocked(harness.app.viewport()?.zoomToBounds)
    const [bounds] = zoom?.mock.calls.at(-1) ?? []
    expect(bounds).toBeDefined()
    expect(bounds?.min[0]).toBeCloseTo(-0.4)
    expect(bounds?.max[0]).toBeCloseTo(0.4)
    expect(bounds?.min[1]).toBeCloseTo(0)
    expect(bounds?.max[1]).toBeCloseTo(1.9)
    expect(bounds?.min[2]).toBeCloseTo(0)
    expect(bounds?.max[2]).toBeCloseTo(0)
  })

  it('"Select them" puts the free geometry in the sketch selection', async () => {
    const circle = circleEntity('e-2', [30, 0], 5)
    const harness = sketchHarness({
      entities: [LINE, circle],
      solution: solutionFor([LINE, circle], {
        dof: 7,
        entities: [solvedAs(LINE, 'under_constrained'), solvedAs(circle, 'under_constrained')],
      }),
    })
    render(
      <AppProvider app={harness.app}>
        <SketchHeader />
      </AppProvider>,
    )
    await userEvent.click(screen.getByTestId('sketch-header'))
    expect(screen.getByTestId('sketch-problems')).toHaveTextContent(
      'Free geometry: 1 line, 1 circle',
    )
    await userEvent.click(screen.getByRole('button', { name: 'Select them' }))
    expect(harness.app.store.getState().sketch?.selection.map((item) => item.kind)).toEqual([
      'sketch_curve',
      'sketch_curve',
    ])
  })
})
