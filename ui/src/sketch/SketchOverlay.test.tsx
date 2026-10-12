import { act, cleanup, render, screen, within } from '@testing-library/react'
import { Profiler } from 'react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it } from 'vitest'
import { AppProvider } from '../app/AppProvider'
import type { App } from '../app/createApp'
import type { SketchConstraint } from '../generated/protocol'
import { COLOR_CLASSES, FONT_CLASSES } from '../theme/roles'
import { circleEntity, lineEntity, SKETCH_ID, solutionFor, solvedSketch } from './fixtures.testing'
import type { SketchToolId } from './session'
import { SketchOverlay } from './SketchOverlay'
import {
  clickAt,
  mountView,
  pointerAt,
  pressKey,
  settleCommits,
  sketchHarness,
  type SketchHarness,
  type TestView,
} from './view.testing'

let view: TestView | null = null

afterEach(() => {
  cleanup()
  view?.remove()
  view = null
})

const BASE = lineEntity('e-1', [0, 0], [20, 0])
const HORIZONTAL: SketchConstraint = { id: 'c-1', type: 'horizontal', line: 'e-1' }
const LENGTH: SketchConstraint = { id: 'c-2', type: 'distance', line: 'e-1', expr: '20' }

/** Renders the overlay inside the test view's viewport region, beside its canvas. */
function renderOverlay(
  harness: SketchHarness,
  tool: SketchToolId | null = null,
): HTMLCanvasElement {
  harness.app.store.setState((state) =>
    state.sketch === null ? {} : { sketch: { ...state.sketch, tool } },
  )
  view = mountView()
  const container = document.createElement('div')
  view.region.append(container)
  render(
    <AppProvider app={harness.app}>
      <SketchOverlay />
    </AppProvider>,
    { container },
  )
  return view.canvas
}

/** Lets the controller's scheduled paint run. */
async function nextFrame(): Promise<void> {
  await act(async () => {
    await new Promise((resolve) => {
      requestAnimationFrame(resolve)
    })
  })
}

function setTool(app: App, tool: SketchToolId | null): void {
  act(() => {
    app.store.setState((state) =>
      state.sketch === null ? {} : { sketch: { ...state.sketch, tool } },
    )
  })
}

/** A line along X to `end` with a satisfied horizontal c-1 and a reference length c-3 of `end`. */
function referenceSolution(end: number) {
  return solutionFor([lineEntity('e-1', [0, 0], [end, 0])], {
    constraints: [
      { id: 'c-1', state: 'satisfied' },
      { id: 'c-3', state: 'satisfied', value: end },
    ],
  })
}

describe('SketchOverlay in sketch mode (3.2, 3.16.3)', () => {
  it('holds the sketch header and the hint the controller gives for the active tool', async () => {
    const harness = sketchHarness({ entities: [BASE] })
    renderOverlay(harness, 'line')
    await nextFrame()
    expect(screen.getByTestId('sketch-header')).toHaveAttribute('data-state', 'full')
    expect(screen.getByTestId('sketch-hint')).toHaveTextContent('Click to start a line.')
  })

  it('shows the plane picking header without an overlay while a plane is picked', () => {
    const harness = sketchHarness({
      state: {
        sketch: {
          sketchId: null,
          tool: null,
          construction: false,
          showConstraints: true,
          selection: [],
          dragging: false,
          cameraBefore: null,
        },
      },
    })
    renderOverlay(harness)
    expect(screen.getByTestId('sketch-header')).toHaveAttribute('data-state', 'picking')
    expect(screen.queryByTestId('sketch-overlay')).toBeNull()
  })
})

