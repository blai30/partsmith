import { act, cleanup, fireEvent, renderHook, screen, within } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { installShortcutDispatcher } from '../commands/dispatcher'
import { installPopoverPolyfill } from '../components/popover-polyfill.testing'
import type { Document, Edit } from '../generated/protocol'
import { useEscapeLadder } from '../layout/ladders'
import { SidePanel } from '../layout/SidePanel'
import { StatusBar } from '../layout/StatusBar'
import { renderWithApp, stubElementSize, stubMatchMedia } from '../layout/test-app.testing'
import { MESSAGES } from '../messages'
import {
  clickAction,
  currentDocument,
  startWithDocument,
  WINDOWS_DESKTOP,
  type FakeApp,
  type FakeAppOptions,
} from '../test/fakes/app'
import { notRunReply } from '../test/fakes/core'
import { deferred } from '../test/fakes/deferred'
import {
  CUBE_DOCUMENT,
  CUBE_EXTRUDE,
  CUBE_REGION,
  CUBE_SKETCH,
  CUBE_SKETCH_SOLUTION,
} from '../test/fakes/documents'
import { regionItem } from '../test/fakes/selection'
import { settle } from '../test/fakes/settle'
import { Viewport } from '../viewport/Viewport'
import { setCommandField, startFeatureCommand } from './actions/command'
import { setContext } from './actions/context'
import { applyEdits } from './actions/document'
import { openPrintHealth } from './actions/print'
import { select } from './actions/selection'
import type { App } from './createApp'

// Stopping a request (ui.md 2.5, 3.7, 3.9, 3.21 and 4.4, ADR 0063): the Stop button, the
// still-working panel's Stop, Esc on a running preview, the rebuild_stopped notification and the
// status bar after a stop, over the fake transport, which records every cancel.

// The WebGL2 backend cannot run in jsdom; the viewport gets a FakeBackend instead.
vi.mock('../viewport/webgl2/backend', async () => {
  const fakes = await import('../test/fakes/backend')
  return { createWebGl2Backend: () => fakes.createFakeBackend() }
})

const RENAME_TO_BOX: readonly Edit[] = [
  { type: 'update_body', body: { id: 'b-cube', name: 'Box' } },
]
const RENAME_TO_BLOCK: readonly Edit[] = [
  { type: 'update_body', body: { id: 'b-cube', name: 'Block' } },
]

const PRINT_STOPPED =
  'The print check was stopped before it finished. Run it again to see its findings.'

/** cube.ptsm with "Fillet 1" and "Chamfer 1" after "Extrude 1", for the not_run statuses of U-273. */
const STOPPED_DOCUMENT: Document = {
  ...CUBE_DOCUMENT,
  features: [
    CUBE_SKETCH,
    CUBE_EXTRUDE,
    { id: 'f-fil1', type: 'fillet', body: 'b-cube', edges: [], radius: '2 mm' },
    { id: 'f-cha1', type: 'chamfer', body: 'b-cube', edges: [], distance: '1 mm' },
  ],
}

let restoreSize: () => void
let uninstallDispatcher: () => void = () => undefined

beforeEach(installPopoverPolyfill)

beforeEach(() => {
  stubMatchMedia({ reducedMotion: true })
  restoreSize = stubElementSize(HTMLCanvasElement.prototype, { width: 800, height: 600 })
  vi.useFakeTimers()
})

afterEach(() => {
  uninstallDispatcher()
  uninstallDispatcher = () => undefined
  cleanup()
  restoreSize()
  vi.useRealTimers()
  vi.restoreAllMocks()
})

/** The viewport and the status bar, where the Stop button and the still-working panel show. */
function renderCoreViews(app: App): void {
  renderWithApp(
    app,
    <>
      <Viewport />
      <StatusBar />
    </>,
  )
}

/** Advances the fake clock inside act, then lets the work it started finish. */
async function advance(milliseconds: number): Promise<void> {
  await act(async () => {
    await vi.advanceTimersByTimeAsync(milliseconds)
    await settle()
  })
}

/** A core that never answers, as one stuck in a step that cannot stop. */
function neverAnswer(): Promise<never> {
  return new Promise<never>(() => undefined)
}

