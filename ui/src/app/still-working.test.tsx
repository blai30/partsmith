import { act, cleanup, fireEvent, screen, within } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { installPopoverPolyfill } from '../components/popover-polyfill.testing'
import { createWorkerTransport } from '../core/transport'
import { createMemoryRecentFiles } from '../files/recent'
import { createMemoryRecovery, type RecoveryStore } from '../files/recovery'
import type { Document, Edit } from '../generated/protocol'
import { StatusBar } from '../layout/StatusBar'
import { renderWithApp, stubElementSize, stubMatchMedia } from '../layout/test-app.testing'
import { currentDocument, LINUX_WEB, startWithDocument, WINDOWS_DESKTOP } from '../test/fakes/app'
import { createFakeCoreWorkers, type FakeCoreWorkers } from '../test/fakes/core-worker'
import { deferred } from '../test/fakes/deferred'
import {
  CUBE_DOCUMENT,
  CUBE_REGION,
  CUBE_SKETCH_SOLUTION,
  documentFile,
  TWO_BODIES_DOCUMENT,
} from '../test/fakes/documents'
import {
  createFakeFileHandle,
  createFakeFileService,
  type FakeFileService,
} from '../test/fakes/files'
import { editsOf } from '../test/fakes/protocol'
import { regionItem } from '../test/fakes/selection'
import { settle } from '../test/fakes/settle'
import { Viewport } from '../viewport/Viewport'
import { renameBody, setBodyVisible } from './actions/bodies'
import { editFeature, startFeatureCommand } from './actions/command'
import { applyEdits, newDocument, openFile, save, startCore } from './actions/document'
import { exportModel } from './actions/export'
import { select } from './actions/selection'
import { createApp, type App } from './createApp'

// A core that has answered nothing for 10 s (ui.md 3.21, 4.4): the still-working state, its
// status text and panel, and "Stop and restart core", which terminates the worker, drops the
// waiting changes and reopens the settled recipe.

// The WebGL2 backend cannot run in jsdom; the viewport gets a FakeBackend instead.
vi.mock('../viewport/webgl2/backend', async () => {
  const fakes = await import('../test/fakes/backend')
  return { createWebGl2Backend: () => fakes.createFakeBackend() }
})

const STILL_WORKING =
  'The geometry core has been working for more than 10 seconds, and Partsmith cannot tell whether it will finish. Stopping it reopens your document as it was after the last change the core finished, and closes any open feature command without keeping it.'
const STILL_WORKING_DESKTOP =
  'The geometry core has been working for more than 10 seconds, and Partsmith cannot tell whether it will finish. The desktop app cannot stop it yet: wait, or quit Partsmith, which keeps your changes up to the last one the core finished, except those of an open feature command.'
const CHANGES_NOT_APPLIED =
  'Your latest changes were not applied, because the geometry core was stopped before it finished them. Make them again if you still want them.'
const EXPORT_STOPPED =
  'cube.stl was not exported, because the geometry core stopped before exporting it. Export it again.'
const SAVE_STOPPED =
  'cube.ptsm was not saved, because the geometry core stopped before saving it. Save it again.'
const OPEN_STOPPED =
  'two-bodies.ptsm was not opened, because the geometry core stopped before opening it. Open it again.'
const NEW_STOPPED =
  'The new document was not opened, because the geometry core stopped before opening it. Choose New again.'

const RENAME_TO_BOX: readonly Edit[] = [
  { type: 'update_body', body: { id: 'b-cube', name: 'Box' } },
]

let restoreSize: () => void

beforeEach(installPopoverPolyfill)

beforeEach(() => {
  stubMatchMedia({ reducedMotion: true })
  restoreSize = stubElementSize(HTMLCanvasElement.prototype, { width: 800, height: 600 })
  vi.useFakeTimers()
})

afterEach(() => {
  cleanup()
  restoreSize()
  vi.useRealTimers()
  vi.unstubAllGlobals()
  vi.restoreAllMocks()
})

/** The viewport and the status bar, where the still-working state shows. */
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

function stillWorking(app: App): boolean {
  return app.store.getState().core.stillWorking
}

/** The still-working panel, which the viewport shows. */
function workingPanel(): HTMLElement {
  return within(screen.getByTestId('viewport')).getByTestId('core-working')
}

