import { act, cleanup, fireEvent, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { cancelCommand, editFeature } from '../app/actions/command'
import type { App } from '../app/createApp'
import { installPopoverPolyfill } from '../components/popover-polyfill.testing'
import { cubeDocumentState, readyState } from '../layout/fixtures.testing'
import {
  createModelApp,
  renderWithApp,
  stubAnimationFrames,
  stubElementSize,
  stubMatchMedia,
  type FrameQueue,
} from '../layout/test-app.testing'
import { requestUi } from '../layout/requests'
import { MESSAGES } from '../messages'
import { DEFAULT_SETTINGS } from '../settings/settings'
import { INITIAL_STATE, type AppState } from '../state/store'
import type { FakeBackend } from '../test/fakes/backend'
import { bodyItem } from '../test/fakes/selection'
import { settle } from '../test/fakes/settle'
import { CUBE_FEATURES_DOCUMENT } from '../test/fakes/documents'
import { lostEdgeRepair } from '../test/fakes/repairs'
import { checkResult, finding, printReport } from '../test/fakes/state'
import { COLOR_CLASSES } from '../theme/roles'
import { DEFAULT_CAMERA, forward, visibleHeightAtTarget, type Camera } from './camera'
import { Viewport } from './Viewport'

// The WebGL2 backend cannot run in jsdom; the viewport gets a FakeBackend instead.
const created = vi.hoisted(() => ({ backends: [] as FakeBackend[] }))

vi.mock('./webgl2/backend', async () => {
  const fakes = await import('../test/fakes/backend')
  return {
    createWebGl2Backend: () => {
      const backend = fakes.createFakeBackend()
      created.backends.push(backend)
      return backend
    },
  }
})

let restoreSize: () => void
let frames: FrameQueue

beforeEach(installPopoverPolyfill)

beforeEach(() => {
  restoreSize = stubElementSize(HTMLCanvasElement.prototype, { width: 800, height: 600 })
  stubMatchMedia({ reducedMotion: true })
  frames = stubAnimationFrames()
  created.backends.length = 0
})

afterEach(() => {
  cleanup()
  restoreSize()
  vi.unstubAllGlobals()
  vi.restoreAllMocks()
})

function setUp(state: Partial<AppState> = {}): {
  app: App
  viewport: HTMLElement
  surface: HTMLElement
} {
  const { app } = createModelApp({ state: readyState(cubeDocumentState(), state) })
  renderWithApp(app, <Viewport />)
  act(() => {
    frames.flush()
  })
  const viewport = screen.getByTestId('viewport')
  return { app, viewport, surface: within(viewport).getByRole('application') }
}

function handleOf(app: App) {
  const handle = app.viewport()
  if (handle === null) {
    throw new Error('The viewport is not attached.')
  }
  return handle
}

function flush(): void {
  act(() => {
    frames.flush()
  })
}

/** The angle between two cameras' viewing directions projected onto the XY plane, in degrees. */
function turnAboutZ(before: Camera, after: Camera): number {
  const [beforeX, beforeY] = forward(before)
  const [afterX, afterY] = forward(after)
  const angle = Math.atan2(beforeX * afterY - beforeY * afterX, beforeX * afterX + beforeY * afterY)
  return Math.abs((angle * 180) / Math.PI)
}

describe('the viewport region', () => {
  it('is a main landmark named "3D view"', () => {
    const { viewport } = setUp()
    expect(viewport.tagName).toBe('MAIN')
    expect(viewport).toHaveAccessibleName('3D view')
  })

  it('is one focusable application with the role description "3D view" (3.13)', () => {
    const { surface } = setUp()
    expect(surface).toHaveAttribute('tabindex', '0')
    expect(surface).toHaveAttribute('aria-roledescription', '3D view')
  })

  it('attaches its handle to the app while mounted', () => {
    const { app } = setUp()
    expect(app.viewport()).not.toBeNull()
    cleanup()
    expect(app.viewport()).toBeNull()
  })

  it('mirrors the initial camera, preset, section, hover and selection', () => {
    const { viewport } = setUp()
    expect(viewport).toHaveAttribute('data-view', 'iso')
    expect(viewport).toHaveAttribute('data-projection', 'perspective')
    expect(viewport).toHaveAttribute('data-camera-target', '0,0,0')
    expect(viewport).toHaveAttribute('data-nav-preset', 'fusion')
    expect(viewport).toHaveAttribute('data-section', 'off')
    expect(viewport).toHaveAttribute('data-hover-kind', 'none')
    expect(viewport).toHaveAttribute('data-selection-count', '0')
  })

  it('mirrors the standard view and projection after camera changes', () => {
    const { app, viewport } = setUp()
    act(() => {
      handleOf(app).setStandardView('front')
    })
    flush()
    expect(viewport).toHaveAttribute('data-view', 'front')
    act(() => {
      handleOf(app).toggleProjection()
    })
    flush()
    expect(viewport).toHaveAttribute('data-projection', 'orthographic')
  })

  it('writes the camera target rounded to 2 decimals, trimmed, with -0 as 0', () => {
    const { app, viewport } = setUp()
    act(() => {
      handleOf(app).setCamera(
        { ...DEFAULT_CAMERA, target: [1.234, -0.001, 10.5] },
        { animate: false },
      )
    })
    flush()
    expect(viewport).toHaveAttribute('data-camera-target', '1.23,0,10.5')
  })

  it('mirrors the navigation preset, the section, the hover and the selection count', () => {
    const { app, viewport } = setUp()
    act(() => {
      const state = app.store.getState()
      app.store.setState({
        settings: { ...state.settings, navigation: 'freecad' },
        view: { ...state.view, section: { plane: 'XY', offset: '5', flipped: false } },
        hover: { key: 'mesh:1:face:3', kind: 'face' },
        selection: [bodyItem('b-cube', 'Cube'), bodyItem('b-peg', 'Peg')],
      })
    })
    flush()
    expect(viewport).toHaveAttribute('data-nav-preset', 'freecad')
    expect(viewport).toHaveAttribute('data-section', 'on')
    expect(viewport).toHaveAttribute('data-hover-kind', 'face')
    expect(viewport).toHaveAttribute('data-hover-key', 'mesh:1:face:3')
    expect(viewport).toHaveAttribute('data-selection-count', '2')
  })
})

describe('the grid spacing (3.12)', () => {
  it('mirrors the minor grid spacing in the document unit, larger after zooming out', () => {
    const { app, viewport } = setUp()
    const before = Number(viewport.getAttribute('data-grid-spacing'))
    expect(before).toBeGreaterThan(0)
    act(() => {
      const handle = handleOf(app)
      const camera = handle.getCamera()
      handle.setCamera({ ...camera, distance: camera.distance * 20 })
    })
    flush()
    expect(Number(viewport.getAttribute('data-grid-spacing'))).toBeGreaterThan(before)
  })

  it('is empty in Print, where no grid is drawn', () => {
    const { app, viewport } = setUp()
    act(() => {
      app.store.setState({ context: 'print' })
    })
    flush()
    expect(viewport).toHaveAttribute('data-grid-spacing', '')
  })
})

describe('keyboard navigation of the focused viewport (3.5)', () => {
  it('orbits 15 degrees about world Z with ArrowLeft and back with ArrowRight', () => {
    const { app, surface } = setUp()
    act(() => {
      handleOf(app).setStandardView('front')
    })
    flush()
    surface.focus()
    const before = handleOf(app).getCamera()
    const handled = fireEvent.keyDown(surface, { key: 'ArrowLeft' })
    flush()
    expect(handled).toBe(false)
    expect(turnAboutZ(before, handleOf(app).getCamera())).toBeCloseTo(15, 6)
    fireEvent.keyDown(surface, { key: 'ArrowRight' })
    flush()
    expect(turnAboutZ(before, handleOf(app).getCamera())).toBeCloseTo(0, 6)
  })

  it('pans by 10 percent of the viewport height with Shift and an arrow', () => {
    const { app, surface } = setUp()
    surface.focus()
    const before = handleOf(app).getCamera()
    fireEvent.keyDown(surface, { key: 'ArrowUp', shiftKey: true })
    flush()
    const after = handleOf(app).getCamera()
    const moved = Math.hypot(...after.target.map((value, i) => value - (before.target[i] ?? 0)))
    expect(moved).toBeCloseTo(0.1 * visibleHeightAtTarget(before), 6)
  })

  it('zooms in and out by 1.25 at the center with = and -', () => {
    const { app, surface } = setUp()
    surface.focus()
    fireEvent.keyDown(surface, { key: '=' })
    flush()
    expect(handleOf(app).getCamera().distance).toBeCloseTo(DEFAULT_CAMERA.distance / 1.25, 6)
    fireEvent.keyDown(surface, { key: '-' })
    flush()
    expect(handleOf(app).getCamera().distance).toBeCloseTo(DEFAULT_CAMERA.distance, 6)
  })
})

describe('the view cube and viewport controls (3.14)', () => {
  it('has the view cube and the viewport controls', () => {
    const { viewport } = setUp()
    expect(within(viewport).getByTestId('view-cube')).toHaveAccessibleName('View cube')
    expect(within(viewport).getByTestId('viewport-controls')).toBeInTheDocument()
  })

  it('turns the cube rotor with the camera through a transform only', () => {
    const { app, viewport } = setUp()
    act(() => {
      handleOf(app).setStandardView('top')
    })
    flush()
    const rotor = within(viewport).getByTestId('view-cube').querySelector('[data-positioned]')
    expect(rotor).not.toBeNull()
    const style = rotor?.getAttribute('style') ?? ''
    expect(style).toMatch(/^transform: matrix3d\([^)]*\);?$/)
  })

  it('sets a standard view when a cube face is clicked', async () => {
    const { viewport } = setUp()
    await userEvent.click(within(viewport).getByRole('button', { name: 'Front view' }))
    flush()
    expect(viewport).toHaveAttribute('data-view', 'front')
  })

  it('returns to the iso view with the Home view button', async () => {
    const { app, viewport } = setUp()
    act(() => {
      handleOf(app).setStandardView('top')
    })
    flush()
    await userEvent.click(within(viewport).getByRole('button', { name: 'Home view' }))
    flush()
    expect(viewport).toHaveAttribute('data-view', 'iso')
  })

  it('switches to orthographic with the Orthographic control', async () => {
    const { viewport } = setUp()
    const control = within(viewport).getByRole('button', { name: 'Orthographic' })
    expect(control).toHaveAttribute('aria-pressed', 'false')
    await userEvent.click(control)
    flush()
    expect(viewport).toHaveAttribute('data-projection', 'orthographic')
    expect(control).toHaveAttribute('aria-pressed', 'true')
  })

  it('turns the section on with the Section view control and opens its popover', async () => {
    const { app, viewport } = setUp()
    await userEvent.click(within(viewport).getByRole('button', { name: 'Section view' }))
    flush()
    expect(viewport).toHaveAttribute('data-section', 'on')
    expect(app.store.getState().view.section?.plane).toBe('XY')
    expect(screen.getByTestId('section-popover')).toBeVisible()
  })

  it('toggles a selection filter kind from the Selection filter menu', async () => {
    const { app, viewport } = setUp()
    await userEvent.click(within(viewport).getByRole('button', { name: 'Selection filter' }))
    await userEvent.click(screen.getByRole('menuitemcheckbox', { name: 'Bodies' }))
    expect(app.store.getState().view.filter.body).toBe(true)
  })

  it('hides the selection filter in sketch mode', () => {
    const { viewport } = setUp({
      context: 'sketch',
      sketch: {
        sketchId: 'f-sk1',
        reattaching: null,
        tool: null,
        construction: false,
        showConstraints: true,
        selection: [],
        dragging: false,
        cameraBefore: null,
      },
    })
    expect(within(viewport).queryByRole('button', { name: 'Selection filter' })).toBeNull()
  })
})

