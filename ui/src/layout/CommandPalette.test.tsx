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
import { renderModelApp, stubMatchMedia } from './test-app.testing'

let uninstallDispatcher: () => void = () => undefined

beforeEach(installPopoverPolyfill)

beforeEach(() => {
  stubMatchMedia()
})

afterEach(() => {
  uninstallDispatcher()
  cleanup()
  vi.unstubAllGlobals()
})

function renderApp(state: Partial<AppState> = {}, platform?: Platform) {
  const test = renderModelApp({ state, ...(platform === undefined ? {} : { platform }) })
  uninstallDispatcher = installShortcutDispatcher(test.app, window)
  return test
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
    expect(screen.getByRole('dialog', { name: 'Command palette' })).toBe(palette)
    const input = within(palette).getByRole('combobox')
    expect(input).toBe(screen.getByTestId('palette-input'))
    expect(input).toHaveAttribute(
      'placeholder',
      'Search commands, features, bodies, parameters and settings',
    )
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

  it('groups every command under category headings with an empty query, Create first in Model', async () => {
    const { user } = renderApp()
    await openPalette(user)
    const headings = screen.getAllByTestId('palette-heading').map((heading) => heading.textContent)
    expect(headings[0]).toBe('Create')
    expect(headings.slice(0, 5)).toEqual(['Create', 'Modify', 'Pattern', 'Construct', 'Edit'])
    const create = screen.getByRole('group', { name: 'Create' })
    expect(
      within(create)
        .getAllByRole('option')
        .map((option) => option.textContent),
    ).toContain('Box')
    expect(
      within(screen.getByRole('group', { name: 'Modify' }))
        .getAllByRole('option')
        .some((option) => option.getAttribute('data-item-id') === 'feature.fillet'),
    ).toBe(true)
    // Document items appear only for a query.
    expect(options().some((option) => option.getAttribute('data-kind') === 'body')).toBe(false)
  })

  it('starts with the Sketch heading in sketch mode', async () => {
    const { user } = renderApp({
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
    await openPalette(user)
    expect(screen.getAllByTestId('palette-heading')[0]).toHaveTextContent('Sketch')
  })

  it('heads the recent items "Recent"', async () => {
    const { app, user } = renderApp()
    app.store.setState({
      settings: { ...app.store.getState().settings, paletteRecent: ['view.parameters'] },
    })
    await openPalette(user)
    expect(screen.getAllByTestId('palette-heading')[0]).toHaveTextContent('Recent')
    expect(
      within(screen.getByRole('group', { name: 'Recent' }))
        .getAllByRole('option')
        .map((option) => option.getAttribute('data-item-id')),
    ).toEqual(['view.parameters'])
  })

  it('shows no headings for a query', async () => {
    const { user } = renderApp()
    await openPalette(user)
    await user.type(screen.getByTestId('palette-input'), 'ext')
    expect(screen.queryAllByTestId('palette-heading')).toEqual([])
  })

  it('says "Nothing matches {query}." in a row that is not an option', async () => {
    const { user } = renderApp()
    await openPalette(user)
    await user.type(screen.getByTestId('palette-input'), 'zzz')
    expect(screen.getByTestId('palette-empty')).toHaveTextContent(/^Nothing matches zzz\.$/)
    expect(screen.queryAllByRole('option')).toEqual([])
  })

  it('describes the active option in the footer, which the option references', async () => {
    const { user } = renderApp()
    await openPalette(user)
    await user.type(screen.getByTestId('palette-input'), 'shell')
    const active = activeOption()
    expect(active).toHaveAttribute('data-item-id', 'feature.shell')
    const footer = screen.getByTestId('palette-footer')
    expect(footer).toHaveTextContent('Hollows out a body')
    const description = within(footer).getByText(/Hollows out a body/)
    expect(active).toHaveAttribute('aria-describedby', description.id)
    expect(description).not.toHaveAttribute('role')
  })

  it('names the keys in the footer when the active option has no description', async () => {
    const { user } = renderApp()
    await openPalette(user)
    await user.type(screen.getByTestId('palette-input'), 'Cube')
    expect(activeOption()).toHaveAttribute('data-kind', 'body')
    const footer = screen.getByTestId('palette-footer')
    expect(footer).toHaveTextContent(/^Enter to run, Esc to close$/)
    await user.clear(screen.getByTestId('palette-input'))
    await user.type(screen.getByTestId('palette-input'), 'zzz')
    expect(footer).toHaveTextContent(/^Esc to close$/)
  })

  it('keeps the palette inside the window, its list scrolling, at any UI scale (3.2)', async () => {
    const { user } = renderApp()
    const palette = await openPalette(user)
    expect(palette.parentElement).toHaveClass('fixed', 'inset-x-4', 'top-1/6', 'bottom-4')
    expect(palette.parentElement).toHaveClass('items-start')
    expect(palette).toHaveClass('max-h-full', 'flex', 'flex-col', 'overflow-hidden')
    expect(within(palette).getByRole('listbox')).toHaveClass(
      'max-h-96',
      'min-h-0',
      'flex-1',
      'overflow-y-auto',
    )
  })

  it('shows another app’s name that matched as the detail', async () => {
    const { user } = renderApp()
    await openPalette(user)
    await user.type(screen.getByTestId('palette-input'), 'pad')
    expect(activeOption()).toHaveAttribute('data-item-id', 'feature.extrude')
    expect(activeOption()).toHaveTextContent('FreeCAD: Pad')
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
