import { act, cleanup, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { installPopoverPolyfill } from '../components/popover-polyfill.testing'
import { MESSAGES } from '../messages'
import type { ActiveCommand, Notification } from '../state/store'
import { COLOR_CLASSES } from '../theme/roles'
import { cubeDocumentState, FILLET_EDGES_COMMAND, readyState } from './fixtures.testing'
import { createModelApp, renderWithApp, stubMatchMedia } from './test-app.testing'

beforeEach(installPopoverPolyfill)

beforeEach(() => {
  stubMatchMedia()
})

afterEach(() => {
  cleanup()
  vi.unstubAllGlobals()
})

function notification(
  id: string,
  severity: Notification['severity'],
  message: string,
  actions: Notification['actions'] = [],
): Notification {
  return { id, severity, message, actions, createdAt: 0 }
}

function renderWith(notifications: readonly Notification[], command: ActiveCommand | null = null) {
  const test = createModelApp({
    state: readyState(cubeDocumentState(), { notifications, command }),
  })
  renderWithApp(test.app)
  return { ...test, user: userEvent.setup() }
}

describe('notifications (3.22)', () => {
  it('shows each notification with its severity and message', () => {
    renderWith([
      notification('n-1', 'info', 'Exported cube.3mf.'),
      notification('n-2', 'error', 'Could not save cube.ptsm: disk full'),
    ])
    const shown = screen.getAllByTestId('notification')
    expect(shown.map((element) => element.getAttribute('data-severity'))).toEqual(['info', 'error'])
    expect(shown[0]).toHaveTextContent('Exported cube.3mf.')
    expect(shown[1]).toHaveTextContent('Could not save cube.ptsm: disk full')
  })

  it('places the stack in the viewport area, last in the tab order after the status bar (3.24)', () => {
    const { app } = renderWith([notification('n-1', 'info', 'Exported cube.3mf.')])
    act(() => {
      app.store.setState((state) => ({ panels: { ...state.panels, side: 'problems' } }))
    })
    const stack = screen.getByTestId('notification').parentElement
    const viewport = screen.getByTestId('viewport')
    for (const area of ['col-start-2', 'row-start-2']) {
      expect(viewport).toHaveClass(area)
      expect(stack).toHaveClass(area)
    }
    expect(stack).toHaveClass('justify-end', 'items-end')
    const statusBar = screen.getByTestId('status-bar')
    expect(statusBar.compareDocumentPosition(stack ?? document.body)).toBe(
      Node.DOCUMENT_POSITION_FOLLOWING,
    )
    const order = ['top-bar', 'browser', 'viewport', 'timeline', 'side-panel', 'status-bar']
    const regions = order.map((testId) => screen.getByTestId(testId))
    for (let i = 1; i < regions.length; i++) {
      expect(regions[i - 1]?.compareDocumentPosition(regions[i] ?? document.body)).toBe(
        Node.DOCUMENT_POSITION_FOLLOWING,
      )
    }
  })

  it('marks each notification with its code when it has one (4.1, 6.1)', () => {
    renderWith([
      { ...notification('n-1', 'error', 'Edges are gone.'), code: 'reference_unresolved' },
      notification('n-2', 'info', 'Nothing to say.'),
    ])
    const [coded, plain] = screen.getAllByTestId('notification')
    expect(coded).toHaveAttribute('data-code', 'reference_unresolved')
    expect(coded).not.toHaveTextContent('reference_unresolved')
    expect(plain).not.toHaveAttribute('data-code')
  })

  it('puts the newest notification at the bottom', () => {
    renderWith([notification('n-1', 'info', 'First.'), notification('n-2', 'info', 'Second.')])
    const [first, second] = screen.getAllByTestId('notification')
    expect(first?.compareDocumentPosition(second ?? document.body)).toBe(
      Node.DOCUMENT_POSITION_FOLLOWING,
    )
    expect(second).toHaveTextContent('Second.')
  })

  it('announces errors in an alert region and the others in a status region', () => {
    renderWith([
      notification('n-1', 'error', 'It failed.'),
      notification('n-2', 'warning', 'Careful.'),
    ])
    const alerts = screen.getByTestId('notification-alerts')
    const status = screen.getByTestId('notification-status')
    expect(alerts).toHaveAttribute('role', 'alert')
    // An output element is a status region.
    expect(status.tagName).toBe('OUTPUT')
    expect(alerts).toHaveTextContent('It failed.')
    expect(alerts).not.toHaveTextContent('Careful.')
    expect(status).toHaveTextContent('Careful.')
    expect(status).not.toHaveTextContent('It failed.')
  })

  it('keeps one alert and one status region mounted, and adds new messages to them', () => {
    const { app } = renderWith([])
    const alerts = screen.getByTestId('notification-alerts')
    const status = screen.getByTestId('notification-status')
    expect(alerts).toBeEmptyDOMElement()
    expect(status).toBeEmptyDOMElement()
    act(() => {
      app.store.setState({
        notifications: [
          notification('n-1', 'info', 'Exported cube.3mf.'),
          notification('n-2', 'error', 'It failed.'),
        ],
      })
    })
    expect(screen.getByTestId('notification-alerts')).toBe(alerts)
    expect(screen.getByTestId('notification-status')).toBe(status)
    expect(alerts).toHaveTextContent('It failed.')
    expect(status).toHaveTextContent('Exported cube.3mf.')
    for (const card of screen.getAllByTestId('notification')) {
      expect(card.closest('[role="alert"], [role="status"], output')).toBeNull()
    }
  })

  it('dismisses a notification with its Dismiss button', async () => {
    const { app, user } = renderWith([notification('n-1', 'info', 'Deleted Fillet 1.')])
    await user.click(
      within(screen.getByTestId('notification')).getByRole('button', { name: 'Dismiss' }),
    )
    expect(app.store.getState().notifications).toEqual([])
    expect(screen.queryByTestId('notification')).toBeNull()
  })

  it('runs an action and dismisses its notification', async () => {
    const run = vi.fn<() => void>()
    const { app, user } = renderWith([
      notification('n-1', 'info', 'Deleted Fillet 1.', [{ label: 'Undo', run }]),
    ])
    await user.click(
      within(screen.getByTestId('notification')).getByRole('button', { name: 'Undo' }),
    )
    expect(run).toHaveBeenCalledTimes(1)
    expect(app.store.getState().notifications).toEqual([])
  })

  it("waits during a feature command with a message's apply action: aria-disabled, it says why and runs nothing (3.7)", async () => {
    const run = vi.fn<() => void>()
    const revert = "Set Count of 'Linear pattern 1' back to 3"
    const { app, user } = renderWith(
      [
        notification('n-1', 'warning', "'Fillet 1' lost the right top edge of 'Box 1'.", [
          { label: revert, run, editsDocument: true },
          { label: 'Undo', run: vi.fn<() => void>() },
        ]),
      ],
      FILLET_EDGES_COMMAND,
    )
    const card = screen.getByTestId('notification')
    const button = within(card).getByRole('button', { name: revert })
    expect(button).toHaveAttribute('aria-disabled', 'true')
    expect(within(card).getByRole('button', { name: 'Undo' })).not.toHaveAttribute('aria-disabled')
    await user.click(button)
    expect(run).not.toHaveBeenCalled()
    expect(app.store.getState().notifications.map((shown) => shown.message)).toEqual([
      "'Fillet 1' lost the right top edge of 'Box 1'.",
      MESSAGES.finish_command_first('Fillet'),
    ])
  })

  it('shows a severity icon in each notification', () => {
    renderWith([
      notification('n-1', 'info', 'Info.'),
      notification('n-2', 'success', 'Done.'),
      notification('n-3', 'warning', 'Careful.'),
    ])
    const icons = screen
      .getAllByTestId('notification')
      .map((element) => element.querySelector('svg')?.getAttribute('class') ?? '')
    expect(icons[0]).toContain('lucide-info')
    expect(icons[1]).toContain('lucide-circle-check')
    expect(icons[2]).toContain('lucide-triangle-alert')
  })
})

describe('cause lines in notifications (3.22, U-132)', () => {
  const LOST = "'Fillet 1' lost the top edge of 'Extrude 1' along a line of 'Sketch 1'."
  const CAUSE = "This started after you deleted the line from (80, 40) to (70, 50) in 'Sketch 1'."

  it('U-132 shows the cause under the message, in textMuted, above the action buttons', () => {
    renderWith([
      {
        ...notification('n-1', 'warning', LOST, [
          { label: "Show what 'Fillet 1' lost", run: () => undefined },
          { label: 'Undo', run: () => undefined },
        ]),
        code: 'feature_lost',
        cause: CAUSE,
      },
    ])
    const shown = screen.getByTestId('notification')
    const cause = within(shown).getByTestId('cause-line')
    expect(cause).toHaveTextContent(CAUSE)
    const muted = COLOR_CLASSES.textMuted.foreground.split(' ')
    expect(
      [cause, ...cause.querySelectorAll('*')].some((element) =>
        muted.every((name) => element.classList.contains(name)),
      ),
    ).toBe(true)
    const message = within(shown).getByText(LOST)
    expect(message.compareDocumentPosition(cause) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy()
    const [show] = within(shown).getAllByRole('button', { name: "Show what 'Fillet 1' lost" })
    expect(
      cause.compareDocumentPosition(show ?? cause) & Node.DOCUMENT_POSITION_FOLLOWING,
    ).toBeTruthy()
  })

  it('U-132 shows no cause line for a notification without a cause', () => {
    renderWith([notification('n-1', 'warning', LOST)])
    expect(screen.queryByTestId('cause-line')).toBeNull()
  })
})
