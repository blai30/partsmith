import { afterEach, describe, expect, it, vi } from 'vitest'
import { setSketchTool } from '../app/actions/sketch'
import type { CoreApi } from '../core/api'
import type { Edit, SketchConstraint, SketchSolution } from '../generated/protocol'
import {
  DOCUMENT_HANDLE,
  SKETCH_ID,
  circleEntity,
  curveItem,
  lineEntity,
  sketchSession,
  solutionFor,
  solvedEntity,
  solvedSketch,
} from './fixtures.testing'
import {
  createSketchController,
  sketchStepHint,
  type OverlayPaint,
  type SketchController,
  type SketchToolId,
} from './session'
import {
  clickAt,
  dragFrom,
  mountView,
  pointerAt,
  pressKey,
  settleCommits,
  sketchHarness,
  type SketchHarness,
  type TestView,
} from './view.testing'

const LINUX = { operatingSystem: 'linux', host: 'web' } as const
const MAC = { operatingSystem: 'mac', host: 'web' } as const

describe('sketchStepHint (2.14)', () => {
  it('U-102 prompts on Linux', () => {
    expect(sketchStepHint({ kind: 'idle', empty: true }, LINUX)).toBe(
      'Choose Line (L), Rectangle (R) or Circle (C) to start drawing.',
    )
    expect(sketchStepHint({ kind: 'idle', empty: false }, LINUX)).toBeNull()
    expect(sketchStepHint({ kind: 'line', chain: false }, LINUX)).toBe('Click to start a line.')
    expect(sketchStepHint({ kind: 'line', chain: true }, LINUX)).toBe(
      'Click to end the segment, or type a length. Click the first point to close the shape, or press Enter to stop.',
    )
    expect(sketchStepHint({ kind: 'circle', clicks: 1 }, LINUX)).toBe(
      'Click to set the size, or type the diameter.',
    )
    expect(sketchStepHint({ kind: 'dimension', picked: 'point' }, LINUX)).toBe(
      'Click a second point or a line.',
    )
    expect(sketchStepHint({ kind: 'dimension', picked: 'round' }, LINUX)).toBe(
      'Click to place the dimension.',
    )
  })

  it('U-102 renders shortcuts for macOS', () => {
    expect(sketchStepHint({ kind: 'line', chain: true }, MAC)).toMatch(/press ↩ to stop\.$/)
  })

  it('maps every step of the table of 3.16.3', () => {
    expect(sketchStepHint({ kind: 'rectangle_2point', clicks: 0 }, LINUX)).toBe(
      'Click the first corner.',
    )
    expect(sketchStepHint({ kind: 'rectangle_2point', clicks: 1 }, LINUX)).toBe(
      'Click the opposite corner, or type the width.',
    )
    expect(sketchStepHint({ kind: 'rectangle_center', clicks: 0 }, LINUX)).toBe('Click the center.')
    expect(sketchStepHint({ kind: 'rectangle_center', clicks: 1 }, LINUX)).toBe(
      'Click a corner, or type the width.',
    )
    expect(sketchStepHint({ kind: 'circle', clicks: 0 }, LINUX)).toBe('Click the center.')
    expect(sketchStepHint({ kind: 'arc_3point', clicks: 0 }, LINUX)).toBe(
      'Click where the arc starts.',
    )
    expect(sketchStepHint({ kind: 'arc_3point', clicks: 1 }, LINUX)).toBe('Click where it ends.')
    expect(sketchStepHint({ kind: 'arc_3point', clicks: 2 }, LINUX)).toBe(
      'Move to shape the arc, then click, or type the radius.',
    )
    expect(sketchStepHint({ kind: 'point' }, LINUX)).toBe('Click to place a point.')
    expect(sketchStepHint({ kind: 'dimension', picked: 'nothing' }, LINUX)).toBe(
      'Click a line, circle, arc or point.',
    )
    expect(sketchStepHint({ kind: 'dimension', picked: 'line' }, LINUX)).toBe(
      'Click to place the dimension, or click a second point or line to measure between them.',
    )
    expect(sketchStepHint({ kind: 'dimension', picked: 'pair' }, LINUX)).toBe(
      'Click to place the dimension.',
    )
  })
})

