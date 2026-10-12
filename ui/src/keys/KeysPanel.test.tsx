import { cleanup, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import type { Platform } from '../app/createApp'
import { installShortcutDispatcher } from '../commands/dispatcher'
import { installPopoverPolyfill } from '../components/popover-polyfill.testing'
import { cubeDocumentState, readyState } from '../layout/fixtures.testing'
import { createModelApp, renderWithApp, stubMatchMedia } from '../layout/test-app.testing'
import type { AppState } from '../state/store'
import { MAC_WEB } from '../test/fakes/app'

let uninstallPopovers: () => void
let uninstallDispatcher: () => void = () => undefined

beforeEach(() => {
  uninstallPopovers = installPopoverPolyfill()
  stubMatchMedia()
})

afterEach(() => {
  uninstallDispatcher()
  cleanup()
  uninstallPopovers()
  vi.unstubAllGlobals()
})

function renderApp(state: Partial<AppState> = {}, platform?: Platform) {
  const test = createModelApp({
    state: readyState(cubeDocumentState(), state),
    ...(platform === undefined ? {} : { platform }),
  })
  renderWithApp(test.app)
  uninstallDispatcher = installShortcutDispatcher(test.app, window)
  return { ...test, user: userEvent.setup() }
}

/** The panel's row whose first cell is exactly this text. */
function row(text: string): HTMLElement {
  const found = within(screen.getByTestId('keys-panel'))
    .getAllByTestId('keys-row')
    .find((element) => element.querySelector('dt')?.textContent === text)
  if (found === undefined) {
    throw new Error(`No row "${text}".`)
  }
  return found
}

describe('the keyboard and mouse panel (3.5)', () => {
  it('opens with F1 in the side panel, named "Keyboard and mouse", and keeps the browser off F1', async () => {
    const { app } = renderApp()
    const event = new KeyboardEvent('keydown', { key: 'F1', bubbles: true, cancelable: true })
    window.dispatchEvent(event)
    await vi.waitFor(() => {
      expect(screen.getByTestId('keys-panel')).toBeInTheDocument()
    })
    expect(event.defaultPrevented).toBe(true)
    expect(app.store.getState().panels.side).toBe('keys')
    expect(screen.getByRole('complementary', { name: 'Keyboard and mouse' })).toBeInTheDocument()
    expect(
      within(screen.getByTestId('keys-panel')).getByRole('heading', { name: 'Keyboard and mouse' }),
    ).toBeInTheDocument()
  })

  it('shows the mouse rows of the current preset, and switches presets from its control', async () => {
    const { app, user } = renderApp({ panels: { ...stateDefaults().panels, side: 'keys' } })
    expect(row('Orbit')).toHaveTextContent('Shift + middle drag')
    await user.click(
      within(screen.getByTestId('keys-panel')).getByRole('radio', { name: 'FreeCAD' }),
    )
    expect(app.store.getState().settings.navigation).toBe('freecad')
    expect(row('Orbit')).toHaveTextContent('Middle + left drag')
  })

  it('lists the keys components handle, with Mod as the platform’s key', () => {
    renderApp({ panels: { ...stateDefaults().panels, side: 'keys' } })
    expect(row('Shift or Ctrl with click')).toHaveTextContent('Add to or remove from the selection')
    expect(row('Hold Ctrl')).toHaveTextContent('Place freely')
    expect(row('Esc')).toHaveTextContent('Step back one level')
  })

  it('writes Mod as ⌘ on macOS', () => {
    renderApp({ panels: { ...stateDefaults().panels, side: 'keys' } }, MAC_WEB)
    expect(row('Hold ⌘')).toBeInTheDocument()
    expect(screen.getByTestId('keys-panel')).toHaveTextContent(
      'Every command and its shortcut: Search (⌘K).',
    )
  })

  it('explains typed values and ends by pointing to the palette', () => {
    renderApp({ panels: { ...stateDefaults().panels, side: 'keys' } })
    const panel = screen.getByTestId('keys-panel')
    expect(row('ArrowUp, ArrowDown')).toHaveTextContent('Step a plain number')
    expect(panel).toHaveTextContent(
      'Any size or angle field takes a number, a value with a unit such as 1 in, or a formula with parameter names.',
    )
    expect(panel).toHaveTextContent('Every command and its shortcut: Search (Ctrl+K).')
  })

  it('closes with its close button', async () => {
    const { app, user } = renderApp({ panels: { ...stateDefaults().panels, side: 'keys' } })
    await user.click(screen.getByRole('button', { name: 'Close Keyboard and mouse' }))
    expect(app.store.getState().panels.side).toBeNull()
  })
})

function stateDefaults(): AppState {
  return createModelApp().app.store.getState()
}
