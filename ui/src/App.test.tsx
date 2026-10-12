import { cleanup, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { installPopoverPolyfill } from './components/popover-polyfill.testing'
import { bodyItem, cubeDocumentState, readyState } from './layout/fixtures.testing'
import { createModelApp, renderWithApp, stubMatchMedia } from './layout/test-app.testing'
import { MESSAGES } from './messages'
import type { ActiveCommand, AppState } from './state/store'
import { settle } from './test/fakes/settle'
import { COLOR_CLASSES, FONT_CLASSES } from './theme/roles'

let uninstallPopovers: () => void

beforeEach(() => {
  uninstallPopovers = installPopoverPolyfill()
  stubMatchMedia()
})

afterEach(() => {
  cleanup()
  uninstallPopovers()
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
  preview: 'none',
}

function renderApp(state: Partial<AppState> = {}) {
  const test = createModelApp({ state: readyState(cubeDocumentState(), state) })
  renderWithApp(test.app)
  return { ...test, user: userEvent.setup() }
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

  it('styles the root with the fixed layout, the interface font and the surface and text roles', () => {
    renderApp()
    const root = screen.getByTestId('app')
    for (const name of [
      'h-dvh',
      'w-dvw',
      'overflow-hidden',
      'antialiased',
      ...FONT_CLASSES.interfaceText.split(' '),
      ...COLOR_CLASSES.surface.background.split(' '),
      ...COLOR_CLASSES.text.foreground.split(' '),
    ]) {
      expect(root).toHaveClass(name)
    }
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
      core: { status: 'loading', engineVersion: null, fatal: null, pendingMutations: 0 },
    })
    expect(screen.getByTestId('app')).toHaveAttribute('data-core-status', 'loading')
    cleanup()
    renderApp({
      core: { status: 'failed', engineVersion: null, fatal: 'HTTP 404', pendingMutations: 0 },
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

  it('keeps the other regions working when one region crashes (4.4)', () => {
    const report = vi.spyOn(console, 'error').mockImplementation(() => undefined)
    const broken = cubeDocumentState()
    // Parameters that are not a list break the browser's Parameters group.
    const recipe = {
      ...broken.recipe,
      parameters: null as unknown as typeof broken.recipe.parameters,
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
        tool: 'line',
        construction: false,
        showConstraints: true,
        selection: [],
        dragging: false,
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
        tool: null,
        construction: false,
        showConstraints: true,
        selection: [],
        dragging: false,
      },
    })
    await user.keyboard('{Escape}')
    await settle()
    expect(app.store.getState().context).toBe('model')
    expect(app.store.getState().sketch).toBeNull()
  })
})
