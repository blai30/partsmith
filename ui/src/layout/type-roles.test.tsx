import { act, render, screen, waitFor, within } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { installPopoverPolyfill } from '../components/popover-polyfill.testing'
import type { Document, FeatureStatus } from '../generated/protocol'
import type { InspectorField } from '../inspector/model'
import { UnsupportedField } from '../inspector/fields/ValueFields'
import { clickAt, fixedViewport, nextFrame, pointerAt, VIEW_PIXELS } from '../sketch/view.testing'
import { INITIAL_STATE, type AppState, type DocumentState } from '../state/store'
import { CUBE_DOCUMENT, CUBE_TOP_FRONT_EDGE } from '../test/fakes/documents'
import { bodyItem, elementItem } from '../test/fakes/selection'
import { settle } from '../test/fakes/settle'
import { checkResult, finding, printReport } from '../test/fakes/state'
import {
  elementsWithClass,
  hasEditableCodeLigatures,
  hasTabularFigures,
  isCode,
  NO_CONTEXTUAL_LIGATURES,
} from '../test/type-roles'
import {
  cubeDocumentState,
  documentState,
  readyState,
  rectSketchDocumentState,
} from './fixtures.testing'
import { createModelApp, renderModelApp, renderWithApp, stubMatchMedia } from './test-app.testing'

// The type roles of ui.md 2.16.7 across the app: code surfaces (U-206), tabular and proportional
// figures (U-207) and no stray code face (U-208). Expression fields are U-205, in the
// ExpressionInput, Inspector and size field tests.

beforeEach(installPopoverPolyfill)

beforeEach(() => {
  stubMatchMedia()
})

afterEach(() => {
  vi.unstubAllGlobals()
  vi.restoreAllMocks()
})

const LENGTH = { length: 1, angle: 0 } as const

/** parametric.ptsm with `size` = 20 mm (comment "Wall") and `gap` = size / 4. */
const SIZE_AND_GAP_DOCUMENT: Document = {
  ...CUBE_DOCUMENT,
  parameters: [
    { id: 'p-size', name: 'size', expr: '20 mm', comment: 'Wall' },
    { id: 'p-gap', name: 'gap', expr: 'size / 4' },
  ],
}

function sizeAndGap(): DocumentState {
  return documentState(SIZE_AND_GAP_DOCUMENT, {
    name: 'parametric',
    parameterValues: {
      'p-size': { id: 'p-size', value: 20, dimension: LENGTH },
      'p-gap': { id: 'p-gap', value: 5, dimension: LENGTH },
    },
  })
}

function browserRow(kind: string, id: string): HTMLElement {
  const found = screen
    .getAllByTestId('browser-row')
    .find(
      (element) =>
        element.getAttribute('data-kind') === kind && element.getAttribute('data-id') === id,
    )
  if (found === undefined) {
    throw new Error(`No browser row ${kind} ${id}`)
  }
  return found
}

/** The innermost element under `root`, `root` included, whose text is exactly `text`. */
function elementWithText(root: HTMLElement, text: string): HTMLElement {
  const found = [root, ...root.querySelectorAll<HTMLElement>('*')].findLast(
    (element) => element.textContent === text,
  )
  if (found === undefined) {
    throw new Error(`No element with the text ${text}`)
  }
  return found
}

function paletteOption(kind: string, title: RegExp): HTMLElement {
  const found = screen
    .getAllByTestId('palette-option')
    .find(
      (option) => option.getAttribute('data-kind') === kind && title.test(option.textContent ?? ''),
    )
  if (found === undefined) {
    throw new Error(`No palette option ${kind} ${String(title)}`)
  }
  return found
}

/** The elements among `elements` whose digits are not tabular, as markup so a failure names them. */
function notTabular(elements: readonly Element[]): string[] {
  return elements
    .filter((element) => !hasTabularFigures(element))
    .map((element) => element.outerHTML)
}

/** The elements among `elements` whose digits are tabular, as markup. */
function notProportional(elements: readonly Element[]): string[] {
  return elements.filter(hasTabularFigures).map((element) => element.outerHTML)
}

