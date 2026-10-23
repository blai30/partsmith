import { screen } from '@testing-library/react'
import { createRoot } from 'react-dom/client'
import { beforeEach, describe, expect, it, onTestFinished, vi } from 'vitest'
import { App } from '../App'
import { AppProvider } from '../app/AppProvider'
import { createCommandRegistry } from '../commands/registry'
import { installPopoverPolyfill } from '../components/popover-polyfill.testing'
import { readyState, rectSketchDocumentState } from '../layout/fixtures.testing'
import { createAppStore } from '../state/store'
import { createFakeCoreApi } from '../test/fakes/core-api'
import { createTestApp, LINUX_WEB } from '../test/fakes/test-app'
import { sketchSession } from './fixtures.testing'
import { loadedSketchOverlay, loadSketchOverlay } from './overlay-chunk'

// U-215: sketch mode's overlay is a chunk of its own (ui.md 3.1). The chunk waits for the test to
// open its gate, and this file renders the shell without `layout/test-app.testing.tsx`, which
// loads the chunk before any test. The shell renders without act, whose queue drops a render
// that waits for data, so React retries it once the chunk has loaded, as in the app.

const gate = await vi.hoisted(async () => {
  const { deferred } = await import('../test/fakes/deferred')
  return deferred<void>()
})

vi.mock('./SketchOverlay', async (importOriginal) => {
  await gate.promise
  return importOriginal()
})

beforeEach(installPopoverPolyfill)

describe("sketch mode's overlay chunk (3.1, U-215)", () => {
  it('shows sketch mode without the overlay until its chunk has loaded, then with it', async () => {
    vi.stubGlobal('IS_REACT_ACT_ENVIRONMENT', false)
    const app = createTestApp({
      store: createAppStore(
        readyState(rectSketchDocumentState(), { context: 'sketch', sketch: sketchSession() }),
      ),
      core: createFakeCoreApi(),
      commands: createCommandRegistry(),
      platform: LINUX_WEB,
    })
    const root = createRoot(document.body.appendChild(document.createElement('div')))
    onTestFinished(() => {
      root.unmount()
      vi.unstubAllGlobals()
    })
    root.render(
      <AppProvider app={app}>
        <App />
      </AppProvider>,
    )
    expect(await screen.findByTestId('app')).toHaveAttribute('data-context', 'sketch')
    expect(screen.queryByTestId('sketch-header')).toBeNull()
    expect(loadedSketchOverlay()).toBeNull()
    gate.resolve()
    expect(await screen.findByTestId('sketch-header')).toBeInTheDocument()
    expect(screen.getByTestId('sketch-overlay')).toBeInTheDocument()
    // The shell's first render started the one load there is.
    const { SketchOverlay } = await import('./SketchOverlay')
    expect(loadedSketchOverlay()).toBe(SketchOverlay)
    expect(await loadSketchOverlay()).toBe(SketchOverlay)
  })
})
