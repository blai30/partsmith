import { render, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { FilePlus } from 'lucide-react'
import { useRef, useState } from 'react'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { COLOR_CLASSES } from '../theme/roles'
import { Menu, type MenuEntry } from './Menu'
import { installPopoverPolyfill } from './popover-polyfill.testing'

beforeEach(installPopoverPolyfill)

function Harness({
  entries,
  onOpenChange,
}: {
  readonly entries: readonly MenuEntry[]
  readonly onOpenChange?: ((open: boolean) => void) | undefined
}) {
  const anchor = useRef<HTMLButtonElement>(null)
  const [open, setOpen] = useState(false)
  return (
    <>
      <button ref={anchor} type="button" onClick={() => setOpen(true)}>
        Document
      </button>
      <button type="button">Elsewhere</button>
      <Menu
        label="Document"
        open={open}
        anchor={anchor}
        entries={entries}
        onOpenChange={(next) => {
          onOpenChange?.(next)
          setOpen(next)
        }}
      />
    </>
  )
}

type Handlers = {
  readonly onNew: () => void
  readonly onSave: () => void
  readonly onConstraints: () => void
  readonly onSystem: () => void
  readonly onLight: () => void
  readonly onDark: () => void
}

function handlers(): Handlers {
  return {
    onNew: vi.fn<() => void>(),
    onSave: vi.fn<() => void>(),
    onConstraints: vi.fn<() => void>(),
    onSystem: vi.fn<() => void>(),
    onLight: vi.fn<() => void>(),
    onDark: vi.fn<() => void>(),
  }
}

function documentEntries(on: Handlers): readonly MenuEntry[] {
  return [
    { kind: 'item', label: 'New', icon: FilePlus, shortcut: 'Ctrl+N', onSelect: on.onNew },
    { kind: 'item', label: 'Save', onSelect: on.onSave },
    { kind: 'separator' },
    { kind: 'checkbox', label: 'Show constraints', checked: true, onSelect: on.onConstraints },
    {
      kind: 'submenu',
      label: 'Theme',
      entries: [
        { kind: 'radio', label: 'System', checked: false, onSelect: on.onSystem },
        { kind: 'radio', label: 'Light', checked: true, onSelect: on.onLight },
        { kind: 'radio', label: 'Dark', checked: false, onSelect: on.onDark },
      ],
    },
  ]
}

async function openMenu(entries: readonly MenuEntry[], onOpenChange?: (open: boolean) => void) {
  const user = userEvent.setup()
  render(<Harness entries={entries} onOpenChange={onOpenChange} />)
  const trigger = screen.getByRole('button', { name: 'Document' })
  await user.click(trigger)
  return { user, trigger, menu: screen.getByRole('menu', { name: 'Document' }) }
}

function item(name: string | RegExp): HTMLElement {
  return screen.getByRole('menuitem', { name })
}

function focused(): HTMLElement | null {
  return document.activeElement instanceof HTMLElement ? document.activeElement : null
}

describe('Menu', () => {
  it('is not shown while closed', () => {
    render(<Harness entries={documentEntries(handlers())} />)
    expect(screen.queryByRole('menu')).toBeNull()
  })

  it('opens as a menu named by its label with focus on the first item', async () => {
    const { menu } = await openMenu(documentEntries(handlers()))
    expect(menu).toBeInTheDocument()
    expect(item(/^New/)).toHaveFocus()
  })

  it('gives each entry kind its menu role', async () => {
    const { menu } = await openMenu(documentEntries(handlers()))
    expect(within(menu).getAllByRole('menuitem').length).toBeGreaterThanOrEqual(3)
    expect(within(menu).getByRole('separator')).toBeInTheDocument()
    // U-188: items are controls, separators quiet dividers inside the popup.
    expect(within(menu).getAllByRole('menuitem')[0]).toHaveClass('rounded-md')
    expect(within(menu).getByRole('separator')).toHaveClass(COLOR_CLASSES.divider.border)
    const checkbox = within(menu).getByRole('menuitemcheckbox', { name: 'Show constraints' })
    expect(checkbox).toBeChecked()
  })

  it('shows the icon and the formatted shortcut of an item', async () => {
    await openMenu(documentEntries(handlers()))
    const newItem = item(/^New/)
    expect(newItem).toHaveTextContent('Ctrl+N')
    expect(newItem.querySelector('svg')).toHaveAttribute('aria-hidden', 'true')
  })

  it('moves focus with ArrowDown and ArrowUp, skipping separators and wrapping', async () => {
    const { user } = await openMenu(documentEntries(handlers()))
    await user.keyboard('{ArrowDown}')
    expect(item('Save')).toHaveFocus()
    await user.keyboard('{ArrowDown}')
    expect(screen.getByRole('menuitemcheckbox', { name: 'Show constraints' })).toHaveFocus()
    await user.keyboard('{ArrowDown}')
    expect(item('Theme')).toHaveFocus()
    await user.keyboard('{ArrowDown}')
    expect(item(/^New/)).toHaveFocus()
    await user.keyboard('{ArrowUp}')
    expect(item('Theme')).toHaveFocus()
  })

  it('moves focus to the first and last item with Home and End', async () => {
    const { user } = await openMenu(documentEntries(handlers()))
    await user.keyboard('{End}')
    expect(item('Theme')).toHaveFocus()
    await user.keyboard('{Home}')
    expect(item(/^New/)).toHaveFocus()
  })

  it('Enter runs the focused item once, closes the menu and returns focus to its button', async () => {
    const on = handlers()
    const onOpenChange = vi.fn<(open: boolean) => void>()
    const { user, trigger } = await openMenu(documentEntries(on), onOpenChange)
    await user.keyboard('{ArrowDown}')
    await user.keyboard('{Enter}')
    expect(on.onSave).toHaveBeenCalledTimes(1)
    expect(on.onNew).not.toHaveBeenCalled()
    expect(onOpenChange).toHaveBeenCalledWith(false)
    expect(screen.queryByRole('menu')).toBeNull()
    expect(trigger).toHaveFocus()
  })

  it('Space toggles a checkbox item', async () => {
    const on = handlers()
    const { user } = await openMenu(documentEntries(on))
    await user.keyboard('{ArrowDown}{ArrowDown}')
    await user.keyboard(' ')
    expect(on.onConstraints).toHaveBeenCalledTimes(1)
  })

  it('a click runs the item and closes the menu', async () => {
    const on = handlers()
    const { user } = await openMenu(documentEntries(on))
    await user.click(item('Save'))
    expect(on.onSave).toHaveBeenCalledTimes(1)
    expect(screen.queryByRole('menu')).toBeNull()
  })

  it('Esc closes the menu without running anything and returns focus to its button', async () => {
    const on = handlers()
    const onOpenChange = vi.fn<(open: boolean) => void>()
    const { user, trigger } = await openMenu(documentEntries(on), onOpenChange)
    await user.keyboard('{Escape}')
    expect(onOpenChange).toHaveBeenCalledWith(false)
    expect(screen.queryByRole('menu')).toBeNull()
    expect(trigger).toHaveFocus()
    for (const handler of Object.values(on)) {
      expect(handler).not.toHaveBeenCalled()
    }
  })

  it('a pointer press outside closes the menu', async () => {
    const onOpenChange = vi.fn<(open: boolean) => void>()
    const { user } = await openMenu(documentEntries(handlers()), onOpenChange)
    await user.click(screen.getByRole('button', { name: 'Elsewhere' }))
    expect(onOpenChange).toHaveBeenCalledWith(false)
    expect(screen.queryByRole('menu')).toBeNull()
  })

  it('a disabled item is marked aria-disabled and does not run', async () => {
    const onExport = vi.fn<() => void>()
    const { user } = await openMenu([
      { kind: 'item', label: 'Save', onSelect: () => undefined },
      { kind: 'item', label: 'Export', disabled: true, onSelect: onExport },
    ])
    const exportItem = item('Export')
    expect(exportItem).toHaveAttribute('aria-disabled', 'true')
    await user.click(exportItem)
    expect(onExport).not.toHaveBeenCalled()
    expect(screen.getByRole('menu', { name: 'Document' })).toBeInTheDocument()
  })

  describe('submenus', () => {
    it('a submenu item announces its menu and is collapsed at first', async () => {
      await openMenu(documentEntries(handlers()))
      const theme = item('Theme')
      expect(theme).toHaveAttribute('aria-haspopup', 'menu')
      expect(theme).toHaveAttribute('aria-expanded', 'false')
      expect(screen.queryByRole('menu', { name: 'Theme' })).toBeNull()
    })

    it('ArrowRight opens the submenu with focus inside it', async () => {
      const { user } = await openMenu(documentEntries(handlers()))
      await user.keyboard('{End}{ArrowRight}')
      const submenu = screen.getByRole('menu', { name: 'Theme' })
      expect(item('Theme')).toHaveAttribute('aria-expanded', 'true')
      expect(submenu).toContainElement(focused())
      expect(within(submenu).getByRole('menuitemradio', { name: 'Light' })).toBeChecked()
      expect(within(submenu).getByRole('menuitemradio', { name: 'Dark' })).not.toBeChecked()
    })

    it('Enter on a submenu item opens the submenu', async () => {
      const { user } = await openMenu(documentEntries(handlers()))
      await user.keyboard('{End}{Enter}')
      expect(screen.getByRole('menu', { name: 'Theme' })).toBeInTheDocument()
    })

    it('ArrowLeft closes the submenu and focuses its item, keeping the menu open', async () => {
      const { user } = await openMenu(documentEntries(handlers()))
      await user.keyboard('{End}{ArrowRight}{ArrowLeft}')
      expect(screen.queryByRole('menu', { name: 'Theme' })).toBeNull()
      expect(screen.getByRole('menu', { name: 'Document' })).toBeInTheDocument()
      expect(item('Theme')).toHaveFocus()
    })

    it('Esc closes only the innermost menu', async () => {
      const { user, trigger } = await openMenu(documentEntries(handlers()))
      await user.keyboard('{End}{ArrowRight}{Escape}')
      expect(screen.queryByRole('menu', { name: 'Theme' })).toBeNull()
      expect(screen.getByRole('menu', { name: 'Document' })).toBeInTheDocument()
      expect(item('Theme')).toHaveFocus()
      await user.keyboard('{Escape}')
      expect(screen.queryByRole('menu')).toBeNull()
      expect(trigger).toHaveFocus()
    })

    it('choosing a submenu item runs it and closes every menu', async () => {
      const on = handlers()
      const { user, trigger } = await openMenu(documentEntries(on))
      await user.keyboard('{End}{ArrowRight}')
      await user.click(screen.getByRole('menuitemradio', { name: 'Dark' }))
      expect(on.onDark).toHaveBeenCalledTimes(1)
      expect(on.onLight).not.toHaveBeenCalled()
      expect(screen.queryByRole('menu')).toBeNull()
      expect(trigger).toHaveFocus()
    })
  })

  it('U-188 is a raised floating panel with the popup shape', async () => {
    const { menu } = await openMenu(documentEntries(handlers()))
    expect(menu).toHaveClass(COLOR_CLASSES.surfaceRaised.background)
    expect(menu).toHaveClass(COLOR_CLASSES.forcedEdge.border)
    expect(menu).toHaveClass(COLOR_CLASSES.elevation.shadow)
    expect(menu).toHaveClass('pointer-events-auto', 'border', 'rounded-lg', 'shadow-lg')
  })

  it('takes pointer input inside a pointer-events-none parent, whose setting the top layer keeps', async () => {
    const { menu } = await openMenu(documentEntries(handlers()))
    expect(menu).toHaveClass('pointer-events-auto')
  })

  it('is positioned at runtime next to its anchor', async () => {
    const { menu } = await openMenu(documentEntries(handlers()))
    expect(menu).toHaveAttribute('data-positioned')
  })
})

describe('Menu keys and pointer details', () => {
  it('a typed letter moves focus to the next item starting with it', async () => {
    const { user } = await openMenu(documentEntries(handlers()))
    await user.keyboard('s')
    expect(item('Save')).toHaveFocus()
    await user.keyboard('s')
    expect(screen.getByRole('menuitemcheckbox', { name: 'Show constraints' })).toHaveFocus()
    await user.keyboard('t')
    expect(item('Theme')).toHaveFocus()
  })

  it('Space on a checkbox item keeps the menu open; Enter closes it', async () => {
    const on = handlers()
    const onOpenChange = vi.fn<(open: boolean) => void>()
    const { user } = await openMenu(documentEntries(on), onOpenChange)
    await user.keyboard('{ArrowDown}{ArrowDown} ')
    expect(on.onConstraints).toHaveBeenCalledTimes(1)
    expect(onOpenChange).not.toHaveBeenCalledWith(false)
    expect(screen.getByRole('menu', { name: 'Document' })).toBeInTheDocument()
    await user.keyboard('{Enter}')
    expect(on.onConstraints).toHaveBeenCalledTimes(2)
    expect(screen.queryByRole('menu')).toBeNull()
  })

  it('a click on a checkbox item that keeps the menu open toggles it without closing (3.21)', async () => {
    const onKeep = vi.fn<() => void>()
    const onOpenChange = vi.fn<(open: boolean) => void>()
    const { user } = await openMenu(
      [
        { kind: 'radio', label: 'mm', checked: true, onSelect: vi.fn<() => void>() },
        { kind: 'checkbox', label: 'Keep', checked: true, keepsOpen: true, onSelect: onKeep },
      ],
      onOpenChange,
    )
    await user.click(screen.getByRole('menuitemcheckbox', { name: 'Keep' }))
    expect(onKeep).toHaveBeenCalledTimes(1)
    expect(onOpenChange).not.toHaveBeenCalledWith(false)
    await user.keyboard('{Enter}')
    expect(onKeep).toHaveBeenCalledTimes(2)
    expect(screen.getByRole('menu', { name: 'Document' })).toBeInTheDocument()
  })

  it("puts an entry's testId on its item", async () => {
    await openMenu([
      { kind: 'item', label: 'New', testId: 'new-item', onSelect: vi.fn<() => void>() },
    ])
    expect(screen.getByTestId('new-item')).toBe(item('New'))
  })

  it('Tab closes the menu', async () => {
    const onOpenChange = vi.fn<(open: boolean) => void>()
    const { user } = await openMenu(documentEntries(handlers()), onOpenChange)
    await user.tab()
    expect(onOpenChange).toHaveBeenCalledWith(false)
    expect(screen.queryByRole('menu')).toBeNull()
  })

  it('hovering a submenu item opens its submenu without moving focus into it', async () => {
    const { user } = await openMenu(documentEntries(handlers()))
    await user.hover(item('Theme'))
    const submenu = screen.getByRole('menu', { name: 'Theme' })
    expect(item('Theme')).toHaveFocus()
    expect(submenu).not.toContainElement(focused())
    await user.keyboard('{ArrowRight}')
    expect(submenu).toContainElement(focused())
  })

  it('a submenu item is the invoker of its open submenu, so clicking it is a press inside the submenu', async () => {
    const { user } = await openMenu(documentEntries(handlers()))
    await user.hover(item('Theme'))
    const submenu = screen.getByRole('menu', { name: 'Theme' })
    expect(item('Theme')).toHaveAttribute('popovertarget', submenu.id)
    expect(item('Theme')).toHaveAttribute('popovertargetaction', 'show')
  })

  it('hovering another item closes an open submenu', async () => {
    const { user } = await openMenu(documentEntries(handlers()))
    await user.hover(item('Theme'))
    expect(screen.getByRole('menu', { name: 'Theme' })).toBeInTheDocument()
    await user.hover(item('Save'))
    expect(screen.queryByRole('menu', { name: 'Theme' })).toBeNull()
    expect(item('Save')).toHaveFocus()
  })

  it('a disabled item can take focus but Enter does nothing', async () => {
    const onExport = vi.fn<() => void>()
    const { user } = await openMenu([
      { kind: 'item', label: 'Save', onSelect: () => undefined },
      { kind: 'item', label: 'Export', disabled: true, onSelect: onExport },
    ])
    await user.keyboard('{ArrowDown}')
    expect(item('Export')).toHaveFocus()
    await user.keyboard('{Enter}')
    expect(onExport).not.toHaveBeenCalled()
    expect(screen.getByRole('menu', { name: 'Document' })).toBeInTheDocument()
  })

  it('marks the focused item with the active background', async () => {
    const { user } = await openMenu(documentEntries(handlers()))
    expect(item(/^New/)).toHaveClass(COLOR_CLASSES.hover.active)
    await user.keyboard('{ArrowDown}')
    expect(item('Save')).toHaveClass(COLOR_CLASSES.hover.active)
    expect(item(/^New/)).not.toHaveClass(COLOR_CLASSES.hover.active)
  })

  it('U-213 outlines the keyboard-active item for forced colors and rests the others transparent', async () => {
    const { user } = await openMenu(documentEntries(handlers()))
    await user.keyboard('{ArrowDown}')
    expect(item('Save')).toHaveClass('outline', '-outline-offset-1')
    expect(item('Save')).toHaveClass(COLOR_CLASSES.forcedEdge.outline)
    expect(item(/^New/)).toHaveClass(COLOR_CLASSES.forcedEdge.outline)
    expect(item(/^New/)).not.toHaveClass('outline')
    expect(item(/^New/)).not.toHaveClass(COLOR_CLASSES.accent.outline)
  })
})