/** Every notification, field error and cause line on the page: sentences, so proportional (U-207). */
function messages(): HTMLElement[] {
  return ['notification', 'field-error', 'cause-line'].flatMap((testId) =>
    screen.queryAllByTestId(testId),
  )
}

/** Every element of the page that carries the code face or the ligature class of editable code (U-208). */
function codeFaceElements(): Element[] {
  return [
    ...elementsWithClass(document.body, 'font-mono'),
    ...elementsWithClass(document.body, NO_CONTEXTUAL_LIGATURES),
  ]
}

function renderParametric() {
  return renderModelApp({
    document: sizeAndGap(),
    state: { panels: { ...INITIAL_STATE.panels, browserOpen: true, side: 'parameters' } },
  })
}

describe('code surfaces (2.16.7, U-206)', () => {
  it('U-206 sets the Parameters panel name and expression as editable code', () => {
    renderParametric()
    for (const name of ['Name of size', 'Expression of size']) {
      const input = screen.getByLabelText(name)
      expect(isCode(input)).toBe(true)
      expect(hasEditableCodeLigatures(input)).toBe(true)
    }
  })

  it('U-206 keeps the comment field and the value out of the code face', () => {
    renderParametric()
    const comment = screen.getByRole('textbox', { name: 'Comment of size' })
    expect(isCode(comment)).toBe(false)
    expect(hasEditableCodeLigatures(comment)).toBe(false)
    const row = screen
      .getAllByTestId('parameter-row')
      .find((element) => element.getAttribute('data-id') === 'p-size')
    const value = within(row ?? document.body).getByTestId('parameter-value')
    expect(value).toHaveTextContent('20 mm')
    expect(hasTabularFigures(value)).toBe(true)
    expect(isCode(value)).toBe(false)
  })

  it('U-206 sets each browser parameter row as read-only code, and its value in tabular figures', () => {
    renderParametric()
    const size = elementWithText(browserRow('parameter', 'p-size'), 'size = 20 mm')
    expect(isCode(size)).toBe(true)
    expect(hasEditableCodeLigatures(size)).toBe(false)
    const gapRow = browserRow('parameter', 'p-gap')
    const gap = elementWithText(gapRow, 'gap = size / 4')
    expect(isCode(gap)).toBe(true)
    expect(hasEditableCodeLigatures(gap)).toBe(false)
    const value = elementWithText(gapRow, '5 mm')
    expect(hasTabularFigures(value)).toBe(true)
    expect(isCode(value)).toBe(false)
  })

  it('U-206 sets the palette parameter title as code, its detail and kind tag in the interface face', async () => {
    const { user } = renderModelApp({
      document: sizeAndGap(),
      state: { panels: { ...INITIAL_STATE.panels, paletteOpen: true } },
    })
    await user.type(screen.getByTestId('palette-input'), 'size')
    const option = paletteOption('parameter', /^size = 20 mm/)
    const title = elementWithText(option, 'size = 20 mm')
    expect(isCode(title)).toBe(true)
    expect(isCode(elementWithText(option, 'Wall'))).toBe(false)
    expect(isCode(elementWithText(option, 'Parameter'))).toBe(false)
  })

  it('U-206 gives a command option no code element', async () => {
    const { user } = renderModelApp({
      document: sizeAndGap(),
      state: { panels: { ...INITIAL_STATE.panels, paletteOpen: true } },
    })
    await user.type(screen.getByTestId('palette-input'), 'Extrude')
    const option = paletteOption('command', /^Extrude/)
    expect(elementsWithClass(option, 'font-mono')).toEqual([])
  })

  it('U-206 shows the JSON of an unsupported control in a code element with font-mono', () => {
    const field: InspectorField = {
      path: 'entities',
      label: 'Entities',
      help: null,
      essential: true,
      required: false,
      control: { type: 'unsupported' },
      value: [{ id: 'e-1', type: 'point', at: [0, 0] }],
      error: null,
      attachedTo: null,
      axis: null,
    }
    const { container } = render(<UnsupportedField field={field} />)
    const code = container.querySelector('code')
    expect(code).toHaveTextContent('[{"id":"e-1","type":"point","at":[0,0]}]')
    expect(code).toHaveClass('font-mono')
  })
})

