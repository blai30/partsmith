import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { useRef, useState } from 'react'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { COLOR_CLASSES } from '../theme/roles'
import { Popover } from './Popover'
import { installPopoverPolyfill, isPopoverOpen } from './popover-polyfill.testing'

beforeEach(installPopoverPolyfill)

function Harness({ onOpenChange }: { readonly onOpenChange?: (open: boolean) => void }) {
  const anchor = useRef<HTMLButtonElement>(null)
  const [open, setOpen] = useState(false)
  return (
    <>
      <button ref={anchor} type="button" onClick={() => setOpen(true)}>
        Section view
      </button>
      <button type="button">Elsewhere</button>
      <Popover
        label="Section"
        open={open}
        anchor={anchor}
        onOpenChange={(next) => {
          onOpenChange?.(next)
          setOpen(next)
        }}
      >
        <button type="button">Flip</button>
      </Popover>
    </>
  )
}

function popoverElement(): HTMLElement {
  const element = document.querySelector<HTMLElement>('[popover]')
  if (element === null) {
    throw new Error('No element with a popover attribute.')
  }
  return element
}

describe('Popover', () => {
  it('stays in the DOM while closed, as a light-dismiss native popover', () => {
    render(<Harness />)
    const element = popoverElement()
    expect(element).toHaveAttribute('popover', 'auto')
    expect(isPopoverOpen(element)).toBe(false)
    expect(screen.queryByRole('dialog', { name: 'Section' })).toBeNull()
  })

  it('opens through the Popover API when open becomes true', async () => {
    render(<Harness />)
    await userEvent.click(screen.getByRole('button', { name: 'Section view' }))
    expect(isPopoverOpen(popoverElement())).toBe(true)
    expect(screen.getByRole('dialog', { name: 'Section' })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'Flip' })).toBeInTheDocument()
  })

  it('closes through the Popover API when open becomes false', () => {
    const anchor = { current: null }
    const { rerender } = render(
      <Popover label="Section" open anchor={anchor} onOpenChange={() => undefined}>
        <p>Content</p>
      </Popover>,
    )
    expect(isPopoverOpen(popoverElement())).toBe(true)
    rerender(
      <Popover label="Section" open={false} anchor={anchor} onOpenChange={() => undefined}>
        <p>Content</p>
      </Popover>,
    )
    expect(isPopoverOpen(popoverElement())).toBe(false)
  })

  it('reports a close on Esc and returns focus to the anchor', async () => {
    const onOpenChange = vi.fn<(open: boolean) => void>()
    const user = userEvent.setup()
    render(<Harness onOpenChange={onOpenChange} />)
    await user.click(screen.getByRole('button', { name: 'Section view' }))
    screen.getByRole('button', { name: 'Flip' }).focus()
    await user.keyboard('{Escape}')
    expect(onOpenChange).toHaveBeenCalledWith(false)
    expect(isPopoverOpen(popoverElement())).toBe(false)
    expect(screen.getByRole('button', { name: 'Section view' })).toHaveFocus()
  })

  it('reports a close when the browser light-dismisses it with a pointer press outside', async () => {
    const onOpenChange = vi.fn<(open: boolean) => void>()
    const user = userEvent.setup()
    render(<Harness onOpenChange={onOpenChange} />)
    await user.click(screen.getByRole('button', { name: 'Section view' }))
    await user.click(screen.getByRole('button', { name: 'Elsewhere' }))
    expect(onOpenChange).toHaveBeenCalledWith(false)
    expect(isPopoverOpen(popoverElement())).toBe(false)
  })

  it('stays open when its content is clicked', async () => {
    const onOpenChange = vi.fn<(open: boolean) => void>()
    const user = userEvent.setup()
    render(<Harness onOpenChange={onOpenChange} />)
    await user.click(screen.getByRole('button', { name: 'Section view' }))
    await user.click(screen.getByRole('button', { name: 'Flip' }))
    expect(onOpenChange).not.toHaveBeenCalledWith(false)
    expect(isPopoverOpen(popoverElement())).toBe(true)
  })

  it('U-188 is a raised floating panel with the popup shape', async () => {
    render(<Harness />)
    await userEvent.click(screen.getByRole('button', { name: 'Section view' }))
    const dialog = screen.getByRole('dialog', { name: 'Section' })
    expect(dialog).toHaveClass(COLOR_CLASSES.surfaceRaised.background)
    expect(dialog).toHaveClass(COLOR_CLASSES.forcedEdge.border)
    expect(dialog).toHaveClass(COLOR_CLASSES.elevation.shadow)
    expect(dialog).toHaveClass('pointer-events-auto', 'border', 'rounded-lg', 'shadow-lg')
  })

  it('takes pointer input inside a pointer-events-none parent, such as the sketch header slot', async () => {
    render(<Harness />)
    await userEvent.click(screen.getByRole('button', { name: 'Section view' }))
    expect(screen.getByRole('dialog', { name: 'Section' })).toHaveClass('pointer-events-auto')
  })

  it('puts layout classes on a wrapper of the content, never on the popover element', () => {
    // A display class on the popover element would override the browser's display: none while
    // it is closed.
    const anchor = { current: null }
    render(
      <Popover
        label="Section"
        open={false}
        anchor={anchor}
        onOpenChange={() => undefined}
        className="flex flex-col gap-3"
      >
        <p>Content</p>
      </Popover>,
    )
    const element = popoverElement()
    expect(element).not.toHaveClass('flex')
    expect(element).not.toHaveClass('flex-col')
    const wrapper = element.firstElementChild
    expect(wrapper).toHaveClass('flex', 'flex-col', 'gap-3')
    expect(wrapper).toContainElement(screen.getByText('Content'))
  })

  it('is positioned at runtime next to its anchor', async () => {
    render(<Harness />)
    await userEvent.click(screen.getByRole('button', { name: 'Section view' }))
    expect(screen.getByRole('dialog', { name: 'Section' })).toHaveAttribute('data-positioned')
  })
})

describe('Popover focus', () => {
  it('moves focus to its first focusable element when it opens', async () => {
    render(<Harness />)
    await userEvent.click(screen.getByRole('button', { name: 'Section view' }))
    expect(screen.getByRole('button', { name: 'Flip' })).toHaveFocus()
  })

  it('moves focus to an element marked data-autofocus first', () => {
    const anchor = { current: null }
    render(
      <Popover label="Color" open anchor={anchor} onOpenChange={() => undefined}>
        <button type="button">Default</button>
        <input aria-label="Hex" data-autofocus />
      </Popover>,
    )
    expect(screen.getByRole('textbox', { name: 'Hex' })).toHaveFocus()
  })
})
