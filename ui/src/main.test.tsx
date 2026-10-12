import { screen } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { MESSAGES } from './messages'
import { COLOR_CLASSES, FONT_CLASSES } from './theme/roles'

// main.tsx runs when imported: each test imports a fresh copy into a page with a #root element.

beforeEach(() => {
  vi.resetModules()
  document.body.innerHTML = '<div id="root"></div>'
})

afterEach(() => {
  document.body.innerHTML = ''
  vi.unstubAllGlobals()
  vi.restoreAllMocks()
})

describe('the capability check (3.1)', () => {
  it('renders only the unsupported page with unsupported_webgl2 when WebGL2 is missing', async () => {
    // jsdom has WebAssembly but no WebGL2 context.
    const worker = vi.fn<() => void>()
    vi.stubGlobal('Worker', worker)
    await import('./main')
    const page = await screen.findByTestId('unsupported')
    expect(page).toHaveTextContent(MESSAGES.unsupported_webgl2())
    expect(screen.queryByTestId('app')).toBeNull()
    expect(worker).not.toHaveBeenCalled()
  })

  it('renders the unsupported page with unsupported_wasm when WebAssembly is missing', async () => {
    vi.stubGlobal('WebAssembly', undefined)
    await import('./main')
    const page = await screen.findByTestId('unsupported')
    expect(page).toHaveTextContent(MESSAGES.unsupported_wasm())
    expect(screen.queryByTestId('app')).toBeNull()
  })

  it('draws the unsupported page with the interface font and the surface and text roles', async () => {
    await import('./main')
    const page = await screen.findByTestId('unsupported')
    const classes = [FONT_CLASSES.interfaceText, COLOR_CLASSES.surface.background, COLOR_CLASSES.text.foreground]
    for (const name of classes.flatMap((entry) => entry.split(' '))) {
      expect(page).toHaveClass(name)
    }
  })
})
