import { cleanup, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { COLOR_CLASSES } from '../theme/roles'
import { SegmentedControl } from './SegmentedControl'

type Context = 'model' | 'sketch' | 'print'

const OPTIONS: readonly { readonly value: Context; readonly label: string }[] = [
  { value: 'model', label: 'Model' },
  { value: 'sketch', label: 'Sketch' },
  { value: 'print', label: 'Print' },
]

const PLANES: readonly { readonly value: 'XY' | 'XZ' | 'YZ'; readonly label: string }[] = [
  { value: 'XY', label: 'XY' },
  { value: 'XZ', label: 'XZ' },
  { value: 'YZ', label: 'YZ' },
]

afterEach(() => {
  cleanup()
})

// The visible segment of a radio: its label when it is a native input inside one, else itself.
function segment(radio: HTMLElement): HTMLElement {
  return radio.closest('label') ?? radio
}

function setUp(value: Context, onChange: (value: Context) => void = () => undefined) {
  render(
    <>
      <button type="button">Before</button>
      <SegmentedControl label="Context" options={OPTIONS} value={value} onChange={onChange} />
      <button type="button">After</button>
    </>,
  )
  return {
    user: userEvent.setup(),
    radio: (name: string) => screen.getByRole('radio', { name }),
  }
}

describe('SegmentedControl', () => {
  it('is a radio group named by its label with one radio per option', () => {
    const { radio } = setUp('model')
    expect(screen.getByRole('radiogroup', { name: 'Context' })).toBeInTheDocument()
    expect(screen.getAllByRole('radio')).toHaveLength(3)
    expect(radio('Model')).toBeChecked()
    expect(radio('Sketch')).not.toBeChecked()
    expect(radio('Print')).not.toBeChecked()
  })

  it('a click chooses an option', async () => {
    const onChange = vi.fn<(value: Context) => void>()
    const { user, radio } = setUp('model', onChange)
    await user.click(radio('Print'))
    expect(onChange).toHaveBeenCalledWith('print')
  })

  it('has one tab stop, on the checked option', async () => {
    const { user, radio } = setUp('sketch')
    await user.tab()
    await user.tab()
    expect(radio('Sketch')).toHaveFocus()
    await user.tab()
    expect(screen.getByRole('button', { name: 'After' })).toHaveFocus()
  })

  it('arrow keys move focus between the options', async () => {
    const { user, radio } = setUp('model')
    await user.tab()
    await user.tab()
    expect(radio('Model')).toHaveFocus()
    await user.keyboard('{ArrowRight}')
    expect(radio('Sketch')).toHaveFocus()
    await user.keyboard('{ArrowRight}')
    expect(radio('Print')).toHaveFocus()
    await user.keyboard('{ArrowLeft}')
    expect(radio('Sketch')).toHaveFocus()
  })

  it('Home and End move focus to the first and last option', async () => {
    const { user, radio } = setUp('sketch')
    await user.tab()
    await user.tab()
    await user.keyboard('{End}')
    expect(radio('Print')).toHaveFocus()
    await user.keyboard('{Home}')
    expect(radio('Model')).toHaveFocus()
  })

  it('Space chooses the focused option', async () => {
    const onChange = vi.fn<(value: Context) => void>()
    const { user } = setUp('model', onChange)
    await user.tab()
    await user.tab()
    await user.keyboard('{ArrowRight}')
    await user.keyboard(' ')
    expect(onChange).toHaveBeenLastCalledWith('sketch')
  })

  it('shows the checked segment with the selection background', () => {
    const { radio } = setUp('print')
    expect(segment(radio('Print'))).toHaveClass(COLOR_CLASSES.selection.background)
    expect(segment(radio('Model'))).not.toHaveClass(COLOR_CLASSES.selection.background)
  })

  it('follows the value prop', () => {
    const { rerender } = render(
      <SegmentedControl label="Plane" options={PLANES} value="XY" onChange={() => undefined} />,
    )
    expect(screen.getByRole('radio', { name: 'XY' })).toBeChecked()
    rerender(
      <SegmentedControl label="Plane" options={PLANES} value="YZ" onChange={() => undefined} />,
    )
    expect(screen.getByRole('radio', { name: 'YZ' })).toBeChecked()
    expect(screen.getByRole('radio', { name: 'XY' })).not.toBeChecked()
  })
})
