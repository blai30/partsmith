import { act, cleanup, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { installPopoverPolyfill } from '../components/popover-polyfill.testing'
import type { Document } from '../generated/protocol'
import { MESSAGES } from '../messages'
import type { AppState, DocumentState } from '../state/store'
import { CUBE_DOCUMENT, RECT_SKETCH_SOLUTION } from '../test/fakes/documents'
import { settle } from '../test/fakes/settle'
import { COLOR_CLASSES, FONT_CLASSES } from '../theme/roles'
import { cubeDocumentState, readyState, rectSketchDocumentState } from './fixtures.testing'
import { createModelApp, renderWithApp, stubMatchMedia } from './test-app.testing'

let uninstallPopovers: () => void

beforeEach(() => {
  uninstallPopovers = installPopoverPolyfill()
  stubMatchMedia()
})

afterEach(() => {
  cleanup()
  uninstallPopovers()
  vi.useRealTimers()
  vi.unstubAllGlobals()
})

function renderApp(state: Partial<AppState> = {}, document: DocumentState = cubeDocumentState()) {
  const test = createModelApp({ state: readyState(document, state) })
  renderWithApp(test.app)
  return { ...test, user: userEvent.setup(), statusBar: screen.getByTestId('status-bar') }
}

describe('the status bar (3.21)', () => {
  it('is a footer', () => {
    const { statusBar } = renderApp()
    expect(statusBar.tagName).toBe('FOOTER')
  })

  it('shows no core status while the core is idle', () => {
    renderApp()
    expect(screen.queryByTestId('status-core')).toBeNull()
  })

  it('shows "Loading core" while the core loads', () => {
    renderApp({ core: { status: 'loading', engineVersion: null, fatal: null, pendingMutations: 0 } })
    expect(screen.getByTestId('status-core')).toHaveTextContent('Loading core')
  })

  it('shows "Core stopped" in the error text role after a failure', () => {
    renderApp({ core: { status: 'failed', engineVersion: '0.1.0', fatal: 'trap', pendingMutations: 0 } })
    const status = screen.getByTestId('status-core')
    expect(status).toHaveTextContent('Core stopped')
    const roleClasses = COLOR_CLASSES.errorText.foreground.split(' ')
    const carriers = [status, ...status.querySelectorAll('*')]
    expect(carriers.some((element) => roleClasses.every((name) => element.classList.contains(name)))).toBe(true)
  })

  it('shows "Rebuilding" only after a mutation has been pending for more than 300 ms', () => {
    vi.useFakeTimers()
    const { app } = renderApp()
    act(() => {
      app.store.setState({ core: { ...app.store.getState().core, pendingMutations: 1 } })
    })
    act(() => {
      vi.advanceTimersByTime(299)
    })
    expect(screen.queryByTestId('status-core')).toBeNull()
    act(() => {
      vi.advanceTimersByTime(2)
    })
    expect(screen.getByTestId('status-core')).toHaveTextContent('Rebuilding')
    act(() => {
      app.store.setState({ core: { ...app.store.getState().core, pendingMutations: 0 } })
    })
    expect(screen.queryByTestId('status-core')).toBeNull()
  })

  it('shows the last rebuild time in whole milliseconds, the number in the values font', () => {
    renderApp({}, cubeDocumentState({ lastRebuildMilliseconds: 12.4 }))
    const recompute = screen.getByTestId('status-recompute')
    expect(recompute).toHaveTextContent('Rebuilt in 12 ms')
    expect(within(recompute).getByText('12')).toHaveClass(...FONT_CLASSES.values.split(' '))
  })

  it('shows rebuilds of a second or more in seconds with one decimal', () => {
    renderApp({}, cubeDocumentState({ lastRebuildMilliseconds: 1234 }))
    expect(screen.getByTestId('status-recompute')).toHaveTextContent('Rebuilt in 1.2 s')
    cleanup()
    renderApp({}, cubeDocumentState({ lastRebuildMilliseconds: 1000 }))
    expect(screen.getByTestId('status-recompute')).toHaveTextContent('Rebuilt in 1.0 s')
  })

  it('changes units from the units menu with "Change units to {unit}" and the units_changed notification', async () => {
    const { app, core, user } = renderApp()
    const units = screen.getByTestId('status-units')
    expect(units).toHaveTextContent('mm')
    await user.click(units)
    const menu = screen.getByRole('menu')
    expect(within(menu).getAllByRole('menuitemradio').map((item) => item.textContent?.trim())).toEqual([
      'mm',
      'cm',
      'm',
      'in',
    ])
    expect(within(menu).getByRole('menuitemradio', { name: 'mm' })).toHaveAttribute('aria-checked', 'true')
    await user.click(within(menu).getByRole('menuitemradio', { name: 'in' }))
    await settle()
    expect(core.callsTo('applyEdits')).toEqual([[1, 'Change units to in', [{ type: 'set_units', units: 'in' }]]])
    expect(app.store.getState().notifications.map((notification) => notification.message)).toContain(
      MESSAGES.units_changed('in'),
    )
  })

  it('shows "No printer" without a printer and the profile name with one', () => {
    renderApp()
    expect(screen.getByTestId('status-printer')).toHaveTextContent('No printer')
    cleanup()
    const withPrinter: Document = { ...CUBE_DOCUMENT, printer: 'generic-fdm' }
    renderApp({}, cubeDocumentState({ recipe: withPrinter }))
    expect(screen.getByTestId('status-printer')).toHaveTextContent('Generic FDM 0.4 mm')
  })

  it('opens the printer menu from the printer button', async () => {
    const { user } = renderApp()
    await user.click(screen.getByTestId('status-printer'))
    expect(screen.getByTestId('printer-menu')).toBeVisible()
  })

  it('has the print health badge', () => {
    renderApp()
    expect(within(screen.getByTestId('status-bar')).getByTestId('health-badge')).toBeInTheDocument()
  })

  it('shows the sketch DOF in sketch mode with a sketch, as the sketch header maps it', () => {
    const solution = { ...RECT_SKETCH_SOLUTION, status: 'under_constrained' as const, dof: 3 }
    renderApp(
      {
        context: 'sketch',
        sketch: {
          sketchId: 'f-sk1',
          tool: null,
          construction: false,
          showConstraints: true,
          selection: [],
          dragging: false,
        },
      },
      rectSketchDocumentState({ sketches: { 'f-sk1': { solution, display: null } } }),
    )
    const dof = screen.getByTestId('status-dof')
    expect(dof).toHaveTextContent('3 DOF')
    expect(dof).toHaveAttribute('data-dof', '3')
    expect(dof).toHaveAttribute('data-state', 'under')
    expect(within(dof).getByText('3')).toHaveClass(...FONT_CLASSES.values.split(' '))
  })

  it('shows no sketch DOF outside sketch mode', () => {
    renderApp()
    expect(screen.queryByTestId('status-dof')).toBeNull()
  })
})
