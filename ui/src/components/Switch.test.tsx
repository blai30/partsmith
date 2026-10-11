import { cleanup, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { COLOR_CLASSES } from '../theme/roles'
import { Switch } from './Switch'

afterEach(() => {
  cleanup()
})

describe('Switch', () => {
  it('is a switch named by its label that reflects checked', () => {
    const { rerender } = render(<Switch label="Flip" checked={false} onChange={() => undefined} />)
    expect(screen.getByRole('switch', { name: 'Flip' })).not.toBeChecked()
    rerender(<Switch label="Flip" checked onChange={() => undefined} />)
    expect(screen.getByRole('switch', { name: 'Flip' })).toBeChecked()
  })

  it('a click asks for the opposite state', async () => {
    const onChange = vi.fn<(checked: boolean) => void>()
    const { rerender } = render(<Switch label="Flip" checked={false} onChange={onChange} />)
    await userEvent.click(screen.getByRole('switch', { name: 'Flip' }))
    expect(onChange).toHaveBeenLastCalledWith(true)
    rerender(<Switch label="Flip" checked onChange={onChange} />)
    await userEvent.click(screen.getByRole('switch', { name: 'Flip' }))
    expect(onChange).toHaveBeenLastCalledWith(false)
    expect(onChange).toHaveBeenCalledTimes(2)
  })

  it('is controlled: it shows the checked prop, not its own state', async () => {
    render(<Switch label="Flip" checked={false} onChange={() => undefined} />)
    await userEvent.click(screen.getByRole('switch', { name: 'Flip' }))
    expect(screen.getByRole('switch', { name: 'Flip' })).not.toBeChecked()
  })

  it('toggles with Space from the keyboard', async () => {
    const onChange = vi.fn<(checked: boolean) => void>()
    const user = userEvent.setup()
    render(<Switch label="Visible" checked onChange={onChange} />)
    await user.tab()
    expect(screen.getByRole('switch', { name: 'Visible' })).toHaveFocus()
    await user.keyboard(' ')
    expect(onChange).toHaveBeenCalledWith(false)
  })

  it('a disabled switch does not change', async () => {
    const onChange = vi.fn<(checked: boolean) => void>()
    render(<Switch label="Mesh validity" checked disabled onChange={onChange} />)
    const control = screen.getByRole('switch', { name: 'Mesh validity' })
    expect(
      control.hasAttribute('disabled') || control.getAttribute('aria-disabled') === 'true',
    ).toBe(true)
    await userEvent.click(control)
    expect(onChange).not.toHaveBeenCalled()
  })

  it('exposes its help text as the accessible description', () => {
    render(
      <Switch
        label="Mesh validity"
        description="Always checked before export"
        checked
        disabled
        onChange={() => undefined}
      />,
    )
    expect(screen.getByRole('switch', { name: 'Mesh validity' })).toHaveAccessibleDescription(
      'Always checked before export',
    )
    expect(screen.getByText('Always checked before export')).toBeInTheDocument()
  })

  it('draws its boundary with the control border role', () => {
    const { container } = render(<Switch label="Flip" checked={false} onChange={() => undefined} />)
    const bordered = container.getElementsByClassName(COLOR_CLASSES.borderControl.border)
    expect(bordered.length).toBeGreaterThan(0)
  })
})