/** The still-working panel, which the viewport shows. */
function workingPanel(): HTMLElement {
  return within(screen.getByTestId('viewport')).getByTestId('core-working')
}

/** The names of a panel's buttons, in order. */
function buttonNames(panel: HTMLElement): (string | null)[] {
  return within(panel)
    .queryAllByRole('button')
    .map((button) => button.textContent)
}

/** The ids of the apply_edits requests the core received, oldest first. */
function editIds(fake: FakeApp): number[] {
  return fake.transport.requests
    .filter((request) => request.command.type === 'apply_edits')
    .map((request) => request.id)
}

/** The shown notifications whose message is `message`. */
function notificationsSaying(app: App, message: string) {
  return app.store
    .getState()
    .notifications.filter((notification) => notification.message === message)
}

/** cube.ptsm open over a core that can stop a call, with a new Extrude command of its region started. */
async function newExtrude(): Promise<FakeApp> {
  const fake = await startWithDocument('cube.ptsm', CUBE_DOCUMENT, {
    core: { canCancel: true, solutions: { 'f-sk1': CUBE_SKETCH_SOLUTION } },
  })
  select(fake.app, [regionItem('f-sk1', CUBE_REGION)], 'replace')
  await startFeatureCommand(fake.app, 'extrude')
  await settle()
  return fake
}

/** cube.ptsm open with an edit whose answer waits for the returned deferred. */
async function heldEdit(options: FakeAppOptions = {}) {
  const fake = await startWithDocument('cube.ptsm', CUBE_DOCUMENT, options)
  renderCoreViews(fake.app)
  const answer = deferred<void>()
  fake.transport.handle('apply_edits', async (command, core) => {
    await answer.promise
    return notRunReply(core.reply(command), fake.transport.cancels.length > 0 ? ['f-ext1'] : [])
  })
  const edit = applyEdits(fake.app, 'Rename', RENAME_TO_BOX)
  await act(() => settle())
  return { fake, answer, edit }
}

describe('the Stop button (3.21)', () => {
  it('U-270 with canCancel true, a change pending for 400 ms reads Rebuilding and holds Stop, which stops it once and hides', async () => {
    const { fake, answer, edit } = await heldEdit({ core: { canCancel: true } })
    await advance(400)
    const status = screen.getByTestId('status-core')
    expect(status).toHaveTextContent(/^Rebuilding/)
    const stop = within(status).getByTestId('core-stop')
    expect(stop).toHaveTextContent('Stop')
    fireEvent.click(stop)
    expect(fake.transport.cancels).toEqual(editIds(fake))
    expect(fake.transport.cancels).toHaveLength(1)
    expect(screen.queryByTestId('core-stop')).toBeNull()

    answer.resolve()
    await act(async () => {
      await edit
      await settle()
    })
    expect(fake.transport.cancels).toHaveLength(1)
  })

  it('U-270 with canCancel false, Stop never shows', async () => {
    const { fake } = await heldEdit()
    await advance(400)
    expect(screen.getByTestId('status-core')).toHaveTextContent(/^Rebuilding/)
    expect(screen.queryByTestId('core-stop')).toBeNull()
    await advance(10_000)
    expect(screen.getByTestId('status-core')).toHaveTextContent(/^Still working/)
    expect(screen.queryByTestId('core-stop')).toBeNull()
    expect(fake.transport.cancels).toEqual([])
  })

  it('app.stopCore() stops the oldest request with no response, and returns false without one', async () => {
    const { fake, answer, edit } = await heldEdit({ core: { canCancel: true } })
    expect(fake.app.stopCore()).toBe(true)
    expect(fake.transport.cancels).toEqual(editIds(fake))
    answer.resolve()
    await act(async () => {
      await edit
      await settle()
    })
    expect(fake.app.stopCore()).toBe(false)
  })
})