describe('still working (3.21, 4.4)', () => {
  it('U-161 after 10 s without an answer the status bar and the viewport say the core is still working, until it answers', async () => {
    const fake = await startWithDocument('cube.ptsm', CUBE_DOCUMENT)
    renderCoreViews(fake.app)
    const answer = deferred<void>()
    fake.transport.handle('apply_edits', async (command, core) => {
      await answer.promise
      return core.reply(command)
    })
    const edit = applyEdits(fake.app, 'Rename', RENAME_TO_BOX)
    await act(() => settle())

    await advance(9_999)
    expect(stillWorking(fake.app)).toBe(false)
    expect(screen.queryByTestId('core-working')).toBeNull()
    await advance(1)
    expect(stillWorking(fake.app)).toBe(true)
    const status = screen.getByTestId('status-core')
    expect(status).toHaveTextContent(/^Still working$/)
    expect(status.querySelector('svg.animate-spin')).not.toBeNull()
    const panel = workingPanel()
    expect(within(screen.getByTestId('viewport')).getAllByRole('alert')).toContain(panel)
    expect(panel).toHaveTextContent(STILL_WORKING)
    expect(within(panel).getByRole('button', { name: 'Stop and restart core' })).toBeVisible()

    answer.resolve()
    await act(async () => {
      await edit
      await settle()
    })
    expect(stillWorking(fake.app)).toBe(false)
    expect(screen.queryByTestId('core-working')).toBeNull()
    expect(screen.queryByText('Still working')).toBeNull()
  })

  it("U-161 counts from the later of a waiting request's sending and the core's latest response", async () => {
    const fake = await startWithDocument('cube.ptsm', CUBE_DOCUMENT)
    const { documentHandle } = currentDocument(fake.app)
    const firstAnswer = deferred<void>()
    fake.transport.handle('evaluate', async (command, core) => {
      if (command.expr !== '1 mm') {
        // The second request is never answered.
        return new Promise<never>(() => undefined)
      }
      await firstAnswer.promise
      return core.reply(command)
    })
    // Request A at 0 ms.
    const first = fake.app.core.evaluate(documentHandle, '1 mm')
    await settle()
    await advance(5_000)
    // Request B at 5,000 ms, never answered.
    void fake.app.core.evaluate(documentHandle, '2 mm').catch(() => undefined)
    await settle()
    await advance(1_000)
    // A is answered at 6,000 ms.
    firstAnswer.resolve()
    await first
    await settle()
    await advance(9_999)
    expect(stillWorking(fake.app)).toBe(false)
    await advance(1)
    expect(stillWorking(fake.app)).toBe(true)
  })

  it('U-161 with the Tauri transport the panel says the desktop app cannot stop the core, and offers no button', async () => {
    const fake = await startWithDocument('cube.ptsm', CUBE_DOCUMENT, {
      core: { kind: 'tauri' },
      platform: WINDOWS_DESKTOP,
    })
    renderCoreViews(fake.app)
    fake.transport.handle('apply_edits', () => new Promise<never>(() => undefined))
    void applyEdits(fake.app, 'Rename', RENAME_TO_BOX)
    await act(() => settle())
    await advance(10_000)
    expect(stillWorking(fake.app)).toBe(true)
    expect(screen.getByTestId('status-core')).toHaveTextContent(/^Still working$/)
    const panel = workingPanel()
    expect(panel).toHaveTextContent(STILL_WORKING_DESKTOP)
    expect(within(panel).queryByRole('button')).toBeNull()
  })
})

type WorkerApp = {
  readonly app: App
  readonly workers: FakeCoreWorkers
  readonly files: FakeFileService
  readonly recovery: RecoveryStore
  readonly handle: FileSystemFileHandle
  /** The settled recipe R1: cube.ptsm with its body renamed to "Box". */
  readonly settled: Document
  /** The document session's recovery id K. */
  readonly session: string
}

type WorkerAppOptions = {
  /** Wraps the fake workers' `createWorker`, for a worker that cannot be created. */
  readonly createWorker?: (create: () => Worker) => Worker
}

