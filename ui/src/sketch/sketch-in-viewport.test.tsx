import { act, cleanup, render, screen } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { setSketchTool } from '../app/actions/sketch'
import { AppProvider } from '../app/AppProvider'
import { stubMatchMedia } from '../layout/test-app.testing'
import { Viewport } from '../viewport/Viewport'
import { SketchOverlay } from './SketchOverlay'
import { clickAt, nextFrame, settleCommits, sketchHarness, VIEW_PIXELS } from './view.testing'

// The sketch overlay inside the real viewport, mounted as the shell mounts it (ui.md 3.2): it finds
// the viewport's canvas, holds the one sketch header, and its tools draw on that canvas.

beforeEach(() => {
  stubMatchMedia()
  // jsdom has no WebGL2, so the viewport draws nothing and the test's fixed view stays attached.
  vi.spyOn(HTMLCanvasElement.prototype, 'getContext').mockReturnValue(null)
  vi.spyOn(HTMLCanvasElement.prototype, 'clientWidth', 'get').mockReturnValue(VIEW_PIXELS)
  vi.spyOn(HTMLCanvasElement.prototype, 'clientHeight', 'get').mockReturnValue(VIEW_PIXELS)
})

afterEach(() => {
  cleanup()
  vi.unstubAllGlobals()
  vi.restoreAllMocks()
})

describe('the sketch overlay in the viewport (3.2, 3.16.4)', () => {
  it('draws a line with the line tool and two clicks on the viewport canvas', async () => {
    const harness = sketchHarness()
    render(
      <AppProvider app={harness.app}>
        <Viewport>
          <SketchOverlay />
        </Viewport>
      </AppProvider>,
    )
    await nextFrame()
    expect(screen.getAllByTestId('sketch-header')).toHaveLength(1)
    expect(screen.getByTestId('sketch-overlay')).toBeInTheDocument()

    act(() => {
      setSketchTool(harness.app, 'line')
    })
    expect(screen.getByTestId('sketch-hint')).toHaveTextContent('Click to start a line.')
    const canvas = screen.getByTestId('viewport').querySelector('canvas')
    if (canvas === null) {
      throw new Error('The viewport has no canvas.')
    }
    act(() => {
      clickAt(canvas, [0, 0])
      clickAt(canvas, [20, 2])
    })
    await act(settleCommits)
    expect(harness.applied.map((entry) => entry.label)).toEqual(['Line'])
    expect(screen.getByTestId('sketch-hint')).toHaveTextContent(/^Click to end the segment/)
  })
})