// The sketch controller (3.16.3 to 3.16.9), driven through a canvas in the test view.

let view: TestView | null = null
let controller: SketchController | null = null

afterEach(() => {
  controller?.dispose()
  controller = null
  view?.remove()
  view = null
  vi.restoreAllMocks()
})

function start(harness: SketchHarness, tool: SketchToolId | null = null): SketchController {
  harness.app.store.setState((state) =>
    state.sketch === null ? {} : { sketch: { ...state.sketch, tool } },
  )
  view = mountView()
  controller = createSketchController({ app: harness.app, canvas: view.canvas })
  return controller
}

function canvas(): HTMLCanvasElement {
  if (view === null) {
    throw new Error('No view is mounted.')
  }
  return view.canvas
}

type UpdateFeature = Extract<Edit, { type: 'update_feature' }>

/** The sketch record of the `index`th applied edit. */
function appliedSketch(harness: SketchHarness, index = 0): UpdateFeature['feature'] {
  const edit = harness.applied[index]?.edits[0]
  if (edit?.type !== 'update_feature') {
    throw new Error(`Edit ${String(index)} is not a sketch update.`)
  }
  return edit.feature
}

function constraintsOf(feature: UpdateFeature['feature']): SketchConstraint[] {
  const constraints: unknown = feature['constraints']
  return Array.isArray(constraints) ? (constraints as SketchConstraint[]) : []
}

function withoutIds(constraints: readonly SketchConstraint[]): unknown[] {
  return constraints.map(({ id: _id, ...rest }) => rest)
}