describe('the still-working panel since M2 (4.4)', () => {
  it('U-271 with canCancel true and the worker transport it offers Stop, then Stop and restart core', async () => {
    const { fake, answer, edit } = await heldEdit({ core: { canCancel: true } })
    await advance(10_000)
    const panel = workingPanel()
    expect(panel).toHaveTextContent(MESSAGES.core_still_working_stoppable(10))
    expect(buttonNames(panel)).toEqual(['Stop', 'Stop and restart core'])

    fireEvent.click(within(panel).getByRole('button', { name: 'Stop' }))
    expect(fake.transport.cancels).toEqual(editIds(fake))
    await advance(10_000)
    expect(
      within(workingPanel()).getByRole('button', { name: 'Stop and restart core' }),
    ).toBeVisible()

    answer.resolve()
    await act(async () => {
      await edit
      await settle()
    })
    expect(fake.app.store.getState().core.stillWorking).toBe(false)
    expect(screen.queryByTestId('core-working')).toBeNull()
  })

  it('U-271 with the Tauri transport it offers Stop alone, and 10 s after Stop with no response it says the step cannot stop', async () => {
    const fake = await startWithDocument('cube.ptsm', CUBE_DOCUMENT, {
      core: { kind: 'tauri', canCancel: true },
      platform: WINDOWS_DESKTOP,
    })
    renderCoreViews(fake.app)
    fake.transport.handle('apply_edits', neverAnswer)
    void applyEdits(fake.app, 'Rename', RENAME_TO_BOX)
    await act(() => settle())
    await advance(10_000)
    const panel = workingPanel()
    expect(panel).toHaveTextContent(MESSAGES.core_still_working_stoppable(10))
    expect(buttonNames(panel)).toEqual(['Stop'])

    fireEvent.click(within(panel).getByRole('button', { name: 'Stop' }))
    expect(fake.transport.cancels).toHaveLength(1)
    // A background request sent after Stop is no answer to it.
    void fake.app.core.evaluate(currentDocument(fake.app).documentHandle, '1 mm')
    await advance(9_999)
    expect(workingPanel()).not.toHaveTextContent(MESSAGES.core_stop_waiting_desktop())
    await advance(1)
    const waiting = workingPanel()
    expect(waiting).toHaveTextContent(MESSAGES.core_stop_waiting_desktop())
    expect(within(waiting).queryByRole('button')).toBeNull()
  })

  it("U-271 with canCancel false the panel is M1's", async () => {
    await heldEdit()
    await advance(10_000)
    const panel = workingPanel()
    expect(panel).toHaveTextContent(MESSAGES.core_still_working(10))
    expect(buttonNames(panel)).toEqual(['Stop and restart core'])
  })
})

/** STOPPED_DOCUMENT open, with an edit whose recompute reports `stopped` not run. */
async function stoppedEdit(stopped: readonly string[]): Promise<FakeApp> {
  const fake = await startWithDocument('stopped.ptsm', STOPPED_DOCUMENT, {
    core: { canCancel: true },
  })
  fake.transport.handle('apply_edits', (command, core) => notRunReply(core.reply(command), stopped))
  await applyEdits(fake.app, 'Rename', RENAME_TO_BOX)
  await settle()
  return fake
}