describe('SketchOverlay annotations (3.16.2)', () => {
  it('renders nothing outside sketch mode', () => {
    const harness = sketchHarness({ state: { context: 'model' } })
    renderOverlay(harness)
    expect(screen.queryByTestId('sketch-overlay')).toBeNull()
  })

  it('renders one positioned glyph per geometric constraint with its state and title', () => {
    const harness = sketchHarness({
      entities: [BASE],
      constraints: [HORIZONTAL],
      solution: solutionFor([BASE], { constraints: [{ id: 'c-1', state: 'redundant' }] }),
    })
    renderOverlay(harness)
    const glyph = screen.getByTestId('constraint-glyph')
    expect(glyph).toHaveAttribute('data-constraint-id', 'c-1')
    expect(glyph).toHaveAttribute('data-type', 'horizontal')
    expect(glyph).toHaveAttribute('data-state', 'redundant')
    expect(glyph).toHaveAccessibleName('Horizontal')
    // The wrapper sits at the anchor; the glyph centers itself on it with translate classes.
    const anchor = glyph.parentElement
    expect(anchor).toHaveAttribute('data-positioned')
    expect(anchor?.getAttribute('style')).toMatch(/^translate: 592px 498px;?$/)
    expect(glyph).toHaveClass('-translate-x-1/2', '-translate-y-1/2')
    expect(glyph).toBeVisible()
  })

  it('hides glyphs while "Show constraints" is off', () => {
    const harness = sketchHarness({ entities: [BASE], constraints: [HORIZONTAL] })
    harness.app.store.setState((state) =>
      state.sketch === null ? {} : { sketch: { ...state.sketch, showConstraints: false } },
    )
    renderOverlay(harness)
    expect(screen.queryByTestId('constraint-glyph')).toBeNull()
  })

  it('labels dimensions with their solved value, a reference in parentheses', () => {
    const reference: SketchConstraint = { ...LENGTH, id: 'c-3', driving: false }
    const harness = sketchHarness({
      entities: [BASE],
      constraints: [LENGTH, reference],
      solution: solutionFor([BASE], {
        constraints: [
          { id: 'c-2', state: 'satisfied', value: 20 },
          { id: 'c-3', state: 'satisfied', value: 20 },
        ],
      }),
    })
    renderOverlay(harness)
    const [driving, measuring] = screen.getAllByTestId('sketch-dimension')
    expect(driving).toHaveTextContent('20')
    expect(driving).toHaveAttribute('data-type', 'distance')
    expect(driving).toHaveAttribute('data-driving', 'true')
    expect(driving).toHaveAccessibleName('Distance 20')
    expect(driving?.parentElement).toHaveAttribute('data-positioned')
    expect(measuring).toHaveTextContent('(20)')
    expect(measuring).toHaveAttribute('data-driving', 'false')
  })

  it('names a dimension label with its kind and its visible text (WCAG 2.5.3)', () => {
    const circle = circleEntity('e-2', [30, 0], 5)
    const diameter: SketchConstraint = { id: 'c-4', type: 'diameter', curve: 'e-2', expr: '10' }
    const formula: SketchConstraint = { ...LENGTH, id: 'c-5', expr: 'width / 2', driving: false }
    const harness = sketchHarness({
      entities: [BASE, circle],
      constraints: [diameter, formula],
      solution: solutionFor([BASE, circle], {
        constraints: [
          { id: 'c-4', state: 'satisfied', value: 10 },
          { id: 'c-5', state: 'satisfied', value: 20 },
        ],
      }),
    })
    renderOverlay(harness)
    const [round, measured] = screen.getAllByTestId('sketch-dimension')
    expect(round).toHaveAccessibleName('Diameter ⌀10')
    expect(measured).toHaveAccessibleName('Distance (fx 20)')
  })

  it('a drag frame repaints label values and positions without rendering React again (ADR 0002)', async () => {
    const reference: SketchConstraint = { ...LENGTH, id: 'c-3', driving: false }
    const harness = sketchHarness({
      entities: [BASE],
      constraints: [HORIZONTAL, reference],
      solution: referenceSolution(20),
    })
    harness.app.store.setState((state) =>
      state.sketch === null ? {} : { sketch: { ...state.sketch, tool: null } },
    )
    view = mountView()
    const container = document.createElement('div')
    view.region.append(container)
    let commits = 0
    render(
      <AppProvider app={harness.app}>
        <Profiler
          id="sketch"
          onRender={() => {
            commits += 1
          }}
        >
          <SketchOverlay />
        </Profiler>
      </AppProvider>,
      { container },
    )
    await nextFrame()
    await nextFrame()
    const label = screen.getByTestId('sketch-dimension')
    expect(label).toHaveTextContent('(20)')
    const before = commits
    act(() => {
      harness.app.store.setState((state) =>
        state.document === null
          ? {}
          : {
              document: {
                ...state.document,
                sketches: {
                  ...state.document.sketches,
                  [SKETCH_ID]: solvedSketch(referenceSolution(30)),
                },
              },
            },
      )
    })
    await nextFrame()
    expect(commits).toBe(before)
    expect(label).toHaveTextContent('(30)')
    expect(screen.getByTestId('constraint-glyph').parentElement?.getAttribute('style')).toMatch(
      /^translate: 632px 498px;?$/,
    )
  })

  it('renders a glyph again when its solve state changes', async () => {
    const harness = sketchHarness({ entities: [BASE], constraints: [HORIZONTAL] })
    renderOverlay(harness)
    expect(screen.getByTestId('constraint-glyph')).toHaveAttribute('data-state', 'satisfied')
    act(() => {
      harness.app.store.setState((state) =>
        state.document === null
          ? {}
          : {
              document: {
                ...state.document,
                sketches: {
                  ...state.document.sketches,
                  [SKETCH_ID]: solvedSketch(
                    solutionFor([BASE], { constraints: [{ id: 'c-1', state: 'conflicting' }] }),
                  ),
                },
              },
            },
      )
    })
    await nextFrame()
    expect(screen.getByTestId('constraint-glyph')).toHaveAttribute('data-state', 'conflicting')
  })

  it('rings profile issues with their titles', () => {
    const harness = sketchHarness({
      entities: [BASE],
      solution: solutionFor([BASE], {
        profile_findings: [
          { type: 'open_endpoint', entity: 'e-1', point: [20, 0] },
          { type: 'zero_length', entity: 'e-9' },
        ],
      }),
    })
    renderOverlay(harness)
    const [ring, ...others] = screen.getAllByTestId('profile-issue')
    expect(others).toEqual([])
    expect(ring).toHaveTextContent('Open end')
    expect(ring).toHaveAttribute('cx', '672')
    expect(ring).toHaveAttribute('cy', '512')
    expect(ring).toHaveAttribute('visibility', 'visible')
  })

  it('a click on a glyph selects its constraint; Shift toggles it out again', async () => {
    const harness = sketchHarness({ entities: [BASE], constraints: [HORIZONTAL] })
    renderOverlay(harness)
    const user = userEvent.setup()
    await user.click(screen.getByTestId('constraint-glyph'))
    expect(harness.app.store.getState().sketch?.selection.map((item) => item.key)).toEqual([
      `constraint:${SKETCH_ID}:c-1`,
    ])
    expect(screen.getByTestId('constraint-glyph')).toHaveAttribute('aria-pressed', 'true')
    await user.keyboard('{Shift>}')
    await user.click(screen.getByTestId('constraint-glyph'))
    await user.keyboard('{/Shift}')
    expect(harness.app.store.getState().sketch?.selection).toEqual([])
  })

  it('hovering a glyph highlights the constrained line', async () => {
    const harness = sketchHarness({ entities: [BASE], constraints: [HORIZONTAL] })
    renderOverlay(harness)
    await userEvent.hover(screen.getByTestId('constraint-glyph'))
    await nextFrame()
    const highlight = document.querySelector('path[stroke-linecap="round"]')
    expect(highlight?.getAttribute('d')).toBe('M512 512 L672 512')
  })
})