describe('line tool (3.16.4)', () => {
  it('two clicks commit a line from the snapped start, coincident with the origin', async () => {
    const harness = sketchHarness()
    start(harness, 'line')
    clickAt(canvas(), [0.2, 0.1])
    clickAt(canvas(), [20, 2])
    await settleCommits()

    expect(harness.applied.map((entry) => entry.label)).toEqual(['Line'])
    const feature = appliedSketch(harness)
    const [line] = feature['entities'] as { id: string }[]
    expect(feature['entities']).toEqual([
      { id: line?.id, type: 'line', start: [0, 0], end: [20, 2] },
    ])
    expect(withoutIds(constraintsOf(feature))).toEqual([
      { type: 'coincident', points: [`${line?.id ?? ''}.start`, 'origin'] },
    ])
  })

  it('a segment within 3 degrees of horizontal ends on the horizontal with an inferred horizontal', async () => {
    const harness = sketchHarness()
    start(harness, 'line')
    clickAt(canvas(), [0, 0])
    clickAt(canvas(), [20, 1])
    await settleCommits()

    const feature = appliedSketch(harness)
    const [line] = feature['entities'] as { id: string; end: number[] }[]
    expect(line?.end).toEqual([20, 0])
    expect(constraintsOf(feature).map((constraint) => constraint.type)).toEqual([
      'coincident',
      'horizontal',
    ])
    // Inferred constraints are dropped when the probe finds them redundant, never refused.
    expect(harness.probes).toHaveLength(1)
  })

  it('continues the chain from the end with a coincident to the previous line', async () => {
    const harness = sketchHarness()
    start(harness, 'line')
    clickAt(canvas(), [0, 0])
    clickAt(canvas(), [20, 2])
    await settleCommits()
    clickAt(canvas(), [20, 15])
    await settleCommits()

    const [first] = appliedSketch(harness, 0)['entities'] as { id: string }[]
    const second = appliedSketch(harness, 1)
    const [, next] = second['entities'] as { id: string; start: number[] }[]
    expect(next?.start).toEqual([20, 2])
    expect(withoutIds(constraintsOf(second)).slice(1)).toContainEqual({
      type: 'coincident',
      points: [`${first?.id ?? ''}.end`, `${next?.id ?? ''}.start`],
    })
  })

  it('ignores a segment shorter than 2 pixels', async () => {
    const harness = sketchHarness()
    start(harness, 'line')
    clickAt(canvas(), [5, 5])
    clickAt(canvas(), [5.1, 5.1])
    await settleCommits()
    expect(harness.applied).toEqual([])
  })

  it('Esc discards the rubber band and keeps the tool; the next Esc is left to the ladder', () => {
    const harness = sketchHarness()
    const sketch = start(harness, 'line')
    clickAt(canvas(), [0, 0])
    pointerAt(canvas(), 'pointermove', [10, 3])
    expect(sketch.getModel().drawInputs.map((input) => input.name)).toEqual(['Length', 'Angle'])

    expect(pressKey('Escape').defaultPrevented).toBe(true)
    expect(sketch.getModel().drawInputs).toEqual([])
    expect(harness.app.store.getState().sketch?.tool).toBe('line')
    expect(pressKey('Escape').defaultPrevented).toBe(false)
  })

  it('Enter with no draw input focused ends the chain', async () => {
    const harness = sketchHarness()
    const sketch = start(harness, 'line')
    clickAt(canvas(), [0, 0])
    clickAt(canvas(), [20, 2])
    await settleCommits()
    expect(sketch.getModel().hint?.text).toMatch(/^Click to end the segment/)
    expect(pressKey('Enter').defaultPrevented).toBe(true)
    expect(sketch.getModel().hint?.text).toBe('Click to start a line.')
  })

  it('leaves Esc and Enter to an open popover: the chain goes on (3.5)', async () => {
    const harness = sketchHarness()
    const sketch = start(harness, 'line')
    clickAt(canvas(), [0, 0])
    clickAt(canvas(), [20, 2])
    await settleCommits()
    const popover = document.createElement('div')
    popover.setAttribute('popover', 'auto')
    document.body.append(popover)
    // jsdom never matches :popover-open, so the document reports the popover open as a browser does.
    const querySelector = document.querySelector.bind(document)
    vi.spyOn(document, 'querySelector').mockImplementation((selectors: string) =>
      selectors.includes(':popover-open') ? popover : querySelector(selectors),
    )
    expect(pressKey('Escape').defaultPrevented).toBe(false)
    expect(pressKey('Enter').defaultPrevented).toBe(false)
    expect(sketch.getModel().hint?.text).toMatch(/^Click to end the segment/)
    popover.remove()
  })

  it('leaves Esc to the open palette', () => {
    const harness = sketchHarness()
    const sketch = start(harness, 'line')
    clickAt(canvas(), [0, 0])
    pointerAt(canvas(), 'pointermove', [10, 3])
    harness.app.store.setState((state) => ({ panels: { ...state.panels, paletteOpen: true } }))
    expect(pressKey('Escape').defaultPrevented).toBe(false)
    expect(sketch.getModel().drawInputs.map((input) => input.name)).toEqual(['Length', 'Angle'])
  })

  it('Enter on a focused button activates the button and keeps the chain', async () => {
    const harness = sketchHarness()
    const sketch = start(harness, 'line')
    clickAt(canvas(), [0, 0])
    clickAt(canvas(), [20, 2])
    await settleCommits()
    const button = document.createElement('button')
    document.body.append(button)
    button.focus()
    expect(pressKey('Enter', {}, button).defaultPrevented).toBe(false)
    expect(sketch.getModel().hint?.text).toMatch(/^Click to end the segment/)
    expect(pressKey('Enter', {}, view?.focusTarget).defaultPrevented).toBe(true)
    expect(sketch.getModel().hint?.text).toBe('Click to start a line.')
    button.remove()
  })

  it('a double-click ends the chain after committing the segment', async () => {
    const harness = sketchHarness()
    const sketch = start(harness, 'line')
    clickAt(canvas(), [0, 0])
    clickAt(canvas(), [20, 2])
    canvas().dispatchEvent(new MouseEvent('dblclick', { bubbles: true }))
    await settleCommits()
    expect(harness.applied).toHaveLength(1)
    expect(sketch.getModel().drawInputs).toEqual([])
  })

  it('Mod+Z cancels the operation in progress only', () => {
    const harness = sketchHarness()
    const sketch = start(harness, 'line')
    clickAt(canvas(), [0, 0])
    expect(pressKey('z', { ctrlKey: true }).defaultPrevented).toBe(true)
    expect(sketch.getModel().drawInputs).toEqual([])
    expect(pressKey('z', { ctrlKey: true }).defaultPrevented).toBe(false)
  })

  it('activating another tool cancels the drawing', () => {
    const harness = sketchHarness()
    const sketch = start(harness, 'line')
    clickAt(canvas(), [0, 0])
    setSketchTool(harness.app, 'circle')
    expect(sketch.getModel().hint?.text).toBe('Click the center.')
    expect(sketch.getModel().drawInputs).toEqual([])
  })
})

