import { cleanup, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import type { App, Platform } from '../app/createApp'
import { installShortcutDispatcher } from '../commands/dispatcher'
import { installPopoverPolyfill } from '../components/popover-polyfill.testing'
import { MESSAGES } from '../messages'
import type { AppState } from '../state/store'
import { MAC_WEB } from '../test/fakes/app'
import { settle } from '../test/fakes/settle'
import { COLOR_CLASSES } from '../theme/roles'
import { cubeDocumentState, readyState } from './fixtures.testing'
import { createModelApp, renderWithApp, stubMatchMedia } from './test-app.testing'

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

async function openPalette(user: ReturnType<typeof userEvent.setup>): Promise<HTMLElement> {
  await user.click(screen.getByTestId('palette-button'))
  return screen.getByTestId('palette')
}

function options(): HTMLElement[] {
  return screen.getAllByTestId('palette-option')
}

function activeOption(): HTMLElement | undefined {
  const input = screen.getByTestId('palette-input')
  const id = input.getAttribute('aria-activedescendant')
  return options().find((option) => option.id === id)
}

function open(app: App): boolean {
  return app.store.getState().panels.paletteOpen
}

describe('the command palette (3.4)', () => {
  it('is a dialog named "Command palette" with a combobox controlling a listbox', async () => {
    const { user } = renderApp()
    const palette = await openPalette(user)
    expect(palette).toHaveAttribute('role', 'dialog')
    expect(palette).toHaveAccessibleName('Command palette')
    const input = within(palette).getByRole('combobox')
    expect(input).toBe(screen.getByTestId('palette-input'))
    expect(input).toHaveAttribute('placeholder', 'Search commands, bodies and parameters')
    const listbox = within(palette).getByRole('listbox')
    expect(input).toHaveAttribute('aria-controls', listbox.id)
    expect(input).toHaveFocus()
  })

  it('opens with Mod+K from anywhere and closes with Mod+K', async () => {
    const { app, user } = renderApp()
    await user.keyboard('{Control>}k{/Control}')
    expect(open(app)).toBe(true)
    expect(screen.getByTestId('palette')).toBeInTheDocument()
    await user.keyboard('{Control>}k{/Control}')
    expect(open(app)).toBe(false)
    expect(screen.queryByTestId('palette')).toBeNull()
  })

  it('opens with S when focus is not in a text field', async () => {
    const { app, user } = renderApp()
    await user.keyboard('s')
    expect(open(app)).toBe(true)
  })

  it('ranks Extrude first for "ext" and makes the first option active', async () => {
    const { user } = renderApp()
    await openPalette(user)
    await user.type(screen.getByTestId('palette-input'), 'ext')
    const first = options()[0]
    expect(first).toHaveAttribute('data-item-id', 'feature.extrude')
    expect(first).toHaveAttribute('data-kind', 'command')
    expect(activeOption()).toBe(first)
    for (const name of COLOR_CLASSES.hover.active.split(' ')) {
      expect(first).toHaveClass(name)
    }
  })

  it('moves the active option with the arrow keys and wraps', async () => {
    const { user } = renderApp()
    await openPalette(user)
    await user.type(screen.getByTestId('palette-input'), 'ext')
    const all = options()
    await user.keyboard('{ArrowDown}')
    expect(activeOption()).toBe(all[1])
    await user.keyboard('{ArrowUp}{ArrowUp}')
    expect(activeOption()).toBe(all.at(-1))
    await user.keyboard('{ArrowDown}')
    expect(activeOption()).toBe(all[0])
  })

  it('runs the active option with Enter, closes, and records it first in paletteRecent', async () => {
    const { app, user } = renderApp()
    await openPalette(user)
    await user.type(screen.getByTestId('palette-input'), 'Toggle timeline')
    expect(activeOption()).toHaveAttribute('data-item-id', 'view.timeline')
    await user.keyboard('{Enter}')
    await settle()
    expect(app.store.getState().panels.timelineCollapsed).toBe(true)
    expect(open(app)).toBe(false)
    expect(app.store.getState().settings.paletteRecent[0]).toBe('view.timeline')
  })

  it('runs an option on click', async () => {
    const { app, user } = renderApp()
    await openPalette(user)
    await user.type(screen.getByTestId('palette-input'), 'Theme: Dark')
    await user.click(
      options().find((option) => option.textContent?.includes('Theme: Dark')) ?? document.body,
    )
    expect(app.store.getState().settings.theme).toBe('dark')
    expect(open(app)).toBe(false)
  })

  it('closes with Esc and returns focus to where it was', async () => {
    const { app, user } = renderApp()
    const button = screen.getByTestId('palette-button')
    await user.click(button)
    expect(open(app)).toBe(true)
    await user.keyboard('{Escape}')
    expect(open(app)).toBe(false)
    expect(button).toHaveFocus()
  })

  it('shows a disabled option’s reason and keeps the palette open on Enter', async () => {
    const { app, user } = renderApp()
    await openPalette(user)
    await user.type(screen.getByTestId('palette-input'), 'Undo')
    const undo = activeOption()
    expect(undo).toHaveAttribute('data-item-id', 'edit.undo')
    expect(undo).toHaveTextContent(MESSAGES.nothing_to_undo())
    await user.keyboard('{Enter}')
    expect(open(app)).toBe(true)
    expect(within(screen.getByTestId('palette')).getByRole('status')).toHaveTextContent(
      MESSAGES.nothing_to_undo(),
    )
  })

  it('tags items that are not commands with their kind', async () => {
    const { user } = renderApp()
    await openPalette(user)
    await user.type(screen.getByTestId('palette-input'), 'Cube')
    const body = options().find((option) => option.getAttribute('data-kind') === 'body')
    expect(body).toHaveTextContent('Cube')
    expect(body).toHaveTextContent('Body')
    await user.clear(screen.getByTestId('palette-input'))
    await user.type(screen.getByTestId('palette-input'), 'Extrude 1')
    const feature = options().find((option) => option.getAttribute('data-kind') === 'feature')
    expect(feature).toHaveTextContent('Feature')
  })

  it('shows each command’s formatted shortcut', async () => {
    const { user } = renderApp()
    await openPalette(user)
    await user.type(screen.getByTestId('palette-input'), 'Save as')
    const saveAs = options().find(
      (option) => option.getAttribute('data-item-id') === 'file.save_as',
    )
    expect(within(saveAs ?? document.body).getByTestId('palette-shortcut')).toHaveTextContent(
      'Ctrl+Shift+S',
    )
  })

  it('formats shortcuts with symbols on macOS', async () => {
    const { user } = renderApp({}, MAC_WEB)
    await openPalette(user)
    await user.type(screen.getByTestId('palette-input'), 'Save as')
    const saveAs = options().find(
      (option) => option.getAttribute('data-item-id') === 'file.save_as',
    )
    expect(within(saveAs ?? document.body).getByTestId('palette-shortcut')).toHaveTextContent('⇧⌘S')
  })

  it('lists recent items first with an empty query', async () => {
    const { app, user } = renderApp()
    app.store.setState({
      settings: {
        ...app.store.getState().settings,
        paletteRecent: ['view.parameters', 'file.save'],
      },
    })
    await openPalette(user)
    expect(
      options()
        .slice(0, 2)
        .map((option) => option.getAttribute('data-item-id')),
    ).toEqual(['view.parameters', 'file.save'])
  })

  it('closes on a pointer press outside', async () => {
    const { app, user } = renderApp()
    await openPalette(user)
    await user.click(screen.getByTestId('status-bar'))
    await waitFor(() => {
      expect(open(app)).toBe(false)
    })
  })
})