describe('the rebuild_stopped notification (4.4)', () => {
  const THREE_STOPPED =
    "The rebuild stopped before 'Extrude 1' was built. 3 features are not built."
  const ONE_STOPPED = "The rebuild stopped before 'Extrude 1' was built."

  it('U-273 names the first feature not built and counts them, and Rebuild sends rebuild', async () => {
    const fake = await stoppedEdit(['f-ext1', 'f-fil1', 'f-cha1'])
    const [notification] = notificationsSaying(fake.app, THREE_STOPPED)
    expect(notification?.severity).toBe('info')
    expect(notification?.actions.map((action) => action.label)).toEqual(['Rebuild'])
    if (notification === undefined) {
      return
    }
    fake.transport.handle('apply_edits', null)
    fake.transport.clearRequests()
    await clickAction(fake.app, notification, 'Rebuild')
    // ui.md's U-273 has `doc: 1`; here the document is the handle the opened file got.
    expect(fake.transport.commands).toEqual([
      { type: 'rebuild', doc: currentDocument(fake.app).documentHandle },
    ])
  })

  it('U-273 with one feature not built it reads rebuild_stopped_one', async () => {
    const fake = await stoppedEdit(['f-ext1'])
    expect(notificationsSaying(fake.app, ONE_STOPPED)).toHaveLength(1)
    expect(notificationsSaying(fake.app, THREE_STOPPED)).toEqual([])
  })

  it('U-273 a later response with no not_run dismisses it', async () => {
    const fake = await stoppedEdit(['f-ext1', 'f-fil1', 'f-cha1'])
    expect(notificationsSaying(fake.app, THREE_STOPPED)).toHaveLength(1)
    fake.transport.handle('apply_edits', null)
    await applyEdits(fake.app, 'Rename', RENAME_TO_BLOCK)
    await settle()
    expect(notificationsSaying(fake.app, THREE_STOPPED)).toEqual([])
  })

  it('U-273 a preview that a newer value superseded shows no notification', async () => {
    const fake = await newExtrude()
    const featureId = fake.app.store.getState().command?.featureId ?? ''
    const first = deferred<void>()
    let previews = 0
    fake.transport.handle('apply_edits', async (command, core) => {
      previews += 1
      if (previews > 1) {
        return core.reply(command)
      }
      await first.promise
      return notRunReply(core.reply(command), [featureId])
    })
    setCommandField(fake.app, 'extent.expr', '30 mm')
    await settle()
    setCommandField(fake.app, 'extent.expr', '40 mm')
    await settle()
    first.resolve()
    await settle()
    expect(fake.transport.cancels).toHaveLength(1)
    const texts = fake.app.store
      .getState()
      .notifications.map((notification) => notification.message)
    expect(texts.filter((text) => text.startsWith('The rebuild stopped before'))).toEqual([])
  })
})

describe('Esc on a running preview (3.7)', () => {
  it('U-269 Esc stops the pending preview before the cancel_transaction frame is sent', async () => {
    const fake = await newExtrude()
    uninstallDispatcher = installShortcutDispatcher(fake.app, window)
    // The Esc ladder listens where App mounts it (3.5); the shortcut dispatcher takes no Esc.
    renderHook(() => {
      useEscapeLadder(fake.app)
    })
    const answer = deferred<void>()
    fake.transport.handle('apply_edits', async (command, core) => {
      await answer.promise
      return core.reply(command)
    })
    setCommandField(fake.app, 'extent.expr', '30 mm')
    await settle()
    const preview = fake.transport.requests.at(-1)
    expect(preview?.command.type).toBe('apply_edits')
    fake.journal.length = 0

    fireEvent.keyDown(document.body, { key: 'Escape' })
    await settle()
    answer.resolve()
    await settle()
    await vi.runAllTimersAsync()
    await settle()

    const stopped = fake.journal.indexOf(`cancel:${String(preview?.id)}`)
    const canceled = fake.journal.indexOf('command:cancel_transaction')
    expect(stopped).toBeGreaterThanOrEqual(0)
    expect(canceled).toBeGreaterThan(stopped)
    expect(fake.app.store.getState().command).toBeNull()
  })

  it('a newer value typed while a preview runs stops that preview before the value is evaluated', async () => {
    const fake = await newExtrude()
    const answer = deferred<void>()
    fake.transport.handle('apply_edits', async (command, core) => {
      await answer.promise
      return core.reply(command)
    })
    setCommandField(fake.app, 'extent.expr', '30 mm')
    await settle()
    const preview = fake.transport.requests.at(-1)
    expect(preview?.command.type).toBe('apply_edits')

    setCommandField(fake.app, 'extent.expr', '35 mm', { typing: true })
    await settle()

    expect(fake.transport.cancels).toEqual([preview?.id])
    answer.resolve()
    await settle()
  })

  it('the first preview of a new feature stops its set_focus too when a newer value comes', async () => {
    const fake = await startWithDocument('cube.ptsm', CUBE_DOCUMENT, {
      core: { canCancel: true, solutions: { 'f-sk1': CUBE_SKETCH_SOLUTION } },
    })
    select(fake.app, [regionItem('f-sk1', CUBE_REGION)], 'replace')
    const answer = deferred<void>()
    fake.transport.handle('apply_edits', async (command, core) => {
      await answer.promise
      return core.reply(command)
    })
    const started = startFeatureCommand(fake.app, 'extrude')
    await settle()
    const preview = fake.transport.requests.at(-1)
    expect(preview?.command.type).toBe('apply_edits')

    setCommandField(fake.app, 'extent.expr', '40 mm')
    await settle()
    answer.resolve()
    await started
    await settle()

    const focus = fake.transport.requests.find((request) => request.command.type === 'set_focus')
    expect(fake.transport.cancels).toEqual([preview?.id, focus?.id])
  })

  it('a preview a newer value replaced while it waited for the queue is never sent', async () => {
    const fake = await newExtrude()
    fake.transport.clearRequests()
    const blocker = deferred<void>()
    const held = fake.app.mutations.run(() => blocker.promise)
    setCommandField(fake.app, 'extent.expr', '30 mm')
    await settle()
    setCommandField(fake.app, 'extent.expr', '40 mm')
    await settle()

    blocker.resolve()
    await held
    await settle()

    const previews = fake.transport.commandsOfType('apply_edits')
    expect(previews).toHaveLength(1)
    expect(JSON.stringify(previews[0]?.edits)).toContain('40 mm')
    expect(fake.transport.cancels).toEqual([])
  })
})