const FAILED_EXTRUDE: FeatureStatus = {
  id: 'f-ext1',
  status: 'error',
  messages: [
    {
      severity: 'error',
      code: 'body_not_empty',
      message: "Body 'Cube' already has a solid. Use op join to add to it.",
    },
  ],
  repairs: [],
}

const TWO_WARNINGS = printReport({
  'b-cube': [
    checkResult('overhangs', [finding('overhangs', 'warning'), finding('overhangs', 'warning')]),
  ],
})

/** The cube with a fake status that puts Extrude 1 in error. */
function failedCube(): DocumentState {
  const cube = cubeDocumentState()
  return { ...cube, featureStatus: { ...cube.featureStatus, 'f-ext1': FAILED_EXTRUDE } }
}

/** Three edges of the cube, as the selection holds them. */
function threeEdges() {
  return ['side-0', 'side-1', 'side-2'].map((side) => {
    const name = `edge(f-ext1/end-cap|f-ext1/${side})`
    return elementItem({ ...CUBE_TOP_FRONT_EDGE, name, reference: { q: name, fp: 'line' } })
  })
}

describe('tabular and proportional figures (2.16.7, U-207)', () => {
  it('U-207 sets the status bar and timeline counts and timings in tabular figures', () => {
    renderModelApp({
      document: failedCube(),
      state: {
        panels: { ...INITIAL_STATE.panels, timelineCollapsed: true },
        view: { ...INITIAL_STATE.view, gridSpacing: 10 },
        print: { ...INITIAL_STATE.print, report: TWO_WARNINGS, reportRevision: 1 },
        notifications: [
          {
            id: 'n-1',
            severity: 'error',
            message: 'Extrude 1 failed: 2 faces are missing.',
            actions: [],
            createdAt: 0,
          },
        ],
      },
    })
    const counted = ['status-recompute', 'status-grid', 'health-badge', 'timeline-problems'].map(
      (testId) => screen.getByTestId(testId),
    )
    counted.push(within(screen.getByTestId('timeline')).getByText('2 features'))
    expect(notTabular(counted)).toEqual([])
    expect(screen.getAllByTestId('notification')).toHaveLength(1)
    expect(notProportional(messages())).toEqual([])
  })

  it('U-207 sets the readouts and the volume in tabular figures, and the title proportional', async () => {
    const { app, core } = createModelApp({
      state: readyState(cubeDocumentState(), { selection: [bodyItem('b-cube', 'Cube')] }),
    })
    core.stub('measure', () => Promise.resolve({ volume: 8000 }))
    renderWithApp(app)
    const numbers = ['body-size', 'body-center'].flatMap((testId) =>
      [...screen.getByTestId(testId).children].slice(0, 3).map((cell) => cell.lastElementChild),
    )
    expect(numbers).toHaveLength(6)
    expect(notTabular(numbers.filter((number) => number !== null))).toEqual([])
    const volume = await waitFor(() => {
      const row = screen
        .getAllByTestId('measure-row')
        .find((candidate) => candidate.getAttribute('data-measure') === 'volume')
      expect(row).toHaveTextContent('8000 mm³')
      return row
    })
    expect(hasTabularFigures(elementWithText(volume ?? document.body, '8000 mm³'))).toBe(true)
    expect(hasTabularFigures(screen.getByTestId('selection-title'))).toBe(false)
    expect(notProportional(messages())).toEqual([])
  })

  it('U-207 sets the title of three edges proportional', () => {
    renderModelApp({ state: { selection: threeEdges() } })
    const title = screen.getByTestId('selection-title')
    expect(title).toHaveTextContent('3 edges')
    expect(hasTabularFigures(title)).toBe(false)
  })

  it('U-207 sets sketch dimensions and the DOF in tabular figures, the header proportional', () => {
    renderModelApp({
      document: rectSketchDocumentState(),
      state: {
        context: 'sketch',
        sketch: {
          sketchId: 'f-sk1',
          tool: null,
          construction: false,
          showConstraints: true,
          selection: [],
          dragging: false,
          cameraBefore: null,
        },
      },
    })
    const dimensions = screen.getAllByTestId('sketch-dimension')
    expect(dimensions).toHaveLength(2)
    expect(notTabular([...dimensions, screen.getByTestId('status-dof')])).toEqual([])
    expect(hasTabularFigures(screen.getByTestId('sketch-header'))).toBe(false)
    expect(notProportional(messages())).toEqual([])
  })

  it('U-207 sets the draw inputs of a started line in tabular figures', async () => {
    // jsdom has no WebGL2, so the viewport draws nothing and the test's fixed view stays attached.
    vi.spyOn(HTMLCanvasElement.prototype, 'getContext').mockReturnValue(null)
    vi.spyOn(HTMLCanvasElement.prototype, 'clientWidth', 'get').mockReturnValue(VIEW_PIXELS)
    vi.spyOn(HTMLCanvasElement.prototype, 'clientHeight', 'get').mockReturnValue(VIEW_PIXELS)
    const { app } = createModelApp({
      state: readyState(rectSketchDocumentState(), {
        context: 'sketch',
        sketch: {
          sketchId: 'f-sk1',
          tool: 'line',
          construction: false,
          showConstraints: true,
          selection: [],
          dragging: false,
          cameraBefore: null,
        },
      }),
    })
    app.attachViewport(fixedViewport())
    renderWithApp(app)
    await settle()
    const canvas = screen.getByTestId('viewport').querySelector('canvas')
    if (canvas === null) {
      throw new Error('The viewport has no canvas.')
    }
    act(() => {
      clickAt(canvas, [-20, -20])
      pointerAt(canvas, 'pointermove', [-5, -20])
    })
    await nextFrame()
    const inputs = screen.getAllByTestId('draw-input')
    expect(inputs.length).toBeGreaterThan(0)
    expect(notTabular(inputs)).toEqual([])
    expect(notProportional(messages())).toEqual([])
  })

  it('U-207 sets the Print check count and the overhang legend labels in tabular figures', () => {
    renderModelApp({
      state: {
        context: 'print',
        print: { ...INITIAL_STATE.print, report: TWO_WARNINGS, reportRevision: 1 },
      },
    })
    const button = within(screen.getByTestId('top-bar')).getByRole('button', {
      name: /^Print check/,
    })
    const legend = screen.getByTestId('overhang-legend')
    const labels = [elementWithText(legend, '45°'), elementWithText(legend, '90°')]
    expect(notTabular([elementWithText(button, '2'), ...labels])).toEqual([])
    const caption = elementWithText(legend, 'Faces steeper than 45° from vertical need support.')
    expect(hasTabularFigures(caption)).toBe(false)
    expect(notProportional(messages())).toEqual([])
  })
})

describe('no stray code face (2.16.7, U-208)', () => {
  it('U-208 shows no code face on the cube with a selection, the timeline, Problems and the palette', async () => {
    renderModelApp({
      state: {
        selection: [bodyItem('b-cube', 'Cube')],
        panels: {
          ...INITIAL_STATE.panels,
          timelineCollapsed: false,
          side: 'problems',
          paletteOpen: true,
        },
      } satisfies Partial<AppState>,
    })
    await settle()
    expect(screen.getByTestId('palette')).toBeInTheDocument()
    expect(screen.getByTestId('timeline')).toBeInTheDocument()
    expect(codeFaceElements()).toEqual([])
  })

  it('U-208 shows no code face in sketch mode with the line tool', async () => {
    renderModelApp({
      document: rectSketchDocumentState(),
      state: {
        context: 'sketch',
        sketch: {
          sketchId: 'f-sk1',
          tool: 'line',
          construction: false,
          showConstraints: true,
          selection: [],
          dragging: false,
          cameraBefore: null,
        },
      },
    })
    await settle()
    expect(screen.getAllByTestId('sketch-dimension')).toHaveLength(2)
    expect(codeFaceElements()).toEqual([])
  })
})
