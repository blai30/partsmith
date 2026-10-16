import { render, screen, within } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { COLOR_CLASSES } from '../theme/roles'
import { AxisLetter, AxisValues } from './AxisLetter'

// Axis letters and coordinate readouts (ui.md 3.7, 3.8): X, Y and Z in their axis colors, always
// as letters, so the color is never the only signal (3.24).

const AXIS_FOREGROUNDS = [
  COLOR_CLASSES.axisX.foreground,
  COLOR_CLASSES.axisY.foreground,
  COLOR_CLASSES.axisZ.foreground,
]

describe('AxisLetter', () => {
  it('shows a world axis letter in its axis role, semibold', () => {
    render(<AxisLetter axis="Y" />)
    const letter = screen.getByTestId('axis-letter')
    expect(letter).toHaveTextContent(/^Y$/)
    expect(letter).toHaveAttribute('data-axis', 'Y')
    expect(letter).toHaveAttribute('data-world', 'true')
    expect(letter).toHaveClass('font-semibold', COLOR_CLASSES.axisY.foreground)
    expect(letter).not.toHaveAttribute('aria-hidden')
  })

  it('mutes a letter that names an axis of a plane’s own frame', () => {
    render(<AxisLetter axis="X" world={false} />)
    const letter = screen.getByTestId('axis-letter')
    expect(letter).toHaveAttribute('data-world', 'false')
    expect(letter).toHaveClass(COLOR_CLASSES.textMuted.foreground)
    for (const foreground of AXIS_FOREGROUNDS) {
      expect(letter).not.toHaveClass(foreground)
    }
  })

  it('hides a decorative letter from assistive technology and keeps the layout classes it is given', () => {
    render(<AxisLetter axis="Z" decorative className="absolute left-0" />)
    const letter = screen.getByTestId('axis-letter')
    expect(letter).toHaveAttribute('aria-hidden', 'true')
    expect(letter).toHaveClass('absolute', 'left-0', 'font-semibold')
  })
})

/** A size readout of 40 by 20 by 10 mm. */
function renderReadout() {
  render(
    <AxisValues testId="body-size" values={['40', '20', '10']} unit="mm" text="40 × 20 × 10 mm" />,
  )
  return screen.getByTestId('body-size')
}

describe('AxisValues (U-182)', () => {
  it('reads each value after its letter, then the unit, with single spaces', () => {
    const readout = renderReadout()
    expect(readout.textContent).toBe('X 40 Y 20 Z 10 mm')
    expect(readout).toHaveAttribute('data-value', '40 × 20 × 10 mm')
  })

  it('wraps three equal cells that never shrink below their content', () => {
    const readout = renderReadout()
    expect(readout).toHaveClass(
      'flex',
      'w-full',
      'flex-wrap',
      'items-baseline',
      'gap-x-2',
      'gap-y-1',
    )
    const cells = [...readout.children].slice(0, 3)
    expect(cells).toHaveLength(3)
    for (const cell of cells) {
      expect(cell).toHaveClass('flex', 'flex-1', 'basis-0', 'items-baseline', 'gap-1')
    }
    const unit = readout.lastElementChild
    expect(unit).toHaveTextContent(/^mm$/)
    expect(unit).toHaveClass('shrink-0')
  })

  it('gives each cell a letter in its axis role and a number in tabular figures, not code', () => {
    const readout = renderReadout()
    const letters = within(readout).getAllByTestId('axis-letter')
    expect(letters.map((letter) => letter.textContent)).toEqual(['X', 'Y', 'Z'])
    letters.forEach((letter, index) => {
      expect(letter).toHaveAttribute('data-axis', letter.textContent)
      expect(letter).not.toHaveAttribute('aria-hidden')
      expect(letter).toHaveClass('font-semibold', AXIS_FOREGROUNDS[index] ?? '')
      const number = letter.parentElement?.lastElementChild
      expect(number).toHaveTextContent(/^\d+$/)
      expect(number).toHaveClass('tabular-nums')
      expect(number).not.toHaveClass('font-mono')
    })
  })
})