describe('inference and draw inputs (3.16.4, 3.16.5)', () => {
  it('shows a coincident glyph near the origin and none while Ctrl is held', () => {
    const harness = sketchHarness()
    const sketch = start(harness, 'rectangle_2point')
    pointerAt(canvas(), 'pointermove', [0.3, 0.2])
    expect(sketch.getModel().inference).toEqual(['coincident'])
    expect(sketch.getModel().freePlacement).toBe(true)
    pointerAt(canvas(), 'pointermove', [0.3, 0.2], { ctrlKey: true })
    expect(sketch.getModel().inference).toEqual([])
  })

  it('reads the held Cmd key on macOS', () => {
    const harness = sketchHarness()
    Object.assign(harness.app, { platform: MAC })
    const sketch = start(harness, 'point')
    pointerAt(canvas(), 'pointermove', [0.3, 0.2], { ctrlKey: true })
    expect(sketch.getModel().inference).toEqual(['coincident'])
    pointerAt(canvas(), 'pointermove', [0.3, 0.2], { metaKey: true })
    expect(sketch.getModel().inference).toEqual([])
  })

  it('typing a digit while rubber-banding puts it in the first draw input and asks for focus', () => {
    const harness = sketchHarness()
    const sketch = start(harness, 'line')
    clickAt(canvas(), [0, 0])
    pointerAt(canvas(), 'pointermove', [15, 0])
    const before = sketch.getModel().focusRequest
    expect(pressKey('2').defaultPrevented).toBe(true)
    pressKey('5')
    const [length] = sketch.getModel().drawInputs
    expect(length).toMatchObject({ name: 'Length', text: '25' })
    expect(sketch.getModel().focusRequest).toBe(before + 2)
  })

  it('letters stay shortcuts while rubber-banding', () => {
    const harness = sketchHarness()
    const sketch = start(harness, 'line')
    clickAt(canvas(), [0, 0])
    expect(pressKey('e').defaultPrevented).toBe(false)
    expect(sketch.getModel().drawInputs[0]?.text).toBe('')
  })

  it('Enter commits the shape with the typed length as a driving distance', async () => {
    const harness = sketchHarness()
    const sketch = start(harness, 'line')
    clickAt(canvas(), [0, 0])
    pointerAt(canvas(), 'pointermove', [15, 0])
    sketch.setDrawText('Length', '25')
    await sketch.commitDrawInputs()
    await settleCommits()

    const feature = appliedSketch(harness)
    const [line] = feature['entities'] as { id: string; end: number[] }[]
    expect(line?.end).toEqual([25, 0])
    expect(withoutIds(constraintsOf(feature))).toEqual([
      { type: 'coincident', points: [`${line?.id ?? ''}.start`, 'origin'] },
      { type: 'horizontal', line: line?.id },
      { type: 'distance', line: line?.id, expr: '25' },
    ])
  })

  it('a text that is not a value shows its error and commits nothing', async () => {
    const harness = sketchHarness()
    const sketch = start(harness, 'circle')
    clickAt(canvas(), [0, 0])
    pointerAt(canvas(), 'pointermove', [5, 0])
    sketch.setDrawText('Diameter', 'wall')
    await sketch.commitDrawInputs()
    await settleCommits()
    expect(sketch.getModel().drawInputs[0]?.error).toBe("Unknown name 'wall'.")
    expect(harness.applied).toEqual([])
  })

  it('a typed diameter adds a driving diameter to the circle', async () => {
    const harness = sketchHarness()
    const sketch = start(harness, 'circle')
    clickAt(canvas(), [10, 10])
    pointerAt(canvas(), 'pointermove', [15, 10])
    sketch.setDrawText('Diameter', '8')
    await sketch.commitDrawInputs()
    await settleCommits()
    const feature = appliedSketch(harness)
    const [circle] = feature['entities'] as { id: string; radius: number }[]
    expect(circle?.radius).toBe(4)
    expect(withoutIds(constraintsOf(feature))).toEqual([
      { type: 'diameter', curve: circle?.id, expr: '8' },
    ])
  })

  it('three points in a line show the arc_collinear warning', () => {
    const harness = sketchHarness()
    const sketch = start(harness, 'arc_3point')
    clickAt(canvas(), [5, 5])
    clickAt(canvas(), [25, 5])
    pointerAt(canvas(), 'pointermove', [15, 5])
    expect(sketch.getModel().hint).toEqual({
      text: 'These three points are in a line. Move the last point off the line to make an arc.',
      warning: true,
    })
  })
})

