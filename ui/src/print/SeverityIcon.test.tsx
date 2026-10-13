import { cleanup, render } from '@testing-library/react'
import { afterEach, describe, expect, it } from 'vitest'
import { SeverityIcon } from './SeverityIcon'

afterEach(() => {
  cleanup()
})

function icon(element: React.ReactElement): SVGElement {
  const { container } = render(element)
  const svg = container.querySelector('svg')
  if (svg === null) {
    throw new Error('No icon rendered.')
  }
  return svg
}

describe('SeverityIcon', () => {
  it('is decorative without a label', () => {
    const svg = icon(<SeverityIcon severity="warning" />)
    expect(svg).toHaveAttribute('aria-hidden', 'true')
    expect(svg).not.toHaveAttribute('aria-label')
  })

  it('names the severity with a label', () => {
    const svg = icon(<SeverityIcon severity="error" label="Error" />)
    expect(svg).toHaveAttribute('aria-label', 'Error')
    expect(svg).not.toHaveAttribute('aria-hidden')
  })

  it('puts the given classes before its own', () => {
    const svg = icon(<SeverityIcon severity="info" className="mt-0.5" />)
    expect(svg.getAttribute('class')).toMatch(/^lucide lucide-info mt-0\.5 size-4 shrink-0 /)
  })
})
