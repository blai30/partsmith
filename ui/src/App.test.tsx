import { act, cleanup, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { installPopoverPolyfill } from './components/popover-polyfill.testing'
import type { App as AppInstance } from './app/createApp'
import type { Edit, SketchEntity } from './generated/protocol'
import { cubeDocumentState, documentState, readyState } from './layout/fixtures.testing'
import {
  createModelApp,
  renderModelApp,
  renderWithApp,
  stubMatchMedia,
} from './layout/test-app.testing'
import { MESSAGES } from './messages'
import { DEFAULT_SETTINGS } from './settings/settings'
import { INITIAL_STATE, type ActiveCommand, type AppState } from './state/store'
import { solutionFor, solvedSketch } from './sketch/fixtures.testing'
import {
  clickAt,
  fixedViewport,
  settleCommits,
  TOP_CAMERA,
  VIEW_PIXELS,
} from './sketch/view.testing'
import { emptyDocument } from './test/fakes/documents'
import { bodyItem } from './test/fakes/selection'
import { settle } from './test/fakes/settle'
import { checkResult, finding, printReport } from './test/fakes/state'
import { COLOR_CLASSES } from './theme/roles'
import { classifyView } from './viewport/camera'

beforeEach(installPopoverPolyfill)

beforeEach(() => {
  stubMatchMedia()
})

afterEach(() => {
  cleanup()
  vi.unstubAllGlobals()
  vi.restoreAllMocks()
})

const EXTRUDE_COMMAND: ActiveCommand = {
  featureType: 'extrude',
  featureId: 'f-new1',
  editing: false,
  title: 'Extrude',
  params: { extent: { kind: 'distance', expr: '10 mm' }, op: 'join', body: 'b-cube' },
  activeField: 'profile',
  fieldErrors: {},
  applied: false,
  newBody: null,
  helper: null,
  changedPaths: [],
  history: [],
  future: [],
  preview: 'none',
}

function renderApp(state: Partial<AppState> = {}) {
  return renderModelApp({ state })
}

describe('the app shell (3.2)', () => {
  it('U-83 has the banner, the "Browser" navigation, the "3D view" main, the "Timeline" region and the contentinfo', () => {
    renderApp()
    expect(screen.getByRole('banner')).toBe(screen.getByTestId('top-bar'))
    expect(screen.getByRole('navigation', { name: 'Browser' })).toBe(screen.getByTestId('browser'))
    expect(screen.getByRole('main', { name: '3D view' })).toBe(screen.getByTestId('viewport'))
    expect(screen.getByRole('region', { name: 'Timeline' })).toBe(screen.getByTestId('timeline'))
    expect(screen.getByRole('contentinfo')).toBe(screen.getByTestId('status-bar'))
  })

  it('U-83 gives every button a non-empty accessible name', () => {
    renderApp()
    const buttons = screen.getAllByRole('button', { hidden: true })
    expect(buttons.length).toBeGreaterThan(0)
    for (const button of buttons) {
      expect(button).toHaveAccessibleName(/\S/)
    }
  })

  it('U-91 renders no element with a style attribute when there is no viewport controller', () => {
    renderApp()
    expect(document.querySelectorAll('[style]')).toHaveLength(0)
  })

  it('U-91 renders no element with a style attribute with a selection and an open side panel', () => {
    renderApp({
      selection: [bodyItem('b-cube', 'Cube')],
      panels: {
        browserOpen: true,
        timelineCollapsed: false,
        paletteOpen: false,
        side: 'parameters',
        inspectorPosition: null,
      },
    })
    // Positioned elements (the inspector) carry only the runtime translate (3.12).
    for (const element of document.querySelectorAll('[style]')) {
      expect(element).toHaveAttribute('data-positioned')
      expect(element.getAttribute('style')).toMatch(/^translate: [-\d.]+px [-\d.]+px;?$/)
    }
  })

  it('styles the root with the fixed layout and the stage and text roles', () => {
    renderApp()
    const root = screen.getByTestId('app')
    for (const name of [
      'h-dvh',
      'w-dvw',
      'overflow-hidden',
      'p-2',
      'antialiased',
      ...COLOR_CLASSES.stage.background.split(' '),
      ...COLOR_CLASSES.text.foreground.split(' '),
    ]) {
      expect(root).toHaveClass(name)
    }
  })

  it('leaves the root in the interface face it inherits from the root element (2.16.7)', () => {
    renderApp()
    expect(screen.getByTestId('app').className).not.toMatch(/(?:^|\s)font-(?:\[|mono|sans)/)
  })

  it('mirrors the context in data-context', () => {
    renderApp({ context: 'print' })
    expect(screen.getByTestId('app')).toHaveAttribute('data-context', 'print')
  })

  it('reports data-core-status ready with a document and loading or failed otherwise', () => {
    renderApp()
    expect(screen.getByTestId('app')).toHaveAttribute('data-core-status', 'ready')
    cleanup()
    renderApp({
      core: {
        status: 'loading',
        engineVersion: null,
        fatal: null,
        pendingMutations: 0,
        stillWorking: false,
      },
    })
    expect(screen.getByTestId('app')).toHaveAttribute('data-core-status', 'loading')
    cleanup()
    renderApp({
      core: {
        status: 'failed',
        engineVersion: null,
        fatal: 'HTTP 404',
        pendingMutations: 0,
        stillWorking: false,
      },
    })
    expect(screen.getByTestId('app')).toHaveAttribute('data-core-status', 'failed')
  })

  it('reports data-core-status loading while the core is ready but the first document is not open', () => {
    const test = createModelApp({ state: readyState(null) })
    renderWithApp(test.app)
    expect(screen.getByTestId('app')).toHaveAttribute('data-core-status', 'loading')
  })

  it('leaves the browser out while it is closed', () => {
    renderApp({
      panels: {
        browserOpen: false,
        timelineCollapsed: false,
        paletteOpen: false,
        side: null,
        inspectorPosition: null,
      },
    })
    expect(screen.queryByTestId('browser')).toBeNull()
  })

  it('shows the side panel only while one is open', () => {
    renderApp()
    expect(screen.queryByTestId('side-panel')).toBeNull()
    cleanup()
    renderApp({
      panels: {
        browserOpen: true,
        timelineCollapsed: false,
        paletteOpen: false,
        side: 'parameters',
        inspectorPosition: null,
      },
    })
    const side = screen.getByTestId('side-panel')
    expect(side.tagName).toBe('ASIDE')
    expect(within(side).getByTestId('parameters-panel')).toBeInTheDocument()
  })

  it('names the print check side panel "Print check" and its region by the visible heading (3.17)', () => {
    renderApp({
      panels: {
        browserOpen: true,
        timelineCollapsed: false,
        paletteOpen: false,
        side: 'health',
        inspectorPosition: null,
      },
    })
    const side = screen.getByTestId('side-panel')
    expect(side).toHaveAccessibleName('Print check')
    const heading = within(side).getByRole('heading')
    expect(heading).toHaveTextContent('Print check: Generic FDM 0.4 mm')
    expect(within(side).getByRole('region')).toHaveAccessibleName(heading.textContent ?? '')
    expect(within(side).getAllByRole('heading')).toHaveLength(1)
  })

  it('shows the overhang legend in the viewport in Print context while the overhang map is on (3.17)', () => {
    const report = printReport({
      'b-cube': [checkResult('overhangs', [finding('overhangs', 'warning')])],
    })
    const print = { ...INITIAL_STATE.print, report, reportRevision: cubeDocumentState().revision }
    const { app } = renderApp({ context: 'print', print })
    const legend = within(screen.getByTestId('viewport')).getByTestId('overhang-legend')
    expect(legend).toHaveTextContent('Overhangs')
    act(() => {
      app.store.setState((state) => ({ print: { ...state.print, overhangMap: false } }))
    })
    expect(screen.queryByTestId('overhang-legend')).toBeNull()
    cleanup()
    renderApp({ context: 'model', print })
    expect(screen.queryByTestId('overhang-legend')).toBeNull()
  })

  it('E-50 keeps the overhang map while the Overhangs check is hidden; the toggle alone hides it (3.17)', () => {
    const report = printReport({
      'b-cube': [checkResult('overhangs', [finding('overhangs', 'warning')])],
    })
    const print = { ...INITIAL_STATE.print, report, reportRevision: cubeDocumentState().revision }
    renderApp({
      context: 'print',
      print,
      settings: { ...DEFAULT_SETTINGS, theme: 'light', hiddenChecks: ['overhangs'] },
    })
    expect(within(screen.getByTestId('viewport')).getByTestId('overhang-legend')).toBeVisible()
  })

  it('"Lay on a face" in the print check panel switches to Print and starts the face pick (3.17)', async () => {
    const report = printReport({
      'b-cube': [checkResult('overhangs', [finding('overhangs', 'warning')])],
    })
    const { app } = renderApp({
      print: { ...INITIAL_STATE.print, report, reportRevision: cubeDocumentState().revision },
      panels: { ...INITIAL_STATE.panels, side: 'health' },
    })
    await userEvent.click(screen.getByRole('button', { name: 'Lay on a face' }))
    await settle()
    expect(app.store.getState().context).toBe('print')
    expect(app.store.getState().print.pickingFace).toBe(true)
    expect(screen.getByTestId('print-hint')).toHaveTextContent(MESSAGES.print_pick_face())
  })

  it('keeps the other regions working when one region crashes (4.4)', () => {
    const report = vi.spyOn(console, 'error').mockImplementation(() => undefined)
    const broken = cubeDocumentState()
    // Parameters that are not a list break the browser's Parameters group.
    const recipe = {
      ...broken.recipe,
      parameters: null as unknown as NonNullable<typeof broken.recipe.parameters>,
    }
    renderApp({ document: { ...broken, recipe } })
    expect(screen.getByText(MESSAGES.region_crashed())).toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'Reload' })).toBeInTheDocument()
    expect(screen.getByTestId('document-name')).toHaveTextContent('cube')
    expect(screen.getAllByTestId('timeline-item')).toHaveLength(2)
    expect(report).toHaveBeenCalled()
  })
})