describe('constraint tools in pick mode (3.16.6)', () => {
  const SLANTED = lineEntity('e-1', [0, 0], [10, 2])
  const CIRCLE = circleEntity('e-2', [30, 30], 5)

  it('applies the constraint once the picks form a valid set', async () => {
    const harness = sketchHarness({ entities: [SLANTED, CIRCLE] })
    const sketch = start(harness)
    setSketchTool(harness.app, 'constrain_horizontal')
    expect(sketch.getModel().hint).toEqual({ text: 'Select a line or two points.', warning: false })
    clickAt(canvas(), [5, 1])
    await settleCommits()
    expect(harness.applied.map((entry) => entry.label)).toEqual(['Horizontal'])
    expect(withoutIds(constraintsOf(appliedSketch(harness)))).toEqual([
      { type: 'horizontal', line: 'e-1' },
    ])
    expect(harness.app.store.getState().sketch?.selection).toEqual([])
    expect(harness.app.store.getState().sketch?.tool).toBe('constrain_horizontal')
  })

  it('E-103 H on a line whose horizontal is already implied says so and adds nothing (3.16.6)', async () => {
    const TOP = lineEntity('e-3', [40, 20], [0, 20])
    const IMPLIED: SketchConstraint = { id: 'c-6', type: 'horizontal', line: 'e-3' }
    const harness = sketchHarness({
      entities: [TOP],
      constraints: [IMPLIED],
      // The probe finds the new horizontal implied by the stored one.
      solve: (candidate) => {
        const added = (candidate.constraints ?? []).filter((constraint) => constraint.id !== 'c-6')
        return solutionFor(candidate.entities ?? [], {
          redundant: added.map((constraint) => constraint.id),
        })
      },
      state: { sketch: sketchSession({ selection: [curveItem('e-3', 'line')] }) },
    })
    start(harness)
    // H runs sketch.constrain_horizontal, which sets the tool; the selection fits, so it applies at once.
    setSketchTool(harness.app, 'constrain_horizontal')
    await settleCommits()
    expect(harness.applied).toEqual([])
    expect(harness.app.store.getState().notifications).toEqual([
      expect.objectContaining({
        severity: 'info',
        message: 'Horizontal is already implied by other constraints, so it was not added.',
      }),
    ])
  })

  it('keeps a partial pick list in the sketch selection', () => {
    const harness = sketchHarness({ entities: [SLANTED, CIRCLE] })
    start(harness, 'constrain_parallel')
    clickAt(canvas(), [5, 1])
    expect(harness.app.store.getState().sketch?.selection.map((item) => item.key)).toEqual([
      `sketch:${SKETCH_ID}:e-1`,
    ])
  })

  it('starts pick mode from a selection more picks can complete, else from nothing', () => {
    const harness = sketchHarness({ entities: [SLANTED, CIRCLE] })
    start(harness)
    harness.app.store.setState((state) =>
      state.sketch === null
        ? {}
        : { sketch: { ...state.sketch, selection: [curveItem('e-1', 'line')] } },
    )
    setSketchTool(harness.app, 'constrain_parallel')
    expect(harness.app.store.getState().sketch?.selection).toHaveLength(1)
    setSketchTool(harness.app, null)
    harness.app.store.setState((state) =>
      state.sketch === null
        ? {}
        : { sketch: { ...state.sketch, selection: [curveItem('e-2', 'circle')] } },
    )
    setSketchTool(harness.app, 'constrain_parallel')
    expect(harness.app.store.getState().sketch).toMatchObject({
      tool: 'constrain_parallel',
      selection: [],
    })
  })

  it('flashes the requirement for a refused pick', () => {
    const harness = sketchHarness({ entities: [SLANTED, CIRCLE] })
    const sketch = start(harness, 'constrain_parallel')
    clickAt(canvas(), [35, 30])
    expect(sketch.getModel().hint).toEqual({ text: 'Select two lines.', warning: true })
    expect(harness.app.store.getState().sketch?.selection).toEqual([])
  })
})

