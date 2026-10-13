import { act, cleanup, fireEvent, screen, waitFor } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { announce } from '../app/actions/notifications'
import { select } from '../app/actions/selection'
import { installPopoverPolyfill } from '../components/popover-polyfill.testing'
import { MESSAGES } from '../messages'
import { CUBE_DOCUMENT } from '../test/fakes/documents'
import { settle } from '../test/fakes/settle'
import { bodyItem, cubeDocumentState, readyState } from './fixtures.testing'
import { createModelApp, renderWithApp, stubMatchMedia } from './test-app.testing'

// The shell's live regions (3.6) and file drops (3.19).

beforeEach(installPopoverPolyfill)

beforeEach(() => {
  stubMatchMedia()
})

afterEach(() => {
  cleanup()
  vi.useRealTimers()
  vi.unstubAllGlobals()
})

function renderApp() {
  const test = createModelApp({ state: readyState(cubeDocumentState()) })
  renderWithApp(test.app)
  return test
}

/** A drag event carrying files, as the browser builds it for a file dragged from the desktop. */
function fileDrag(files: readonly File[]) {
  return { dataTransfer: { types: ['Files'], files, dropEffect: 'none' } }
}

describe('selection announcements (3.6)', () => {
  it('announces the selection 500 ms after it last changed', () => {
    vi.useFakeTimers()
    const { app } = renderApp()
    act(() => {
      select(app, [bodyItem('b-cube', 'Cube')], 'replace')
    })
    act(() => {
      vi.advanceTimersByTime(499)
    })
    expect(screen.queryByText('Cube selected')).toBeNull()
    act(() => {
      vi.advanceTimersByTime(1)
    })
    expect(screen.getByText('Cube selected')).toBeInTheDocument()
  })

  it('counts several items and says when the selection is cleared', () => {
    vi.useFakeTimers()
    const { app } = renderApp()
    act(() => {
      select(app, [bodyItem('b-cube', 'Cube'), bodyItem('b-peg', 'Peg')], 'replace')
    })
    act(() => {
      vi.advanceTimersByTime(500)
    })
    expect(screen.getByText('2 items selected')).toBeInTheDocument()
    act(() => {
      select(app, [], 'replace')
    })
    act(() => {
      vi.advanceTimersByTime(500)
    })
    expect(screen.getByText('Selection cleared')).toBeInTheDocument()
  })
})

describe('undo and redo announcements (3.20, 3.24)', () => {
  it('says each announcement in a polite live region, a repeated text again', () => {
    const { app } = renderApp()
    act(() => {
      announce(app, 'Undid Rename')
    })
    const first = screen.getByText('Undid Rename')
    expect(first.closest('[aria-live="polite"]')).not.toBeNull()
    act(() => {
      announce(app, 'Undid Rename')
    })
    // A new element, so screen readers read the same words again.
    expect(screen.getByText('Undid Rename')).not.toBe(first)
  })
})

describe('dropping a file on the window (3.19)', () => {
  it('shows the drop overlay while a file is dragged over the window', () => {
    renderApp()
    const root = screen.getByTestId('app')
    fireEvent.dragEnter(root, fileDrag([]))
    expect(screen.getByTestId('drop-overlay')).toHaveTextContent('Drop a .ptsm file to open it')
    fireEvent.dragLeave(root, fileDrag([]))
    expect(screen.queryByTestId('drop-overlay')).toBeNull()
  })

  it('opens the first dropped file', async () => {
    const { core } = renderApp()
    const text = JSON.stringify(CUBE_DOCUMENT)
    const root = screen.getByTestId('app')
    fireEvent.dragEnter(root, fileDrag([]))
    fireEvent.drop(root, fileDrag([new File([text], 'cube.ptsm')]))
    expect(screen.queryByTestId('drop-overlay')).toBeNull()
    await waitFor(() => {
      expect(core.callsTo('openDocument')).toEqual([[text]])
    })
  })

  it('explains a dropped file that is not a model', async () => {
    const { app, core } = renderApp()
    fireEvent.drop(screen.getByTestId('app'), fileDrag([new File(['hello'], 'notes.txt')]))
    await waitFor(() => {
      expect(app.store.getState().notifications.map((entry) => entry.message)).toContain(
        MESSAGES.not_ptsm('notes.txt'),
      )
    })
    await settle()
    expect(core.callsTo('openDocument')).toEqual([])
  })
})

/** Answers the lg breakpoint query as a window narrower than 1024 CSS pixels. */
function stubNarrowWindow(): void {
  vi.stubGlobal('matchMedia', (query: string) => ({
    matches: query.includes('width < 64rem'),
    media: query,
    addEventListener: () => undefined,
    removeEventListener: () => undefined,
  }))
}

function renderWithPanels() {
  const test = createModelApp({
    state: readyState(cubeDocumentState(), {
      panels: {
        browserOpen: true,
        timelineCollapsed: false,
        paletteOpen: false,
        side: 'problems',
        inspectorPosition: null,
      },
    }),
  })
  renderWithApp(test.app)
  return test
}

describe('panels as overlays in narrow windows (3.2)', () => {
  it('closes the browser and the side panel on a press outside them', () => {
    stubNarrowWindow()
    const { app } = renderWithPanels()
    fireEvent.pointerDown(screen.getByTestId('status-bar'))
    expect(app.store.getState().panels.browserOpen).toBe(false)
    expect(app.store.getState().panels.side).toBeNull()
  })

  it('keeps a panel open for a press inside it, and closes it on Esc', () => {
    stubNarrowWindow()
    const { app } = renderWithPanels()
    fireEvent.pointerDown(screen.getByTestId('browser'))
    expect(app.store.getState().panels.browserOpen).toBe(true)
    fireEvent.keyDown(window, { key: 'Escape' })
    expect(app.store.getState().panels.browserOpen).toBe(false)
  })

  it('closes a browser opened after the app started on Esc before the selection clears', () => {
    stubNarrowWindow()
    const { app } = createModelApp({
      state: readyState(cubeDocumentState(), {
        selection: [bodyItem('b-cube', 'Cube')],
        panels: {
          browserOpen: false,
          timelineCollapsed: false,
          paletteOpen: false,
          side: null,
          inspectorPosition: null,
        },
      }),
    })
    renderWithApp(app)
    expect(screen.queryByTestId('browser')).toBeNull()
    act(() => {
      app.store.setState((state) => ({
        panels: { ...state.panels, browserOpen: true, side: null },
      }))
    })
    expect(screen.getByTestId('browser')).toBeInTheDocument()
    fireEvent.keyDown(window, { key: 'Escape' })
    expect(app.store.getState().panels.browserOpen).toBe(false)
    expect(app.store.getState().selection).toHaveLength(1)
    fireEvent.keyDown(window, { key: 'Escape' })
    expect(app.store.getState().selection).toEqual([])
  })

  it('leaves the browser toggle to open and close the browser', () => {
    stubNarrowWindow()
    const { app } = renderWithPanels()
    fireEvent.pointerDown(screen.getByRole('button', { name: 'Toggle browser' }))
    expect(app.store.getState().panels.browserOpen).toBe(true)
  })

  it('keeps both panels open in wide windows', () => {
    const { app } = renderWithPanels()
    fireEvent.pointerDown(screen.getByTestId('status-bar'))
    fireEvent.keyDown(window, { key: 'Escape' })
    expect(app.store.getState().panels.browserOpen).toBe(true)
    expect(app.store.getState().panels.side).toBe('problems')
  })
})