/**
 * The real app over the worker transport and fake core workers, with cube.ptsm opened from a file
 * handle and its body renamed to "Box", and the viewport and status bar rendered.
 */
async function startOnFakeWorkers(options: WorkerAppOptions = {}): Promise<WorkerApp> {
  const workers = createFakeCoreWorkers({ solutions: { 'f-sk1': CUBE_SKETCH_SOLUTION } })
  const wrap = options.createWorker
  const files = createFakeFileService()
  const recovery = createMemoryRecovery()
  const app = createApp({
    transport: createWorkerTransport({
      createWorker: wrap === undefined ? workers.createWorker : () => wrap(workers.createWorker),
    }),
    files,
    recent: createMemoryRecentFiles(),
    recovery,
    platform: LINUX_WEB,
  })
  await startCore(app)
  await settle()
  const handle = createFakeFileHandle('cube.ptsm')
  await openFile(app, documentFile('cube.ptsm', CUBE_DOCUMENT, handle))
  await settle()
  await renameBody(app, 'b-cube', 'Box')
  await settle()
  renderCoreViews(app)
  const document = currentDocument(app)
  return {
    app,
    workers,
    files,
    recovery,
    handle,
    settled: document.recipe,
    session: document.recoveryId,
  }
}

/** Clicks "Stop and restart core" and waits until the restart has finished. */
async function stopAndRestart(): Promise<void> {
  await act(async () => {
    fireEvent.click(within(workingPanel()).getByRole('button', { name: 'Stop and restart core' }))
    await settle(30)
  })
}

function warnings(app: App): string[] {
  return app.store
    .getState()
    .notifications.filter((notification) => notification.severity === 'warning')
    .map((notification) => notification.message)
}

/** From now on, the worker never answers an `apply_edits` that renames a body to "Stall". */
function stallRenames(workers: FakeCoreWorkers): void {
  workers.stallWhen(
    (command) =>
      command.type === 'apply_edits' &&
      editsOf(command).some((edit) => edit.type === 'update_body' && edit.body.name === 'Stall'),
  )
}

/** Waits until the core has answered nothing for 10 s. */
async function untilStillWorking(app: App): Promise<void> {
  await act(() => settle())
  await advance(10_000)
  expect(stillWorking(app)).toBe(true)
}

