import { act, cleanup, fireEvent, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import type { App } from '../app/createApp'
import { installPopoverPolyfill } from '../components/popover-polyfill.testing'
import { bodyItem, cubeDocumentState, readyState } from '../layout/fixtures.testing'
import {
  createModelApp,
  renderWithApp,
  stubAnimationFrames,
  stubElementSize,
  stubMatchMedia,
  type FrameQueue,
} from '../layout/test-app.testing'
import { MESSAGES } from '../messages'
import { DEFAULT_SETTINGS } from '../settings/settings'
import type { AppState } from '../state/store'
import type { FakeBackend } from '../test/fakes/backend'
import { settle } from '../test/fakes/settle'
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

let uninstallPopovers: () => void
let restoreSize: () => void
let frames: FrameQueue

beforeEach(() => {
  uninstallPopovers = installPopoverPolyfill()
  restoreSize = stubElementSize(HTMLCanvasElement.prototype, { width: 800, height: 600 })
  stubMatchMedia({ reducedMotion: true })
  frames = stubAnimationFrames()
  created.backends.length = 0
})

afterEach(() => {
  cleanup()
  uninstallPopovers()
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
  const [x0, y0] = forward(before)
  const [x1, y1] = forward(after)
  const angle = Math.atan2(x0 * y1 - y0 * x1, x0 * x1 + y0 * y1)
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
        tool: null,
        construction: false,
        showConstraints: true,
        selection: [],
        dragging: false,
      },
    })
    expect(within(viewport).queryByRole('button', { name: 'Selection filter' })).toBeNull()
  })
})

describe('overlays', () => {
  it('shows "Loading the geometry core" while the core loads', () => {
    setUp({ core: { status: 'loading', engineVersion: null, fatal: null, pendingMutations: 0 } })
    expect(screen.getByTestId('core-overlay')).toHaveTextContent('Loading the geometry core')
  })

  it('shows no loading overlay once the core is ready', () => {
    setUp()
    expect(screen.queryByTestId('core-overlay')).toBeNull()
  })

  it('shows the core load failure with Reload (4.4)', () => {
    setUp({
      core: { status: 'failed', engineVersion: null, fatal: 'HTTP 404', pendingMutations: 0 },
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

  it('renders with the settings’ UI scale', () => {
    setUp({ settings: { ...DEFAULT_SETTINGS, theme: 'light', uiScale: 1.5 } })
    expect(created.backends[0]?.lastFrame()?.uiScale).toBe(1.5)
  })
})
