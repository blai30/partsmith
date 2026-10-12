import { cleanup, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { installPopoverPolyfill } from '../components/popover-polyfill.testing'
import type { Notification } from '../state/store'
import { cubeDocumentState, readyState } from './fixtures.testing'
import { createModelApp, renderWithApp, stubMatchMedia } from './test-app.testing'

let uninstallPopovers: () => void

beforeEach(() => {
  uninstallPopovers = installPopoverPolyfill()
  stubMatchMedia()
})

afterEach(() => {
  cleanup()
  uninstallPopovers()
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

function renderWith(notifications: readonly Notification[]) {
  const test = createModelApp({ state: readyState(cubeDocumentState(), { notifications }) })
  renderWithApp(test.app)
  return { ...test, user: userEvent.setup() }
}

describe('notifications (3.22)', () => {
  it('shows each notification inside the viewport with its severity and message', () => {
    renderWith([
      notification('n-1', 'info', 'Exported cube.3mf.'),
      notification('n-2', 'error', 'Could not save cube.ptsm: disk full'),
    ])
    const shown = screen.getAllByTestId('notification')
    expect(shown.map((element) => element.getAttribute('data-severity'))).toEqual(['info', 'error'])
    expect(shown[0]).toHaveTextContent('Exported cube.3mf.')
    expect(shown[1]).toHaveTextContent('Could not save cube.ptsm: disk full')
    for (const element of shown) {
      expect(screen.getByTestId('viewport')).toContainElement(element)
    }
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
    const [error, warning] = screen.getAllByTestId('notification')
    expect(error?.closest('[role="alert"]')).not.toBeNull()
    expect(warning?.closest('[role="status"]')).not.toBeNull()
    expect(warning?.closest('[role="alert"]')).toBeNull()
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
