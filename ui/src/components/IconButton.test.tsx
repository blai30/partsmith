import { cleanup, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { Maximize, Moon } from 'lucide-react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { COLOR_CLASSES } from '../theme/roles'
import { IconButton } from './IconButton'

afterEach(() => {
  cleanup()
})

describe('IconButton', () => {
  it('is a button named by its label', () => {
    render(<IconButton label="Fit to view" icon={Maximize} onClick={() => undefined} />)
    const button = screen.getByRole('button', { name: 'Fit to view' })
    expect(button).toHaveAttribute('type', 'button')
  })

  it('shows its icon as a decorative lucide line icon with stroke width 1.75', () => {
    render(<IconButton label="Fit to view" icon={Maximize} onClick={() => undefined} />)
    const icon = screen.getByRole('button', { name: 'Fit to view' }).querySelector('svg')
    expect(icon).not.toBeNull()
    expect(icon).toHaveAttribute('aria-hidden', 'true')
    expect(icon).toHaveAttribute('stroke-width', '1.75')
  })

  it('calls onClick when clicked', async () => {
    const onClick = vi.fn<() => void>()
    render(<IconButton label="Fit to view" icon={Maximize} onClick={onClick} />)
    await userEvent.click(screen.getByRole('button', { name: 'Fit to view' }))
    expect(onClick).toHaveBeenCalledTimes(1)
  })

  it('has no aria-pressed when it is not a toggle', () => {
    render(<IconButton label="Fit to view" icon={Maximize} onClick={() => undefined} />)
    expect(screen.getByRole('button', { name: 'Fit to view' })).not.toHaveAttribute('aria-pressed')
  })

  it('reflects a toggle state in aria-pressed', () => {
    const { rerender } = render(
      <IconButton label="Dark theme" icon={Moon} pressed={false} onClick={() => undefined} />,
    )
    expect(screen.getByRole('button', { name: 'Dark theme' })).toHaveAttribute(
      'aria-pressed',
      'false',
    )
    rerender(<IconButton label="Dark theme" icon={Moon} pressed onClick={() => undefined} />)
    expect(screen.getByRole('button', { name: 'Dark theme' })).toHaveAttribute(
      'aria-pressed',
      'true',
    )
  })

  it('uses the selection background when pressed, which does not change on hover', () => {
    render(<IconButton label="Dark theme" icon={Moon} pressed onClick={() => undefined} />)
    const button = screen.getByRole('button', { name: 'Dark theme' })
    expect(button).toHaveClass(COLOR_CLASSES.selection.background)
    expect(button).not.toHaveClass(COLOR_CLASSES.hover.background)
  })

  it('uses the hover background when not pressed', () => {
    render(<IconButton label="Dark theme" icon={Moon} pressed={false} onClick={() => undefined} />)
    const button = screen.getByRole('button', { name: 'Dark theme' })
    expect(button).toHaveClass(COLOR_CLASSES.hover.background)
    expect(button).not.toHaveClass(COLOR_CLASSES.selection.background)
  })

  it('carries the focus ring and is never pill-shaped', () => {
    render(<IconButton label="Fit to view" icon={Maximize} onClick={() => undefined} />)
    const button = screen.getByRole('button', { name: 'Fit to view' })
    expect(button).toHaveClass(COLOR_CLASSES.focusRing.outline)
    expect(button).not.toHaveClass('rounded-full')
  })

  it('passes other button attributes through', () => {
    render(
      <IconButton
        label="Dark theme"
        icon={Moon}
        onClick={() => undefined}
        data-testid="theme-toggle"
      />,
    )
    expect(screen.getByTestId('theme-toggle')).toHaveAccessibleName('Dark theme')
  })
})