describe('dimension tool (3.16.7)', () => {
  const BASE = lineEntity('e-1', [0, 0], [20, 0])

  it('a line, then empty space, opens the editor with the measured length', () => {
    const harness = sketchHarness({ entities: [BASE] })
    const sketch = start(harness, 'dimension')
    expect(sketch.getModel().hint?.text).toBe('Click a line, circle, arc or point.')
    clickAt(canvas(), [10, 0])
    expect(sketch.getModel().hint?.text).toBe(
      'Click to place the dimension, or click a second point or line to measure between them.',
    )
    pointerAt(canvas(), 'pointermove', [10, -4])
    expect(sketch.getModel().placing).toBe('20')
    clickAt(canvas(), [10, -4])
    expect(sketch.getModel().editor).toMatchObject({
      mode: 'new',
      text: '20',
      unit: 'length',
      round: null,
      reference: false,
      problem: null,
    })
  })

  it('Enter adds a driving distance labeled "Dimension" and keeps the tool', async () => {
    const harness = sketchHarness({ entities: [BASE] })
    const sketch = start(harness, 'dimension')
    clickAt(canvas(), [10, 0])
    clickAt(canvas(), [10, -4])
    sketch.setEditorText('40')
    await sketch.commitEditor()
    await settleCommits()

    expect(harness.applied.map((entry) => entry.label)).toEqual(['Dimension'])
    expect(withoutIds(constraintsOf(appliedSketch(harness)))).toEqual([
      { type: 'distance', line: 'e-1', expr: '40' },
    ])
    expect(sketch.getModel().editor).toBeNull()
    expect(harness.app.store.getState().sketch?.tool).toBe('dimension')
  })

  it('an over-constraining dimension offers a reference dimension', async () => {
    const harness = sketchHarness({
      entities: [BASE],
      solve: (candidate) => {
        const added = (candidate.constraints ?? []).at(-1)
        return solutionFor(candidate.entities ?? [], {
          conflicting: added === undefined || 'driving' in added ? [] : [added.id],
        })
      },
    })
    const sketch = start(harness, 'dimension')
    clickAt(canvas(), [10, 0])
    clickAt(canvas(), [10, -4])
    await sketch.commitEditor()
    expect(sketch.getModel().editor?.problem).toBe('over_constrained')
    expect(harness.applied).toEqual([])

    await sketch.commitEditor({ reference: true })
    await settleCommits()
    expect(withoutIds(constraintsOf(appliedSketch(harness)))).toEqual([
      { type: 'distance', line: 'e-1', expr: '20', driving: false },
    ])
  })

  it('a circle opens the editor with Diameter | Radius, following the choice', () => {
    const harness = sketchHarness({ entities: [circleEntity('e-2', [0, 0], 5)] })
    const sketch = start(harness, 'dimension')
    clickAt(canvas(), [5, 0])
    expect(sketch.getModel().hint?.text).toBe('Click to place the dimension.')
    clickAt(canvas(), [9, 9])
    expect(sketch.getModel().editor).toMatchObject({ text: '10', round: 'diameter' })
    sketch.setEditorRound('radius')
    expect(sketch.getModel().editor).toMatchObject({ text: '5', round: 'radius' })
  })

  it('a point and a circle are refused with cannot_dimension', () => {
    const harness = sketchHarness({
      entities: [lineEntity('e-1', [-20, -20], [-10, -20]), circleEntity('e-2', [10, 10], 5)],
    })
    const sketch = start(harness, 'dimension')
    clickAt(canvas(), [-20, -20])
    clickAt(canvas(), [15, 10])
    expect(sketch.getModel().hint?.warning).toBe(true)
    expect(sketch.getModel().hint?.text).toMatch(/^Partsmith cannot dimension a point and a circle/)
  })

  it('Esc closes the editor without adding anything', async () => {
    const harness = sketchHarness({ entities: [BASE] })
    const sketch = start(harness, 'dimension')
    clickAt(canvas(), [10, 0])
    clickAt(canvas(), [10, -4])
    expect(pressKey('Escape').defaultPrevented).toBe(true)
    expect(sketch.getModel().editor).toBeNull()
    await settleCommits()
    expect(harness.applied).toEqual([])
  })

  it('editing a stored dimension commits "Change dimension", or shows dimension_unsolvable', async () => {
    const stored: SketchConstraint = { id: 'c-1', type: 'distance', line: 'e-1', expr: '20' }
    const failing = { value: false }
    const harness = sketchHarness({
      entities: [BASE],
      constraints: [stored],
      solve: (candidate) =>
        solutionFor(candidate.entities ?? [], {
          failed: failing.value,
          conflicting: failing.value ? ['c-1'] : [],
        }),
    })
    const sketch = start(harness)
    sketch.editDimension('c-1')
    expect(sketch.getModel().editor).toMatchObject({ mode: 'edit', text: '20', reference: false })

    failing.value = true
    sketch.setEditorText('-5')
    await sketch.commitEditor()
    expect(sketch.getModel().editor).toMatchObject({ problem: 'unsolvable', text: '-5' })

    failing.value = false
    sketch.setEditorText('30')
    await sketch.commitEditor()
    await settleCommits()
    expect(harness.applied.map((entry) => entry.label)).toEqual(['Change dimension'])
    expect(constraintsOf(appliedSketch(harness))).toEqual([{ ...stored, expr: '30' }])
  })
})

