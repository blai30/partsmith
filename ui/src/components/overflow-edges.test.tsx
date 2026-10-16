import { act, fireEvent, render, screen } from '@testing-library/react'
import { useRef } from 'react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { stubResizeObserver, type FakeResizeObservers } from '../test/fakes/resize-observer'
import { useOverflowEdges } from './overflow-edges'

// The rule of a scrolling body shows only while content runs under its header or footer (3.2).

/** A scrolling body with one row, marked by the hook. */
function Body() {
  const body = useRef<HTMLDivElement>(null)
  useOverflowEdges(body)
  return (
    <div ref={body} data-testid="body">
      <p>Row</p>
    </div>
  )
}

let scrollTop = 0
let scrollHeight = 300
let observers: FakeResizeObservers

beforeEach(() => {
  scrollTop = 0
  scrollHeight = 300
  observers = stubResizeObserver()
  vi.spyOn(Element.prototype, 'clientHeight', 'get').mockReturnValue(100)
  vi.spyOn(Element.prototype, 'scrollHeight', 'get').mockImplementation(() => scrollHeight)
  vi.spyOn(Element.prototype, 'scrollTop', 'get').mockImplementation(() => scrollTop)
})

afterEach(() => {
  vi.unstubAllGlobals()
  vi.restoreAllMocks()
})

/** Scrolls the body to `top` and fires its scroll event. */
function scrollTo(body: HTMLElement, top: number): void {
  scrollTop = top
  fireEvent.scroll(body)
}

/** The overflow attributes the body has, in order. */
function edges(body: HTMLElement): string[] {
  return ['data-overflow-top', 'data-overflow-bottom'].filter((name) => body.hasAttribute(name))
}

describe('useOverflowEdges (U-212)', () => {
  it('U-212 marks the bottom edge at the top of a body taller than its box', () => {
    render(<Body />)
    expect(edges(screen.getByTestId('body'))).toEqual(['data-overflow-bottom'])
  })

  it('U-212 marks both edges while scrolled to the middle', () => {
    render(<Body />)
    const body = screen.getByTestId('body')
    scrollTo(body, 50)
    expect(edges(body)).toEqual(['data-overflow-top', 'data-overflow-bottom'])
  })

  it('U-212 marks only the top edge at the end, also half a pixel short of it', () => {
    render(<Body />)
    const body = screen.getByTestId('body')
    scrollTo(body, 200)
    expect(edges(body)).toEqual(['data-overflow-top'])
    scrollTo(body, 199.5)
    expect(edges(body)).toEqual(['data-overflow-top'])
  })

  it('U-212 marks neither edge once the content fits, on a resize', () => {
    render(<Body />)
    const body = screen.getByTestId('body')
    expect(observers.observed()).toEqual(expect.arrayContaining([body, body.firstElementChild]))
    scrollTop = 0
    scrollHeight = 100
    act(() => {
      observers.fire()
    })
    expect(edges(body)).toEqual([])
  })

  it('U-212 measures again when a child is added, and observes the new child', async () => {
    render(<Body />)
    const body = screen.getByTestId('body')
    scrollHeight = 100
    act(() => {
      observers.fire()
    })
    expect(edges(body)).toEqual([])
    const row = document.createElement('p')
    scrollHeight = 300
    await act(async () => {
      body.append(row)
      await Promise.resolve()
    })
    expect(edges(body)).toEqual(['data-overflow-bottom'])
    expect(observers.observed()).toContain(row)
  })

  it('U-212 stops listening after unmount', () => {
    const { unmount } = render(<Body />)
    const body = screen.getByTestId('body')
    unmount()
    scrollTo(body, 50)
    expect(edges(body)).toEqual(['data-overflow-bottom'])
    expect(observers.observed()).toEqual([])
  })
})
