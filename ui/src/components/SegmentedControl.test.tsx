import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it, vi } from 'vitest'
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

const LETTERS: readonly { readonly value: 'A' | 'B' | 'C'; readonly label: string }[] = [
  { value: 'A', label: 'A' },
  { value: 'B', label: 'B' },
  { value: 'C', label: 'C' },
]

/** The classes of a role facet, one by one. */
function classesOf(classes: string): string[] {
  return classes.split(' ')
}

/** Whether an element has every class of a role facet. */
function hasFacet(element: HTMLElement, classes: string): boolean {
  return classesOf(classes).every((name) => element.classList.contains(name))
}

/** The label of the segment named `name`. */
function label(name: string): HTMLElement {
  return segment(screen.getByRole('radio', { name }))
}

/** The thumb's classes a label lacks, and `selection` when it has it (3.23). */
function thumbDefects(element: HTMLElement): string[] {
  const required = [
    'rounded-sm',
    'shadow-xs',
    'font-semibold',
    'outline',
    '-outline-offset-1',
    ...classesOf(COLOR_CLASSES.thumb.background),
    ...classesOf(COLOR_CLASSES.elevation.shadow),
    ...classesOf(COLOR_CLASSES.forcedEdge.outline),
  ]
  const missing = required.filter((name) => !element.classList.contains(name))
  return hasFacet(element, COLOR_CLASSES.selection.background) ? [...missing, 'selection'] : missing
}

/** The unchecked classes a label lacks, and the outline, fill and shadow classes it has. */
function uncheckedDefects(element: HTMLElement): string[] {
  const required = [
    ...classesOf(COLOR_CLASSES.forcedEdge.outline),
    ...classesOf(COLOR_CLASSES.textMuted.foreground),
    ...classesOf(COLOR_CLASSES.hover.foreground),
  ]
  const missing = required.filter((name) => !element.classList.contains(name))
  const unexpected = [...element.classList].filter(
    (name) =>
      name === 'outline' ||
      name === '-outline-offset-1' ||
      name.includes('bg-') ||
      name.includes('shadow-'),
  )
  return [...missing, ...unexpected]
}

describe('segmented control as a tonal track with a raised thumb (U-210)', () => {
  it('U-210 draws the track in tonal with a forcedEdge border, not on field', () => {
    render(<SegmentedControl label="Letter" options={LETTERS} value="B" onChange={() => {}} />)
    const group = screen.getByRole('radiogroup', { name: 'Letter' })
    expect(group).toHaveClass('h-8', 'rounded-md', 'border', 'p-0.5')
    expect(group).toHaveClass(...classesOf(COLOR_CLASSES.tonal.background))
    expect(group).toHaveClass(...classesOf(COLOR_CLASSES.forcedEdge.border))
    expect(hasFacet(group, COLOR_CLASSES.field.background)).toBe(false)
  })

  it('U-210 raises the checked segment as the thumb, semibold in text, with the state outline', () => {
    render(<SegmentedControl label="Letter" options={LETTERS} value="B" onChange={() => {}} />)
    expect(thumbDefects(label('B'))).toEqual([])
    expect(label('B')).toHaveClass(...classesOf(COLOR_CLASSES.text.foreground))
    expect(uncheckedDefects(label('A'))).toEqual([])
    expect(uncheckedDefects(label('C'))).toEqual([])
  })

  it('U-210 choosing C moves the thumb to C and gives B the unchecked classes', () => {
    const { rerender } = render(
      <SegmentedControl label="Letter" options={LETTERS} value="B" onChange={() => {}} />,
    )
    rerender(<SegmentedControl label="Letter" options={LETTERS} value="C" onChange={() => {}} />)
    expect(thumbDefects(label('C'))).toEqual([])
    expect(uncheckedDefects(label('B'))).toEqual([])
    expect(uncheckedDefects(label('A'))).toEqual([])
  })

  it('U-210 keeps the focus ring on every radio', () => {
    render(<SegmentedControl label="Letter" options={LETTERS} value="B" onChange={() => {}} />)
    for (const radio of screen.getAllByRole('radio')) {
      expect(radio).toHaveClass(...classesOf(COLOR_CLASSES.focusRing.outline))
    }
  })

  it('U-210 a disabled control keeps the thumb and its semibold label in textMuted, the others in textDisabled, with no hover', () => {
    render(
      <SegmentedControl disabled label="Letter" options={LETTERS} value="B" onChange={() => {}} />,
    )
    expect(thumbDefects(label('B'))).toEqual([])
    expect(label('B')).toHaveClass(...classesOf(COLOR_CLASSES.textMuted.foreground))
    for (const name of ['A', 'C']) {
      expect(label(name)).toHaveClass(...classesOf(COLOR_CLASSES.textDisabled.foreground))
    }
    for (const name of ['A', 'B', 'C']) {
      expect(hasFacet(label(name), COLOR_CLASSES.hover.foreground)).toBe(false)
    }
  })
})