describe('drag and box selection (3.16.9)', () => {
  it('dragging a point solves each move through dragSketch and ends with one endSketchDrag', async () => {
    const dragSketch = vi.fn<CoreApi['dragSketch']>(
      (_handle: number, _feature: string, _point: string, to: readonly number[]) =>
        Promise.resolve(
          solvedSketch(solutionFor([lineEntity('e-1', [0, 0], [to[0] ?? 0, to[1] ?? 0])])),
        ),
    )
    const endSketchDrag = vi.fn<CoreApi['endSketchDrag']>(() => Promise.resolve(7))
    const harness = sketchHarness({
      entities: [lineEntity('e-1', [0, 0], [20, 10])],
      core: { dragSketch, endSketchDrag },
    })
    start(harness)
    dragFrom(canvas(), [20, 10], [22, 13])
    await settleCommits()

    expect(dragSketch).toHaveBeenCalledWith(DOCUMENT_HANDLE, SKETCH_ID, 'e-1.end', [22, 13])
    expect(endSketchDrag).toHaveBeenCalledExactlyOnceWith(DOCUMENT_HANDLE, SKETCH_ID, true)
    expect(harness.app.store.getState().sketch?.dragging).toBe(false)
  })

  it('a point of a fully constrained entity shows not-allowed and does not drag', async () => {
    const line = lineEntity('e-1', [0, 0], [20, 10])
    const dragSketch = vi.fn<CoreApi['dragSketch']>(() => Promise.resolve(null))
    const harness = sketchHarness({
      entities: [line],
      solution: solutionFor([line], { entities: [solvedEntity(line, 'fully_constrained')] }),
      core: { dragSketch },
    })
    start(harness)
    pointerAt(canvas(), 'pointermove', [20, 10])
    expect(canvas().classList.contains('cursor-not-allowed!')).toBe(true)
    dragFrom(canvas(), [20, 10], [25, 15])
    await settleCommits()
    expect(dragSketch).not.toHaveBeenCalled()
  })

  it('a drawing tool shows the crosshair cursor', () => {
    start(sketchHarness(), 'line')
    expect(canvas().classList.contains('cursor-crosshair!')).toBe(true)
  })

  const LEFT = lineEntity('e-1', [-12, -12], [-6, -6])
  const RIGHT = lineEntity('e-2', [5, 5], [30, 30])

  it('dragging right is a window selection of entities entirely inside', () => {
    const harness = sketchHarness({ entities: [LEFT, RIGHT] })
    start(harness)
    dragFrom(canvas(), [-15, -15], [10, 10])
    expect(harness.app.store.getState().sketch?.selection.map((item) => item.key)).toEqual([
      `sketch:${SKETCH_ID}:e-1`,
    ])
  })

  it('dragging left is a crossing selection of entities with any part inside', () => {
    const harness = sketchHarness({ entities: [LEFT, RIGHT] })
    const sketch = start(harness)
    pointerAt(canvas(), 'pointerdown', [12, 8])
    pointerAt(canvas(), 'pointermove', [-8, -8], { buttons: 1 })
    expect(sketch.getModel().box).toBe('crossing')
    pointerAt(canvas(), 'pointerup', [-8, -8])
    expect(harness.app.store.getState().sketch?.selection.map((item) => item.key)).toEqual([
      `sketch:${SKETCH_ID}:e-1`,
      `sketch:${SKETCH_ID}:e-2`,
    ])
    expect(sketch.getModel().box).toBeNull()
  })

  it('Esc during a box selection cancels it', () => {
    const harness = sketchHarness({ entities: [LEFT, RIGHT] })
    const sketch = start(harness)
    pointerAt(canvas(), 'pointerdown', [-15, -15])
    pointerAt(canvas(), 'pointermove', [10, 10], { buttons: 1 })
    expect(sketch.getModel().box).toBe('window')
    expect(pressKey('Escape').defaultPrevented).toBe(true)
    expect(sketch.getModel().box).toBeNull()
  })
})