describe('Stop and restart core (4.4)', () => {
  it('U-162 terminates the worker, drops the waiting changes and reopens the settled recipe in the same session', async () => {
    const { app, workers, handle, settled, session } = await startOnFakeWorkers()
    stallRenames(workers)
    // Their rejections are observed from the start; no action shows them.
    const changes = Promise.allSettled([
      renameBody(app, 'b-cube', 'Stall'),
      setBodyVisible(app, 'b-cube', false),
    ])
    await act(() => settle())
    expect(app.mutations.pending).toBe(2)
    await advance(10_000)
    expect(stillWorking(app)).toBe(true)

    await stopAndRestart()
    await act(async () => {
      await changes
    })
    const [first, second] = workers.workers
    expect(workers.workers).toHaveLength(2)
    expect(first?.terminated).toBe(true)
    const types = second?.commands.map((command) => command.type) ?? []
    expect(types.slice(0, 2).toSorted()).toEqual(['feature_catalog', 'list_printer_profiles'])
    expect(second?.commands[2]).toMatchObject({
      type: 'open_document',
      text: JSON.stringify(settled, null, 2),
    })
    expect(types).not.toContain('apply_edits')

    const document = currentDocument(app)
    expect(document).toMatchObject({ name: 'cube', recoveryId: session, localUnsaved: true })
    expect(document.file?.handle).toBe(handle)
    expect(document.recipe).toEqual(settled)
    expect(app.store.getState().core).toMatchObject({ status: 'ready', stillWorking: false })
    expect(screen.queryByTestId('core-fatal')).toBeNull()
    expect(screen.queryByTestId('core-working')).toBeNull()
    expect(
      app.store.getState().notifications.map(({ severity, message }) => ({ severity, message })),
    ).toEqual([{ severity: 'warning', message: CHANGES_NOT_APPLIED }])
  })

  it('U-162 drops an open command whose preview never ends, without cancel_transaction or command_canceled', async () => {
    const { app, workers, settled } = await startOnFakeWorkers()
    workers.stallWhen(
      (command) =>
        command.type === 'apply_edits' &&
        editsOf(command).some(
          (edit) => edit.type === 'add_feature' && edit.feature.type === 'extrude',
        ),
    )
    select(app, [regionItem('f-sk1', CUBE_REGION)], 'replace')
    void Promise.allSettled([startFeatureCommand(app, 'extrude')])
    await untilStillWorking(app)
    expect(app.store.getState().command?.featureType).toBe('extrude')

    await stopAndRestart()
    const [first, second] = workers.workers
    expect(first?.terminated).toBe(true)
    expect(second?.commands.find((command) => command.type === 'open_document')).toMatchObject({
      text: JSON.stringify(settled, null, 2),
    })
    expect(currentDocument(app).recipe).toEqual(settled)
    expect(app.store.getState().command).toBeNull()
    expect(first?.commands.map((command) => command.type)).not.toContain('cancel_transaction')
    const notifications = app.store.getState().notifications
    expect(notifications.map((notification) => notification.code)).not.toContain('command_canceled')
    expect(warnings(app)).toEqual([CHANGES_NOT_APPLIED])
  })

  it('U-162 an Edit whose set_focus never ends restarts without cancel_transaction or an error', async () => {
    const { app, workers, settled } = await startOnFakeWorkers()
    workers.stallWhen((command) => command.type === 'set_focus')
    const editing = Promise.allSettled([editFeature(app, 'f-ext1')])
    await untilStillWorking(app)

    await stopAndRestart()
    await act(async () => {
      await editing
    })
    const types = workers.workers.flatMap((worker) => worker.commands.map(({ type }) => type))
    expect(types).not.toContain('cancel_transaction')
    expect(currentDocument(app).recipe).toEqual(settled)
    expect(app.store.getState().command).toBeNull()
    const errors = app.store
      .getState()
      .notifications.filter((notification) => notification.severity === 'error')
    expect(errors).toEqual([])
  })

  it('U-162 a stuck export with nothing waiting and no command shows only that the export was not done', async () => {
    const { app, workers, settled } = await startOnFakeWorkers()
    workers.stallWhen((command) => command.type === 'export')
    const exporting = Promise.allSettled([exportModel(app, { format: 'stl', scope: 'visible' })])
    await untilStillWorking(app)

    await stopAndRestart()
    await act(async () => {
      await exporting
    })
    expect(workers.workers[0]?.terminated).toBe(true)
    expect(currentDocument(app).recipe).toEqual(settled)
    expect(stillWorking(app)).toBe(false)
    expect(
      app.store.getState().notifications.map(({ severity, message }) => ({ severity, message })),
    ).toEqual([{ severity: 'warning', message: EXPORT_STOPPED }])
  })
})

describe('the other work a stop drops (4.4)', () => {
  it('U-165 a Save waiting behind a stuck export writes no file, and both say they were not done', async () => {
    const { app, workers, files } = await startOnFakeWorkers()
    workers.stallWhen((command) => command.type === 'export')
    const exporting = exportModel(app, { format: 'stl', scope: 'visible' })
    // The export asks for its target first, so the Save is queued once the export is sent.
    await act(() => settle())
    const dropped = Promise.allSettled([exporting, save(app)])
    await untilStillWorking(app)

    await stopAndRestart()
    await act(async () => {
      await dropped
    })
    expect(files.writes).toEqual([])
    expect(warnings(app).toSorted()).toEqual([EXPORT_STOPPED, SAVE_STOPPED].toSorted())
  })

  it('U-165 an Open waiting behind a stuck change leaves the document open and says so', async () => {
    const { app, workers } = await startOnFakeWorkers()
    stallRenames(workers)
    const dropped = Promise.allSettled([
      renameBody(app, 'b-cube', 'Stall'),
      openFile(app, documentFile('two-bodies.ptsm', TWO_BODIES_DOCUMENT)),
    ])
    await untilStillWorking(app)

    await stopAndRestart()
    await act(async () => {
      await dropped
    })
    expect(currentDocument(app).name).toBe('cube')
    expect(warnings(app).toSorted()).toEqual([CHANGES_NOT_APPLIED, OPEN_STOPPED].toSorted())
  })

  it('U-165 a New waiting behind a stuck change leaves the document open and says so', async () => {
    const { app, workers } = await startOnFakeWorkers()
    stallRenames(workers)
    const dropped = Promise.allSettled([renameBody(app, 'b-cube', 'Stall'), newDocument(app)])
    await untilStillWorking(app)

    await stopAndRestart()
    await act(async () => {
      await dropped
    })
    expect(currentDocument(app).name).toBe('cube')
    expect(warnings(app).toSorted()).toEqual([CHANGES_NOT_APPLIED, NEW_STOPPED].toSorted())
  })
})