describe('the status bar after a stop (3.21)', () => {
  it('U-274 reads "Stopped after", keeps the print badge stale and sends no automatic print check until a full rebuild', async () => {
    const fake = await startWithDocument('stopped.ptsm', STOPPED_DOCUMENT, {
      core: { canCancel: true },
    })
    renderCoreViews(fake.app)
    fake.transport.handle('apply_edits', (command, core) =>
      notRunReply(core.reply(command), ['f-cha1']),
    )
    await act(async () => {
      await applyEdits(fake.app, 'Rename', RENAME_TO_BOX)
      await settle()
    })
    const recompute = screen.getByTestId('status-recompute')
    expect(recompute).toHaveTextContent(/^Stopped after \d+ ms$/)

    fake.transport.clearRequests()
    await act(async () => {
      await setContext(fake.app, 'print')
      await settle()
    })
    expect(fake.transport.commandTypes()).not.toContain('analyze_print')
    expect(screen.getByTestId('health-badge')).toHaveAttribute('data-stale', 'true')

    fake.transport.handle('apply_edits', null)
    await act(async () => {
      await applyEdits(fake.app, 'Rename', RENAME_TO_BLOCK)
      await settle()
    })
    expect(screen.getByTestId('status-recompute')).toHaveTextContent(/^Rebuilt in /)
    expect(fake.transport.commandTypes()).toContain('analyze_print')
  })
})

describe('a stopped print check (4.4)', () => {
  it('U-297 an automatic print check the core stopped shows nothing and leaves the badge stale', async () => {
    const fake = await startWithDocument('cube.ptsm', CUBE_DOCUMENT, { core: { canCancel: true } })
    renderCoreViews(fake.app)
    const before = fake.app.store.getState().notifications
    fake.transport.handle('analyze_print', () => ({
      error: { code: 'canceled', message: PRINT_STOPPED },
    }))
    await act(async () => {
      await setContext(fake.app, 'print')
      await settle()
    })
    expect(fake.transport.commandTypes()).toContain('analyze_print')
    expect(fake.app.store.getState().notifications).toEqual(before)
    expect(screen.getByTestId('health-badge')).toHaveAttribute('data-stale', 'true')
  })

  it('U-297 a print check the user asked for that the core stopped shows the core message in the print check panel', async () => {
    const fake = await startWithDocument('cube.ptsm', CUBE_DOCUMENT, { core: { canCancel: true } })
    renderWithApp(fake.app, <SidePanel />)
    const before = fake.app.store.getState().notifications
    fake.transport.handle('analyze_print', () => ({
      error: { code: 'canceled', message: PRINT_STOPPED },
    }))
    await act(async () => {
      await openPrintHealth(fake.app)
      await settle()
    })
    expect(within(screen.getByTestId('health-list')).getByText(PRINT_STOPPED)).toBeVisible()
    expect(fake.app.store.getState().notifications).toEqual(before)
  })
})
