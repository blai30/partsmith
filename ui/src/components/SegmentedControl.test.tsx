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

  it('sizes each segment to its label by default', () => {
    const { radio } = setUp('model')
    expect(screen.getByRole('radiogroup')).toHaveClass('inline-flex', 'shrink-0')
    expect(segment(radio('Model'))).toHaveClass('px-3')
    expect(segment(radio('Model'))).not.toHaveClass('flex-1')
  })

  it('stretched, spans the rest of its row and shares the width between the segments', () => {
    render(
      <SegmentedControl stretch label="Plane" options={PLANES} value="XY" onChange={() => {}} />,
    )
    const group = screen.getByRole('radiogroup', { name: 'Plane' })
    expect(group).toHaveClass('flex', 'flex-1')
    expect(group).not.toHaveClass('inline-flex', 'shrink-0')
    for (const radio of screen.getAllByRole('radio')) {
      expect(segment(radio)).toHaveClass('flex-1', 'justify-center', 'px-2')
    }
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

describe('SegmentedControl keys and states', () => {
  it('arrow keys, Home and End only move focus, without choosing', async () => {
    const onChange = vi.fn<(value: Context) => void>()
    const { user, radio } = setUp('model', onChange)
    await user.tab()
    await user.tab()
    await user.keyboard('{ArrowRight}{End}{Home}{ArrowLeft}')
    expect(radio('Print')).toHaveFocus()
    expect(onChange).not.toHaveBeenCalled()
    expect(radio('Model')).toBeChecked()
  })

  it('clicking the checked option does not report a change', async () => {
    const onChange = vi.fn<(value: Context) => void>()
    const { user, radio } = setUp('model', onChange)
    await user.click(radio('Model'))
    expect(onChange).not.toHaveBeenCalled()
  })

  it('a disabled control does not change', async () => {
    const onChange = vi.fn<(value: Context) => void>()
    render(
      <SegmentedControl
        label="Context"
        options={OPTIONS}
        value="model"
        disabled
        onChange={onChange}
      />,
    )
    const print = screen.getByRole('radio', { name: 'Print' })
    expect(print).toBeDisabled()
    await userEvent.click(print)
    expect(onChange).not.toHaveBeenCalled()
  })
})

describe('SegmentedControl option tooltips', () => {
  it('describes the radio of an option with a tooltip and leaves the others alone', () => {
    render(
      <SegmentedControl
        label="Context"
        options={[
          { value: 'model', label: 'Model' },
          { value: 'sketch', label: 'Sketch', tooltip: 'Edit Sketch 1' },
          { value: 'print', label: 'Print', tooltip: null },
        ]}
        value="model"
        onChange={() => undefined}
      />,
    )
    expect(screen.getByRole('radio', { name: 'Sketch' })).toHaveAccessibleDescription(
      'Edit Sketch 1',
    )
    expect(screen.getByRole('radio', { name: 'Model' })).not.toHaveAccessibleDescription()
    expect(screen.getByRole('radio', { name: 'Print' })).not.toHaveAccessibleDescription()
  })

  it('moves focus over options with and without tooltips', async () => {
    render(
      <SegmentedControl
        label="Context"
        options={[
          { value: 'model', label: 'Model' },
          { value: 'sketch', label: 'Sketch', tooltip: 'New sketch' },
          { value: 'print', label: 'Print' },
        ]}
        value="model"
        onChange={() => undefined}
      />,
    )
    const user = userEvent.setup()
    screen.getByRole('radio', { name: 'Model' }).focus()
    await user.keyboard('{ArrowRight}')
    expect(screen.getByRole('radio', { name: 'Sketch' })).toHaveFocus()
    await user.keyboard('{ArrowRight}')
    expect(screen.getByRole('radio', { name: 'Print' })).toHaveFocus()
  })
})
