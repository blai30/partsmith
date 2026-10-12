import { screen } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { stubMatchMedia } from './layout/test-app.testing'
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
    const classes = [
      FONT_CLASSES.interfaceText,
      COLOR_CLASSES.surface.background,
      COLOR_CLASSES.text.foreground,
    ]
    for (const name of classes.flatMap((entry) => entry.split(' '))) {
      expect(page).toHaveClass(name)
    }
  })
})

/** Lets the capability check pass: its canvas gets a WebGL2 context, later canvases (the viewport's) get none. */
function stubWebGl2CapabilityCheck(): void {
  vi.spyOn(HTMLCanvasElement.prototype, 'getContext')
    .mockReturnValueOnce({} as RenderingContext)
    .mockReturnValue(null)
}

/** Records the environments main.tsx chooses a file service for, passing them to the real chooser. */
function spyOnChooseFileService(): ReturnType<typeof vi.fn<(environment: object) => void>> {
  const chosen = vi.fn<(environment: object) => void>()
  vi.doMock('./files/choose-file-service', async (importOriginal) => {
    const actual = await importOriginal<typeof import('./files/choose-file-service')>()
    return {
      chooseFileService: (environment: Window & typeof globalThis) => {
        chosen(environment)
        return actual.chooseFileService(environment)
      },
    }
  })
  return chosen
}

describe('startup wiring (3.1, desktop.md 3.6 and 3.7)', () => {
  beforeEach(() => {
    stubMatchMedia()
    stubWebGl2CapabilityCheck()
    // The recent files and recovery stores open their database only when first used.
    vi.stubGlobal('indexedDB', {} as IDBFactory)
    // The worker transport of the web build starts with the core, which these tests never answer.
    vi.stubGlobal(
      'Worker',
      class {
        addEventListener(): void {
          // No messages arrive.
        }
        postMessage(): void {
          // Nothing answers.
        }
        terminate(): void {
          // Nothing to stop.
        }
      },
    )
  })

  afterEach(() => {
    vi.doUnmock('./files/choose-file-service')
    Reflect.deleteProperty(window, '__TAURI__')
  })

  it('chooses the file service for the window with chooseFileService', async () => {
    const chosen = spyOnChooseFileService()
    await import('./main')
    await screen.findByTestId('app')
    expect(chosen).toHaveBeenCalledWith(window)
  })

  it('tells the desktop host whether closing would lose changes', async () => {
    const invoke = vi.fn((command: string): Promise<unknown> =>
      // The core never answers, so the document stays unopened and nothing is unsaved.
      command === 'set_unsaved' ? Promise.resolve(null) : new Promise(() => undefined),
    )
    Reflect.set(window, '__TAURI__', { core: { invoke } })
    await import('./main')
    await screen.findByTestId('app')
    expect(invoke).toHaveBeenCalledWith('set_unsaved', { unsaved: false })
  })
})