describe('the recovery session a restart reopens (3.19, 4.4)', () => {
  it('U-166 a New that the stop dropped had ended the session: the restart continues it and writes it at once', async () => {
    const { app, workers, recovery, settled, session } = await startOnFakeWorkers()
    await advance(2_000)
    expect(await recovery.list()).toEqual([
      expect.objectContaining({ id: session, reason: 'autosave' }),
    ])
    stallRenames(workers)
    const dropped = Promise.allSettled([renameBody(app, 'b-cube', 'Stall'), newDocument(app)])
    await untilStillWorking(app)
    expect(await recovery.list()).toEqual([
      expect.objectContaining({ id: session, reason: 'replaced' }),
    ])

    await stopAndRestart()
    await act(async () => {
      await dropped
    })
    expect(currentDocument(app).recoveryId).toBe(session)
    expect(await recovery.list()).toEqual([
      expect.objectContaining({
        id: session,
        reason: 'autosave',
        text: JSON.stringify(settled, null, 2),
      }),
    ])

    await act(async () => {
      await renameBody(app, 'b-cube', 'Lid')
      await settle()
    })
    await advance(2_000)
    const entries = await recovery.list()
    expect(entries).toHaveLength(1)
    expect(entries[0]).toMatchObject({ id: session, reason: 'autosave' })
    expect(entries[0]?.text).toContain('"name": "Lid"')
  })
})

/** Clicks "Restart core" in the fatal panel and waits until the restart has finished. */
async function restartFromFatalPanel(): Promise<void> {
  await act(async () => {
    const panel = within(screen.getByTestId('viewport')).getByTestId('core-fatal')
    fireEvent.click(within(panel).getByRole('button', { name: 'Restart core' }))
    await settle(30)
  })
}

describe('a fatal error, and a restart that fails after a stop (4.4)', () => {
  it('U-169 a restart after the core failed shows no warning for the changes it lost', async () => {
    const { app, workers, settled } = await startOnFakeWorkers()
    stallRenames(workers)
    const lost = Promise.allSettled([
      renameBody(app, 'b-cube', 'Stall'),
      setBodyVisible(app, 'b-cube', false),
    ])
    await act(() => settle())
    await act(async () => {
      workers.workers[0]?.fail('The core trapped.')
      await lost
      await settle()
    })
    expect(screen.getByTestId('core-fatal')).toBeVisible()

    await restartFromFatalPanel()
    expect(app.store.getState().core.status).toBe('ready')
    expect(currentDocument(app).recipe).toEqual(settled)
    expect(warnings(app)).toEqual([])
  })

  it('U-169 the warning of a stop whose restart fails shows once a later restart succeeds', async () => {
    let created = 0
    const { app, workers } = await startOnFakeWorkers({
      createWorker: (create) => {
        created += 1
        if (created === 2) {
          throw new Error('The worker script could not be loaded.')
        }
        return create()
      },
    })
    stallRenames(workers)
    const dropped = Promise.allSettled([
      renameBody(app, 'b-cube', 'Stall'),
      setBodyVisible(app, 'b-cube', false),
    ])
    await untilStillWorking(app)

    await stopAndRestart()
    await act(async () => {
      await dropped
    })
    expect(app.store.getState().core.status).toBe('failed')
    expect(screen.getByTestId('core-fatal')).toHaveTextContent(/^The geometry core stopped: /)
    expect(warnings(app)).toEqual([])

    await restartFromFatalPanel()
    expect(app.store.getState().core.status).toBe('ready')
    expect(currentDocument(app).name).toBe('cube')
    expect(warnings(app)).toEqual([CHANGES_NOT_APPLIED])
  })
})