describe('the Esc ladder (3.5)', () => {
  it('U-69 closes the open menu, then cancels the command, then clears the selection', async () => {
    const { app, core, user } = renderApp({
      command: EXTRUDE_COMMAND,
      selection: [bodyItem('b-cube', 'Cube')],
    })
    await user.click(screen.getByTestId('document-name'))
    expect(screen.getByRole('menu')).toBe(screen.getByTestId('document-menu'))

    await user.keyboard('{Escape}')
    await settle()
    expect(screen.queryByRole('menu')).toBeNull()
    expect(app.store.getState().command).not.toBeNull()
    expect(core.callsTo('cancelTransaction')).toEqual([])

    await user.keyboard('{Escape}')
    await settle()
    expect(core.callsTo('cancelTransaction')).toEqual([[1]])
    expect(app.store.getState().command).toBeNull()
    expect(app.store.getState().selection).toHaveLength(1)

    await user.keyboard('{Escape}')
    await settle()
    expect(app.store.getState().selection).toEqual([])
  })

  it('U-69 exits the active sketch tool, and never leaves sketch mode', async () => {
    const { app, user } = renderApp({
      context: 'sketch',
      sketch: {
        sketchId: 'f-sk1',
        reattaching: null,
        tool: 'line',
        construction: false,
        showConstraints: true,
        selection: [],
        dragging: false,
        cameraBefore: null,
      },
    })
    await user.keyboard('{Escape}')
    await settle()
    expect(app.store.getState().sketch?.tool).toBeNull()
    await user.keyboard('{Escape}')
    await user.keyboard('{Escape}')
    await settle()
    expect(app.store.getState().context).toBe('sketch')
    expect(app.store.getState().sketch?.sketchId).toBe('f-sk1')
  })

  it('U-69 cancels the plane pick of a new sketch, back to Model', async () => {
    const { app, user } = renderApp({
      context: 'sketch',
      sketch: {
        sketchId: null,
        reattaching: null,
        tool: null,
        construction: false,
        showConstraints: true,
        selection: [],
        dragging: false,
        cameraBefore: null,
      },
    })
    await user.keyboard('{Escape}')
    await settle()
    expect(app.store.getState().context).toBe('model')
    expect(app.store.getState().sketch).toBeNull()
  })
})

