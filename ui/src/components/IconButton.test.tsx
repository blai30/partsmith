import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { Maximize, Moon } from 'lucide-react'
import { describe, expect, it, vi } from 'vitest'
import { COLOR_CLASSES } from '../theme/roles'
import { IconButton } from './IconButton'

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

  it('U-213 uses the selection background and the state outline when pressed, which do not change on hover', () => {
    render(<IconButton label="Dark theme" icon={Moon} pressed onClick={() => undefined} />)
    const button = screen.getByRole('button', { name: 'Dark theme' })
    expect(button).toHaveClass(COLOR_CLASSES.selection.background)
    expect(button).toHaveClass('outline', '-outline-offset-1', COLOR_CLASSES.forcedEdge.outline)
    expect(button).not.toHaveClass(COLOR_CLASSES.hover.background)
    expect(button).not.toHaveClass(COLOR_CLASSES.accent.outline)
  })

  it('U-213 uses the neutral active fill when pressed with pressedFill neutral, a toggle of what the window shows', () => {
    render(
      <IconButton
        label="Toggle browser"
        icon={Maximize}
        pressed
        pressedFill="neutral"
        onClick={() => undefined}
      />,
    )
    const button = screen.getByRole('button', { name: 'Toggle browser' })
    expect(button).toHaveClass(...COLOR_CLASSES.hover.active.split(' '))
    expect(button).toHaveClass(...COLOR_CLASSES.text.foreground.split(' '))
    expect(button).toHaveClass('outline', '-outline-offset-1', COLOR_CLASSES.forcedEdge.outline)
    expect(button).not.toHaveClass(COLOR_CLASSES.selection.background)
    expect(button).not.toHaveClass(COLOR_CLASSES.hover.background)
  })

  it('U-213 uses the hover background when not pressed, resting with the transparent outline', () => {
    render(<IconButton label="Dark theme" icon={Moon} pressed={false} onClick={() => undefined} />)
    const button = screen.getByRole('button', { name: 'Dark theme' })
    expect(button).toHaveClass(COLOR_CLASSES.hover.background)
    expect(button).toHaveClass(COLOR_CLASSES.forcedEdge.outline)
    expect(button).not.toHaveClass('outline')
    expect(button).not.toHaveClass(COLOR_CLASSES.accent.outline)
    expect(button).not.toHaveClass(COLOR_CLASSES.selection.background)
  })

  it('a plain icon button rests with the accent outline color of its focus ring', () => {
    render(<IconButton label="Fit to view" icon={Maximize} onClick={() => undefined} />)
    const button = screen.getByRole('button', { name: 'Fit to view' })
    expect(button).toHaveClass(COLOR_CLASSES.accent.outline)
    expect(button).not.toHaveClass(COLOR_CLASSES.forcedEdge.outline)
  })

  it('U-188 carries the focus ring and is rounded-md, never pill-shaped', () => {
    render(<IconButton label="Fit to view" icon={Maximize} onClick={() => undefined} />)
    const button = screen.getByRole('button', { name: 'Fit to view' })
    expect(button).toHaveClass(COLOR_CLASSES.focusRing.outline, 'rounded-md')
    expect(button).not.toHaveClass('rounded-full')
  })

  it('draws its focus ring inside the button in a slim island (3.23)', () => {
    render(<IconButton label="Expand timeline" icon={Maximize} focusRing="inset" />)
    const button = screen.getByRole('button', { name: 'Expand timeline' })
    expect(button).toHaveClass(COLOR_CLASSES.focusRing.inset)
    expect(button).not.toHaveClass('focus-visible:outline-offset-2')
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
