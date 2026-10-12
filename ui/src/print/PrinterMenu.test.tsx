import { cleanup, render, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { useRef, useState } from 'react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { installPopoverPolyfill } from '../components/popover-polyfill.testing'
import type { PrinterProfile } from '../generated/protocol'
import { PrinterMenu } from './PrinterMenu'
import { GENERIC_FDM } from '../test/fakes/printers'
import { PRINTERS } from './fixtures.testing'

let uninstallPopovers: () => void

beforeEach(() => {
  uninstallPopovers = installPopoverPolyfill()
})

afterEach(() => {
  cleanup()
  uninstallPopovers()
})

type HarnessProps = {
  readonly printers?: readonly PrinterProfile[]
  readonly current: string | null
  readonly onChoose: (profileId: string | null) => void
  readonly onOpenChange?: (open: boolean) => void
}

// The menu under the toolbar's printer button, as the Print toolbar and the status bar use it.
function Harness({ printers = PRINTERS, current, onChoose, onOpenChange }: HarnessProps) {
  const anchor = useRef<HTMLButtonElement>(null)
  const [open, setOpen] = useState(false)
  return (
    <>
      <button ref={anchor} type="button" onClick={() => setOpen(true)}>
        Printer: none
      </button>
      <PrinterMenu
        open={open}
        anchor={anchor}
        onOpenChange={(next) => {
          onOpenChange?.(next)
          setOpen(next)
        }}
        printers={printers}
        current={current}
        onChoose={onChoose}
      />
    </>
  )
}

async function openMenu(props: Partial<HarnessProps> = {}) {
  const onChoose = vi.fn<(profileId: string | null) => void>()
  const onOpenChange = vi.fn<(open: boolean) => void>()
  render(<Harness current={null} onChoose={onChoose} onOpenChange={onOpenChange} {...props} />)
  await userEvent.click(screen.getByRole('button', { name: 'Printer: none' }))
  return { menu: screen.getByTestId('printer-menu'), onChoose, onOpenChange }
}

function radio(container: HTMLElement, name: string): HTMLElement {
  return within(container).getByRole('menuitemradio', { name })
}

describe('PrinterMenu', () => {
  it('is not shown while closed', () => {
    render(<Harness current={null} onChoose={vi.fn<(profileId: string | null) => void>()} />)
    expect(screen.queryByRole('menu')).toBeNull()
  })

  it('opens as the menu printer-menu', async () => {
    const { menu } = await openMenu()
    expect(screen.getByRole('menu')).toBe(menu)
  })

  it('lists the FDM profiles under the heading "FDM" and the resin profiles under "Resin"', async () => {
    const { menu } = await openMenu()
    const fdm = within(menu).getByRole('group', { name: 'FDM' })
    const resin = within(menu).getByRole('group', { name: 'Resin' })
    expect(within(fdm).getAllByRole('menuitemradio')).toEqual([
      radio(fdm, 'Generic FDM 0.4 mm'),
      radio(fdm, 'My MK4'),
    ])
    expect(within(resin).getAllByRole('menuitemradio')).toEqual([
      radio(resin, 'Generic resin MSLA'),
    ])
  })

  it('offers "No printer" after the profiles', async () => {
    const { menu } = await openMenu()
    expect(within(menu).getAllByRole('menuitemradio')).toEqual([
      radio(menu, 'Generic FDM 0.4 mm'),
      radio(menu, 'My MK4'),
      radio(menu, 'Generic resin MSLA'),
      radio(menu, 'No printer'),
    ])
  })

  it('leaves out a heading without profiles', async () => {
    const { menu } = await openMenu({ printers: [GENERIC_FDM] })
    expect(within(menu).getByRole('group', { name: 'FDM' })).toBeInTheDocument()
    expect(within(menu).queryByRole('group', { name: 'Resin' })).toBeNull()
  })

  it("checks the document's printer", async () => {
    const { menu } = await openMenu({ current: 'my-mk4' })
    expect(radio(menu, 'My MK4')).toBeChecked()
    expect(radio(menu, 'Generic FDM 0.4 mm')).not.toBeChecked()
    expect(radio(menu, 'Generic resin MSLA')).not.toBeChecked()
    expect(radio(menu, 'No printer')).not.toBeChecked()
  })

  it('checks "No printer" when the document has none', async () => {
    const { menu } = await openMenu({ current: null })
    expect(radio(menu, 'No printer')).toBeChecked()
    expect(radio(menu, 'Generic FDM 0.4 mm')).not.toBeChecked()
  })

  it('chooses a profile by its id and closes', async () => {
    const { menu, onChoose, onOpenChange } = await openMenu()
    await userEvent.click(radio(menu, 'Generic resin MSLA'))
    expect(onChoose).toHaveBeenCalledTimes(1)
    expect(onChoose).toHaveBeenCalledWith('generic-resin')
    expect(onOpenChange).toHaveBeenLastCalledWith(false)
    expect(screen.queryByRole('menu')).toBeNull()
  })

  it('chooses null for "No printer"', async () => {
    const { menu, onChoose } = await openMenu({ current: 'generic-fdm' })
    await userEvent.click(radio(menu, 'No printer'))
    expect(onChoose).toHaveBeenCalledWith(null)
  })

  it('focuses the first profile on opening, moves with the arrow keys and chooses with Enter', async () => {
    const { menu, onChoose } = await openMenu()
    expect(radio(menu, 'Generic FDM 0.4 mm')).toHaveFocus()
    await userEvent.keyboard('{ArrowDown}')
    expect(radio(menu, 'My MK4')).toHaveFocus()
    await userEvent.keyboard('{ArrowDown}')
    expect(radio(menu, 'Generic resin MSLA')).toHaveFocus()
    await userEvent.keyboard('{Enter}')
    expect(onChoose).toHaveBeenCalledWith('generic-resin')
  })

  it('closes on Escape and returns focus to its button', async () => {
    const { onChoose, onOpenChange } = await openMenu()
    await userEvent.keyboard('{Escape}')
    expect(onOpenChange).toHaveBeenLastCalledWith(false)
    expect(screen.queryByRole('menu')).toBeNull()
    expect(screen.getByRole('button', { name: 'Printer: none' })).toHaveFocus()
    expect(onChoose).not.toHaveBeenCalled()
  })
})

describe('PrinterMenu keyboard and dismissal', () => {
  it('checks a profile with Space and stays open', async () => {
    const { menu, onChoose } = await openMenu()
    await userEvent.keyboard('{ArrowDown}')
    await userEvent.keyboard(' ')
    expect(onChoose).toHaveBeenCalledWith('my-mk4')
    expect(screen.getByRole('menu')).toBe(menu)
  })

  it('wraps around with the arrow keys and jumps with Home and End', async () => {
    const { menu } = await openMenu()
    await userEvent.keyboard('{ArrowUp}')
    expect(radio(menu, 'No printer')).toHaveFocus()
    await userEvent.keyboard('{ArrowDown}')
    expect(radio(menu, 'Generic FDM 0.4 mm')).toHaveFocus()
    await userEvent.keyboard('{End}')
    expect(radio(menu, 'No printer')).toHaveFocus()
    await userEvent.keyboard('{Home}')
    expect(radio(menu, 'Generic FDM 0.4 mm')).toHaveFocus()
  })

  it('moves to the next item starting with a typed letter', async () => {
    const { menu } = await openMenu()
    await userEvent.keyboard('m')
    expect(radio(menu, 'My MK4')).toHaveFocus()
    await userEvent.keyboard('n')
    expect(radio(menu, 'No printer')).toHaveFocus()
  })

  it('closes on Tab without choosing', async () => {
    const { onChoose, onOpenChange } = await openMenu()
    await userEvent.keyboard('{Tab}')
    expect(onOpenChange).toHaveBeenLastCalledWith(false)
    expect(screen.queryByRole('menu')).toBeNull()
    expect(onChoose).not.toHaveBeenCalled()
  })

  it('closes on a press outside it', async () => {
    const { onChoose, onOpenChange } = await openMenu()
    await userEvent.click(document.body)
    expect(onOpenChange).toHaveBeenLastCalledWith(false)
    expect(screen.queryByRole('menu')).toBeNull()
    expect(onChoose).not.toHaveBeenCalled()
  })

  it('offers only "No printer" without profiles', async () => {
    const { menu } = await openMenu({ printers: [] })
    expect(within(menu).getAllByRole('menuitemradio')).toEqual([radio(menu, 'No printer')])
    expect(within(menu).queryByRole('group')).toBeNull()
  })
})