describe('overlays', () => {
  it('shows "Loading the geometry core" while the core loads', () => {
    setUp({
      core: {
        status: 'loading',
        engineVersion: null,
        fatal: null,
        pendingMutations: 0,
        stillWorking: false,
      },
    })
    expect(screen.getByTestId('core-overlay')).toHaveTextContent('Loading the geometry core')
  })

  it('shows no loading overlay once the core is ready', () => {
    setUp()
    expect(screen.queryByTestId('core-overlay')).toBeNull()
  })

  it('shows the core load failure with Reload (4.4)', () => {
    setUp({
      core: {
        status: 'failed',
        engineVersion: null,
        fatal: 'HTTP 404',
        pendingMutations: 0,
        stillWorking: false,
      },
    })
    const panel = screen.getByTestId('core-fatal')
    expect(panel).toHaveAttribute('role', 'alert')
    expect(panel).toHaveTextContent(MESSAGES.core_load_failed('HTTP 404'))
    expect(within(panel).getByRole('button', { name: 'Reload' })).toBeInTheDocument()
  })

  it('shows a core that stopped later with Restart core, which restarts it', async () => {
    const { app } = setUp({
      core: {
        status: 'failed',
        engineVersion: '0.1.0',
        fatal: 'out of memory',
        pendingMutations: 0,
        stillWorking: false,
      },
    })
    const restart = vi.spyOn(app, 'restartCore')
    const panel = screen.getByTestId('core-fatal')
    expect(panel).toHaveTextContent(MESSAGES.core_stopped('out of memory'))
    await userEvent.click(within(panel).getByRole('button', { name: 'Restart core' }))
    expect(restart).toHaveBeenCalledTimes(1)
  })

  it('shows "Restoring the 3D view" while the WebGL context is lost', async () => {
    setUp()
    const backend = created.backends[0]
    act(() => {
      backend?.loseContext()
    })
    const restoring = screen.getByText('Restoring the 3D view')
    expect(restoring.closest('[role="status"]')).not.toBeNull()
    act(() => {
      backend?.restoreContext()
    })
    flush()
    await settle()
    expect(screen.queryByText('Restoring the 3D view')).toBeNull()
  })

  it('draws the finding marker, a ring 16 pixels across in errorMark, at the focused finding (3.17)', () => {
    const report = printReport({
      'b-cube': [checkResult('overhangs', [finding('overhangs', 'warning')])],
    })
    const { app } = setUp({
      context: 'print',
      print: {
        ...INITIAL_STATE.print,
        report,
        reportRevision: cubeDocumentState().revision,
        focusedFinding: { body: 'b-cube', check: 'overhangs', index: 0 },
      },
    })
    flush()
    const marker = screen.getByTestId('finding-marker')
    expect(marker.tagName.toLowerCase()).toBe('circle')
    expect(marker).toHaveAttribute('r', '8')
    expect(marker).toHaveClass(COLOR_CLASSES.errorMark.stroke)
    // The finding is at the origin, which the default camera sees: the ring is placed and shown.
    expect(marker).not.toHaveAttribute('display')
    expect(Number(marker.getAttribute('cx'))).toBeGreaterThan(0)
    expect(Number(marker.getAttribute('cy'))).toBeGreaterThan(0)
    act(() => {
      app.store.setState((state) => ({ print: { ...state.print, focusedFinding: null } }))
    })
    expect(screen.queryByTestId('finding-marker')).toBeNull()
    act(() => {
      app.store.setState((state) => ({
        print: { ...state.print, focusedFinding: { body: 'b-cube', check: 'overhangs', index: 0 } },
      }))
    })
    flush()
    expect(screen.getByTestId('finding-marker')).not.toHaveAttribute('display')
  })

  it('labels the origin planes while a plane is picked, at the positive corner of each square (3.16.1)', () => {
    setUp({
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
    flush()
    const labels = screen.getAllByTestId('plane-label')
    expect(labels.map((label) => label.textContent)).toEqual([
      'Top (XY)',
      'Front (XZ)',
      'Right (YZ)',
    ])
    // The cube spans z 0 to 20, so the planes are 50 mm squares about the projection of (0, 0, 10).
    expect(labels.map((label) => label.dataset['anchor'])).toEqual([
      '25,25,0',
      '25,0,35',
      '0,25,35',
    ])
    for (const label of labels) {
      expect(label).toHaveClass('pointer-events-none', 'text-xs')
      expect(label).toHaveClass(COLOR_CLASSES.surface.background)
      expect(label).toHaveAttribute('data-positioned')
      expect(label).not.toHaveAttribute('hidden')
    }
  })

  it('U-183 labels the shown origin axes with their letters in their axis roles on a surface chip (3.12)', () => {
    setUp({
      view: {
        ...INITIAL_STATE.view,
        sessionVisible: { 'origin:X': true, 'origin:Y': true, 'origin:Z': true },
      },
    })
    flush()
    const labels = screen.getAllByTestId('axis-label')
    expect(labels.map((label) => label.textContent)).toEqual(['X', 'Y', 'Z'])
    const foregrounds = [
      COLOR_CLASSES.axisX.foreground,
      COLOR_CLASSES.axisY.foreground,
      COLOR_CLASSES.axisZ.foreground,
    ]
    for (const [index, label] of labels.entries()) {
      expect(label).toHaveAttribute('data-axis', label.textContent)
      expect(label).toHaveClass(
        'font-semibold',
        foregrounds[index] ?? '',
        COLOR_CLASSES.surface.background,
      )
      expect(label).toHaveAttribute('data-positioned')
    }
  })

  it('labels only the shown origin axes, and none while a sketch is edited (3.16.2)', () => {
    setUp({ view: { ...INITIAL_STATE.view, sessionVisible: { 'origin:Z': true } } })
    flush()
    expect(screen.getAllByTestId('axis-label').map((label) => label.dataset['axis'])).toEqual(['Z'])
    cleanup()
    setUp({
      context: 'sketch',
      sketch: {
        sketchId: 'f-sk1',
        reattaching: null,
        tool: null,
        construction: false,
        showConstraints: true,
        selection: [],
        dragging: false,
        cameraBefore: null,
      },
      view: { ...INITIAL_STATE.view, sessionVisible: { 'origin:Z': true } },
    })
    flush()
    expect(screen.queryAllByTestId('axis-label')).toEqual([])
  })

  it('labels only the origin planes that are drawn', () => {
    setUp({ view: { ...INITIAL_STATE.view, sessionVisible: { 'origin:XZ': true } } })
    flush()
    expect(screen.getAllByTestId('plane-label').map((label) => label.textContent)).toEqual([
      'Front (XZ)',
    ])
  })

  it('shows no plane labels while no origin plane is drawn', () => {
    setUp()
    expect(screen.queryAllByTestId('plane-label')).toEqual([])
  })

  it('renders with the settings’ UI scale', () => {
    setUp({ settings: { ...DEFAULT_SETTINGS, theme: 'light', uiScale: 1.5 } })
    expect(created.backends[0]?.lastFrame()?.uiScale).toBe(1.5)
  })
})

describe('repair markers (3.7, U-110)', () => {
  it('U-110 editing a feature with a lost edge rings where it was, until the command ends', async () => {
    const document = cubeDocumentState({
      recipe: CUBE_FEATURES_DOCUMENT,
      featureStatus: {
        'f-fil1': {
          id: 'f-fil1',
          status: 'error',
          messages: [],
          repairs: [lostEdgeRepair()],
        },
      },
    })
    const { app } = createModelApp({ state: readyState(document) })
    renderWithApp(app, <Viewport />)
    flush()
    expect(screen.queryByTestId('repair-marker')).toBeNull()
    await act(async () => {
      await editFeature(app, 'f-fil1')
    })
    flush()
    const [marker, ...others] = screen.getAllByTestId('repair-marker')
    expect(others).toEqual([])
    expect(marker?.tagName.toLowerCase()).toBe('circle')
    expect(marker).toHaveAttribute('r', '8')
    expect(marker).toHaveAttribute('stroke-dasharray', '4 3')
    expect(marker).toHaveClass(COLOR_CLASSES.errorMark.stroke)
    expect(marker?.querySelector('title')).toHaveTextContent(
      "The front top edge of 'Extrude 1' (missing)",
    )
    expect(marker).not.toHaveAttribute('display')
    await act(async () => {
      await cancelCommand(app)
    })
    expect(screen.queryByTestId('repair-marker')).toBeNull()
  })
})

describe('the viewport context menu (3.13)', () => {
  it('opens at the request with the items for the selection, and a choice runs and returns focus', async () => {
    const { app, surface } = setUp({ selection: [bodyItem('b-cube', 'Cube')] })
    act(() => {
      requestUi(app, { kind: 'viewport_menu', x: 120, y: 80 })
    })
    const menu = screen.getByTestId('viewport-menu')
    expect(menu).toHaveRole('menu')
    const names = within(menu)
      .getAllByRole('menuitem')
      .map((item) => item.textContent)
    expect(names).toEqual(
      expect.arrayContaining([expect.stringContaining('Hide'), expect.stringContaining('Mirror')]),
    )
    surface.focus()
    await userEvent.click(within(menu).getByRole('menuitem', { name: /^Hide/ }))
    await settle()
    expect(screen.queryByTestId('viewport-menu')).toBeNull()
    expect(document.activeElement).toBe(surface)
  })

  it('opens with Shift+F10 and with the ContextMenu key on the focused viewport', () => {
    const { surface } = setUp()
    surface.focus()
    fireEvent.keyDown(surface, { key: 'F10', shiftKey: true })
    expect(screen.getByTestId('viewport-menu')).toBeInTheDocument()
    fireEvent.keyDown(screen.getByTestId('viewport-menu'), { key: 'Escape' })
    expect(screen.queryByTestId('viewport-menu')).toBeNull()
    fireEvent.keyDown(surface, { key: 'ContextMenu' })
    expect(
      within(screen.getByTestId('viewport-menu')).getByRole('menuitem', { name: /^Fit to view/ }),
    ).toBeInTheDocument()
  })
})
