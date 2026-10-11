import { cleanup, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { COLOR_CLASSES } from '../theme/roles'
import { Button } from './Button'

afterEach(() => {
  cleanup()
})

describe('Button', () => {
  it('renders a button named by its content that does not submit forms', () => {
    render(<Button>Save as</Button>)
    const button = screen.getByRole('button', { name: 'Save as' })
    expect(button).toHaveAttribute('type', 'button')
  })

  it('calls onClick when clicked', async () => {
    const onClick = vi.fn<() => void>()
    render(<Button onClick={onClick}>Undo</Button>)
    await userEvent.click(screen.getByRole('button', { name: 'Undo' }))
    expect(onClick).toHaveBeenCalledTimes(1)
  })

  it('activates with Enter and Space', async () => {
    const onClick = vi.fn<() => void>()
    const user = userEvent.setup()
    render(<Button onClick={onClick}>Undo</Button>)
    await user.tab()
    expect(screen.getByRole('button', { name: 'Undo' })).toHaveFocus()
    await user.keyboard('{Enter}')
    await user.keyboard(' ')
    expect(onClick).toHaveBeenCalledTimes(2)
  })

  it('does not call onClick while disabled', async () => {
    const onClick = vi.fn<() => void>()
    render(
      <Button disabled onClick={onClick}>
        Export
      </Button>,
    )
    const button = screen.getByRole('button', { name: 'Export' })
    expect(button).toBeDisabled()
    await userEvent.click(button)
    expect(onClick).not.toHaveBeenCalled()
  })

  it('passes other button attributes through', () => {
    render(
      <Button aria-disabled="true" data-testid="inspector-ok">
        OK
      </Button>,
    )
    expect(screen.getByTestId('inspector-ok')).toHaveAttribute('aria-disabled', 'true')
  })

  it('carries the focus ring', () => {
    render(<Button>Cancel</Button>)
    expect(screen.getByRole('button', { name: 'Cancel' })).toHaveClass(
      COLOR_CLASSES.focusRing.outline,
    )
  })

  it('primary uses the accent fill, its foreground and its hover', () => {
    render(<Button variant="primary">OK</Button>)
    const button = screen.getByRole('button', { name: 'OK' })
    expect(button).toHaveClass(COLOR_CLASSES.accent.background)
    expect(button).toHaveClass(COLOR_CLASSES.accent.foreground)
    expect(button).toHaveClass(COLOR_CLASSES.accentHover.background)
    expect(button).toHaveClass(COLOR_CLASSES.focusRing.outline)
  })

  it('the default variant has no accent fill', () => {
    render(<Button>Cancel</Button>)
    expect(screen.getByRole('button', { name: 'Cancel' })).not.toHaveClass(
      COLOR_CLASSES.accent.background,
    )
  })

  it('uses rounded-sm, never a pill shape', () => {
    render(<Button variant="primary">OK</Button>)
    const button = screen.getByRole('button', { name: 'OK' })
    expect(button).toHaveClass('rounded-sm')
    expect(button).not.toHaveClass('rounded-full')
  })

  it('adds the caller className', () => {
    render(<Button className="w-full">Export</Button>)
    expect(screen.getByRole('button', { name: 'Export' })).toHaveClass('w-full')
  })
})