describe('SketchOverlay drawing (3.16.4, 3.16.5)', () => {
  it('shows the inference glyph near the origin', async () => {
    const harness = sketchHarness()
    const canvas = renderOverlay(harness, 'rectangle_2point')
    act(() => {
      pointerAt(canvas, 'pointermove', [0.1, 0.1])
    })
    await nextFrame()
    const glyph = screen.getByTestId('inference-glyph')
    expect(glyph).toHaveAttribute('data-type', 'coincident')
    expect(glyph.parentElement).toHaveAttribute('data-positioned')
  })

  it('typing a digit while a line rubber-bands types into the focused Length input', async () => {
    const harness = sketchHarness()
    const canvas = renderOverlay(harness, 'line')
    act(() => {
      clickAt(canvas, [0, 0])
      pointerAt(canvas, 'pointermove', [15, 0])
    })
    await nextFrame()
    const inputs = screen.getAllByTestId('draw-input')
    expect(inputs.map((input) => input.getAttribute('aria-label'))).toEqual(['Length', 'Angle'])
    expect(inputs[0]).toHaveAttribute('placeholder', '15')
    act(() => {
      pressKey('2')
      pressKey('5')
    })
    const length = screen.getByRole('textbox', { name: 'Length' })
    expect(length).toHaveValue('25')
    expect(length).toHaveFocus()
  })

  it('shows the live measures in the textMuted placeholder facet and the values font (3.16.4)', async () => {
    const harness = sketchHarness()
    const canvas = renderOverlay(harness, 'line')
    act(() => {
      clickAt(canvas, [0, 0])
      pointerAt(canvas, 'pointermove', [15, 0])
    })
    await nextFrame()
    for (const input of screen.getAllByTestId('draw-input')) {
      for (const name of [
        ...COLOR_CLASSES.textMuted.placeholder.split(' '),
        ...FONT_CLASSES.values.split(' '),
      ]) {
        expect(input).toHaveClass(name)
      }
    }
  })

  it('Enter in a draw input commits the line with the typed length', async () => {
    const harness = sketchHarness()
    const canvas = renderOverlay(harness, 'line')
    act(() => {
      clickAt(canvas, [0, 0])
      pointerAt(canvas, 'pointermove', [15, 0])
    })
    const user = userEvent.setup()
    await user.type(screen.getByRole('textbox', { name: 'Length' }), '25{Enter}')
    await act(settleCommits)
    expect(harness.applied.map((entry) => entry.label)).toEqual(['Line'])
  })

  it('Tab cycles between the draw inputs and Esc clears them', async () => {
    const harness = sketchHarness()
    const canvas = renderOverlay(harness, 'rectangle_2point')
    act(() => {
      clickAt(canvas, [5, 5])
      pointerAt(canvas, 'pointermove', [15, 10])
    })
    const user = userEvent.setup()
    const width = screen.getByRole('textbox', { name: 'Width' })
    await user.type(width, '12')
    await user.keyboard('{Tab}')
    expect(screen.getByRole('textbox', { name: 'Height' })).toHaveFocus()
    await user.keyboard('{Tab}')
    expect(width).toHaveFocus()
    await user.keyboard('{Escape}')
    expect(width).toHaveValue('')
    expect(harness.app.store.getState().sketch?.tool).toBe('rectangle_2point')
  })

  it('shows why a typed value is not used on a visible chip under the inputs', async () => {
    const harness = sketchHarness()
    const canvas = renderOverlay(harness, 'line')
    act(() => {
      clickAt(canvas, [0, 0])
      pointerAt(canvas, 'pointermove', [15, 0])
    })
    const user = userEvent.setup()
    const length = screen.getByRole('textbox', { name: 'Length' })
    await user.type(length, 'widht')
    await act(settleCommits)
    expect(length).toHaveAttribute('aria-invalid', 'true')
    expect(length).toHaveAccessibleDescription("Unknown name 'widht'.")
    expect(screen.getByText("Unknown name 'widht'.")).toBeVisible()
  })

  it('draws the rubber band dashed in construction mode', async () => {
    const harness = sketchHarness()
    harness.app.store.setState((state) =>
      state.sketch === null ? {} : { sketch: { ...state.sketch, construction: true } },
    )
    const canvas = renderOverlay(harness, 'line')
    act(() => {
      clickAt(canvas, [0, 0])
      pointerAt(canvas, 'pointermove', [10, 5])
    })
    await nextFrame()
    const band = screen.getByTestId('rubber-band')
    expect(band).toHaveAttribute('d', 'M512 512 L592 472')
    expect(band).toHaveAttribute('stroke-dasharray', '6 4')
  })
})