function lastPaint(sketch: SketchController): () => OverlayPaint {
  let paint: OverlayPaint | null = null
  sketch.setPainter((next) => {
    paint = next
  })
  return () => {
    sketch.repaint()
    if (paint === null) {
      throw new Error('Nothing was painted.')
    }
    return paint
  }
}

describe('painting (3.12, 3.16.2)', () => {
  it('places a glyph 14 pixels above the middle of its line', () => {
    const line = lineEntity('e-1', [0, 0], [20, 0])
    const horizontal: SketchConstraint = { id: 'c-1', type: 'horizontal', line: 'e-1' }
    const harness = sketchHarness({ entities: [line], constraints: [horizontal] })
    const paint = lastPaint(start(harness))
    const [x = 0, y = 0] = paint().glyphs.get('c-1') ?? []
    expect(x).toBeCloseTo(592, 9)
    expect(y).toBeCloseTo(498, 9)
  })

  it('draws the rubber band from the start to the cursor', () => {
    const harness = sketchHarness()
    const sketch = start(harness, 'line')
    const paint = lastPaint(sketch)
    clickAt(canvas(), [-10, -10])
    pointerAt(canvas(), 'pointermove', [10, -5])
    expect(paint().rubberBand).toBe('M432 592 L592 552')
    expect(paint().drawInputs).not.toBeNull()
    expect(paint().measures['Length']).toBe('20.616')
  })

  it('places a dimension label and a profile issue ring', () => {
    const line = lineEntity('e-1', [0, 0], [20, 0])
    const distance: SketchConstraint = { id: 'c-1', type: 'distance', line: 'e-1', expr: '20' }
    const solution: SketchSolution = solutionFor([line], {
      profile_findings: [{ type: 'open_endpoint', entity: 'e-1', point: [20, 0] }],
    })
    const harness = sketchHarness({ entities: [line], constraints: [distance], solution })
    const paint = lastPaint(start(harness))
    expect(paint().labels.get('c-1')).not.toBeNull()
    expect(paint().dimensionLines).not.toBe('')
    expect(paint().issues).toEqual([[672, 512]])
  })
})
