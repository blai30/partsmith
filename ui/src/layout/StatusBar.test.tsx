import { act, cleanup, screen, within } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { installPopoverPolyfill } from '../components/popover-polyfill.testing'
import type { Document } from '../generated/protocol'
import { MESSAGES } from '../messages'
import type { ActiveCommand, AppState, DocumentState } from '../state/store'
import { CUBE_DOCUMENT, RECT_SKETCH_SOLUTION } from '../test/fakes/documents'
import { settle } from '../test/fakes/settle'
import { COLOR_CLASSES } from '../theme/roles'
import { cubeDocumentState, rectSketchDocumentState } from './fixtures.testing'
import { StatusBar } from './StatusBar'
import { renderModelApp, stubMatchMedia } from './test-app.testing'

beforeEach(installPopoverPolyfill)

beforeEach(() => {
  stubMatchMedia()
})

afterEach(() => {
  cleanup()
  vi.useRealTimers()
  vi.unstubAllGlobals()
})

function renderApp(state: Partial<AppState> = {}, document: DocumentState = cubeDocumentState()) {
  const test = renderModelApp({ document, state, element: <StatusBar /> })
  return { ...test, statusBar: screen.getByTestId('status-bar') }
}

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

describe('the status bar (3.21)', () => {
  it('is a footer', () => {
    const { statusBar } = renderApp()
    expect(statusBar.tagName).toBe('FOOTER')
  })

  it('shows "Grid {spacing}" in the document unit while a grid is drawn (3.21)', () => {
    const { app } = renderApp()
    act(() => {
      app.store.setState((state) => ({ view: { ...state.view, gridSpacing: 10 } }))
    })
    expect(screen.getByTestId('status-grid')).toHaveTextContent(/^Grid 10 mm$/)
  })

  it('shows the grid spacing in inches in an inch document', () => {
    const document = cubeDocumentState({ recipe: { ...CUBE_DOCUMENT, units: 'in' } })
    const { app } = renderApp({}, document)
    act(() => {
      app.store.setState((state) => ({ view: { ...state.view, gridSpacing: 2.54 } }))
    })
    expect(screen.getByTestId('status-grid')).toHaveTextContent(/^Grid 0\.1 in$/)
  })

  it('hides the grid spacing while no grid is drawn', () => {
    renderApp()
    expect(screen.queryByTestId('status-grid')).toBeNull()
  })

  it('shows no core status while the core is idle', () => {
    renderApp()
    expect(screen.queryByTestId('status-core')).toBeNull()
  })

  it('shows "Loading core" while the core loads', () => {
    renderApp({
      core: {
        status: 'loading',
        engineVersion: null,
        fatal: null,
        pendingMutations: 0,
        stillWorking: false,
      },
    })
    expect(screen.getByTestId('status-core')).toHaveTextContent('Loading core')
  })

  it('shows "Core stopped" in the error text role after a failure', () => {
    renderApp({
      core: {
        status: 'failed',
        engineVersion: '0.1.0',
        fatal: 'trap',
        pendingMutations: 0,
        stillWorking: false,
      },
    })
    const status = screen.getByTestId('status-core')
    expect(status).toHaveTextContent('Core stopped')
    const roleClasses = COLOR_CLASSES.errorText.foreground.split(' ')
    const carriers = [status, ...status.querySelectorAll('*')]
    expect(
      carriers.some((element) => roleClasses.every((name) => element.classList.contains(name))),
    ).toBe(true)
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

  it('shows the last rebuild time in whole milliseconds, in tabular figures', () => {
    renderApp({}, cubeDocumentState({ lastRebuildMilliseconds: 12.4 }))
    const recompute = screen.getByTestId('status-recompute')
    expect(recompute).toHaveTextContent('Rebuilt in 12 ms')
    expect(recompute).toHaveClass('tabular-nums')
    expect(recompute.querySelector('.font-mono')).toBeNull()
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
    expect(
      within(menu)
        .getAllByRole('menuitemradio')
        .map((item) => item.textContent?.trim()),
    ).toEqual(['mm', 'cm', 'm', 'in'])
    expect(within(menu).getByRole('menuitemradio', { name: 'mm' })).toHaveAttribute(
      'aria-checked',
      'true',
    )
    await user.click(within(menu).getByRole('menuitemradio', { name: 'in' }))
    await settle()
    expect(core.callsTo('applyEdits')).toEqual([
      [1, 'Change units to in', [{ type: 'set_units', units: 'in' }]],
    ])
    expect(
      app.store.getState().notifications.map((notification) => notification.message),
    ).toContain(MESSAGES.units_changed('in'))
  })

  it('shows "{default name} (default)" without a printer and the profile name with one', () => {
    renderApp()
    expect(screen.getByTestId('status-printer')).toHaveTextContent('Generic FDM 0.4 mm (default)')
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

  it('disables units and printer with finish_command_first during a feature command, and a click says why', async () => {
    const { app, user } = renderApp({ command: EXTRUDE_COMMAND })
    const reason = MESSAGES.finish_command_first('Extrude')
    const units = screen.getByTestId('status-units')
    const printer = screen.getByTestId('status-printer')
    for (const button of [units, printer]) {
      expect(button).toHaveAttribute('aria-disabled', 'true')
      expect(button).toHaveAccessibleDescription(expect.stringContaining(reason))
    }
    await user.click(units)
    expect(screen.queryByRole('menu')).toBeNull()
    await user.click(printer)
    expect(screen.queryByTestId('printer-menu')).toBeNull()
    expect(app.store.getState().notifications).toMatchObject([
      { message: reason, code: 'finish_command_first' },
    ])
  })

  it('enables units and printer again when the command ends', () => {
    const { app } = renderApp({ command: EXTRUDE_COMMAND })
    act(() => {
      app.store.setState({ command: null })
    })
    expect(screen.getByTestId('status-units')).not.toHaveAttribute('aria-disabled')
    expect(screen.getByTestId('status-printer')).not.toHaveAttribute('aria-disabled')
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
          reattaching: null,
          tool: null,
          construction: false,
          showConstraints: true,
          selection: [],
          dragging: false,
          cameraBefore: null,
        },
      },
      rectSketchDocumentState({ sketches: { 'f-sk1': { solution, display: null } } }),
    )
    const dof = screen.getByTestId('status-dof')
    expect(dof).toHaveTextContent('3 DOF')
    expect(dof).toHaveAttribute('data-dof', '3')
    expect(dof).toHaveAttribute('data-state', 'under')
    expect(dof).toHaveClass('tabular-nums')
    expect(dof.querySelector('.font-mono')).toBeNull()
    // "DOF" is explained in plain words (3.16.8, 3.21).
    expect(dof).toHaveAccessibleDescription(
      'Degrees of freedom: how many ways the sketch can still move.',
    )
  })

  it('shows no sketch DOF outside sketch mode', () => {
    renderApp()
    expect(screen.queryByTestId('status-dof')).toBeNull()
  })
})
