import { render, screen } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { MESSAGES } from '../messages'
import { RegionBoundary } from './RegionBoundary'

// A crashed region keeps its island's place and shape, and its message and Reload button fit
// inside it, also in a bar as slim as the status bar (3.2, 4.4).

function Broken(): never {
  throw new Error('The region broke.')
}

beforeEach(() => {
  vi.spyOn(console, 'error').mockImplementation(() => undefined)
})

afterEach(() => {
  vi.restoreAllMocks()
})

describe('RegionBoundary (4.4)', () => {
  it('renders its region while nothing crashed', () => {
    render(
      <RegionBoundary region="browser">
        <p>Tree</p>
      </RegionBoundary>,
    )
    expect(screen.getByText('Tree')).toBeInTheDocument()
    expect(screen.queryByRole('alert')).toBeNull()
  })

  it('shows a crashed panel as an island whose body scrolls, never the island itself', () => {
    render(
      <RegionBoundary region="browser" fallbackClassName="col-start-1 w-66 rounded-xl">
        <Broken />
      </RegionBoundary>,
    )
    const fallback = screen.getByRole('alert')
    expect(fallback).toHaveClass('col-start-1', 'w-66', 'rounded-xl')
    expect(fallback).toHaveClass('flex', 'flex-col', 'overflow-hidden')
    const body = fallback.firstElementChild
    expect(body).toHaveClass('min-h-0', 'flex-1', 'flex-col', 'overflow-y-auto', 'p-4')
    expect(body).toHaveTextContent(MESSAGES.region_crashed())
    expect(screen.getByRole('button', { name: 'Reload' })).toHaveClass('h-8')
  })

  it('shows a crashed bar as one row whose message gives way to a compact Reload', () => {
    render(
      <RegionBoundary region="status bar" bar fallbackClassName="h-7 rounded-lg">
        <Broken />
      </RegionBoundary>,
    )
    const fallback = screen.getByRole('alert')
    expect(fallback).toHaveClass('h-7', 'flex', 'flex-col', 'overflow-hidden')
    const body = fallback.firstElementChild
    expect(body).toHaveClass('min-h-0', 'flex-1', 'items-center', 'overflow-hidden', 'px-3')
    expect(body).not.toHaveClass('p-4')
    expect(screen.getByText(MESSAGES.region_crashed())).toHaveClass('min-w-0', 'truncate')
    expect(screen.getByRole('button', { name: 'Reload' })).toHaveClass('h-6', 'text-xs')
  })
})