describe('SketchOverlay dimension editor (3.16.7)', () => {
  it('opens on the placing click with the measured value, and Enter adds the dimension', async () => {
    const harness = sketchHarness({ entities: [BASE] })
    const canvas = renderOverlay(harness, 'dimension')
    act(() => {
      clickAt(canvas, [10, 0])
      clickAt(canvas, [10, -4])
    })
    const editor = screen.getByTestId('dimension-editor')
    const input = within(editor).getByRole('textbox', { name: 'Dimension value' })
    expect(input).toHaveValue('20')
    expect(input).toHaveFocus()
    expect(within(editor).getByRole('button', { name: 'Reference' })).toHaveAttribute(
      'aria-pressed',
      'false',
    )
    expect(within(editor).queryByRole('radiogroup')).toBeNull()
    const user = userEvent.setup()
    await user.clear(input)
    await user.type(input, '40')
    await act(settleCommits)
    await user.keyboard('{Enter}')
    await act(settleCommits)
    expect(harness.applied.map((entry) => entry.label)).toEqual(['Dimension'])
    expect(screen.queryByTestId('dimension-editor')).toBeNull()
  })

  it('offers Diameter | Radius for a circle', () => {
    const harness = sketchHarness({ entities: [circleEntity('e-2', [0, 0], 5)] })
    const canvas = renderOverlay(harness, 'dimension')
    act(() => {
      clickAt(canvas, [3.5355, 3.5355])
      clickAt(canvas, [9, 9])
    })
    const group = within(screen.getByTestId('dimension-editor')).getByRole('radiogroup', {
      name: 'Dimension kind',
    })
    expect(within(group).getByRole('radio', { name: 'Diameter' })).toBeChecked()
  })

  it('shows the over-constrained offer with Add reference', async () => {
    const harness = sketchHarness({
      entities: [BASE],
      solve: (candidate) => {
        const added = (candidate.constraints ?? []).at(-1)
        return solutionFor(candidate.entities ?? [], {
          conflicting: added === undefined || 'driving' in added ? [] : [added.id],
        })
      },
    })
    const canvas = renderOverlay(harness, 'dimension')
    act(() => {
      clickAt(canvas, [10, 0])
      clickAt(canvas, [10, -4])
    })
    await act(settleCommits)
    const user = userEvent.setup()
    await user.keyboard('{Enter}')
    await act(settleCommits)
    const alert = screen.getByRole('alert')
    expect(alert).toHaveTextContent(
      'Other dimensions and constraints already set this size. Add it as a reference dimension that only shows the measurement?',
    )
    await user.click(within(alert).getByRole('button', { name: 'Add reference' }))
    await act(settleCommits)
    expect(harness.applied).toHaveLength(1)
  })

  it('double-clicking a label edits the stored dimension, and Esc closes the editor', async () => {
    const harness = sketchHarness({ entities: [BASE], constraints: [LENGTH] })
    renderOverlay(harness)
    const user = userEvent.setup()
    await user.dblClick(screen.getByTestId('sketch-dimension'))
    const input = screen.getByRole('textbox', { name: 'Dimension value' })
    expect(input).toHaveValue('20')
    expect(screen.getByRole('group', { name: 'Change dimension' })).toBeInTheDocument()
    await user.keyboard('{Escape}')
    expect(screen.queryByTestId('dimension-editor')).toBeNull()
  })

  it('Enter on a focused label opens the editor', async () => {
    const harness = sketchHarness({ entities: [BASE], constraints: [LENGTH] })
    renderOverlay(harness)
    screen.getByTestId('sketch-dimension').focus()
    await userEvent.keyboard('{Enter}')
    expect(screen.getByTestId('dimension-editor')).toBeInTheDocument()
  })
})

describe('SketchOverlay box selection (3.16.9)', () => {
  it('draws the selection rectangle while dragging, dashed for a crossing selection', async () => {
    const harness = sketchHarness({ entities: [BASE] })
    const canvas = renderOverlay(harness)
    act(() => {
      pointerAt(canvas, 'pointerdown', [10, 10])
      pointerAt(canvas, 'pointermove', [-10, -10], { buttons: 1 })
    })
    await nextFrame()
    const box = screen.getByTestId('box-selection')
    expect(box).toHaveAttribute('data-mode', 'crossing')
    expect(box).toHaveAttribute('stroke-dasharray', '4 3')
    expect(box).toHaveAttribute('x', '432')
    expect(box).toHaveAttribute('width', '160')
    act(() => {
      pointerAt(canvas, 'pointerup', [-10, -10])
    })
    expect(screen.queryByTestId('box-selection')).toBeNull()
    setTool(harness.app, null)
  })
})
