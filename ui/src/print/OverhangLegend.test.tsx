import { render, screen, within } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { COLOR_CLASSES } from '../theme/roles'
import { OverhangLegend } from './OverhangLegend'

function renderLegend(limit: number): HTMLElement {
  render(<OverhangLegend limit={limit} />)
  return screen.getByTestId('overhang-legend')
}

describe('OverhangLegend', () => {
  it('is titled "Overhangs"', () => {
    const legend = renderLegend(Math.PI / 4)
    expect(within(legend).getByText('Overhangs')).toBeInTheDocument()
  })

  it('labels the ends of the ramp with the limit and 90°', () => {
    const legend = renderLegend(Math.PI / 4)
    expect(within(legend).getByText('45°')).toBeInTheDocument()
    expect(within(legend).getByText('90°')).toBeInTheDocument()
  })

  it('explains the colors with the limit', () => {
    const legend = renderLegend(Math.PI / 4)
    expect(legend).toHaveTextContent('Faces steeper than 45° from vertical need support.')
  })

  it('formats the limit in degrees with formatNumber', () => {
    const legend = renderLegend(0.7)
    expect(within(legend).getByText('40.11°')).toBeInTheDocument()
    expect(legend).toHaveTextContent('Faces steeper than 40.11° from vertical need support.')
  })

  it('draws an h-2 bar with the analysis gradient', () => {
    const legend = renderLegend(Math.PI / 6)
    const gradient = COLOR_CLASSES.analysis.background.split(' ')
    const bars = Array.from(legend.querySelectorAll('*')).filter(
      (element) =>
        element.classList.contains('h-2') &&
        gradient.every((name) => element.classList.contains(name)),
    )
    expect(bars).toHaveLength(1)
  })

  it('lets presses through to the viewport, as it is not interactive', () => {
    const legend = renderLegend(Math.PI / 4)
    expect(legend).toHaveClass('pointer-events-none')
    expect(legend).not.toHaveClass('pointer-events-auto')
  })

  it('sits on a surfaceRaised chip over the viewport', () => {
    const legend = renderLegend(Math.PI / 4)
    expect(legend).toHaveClass(COLOR_CLASSES.surfaceRaised.background)
  })
})