/** Applies sketch records to the store as the core's `document_changed` and `sketch_solved` would. */
function applySketchEdits(app: AppInstance, edits: readonly Edit[]): void {
  app.store.setState((state) => {
    const document = state.document
    if (document === null) {
      return {}
    }
    let features = [...(document.recipe.features ?? [])]
    const sketches = { ...document.sketches }
    for (const edit of edits) {
      if (edit.type === 'add_feature') {
        features.splice(edit.index, 0, edit.feature)
      } else if (edit.type === 'update_feature') {
        features = features.map((feature) =>
          feature.id === edit.feature.id ? edit.feature : feature,
        )
      } else {
        continue
      }
      const stored: unknown = edit.feature['entities']
      const entities = Array.isArray(stored) ? (stored as SketchEntity[]) : []
      sketches[edit.feature.id] = solvedSketch(solutionFor(entities, { feature: edit.feature.id }))
    }
    return {
      document: {
        ...document,
        revision: document.revision + 1,
        recipe: { ...document.recipe, features },
        sketches,
      },
    }
  })
}

describe('sketching in the shell (3.16)', () => {
  beforeEach(() => {
    // jsdom has no WebGL2, so the viewport draws nothing and the test's fixed view stays attached.
    vi.spyOn(HTMLCanvasElement.prototype, 'getContext').mockReturnValue(null)
    vi.spyOn(HTMLCanvasElement.prototype, 'clientWidth', 'get').mockReturnValue(VIEW_PIXELS)
    vi.spyOn(HTMLCanvasElement.prototype, 'clientHeight', 'get').mockReturnValue(VIEW_PIXELS)
  })

  it('enters a sketch on the Top plane, draws a line and finishes it', async () => {
    const { app, core } = createModelApp({ state: readyState(documentState(emptyDocument())) })
    const view = fixedViewport()
    app.attachViewport(view)
    const labels: (string | null)[] = []
    core.stub('applyEdits', async (_handle, label, edits) => {
      labels.push(label)
      applySketchEdits(app, edits)
      return app.store.getState().document?.revision ?? 0
    })
    core.stub('solveSketch', async (_handle, feature, candidate) =>
      candidate === undefined
        ? (app.store.getState().document?.sketches[feature] ?? null)
        : solvedSketch(solutionFor(candidate.entities ?? [], { feature })),
    )
    renderWithApp(app)
    const user = userEvent.setup()
    // The hint's live region is there, empty, before sketch mode, and is never replaced (3.24).
    const hint = screen.getByTestId('sketch-hint')
    expect(hint).toHaveTextContent('')

    // Sketch (3.16.1): plane picking offers the three origin planes.
    await user.click(within(screen.getByTestId('top-bar')).getByRole('radio', { name: 'Sketch' }))
    expect(screen.getByTestId('sketch-header')).toHaveAttribute('data-state', 'picking')
    expect(screen.getByTestId('sketch-hint')).toBe(hint)
    expect(hint).toHaveTextContent('Top plane (XY) is the print bed.')
    await user.click(screen.getByRole('button', { name: 'Sketch on Top plane (XY)' }))
    await act(settleCommits)
    expect(screen.getByTestId('sketch-hint')).toBe(hint)
    // The plane button is gone; focus goes on to the 3D view, where the drawing happens (3.24).
    expect(within(screen.getByTestId('viewport')).getByRole('application')).toHaveFocus()
    expect(labels).toEqual(['New sketch'])
    const sketchId = app.store.getState().sketch?.sketchId ?? ''
    expect(sketchId).toMatch(/^f-[0-9a-f]{12}$/)
    expect(view.lookAlong).toHaveBeenCalledWith([0, 0, 1], [0, 1, 0], [0, 0, 0])
    expect(screen.getByTestId('sketch-header')).toHaveAttribute('data-state', 'empty')
    expect(screen.getByTestId('sketch-hint')).toHaveTextContent(
      'Choose Line (L), Rectangle (R) or Circle (C) to start drawing.',
    )

    // Line (3.16.4): two clicks on the canvas commit one line.
    const tools = screen.getByRole('toolbar', { name: 'Sketch tools' })
    await user.click(within(tools).getByRole('button', { name: 'Line' }))
    expect(screen.getByTestId('sketch-hint')).toHaveTextContent('Click to start a line.')
    const canvas = screen.getByTestId('viewport').querySelector('canvas')
    if (canvas === null) {
      throw new Error('The viewport has no canvas.')
    }
    act(() => {
      clickAt(canvas, [0, 0])
      clickAt(canvas, [20, 0])
    })
    await act(settleCommits)
    expect(labels).toEqual(['New sketch', 'Line'])
    expect(app.store.getState().document?.sketches[sketchId]?.solution.entities).toHaveLength(1)
    expect(screen.getByTestId('sketch-hint')).toHaveTextContent(/^Click to end the segment/)

    // Esc ends the chain, then the tool; with nothing left, the hint names the way out (3.5).
    await user.keyboard('{Escape}')
    expect(app.store.getState().sketch?.tool).toBe('line')
    await user.keyboard('{Escape}')
    expect(app.store.getState().sketch?.tool).toBeNull()
    await user.keyboard('{Escape}')
    expect(screen.getByTestId('sketch-hint')).toHaveTextContent(
      'To leave the sketch, click Finish sketch or press Ctrl+Enter.',
    )
    expect(app.store.getState().context).toBe('sketch')

    // Finish sketch (3.16.11): Model, the sketch selected, the view turned off the sketch plane.
    const setCamera = vi.mocked(view.setCamera)
    setCamera.mockClear()
    await user.click(within(tools).getByRole('button', { name: 'Finish sketch' }))
    await settle()
    expect(app.store.getState().context).toBe('model')
    expect(app.store.getState().sketch).toBeNull()
    expect(screen.queryByTestId('sketch-header')).toBeNull()
    expect(screen.queryByTestId('sketch-overlay')).toBeNull()
    expect(app.store.getState().selection.map((item) => item.key)).toEqual([`feature:${sketchId}`])
    expect(setCamera).toHaveBeenCalledTimes(1)
    const [camera, options] = setCamera.mock.calls[0] ?? []
    expect(classifyView(camera ?? TOP_CAMERA)).toBe('iso')
    expect(options).toEqual({ animate: true })
  })
})
