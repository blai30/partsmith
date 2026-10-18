import { afterEach, describe, expect, it, vi } from 'vitest'
import type { Document, Edit, Event as ProtocolEvent } from '../generated/protocol'
import {
  createFakeApp,
  currentDocument,
  LINUX_WEB,
  startFakeApp,
  startWithDocument,
  type FakeApp,
} from '../test/fakes/app'
import {
  CUBE_DOCUMENT,
  CUBE_SKETCH_SOLUTION,
  CUBE_TOP_FACE,
  bodiesOf,
} from '../test/fakes/documents'
import { createFakeFileHandle } from '../test/fakes/files'
import { deferred } from '../test/fakes/deferred'
import { createRecordingRecovery } from '../test/fakes/recovery'
import { elementItem } from '../test/fakes/selection'
import { settle } from '../test/fakes/settle'
import { CoreFatalError } from '../core/transport'
import type { RecoveryStore } from '../files/recovery'
import { selectOriginVisible } from '../state/selectors'
import { createAppStore } from '../state/store'
import { startFeatureCommand } from './actions/command'
import { applyEdits, save, startCore } from './actions/document'
import { exportModel } from './actions/export'
import { notify } from './actions/notifications'
import { select } from './actions/selection'
import { detectPlatform } from './createApp'

const RENAME_TO_BOX: readonly Edit[] = [
  { type: 'update_body', body: { id: 'b-cube', name: 'Box' } },
]
const RENAME_TO_BLOCK: readonly Edit[] = [
  { type: 'update_body', body: { id: 'b-cube', name: 'Block' } },
]

function renamed(name: string): Document {
  return { ...CUBE_DOCUMENT, bodies: [{ id: 'b-cube', name }] }
}

function documentChanged(doc: number, revision: number, document: Document): ProtocolEvent {
  return {
    type: 'document_changed',
    doc,
    revision,
    document,
    dirty: true,
    transaction_open: false,
    bare_numbers: 0,
  }
}

/** What a `document_changed` the test delivers says about the document. */
type Change = {
  readonly recipe: Document
  readonly dirty: boolean
  readonly transactionOpen: boolean
}

/**
 * Delivers a `document_changed` of the shown document at the current (fake) time, in the response
 * to an `evaluate` request, with the next revision.
 */
async function deliverChange(fake: FakeApp, change: Change): Promise<void> {
  const { documentHandle, revision } = currentDocument(fake.app)
  const event: ProtocolEvent = {
    type: 'document_changed',
    doc: documentHandle,
    revision: revision + 1,
    document: change.recipe,
    dirty: change.dirty,
    transaction_open: change.transactionOpen,
    transaction: change.transactionOpen ? 'Box' : null,
    bare_numbers: 0,
  }
  fake.transport.handle('evaluate', (command, core) => ({
    ...core.reply(command),
    events: [event],
  }))
  await fake.app.core.evaluate(documentHandle, '1 mm')
  fake.transport.handle('evaluate', null)
  await settle()
}

/** Advances the fake clock and lets the writes it starts finish. */
async function advance(milliseconds: number): Promise<void> {
  await vi.advanceTimersByTimeAsync(milliseconds)
  await settle()
}

/** The cube renamed, as a settled change makes it. */
const R1 = renamed('Box')
/** R1 with a box the open Box command previews. */
const R2: Document = {
  ...R1,
  bodies: [...bodiesOf(R1), { id: 'b-2', name: 'Body 2' }],
  features: [
    ...(R1.features ?? []),
    {
      id: 'f-box1',
      type: 'box',
      body: 'b-2',
      size: ['10', '10', '10'],
      position: ['30', '0', '0'],
      op: 'new',
    },
  ],
}
/** The recipe after the Box command is committed: R1 with the box. */
const R3: Document = R2

/** The recipe of the settled change made at a time, in U-154. */
function recipeAt(milliseconds: number): Document {
  return renamed(`Box at ${String(milliseconds)}`)
}

function navigatorWith(fields: {
  readonly platform: string
  readonly userAgentData?: { readonly platform: string }
}): Navigator {
  return fields as unknown as Navigator
}

/** cube.ptsm opened from a file handle; clean until a test edits it. */
async function cubeWithHandle(
  recovery?: RecoveryStore,
): Promise<{ fake: FakeApp; handle: FileSystemFileHandle }> {
  const handle = createFakeFileHandle('cube.ptsm')
  const fake = await startWithDocument('cube.ptsm', CUBE_DOCUMENT, {
    handle,
    ...(recovery === undefined ? {} : { recovery }),
  })
  return { fake, handle }
}

afterEach(() => {
  vi.useRealTimers()
  vi.restoreAllMocks()
})

describe('detectPlatform', () => {
  it('maps navigator.platform strings containing Mac, Win and Linux', () => {
    expect(detectPlatform(navigatorWith({ platform: 'MacIntel' }), {}).operatingSystem).toBe('mac')
    expect(detectPlatform(navigatorWith({ platform: 'Win32' }), {}).operatingSystem).toBe('windows')
    expect(detectPlatform(navigatorWith({ platform: 'Linux x86_64' }), {}).operatingSystem).toBe(
      'linux',
    )
    expect(detectPlatform(navigatorWith({ platform: 'iPhone' }), {}).operatingSystem).toBe('other')
  })

  it('prefers navigator.userAgentData.platform when present', () => {
    const navigator = navigatorWith({
      platform: 'Linux x86_64',
      userAgentData: { platform: 'macOS' },
    })
    expect(detectPlatform(navigator, {}).operatingSystem).toBe('mac')
  })

  it('is the desktop host when the Tauri global exists, else the web host', () => {
    const navigator = navigatorWith({ platform: 'Win32' })
    expect(detectPlatform(navigator, { __TAURI__: {} })).toEqual({
      operatingSystem: 'windows',
      host: 'desktop',
    })
    expect(detectPlatform(navigator, {})).toEqual({ operatingSystem: 'windows', host: 'web' })
  })
})

describe('createApp', () => {
  it('assembles the app from its options without starting the core', () => {
    const store = createAppStore()
    const fake = createFakeApp({ store })
    expect(fake.app.store).toBe(store)
    expect(fake.app.files).toBe(fake.files)
    expect(fake.app.recent).toBe(fake.recent)
    expect(fake.app.recovery).toBe(fake.recovery)
    expect(fake.app.platform).toEqual(LINUX_WEB)
    expect(fake.transport.starts).toBe(0)
    expect(store.getState().core.status).toBe('loading')
  })

  it('creates its own store when none is given', () => {
    const { app } = createFakeApp()
    expect(app.store.getState().document).toBeNull()
  })

  it('has no viewport until one is attached', () => {
    const { app } = createFakeApp()
    expect(app.viewport()).toBeNull()
    app.attachViewport(null)
    expect(app.viewport()).toBeNull()
  })
})

describe('core events in the store', () => {
  it('applies the events of opening a document: recipe, statuses, sketch solutions and model meshes', async () => {
    const fake = await startWithDocument('cube.ptsm', CUBE_DOCUMENT, {
      core: { solutions: { 'f-sk1': CUBE_SKETCH_SOLUTION } },
    })
    const document = currentDocument(fake.app)
    expect(document.recipe).toEqual(CUBE_DOCUMENT)
    expect(document.dirty).toBe(false)
    expect(document.undoLabel).toBeNull()
    expect(document.featureStatus['f-ext1']?.status).toBe('ok')
    expect(document.featureStatus['f-sk1']?.status).toBe('ok')
    expect(document.sketches['f-sk1']?.solution).toEqual(CUBE_SKETCH_SOLUTION)
    const meshId = fake.transport.core.document(document.documentHandle).meshIds['b-cube']
    expect(document.meshes['b-cube']).toMatchObject({
      meshId,
      boundingBox: { min: [-10, -10, 0], max: [10, 10, 20] },
      faceCount: 1,
      edgeCount: 1,
      vertexCount: 1,
    })
  })

  it('applies the undo label and dirty flag of an edit', async () => {
    const fake = await startWithDocument('cube.ptsm', CUBE_DOCUMENT)
    await applyEdits(fake.app, 'Rename', RENAME_TO_BOX)
    await settle()
    const document = currentDocument(fake.app)
    expect(bodiesOf(document.recipe)[0]?.name).toBe('Box')
    expect(document.dirty).toBe(true)
    expect(document.undoLabel).toBe('Rename')
    expect(document.revision).toBe(fake.transport.core.document(document.documentHandle).revision)
  })

  it('U-18 ignores a document_changed whose revision is lower than the stored one', async () => {
    const fake = await startWithDocument('cube.ptsm', CUBE_DOCUMENT)
    await applyEdits(fake.app, 'Rename', RENAME_TO_BOX)
    await settle()
    const { documentHandle, revision } = currentDocument(fake.app)
    fake.transport.handle('apply_edits', () => ({
      result: { type: 'apply_edits', revision },
      events: [documentChanged(documentHandle, revision - 1, renamed('Stale'))],
    }))
    await applyEdits(fake.app, 'Rename', RENAME_TO_BLOCK)
    await settle()
    expect(currentDocument(fake.app).revision).toBe(revision)
    expect(bodiesOf(currentDocument(fake.app).recipe)[0]?.name).toBe('Box')
  })

  it('U-18 applies a document_changed whose revision equals the stored one', async () => {
    const fake = await startWithDocument('cube.ptsm', CUBE_DOCUMENT)
    await applyEdits(fake.app, 'Rename', RENAME_TO_BOX)
    await settle()
    const { documentHandle, revision } = currentDocument(fake.app)
    fake.transport.handle('apply_edits', () => ({
      result: { type: 'apply_edits', revision },
      events: [documentChanged(documentHandle, revision, renamed('Block'))],
    }))
    await applyEdits(fake.app, 'Rename', RENAME_TO_BLOCK)
    await settle()
    expect(currentDocument(fake.app).revision).toBe(revision)
    expect(bodiesOf(currentDocument(fake.app).recipe)[0]?.name).toBe('Block')
  })

  it('ignores a recomputed whose revision is lower than the stored one', async () => {
    const fake = await startWithDocument('cube.ptsm', CUBE_DOCUMENT)
    const { documentHandle, revision } = currentDocument(fake.app)
    fake.transport.handle('apply_edits', () => ({
      result: { type: 'apply_edits', revision },
      events: [
        {
          type: 'recomputed',
          doc: documentHandle,
          revision: revision - 1,
          features: [
            { id: 'f-sk1', status: 'ok', messages: [], repairs: [] },
            { id: 'f-ext1', status: 'error', messages: [], repairs: [] },
          ],
          parameters: [],
          diagnostics: [],
        },
      ],
    }))
    await applyEdits(fake.app, 'Rename', RENAME_TO_BOX)
    await settle()
    expect(currentDocument(fake.app).featureStatus['f-ext1']?.status).toBe('ok')
  })

  it('counts mutating requests sent and not yet answered', async () => {
    const fake = await startWithDocument('cube.ptsm', CUBE_DOCUMENT)
    fake.transport.pause()
    const edit = applyEdits(fake.app, 'Rename', RENAME_TO_BOX)
    await settle()
    expect(fake.app.store.getState().core.pendingMutations).toBe(1)
    fake.transport.resume()
    await edit
    await settle()
    expect(fake.app.store.getState().core.pendingMutations).toBe(0)
  })

  it('counts and times only the requests that rebuild the document, not a save or an export', async () => {
    const { fake } = await cubeWithHandle()
    const before = currentDocument(fake.app).lastRebuildMilliseconds
    fake.transport.pause()
    const exporting = exportModel(fake.app, { format: 'step', scope: 'visible' })
    await settle()
    expect(fake.app.store.getState().core.pendingMutations).toBe(0)
    fake.transport.resume()
    await exporting
    fake.transport.pause()
    const saving = save(fake.app)
    await settle()
    expect(fake.app.store.getState().core.pendingMutations).toBe(0)
    fake.transport.resume()
    await saving
    await settle()
    expect(fake.transport.commandTypes()).toEqual(
      expect.arrayContaining(['export', 'save_document', 'mark_saved']),
    )
    expect(currentDocument(fake.app).lastRebuildMilliseconds).toBe(before)
  })

  it('records the rebuild time of an edit on the document', async () => {
    const fake = await startWithDocument('cube.ptsm', CUBE_DOCUMENT)
    fake.app.store.setState((state) => ({
      document:
        state.document === null ? null : { ...state.document, lastRebuildMilliseconds: null },
    }))
    await applyEdits(fake.app, 'Rename', RENAME_TO_BOX)
    await settle()
    expect(currentDocument(fake.app).lastRebuildMilliseconds).toEqual(expect.any(Number))
  })

  it('stamps notifications with the injected clock', () => {
    const { app } = createFakeApp({ now: () => 1_234 })
    notify(app, { severity: 'info', message: 'Hello.', actions: [] })
    expect(app.store.getState().notifications[0]?.createdAt).toBe(1_234)
  })

  it('a fatal transport error sets the core status to failed with the reason', async () => {
    const fake = await startFakeApp()
    fake.transport.die('The worker crashed.')
    await settle()
    const { core } = fake.app.store.getState()
    expect(core.status).toBe('failed')
    expect(core.fatal).toContain('The worker crashed.')
  })
})

describe('recovery copy (3.19)', () => {
  it('U-77 a dirty document_changed writes an autosave entry under the session id 2 s later', async () => {
    const { fake, handle } = await cubeWithHandle()
    vi.useFakeTimers()
    await applyEdits(fake.app, 'Rename', RENAME_TO_BOX)
    await settle()
    await advance(1_999)
    await expect(fake.recovery.list()).resolves.toEqual([])
    await advance(1)
    const document = currentDocument(fake.app)
    const entries = await fake.recovery.list()
    expect(entries).toEqual([
      {
        id: document.recoveryId,
        name: 'cube',
        text: JSON.stringify(document.recipe, null, 2),
        savedAt: expect.any(Number),
        handle,
        reason: 'autosave',
      },
    ])
  })

  it('U-77 another change within 2 s postpones the write', async () => {
    const { fake } = await cubeWithHandle()
    vi.useFakeTimers()
    await applyEdits(fake.app, 'Rename', RENAME_TO_BOX)
    await settle()
    await advance(1_500)
    await applyEdits(fake.app, 'Rename', RENAME_TO_BLOCK)
    await settle()
    await advance(1_000)
    await expect(fake.recovery.list()).resolves.toEqual([])
    await advance(1_000)
    const entries = await fake.recovery.list()
    expect(entries.map((entry) => entry.text)).toEqual([JSON.stringify(renamed('Block'), null, 2)])
  })

  it('writes nothing for a clean document', async () => {
    const { fake } = await cubeWithHandle()
    vi.useFakeTimers()
    await advance(5_000)
    await expect(fake.recovery.list()).resolves.toEqual([])
  })

  it("U-77 a successful save removes the session's entry", async () => {
    const { fake } = await cubeWithHandle()
    vi.useFakeTimers()
    await applyEdits(fake.app, 'Rename', RENAME_TO_BOX)
    await settle()
    await advance(2_000)
    expect(await fake.recovery.list()).toHaveLength(1)
    await save(fake.app)
    await settle()
    await expect(fake.recovery.list()).resolves.toEqual([])
  })

  it('U-153 the copy is the settled recipe: changes inside a transaction neither write nor delay it', async () => {
    const recording = createRecordingRecovery()
    const { fake } = await cubeWithHandle(recording)
    vi.useFakeTimers()
    await deliverChange(fake, { recipe: R1, dirty: true, transactionOpen: false })
    await advance(500)
    await deliverChange(fake, { recipe: R2, dirty: true, transactionOpen: true })
    await advance(500)
    await deliverChange(fake, { recipe: R2, dirty: true, transactionOpen: true })
    await advance(900)
    await deliverChange(fake, { recipe: R2, dirty: true, transactionOpen: true })
    await advance(99)
    expect(recording.writes).toEqual([])
    await advance(1)
    expect(recording.writes.map((entry) => entry.text)).toEqual([JSON.stringify(R1, null, 2)])
    await advance(4_000)
    expect(recording.writes).toHaveLength(1)
    // The command is committed at 6,000 ms.
    await deliverChange(fake, { recipe: R3, dirty: true, transactionOpen: false })
    await advance(1_999)
    expect(recording.writes).toHaveLength(1)
    await advance(1)
    expect(recording.writes.map((entry) => entry.text)).toEqual([
      JSON.stringify(R1, null, 2),
      JSON.stringify(R3, null, 2),
    ])
    expect(recording.writes.every((entry) => entry.reason === 'autosave')).toBe(true)
  })

  it('U-153 a canceled command writes the recipe from before it, never its preview', async () => {
    const recording = createRecordingRecovery()
    const { fake } = await cubeWithHandle(recording)
    vi.useFakeTimers()
    await deliverChange(fake, { recipe: R1, dirty: true, transactionOpen: false })
    await advance(500)
    await deliverChange(fake, { recipe: R2, dirty: true, transactionOpen: true })
    await advance(500)
    await deliverChange(fake, { recipe: R2, dirty: true, transactionOpen: true })
    await advance(900)
    await deliverChange(fake, { recipe: R2, dirty: true, transactionOpen: true })
    await advance(4_100)
    // The command is canceled at 6,000 ms.
    await deliverChange(fake, { recipe: R1, dirty: true, transactionOpen: false })
    await advance(1_999)
    expect(recording.writes).toHaveLength(1)
    await advance(1)
    expect(recording.writes.map((entry) => entry.text)).toEqual([
      JSON.stringify(R1, null, 2),
      JSON.stringify(R1, null, 2),
    ])
    await advance(10_000)
    const previewText = JSON.stringify(R2, null, 2)
    expect(recording.writes.some((entry) => entry.text === previewText)).toBe(false)
  })

  it('U-154 a stream of settled changes is written at most 10 s after the first one no write stored', async () => {
    const { fake } = await cubeWithHandle()
    vi.useFakeTimers()
    for (const at of [0, 1_500, 3_000, 4_500, 6_000, 7_500, 9_000]) {
      if (at > 0) {
        // The changes arrive one after another on the fake clock.
        // oxlint-disable-next-line no-await-in-loop
        await advance(1_500)
      }
      // oxlint-disable-next-line no-await-in-loop
      await deliverChange(fake, { recipe: recipeAt(at), dirty: true, transactionOpen: false })
    }
    await advance(999)
    await expect(fake.recovery.list()).resolves.toEqual([])
    await advance(1)
    const textsAt = async (): Promise<string[]> =>
      (await fake.recovery.list()).map((entry) => entry.text)
    expect(await textsAt()).toEqual([JSON.stringify(recipeAt(9_000), null, 2)])

    await advance(500)
    await deliverChange(fake, { recipe: recipeAt(10_500), dirty: true, transactionOpen: false })
    await advance(1_999)
    expect(await textsAt()).toEqual([JSON.stringify(recipeAt(9_000), null, 2)])
    await advance(1)
    expect(await textsAt()).toEqual([JSON.stringify(recipeAt(10_500), null, 2)])
  })

  it('U-154 a settled change back to the saved revision removes the entry at once and drops a pending write', async () => {
    const { fake } = await cubeWithHandle()
    vi.useFakeTimers()
    await deliverChange(fake, { recipe: R1, dirty: true, transactionOpen: false })
    await advance(2_000)
    expect(await fake.recovery.list()).toHaveLength(1)
    // An undo back to the saved revision.
    await deliverChange(fake, { recipe: CUBE_DOCUMENT, dirty: false, transactionOpen: false })
    await expect(fake.recovery.list()).resolves.toEqual([])
    await advance(500)
    await deliverChange(fake, { recipe: R1, dirty: true, transactionOpen: false })
    await advance(500)
    await deliverChange(fake, { recipe: CUBE_DOCUMENT, dirty: false, transactionOpen: false })
    await advance(2_000)
    await expect(fake.recovery.list()).resolves.toEqual([])
    await advance(10_000)
    await expect(fake.recovery.list()).resolves.toEqual([])
  })
})

describe('recovery copy across saves and restarts (3.19, 4.4)', () => {
  it('U-167 a save keeps the entry while an edit made during its file write leaves the document unsaved', async () => {
    const { fake } = await cubeWithHandle()
    vi.useFakeTimers()
    await applyEdits(fake.app, 'Rename', RENAME_TO_BOX)
    await advance(2_000)
    expect(await fake.recovery.list()).toHaveLength(1)
    const released = deferred<void>()
    const write = fake.files.write.bind(fake.files)
    vi.spyOn(fake.files, 'write').mockImplementation(async (target, kind, bytes) => {
      await released.promise
      await write(target, kind, bytes)
    })
    const saving = save(fake.app)
    await settle()
    await applyEdits(fake.app, 'Rename', RENAME_TO_BLOCK)
    await settle()
    released.resolve()
    await saving
    await settle()
    expect(currentDocument(fake.app)).toMatchObject({ dirty: true, localUnsaved: false })
    expect(await fake.recovery.list()).toHaveLength(1)
    await advance(2_000)
    const texts = (await fake.recovery.list()).map((entry) => entry.text)
    expect(texts).toEqual([JSON.stringify(renamed('Block'), null, 2)])
  })

  it('U-166 a write that falls due while the core restarts is written right after the restart', async () => {
    const { fake, handle } = await cubeWithHandle()
    vi.useFakeTimers()
    await applyEdits(fake.app, 'Rename', RENAME_TO_BOX)
    await settle()
    fake.transport.die('The worker crashed.')
    await settle()
    fake.transport.failNextStart(new CoreFatalError('The geometry core did not start.'))
    await fake.app.restartCore()
    await settle()
    // The write falls due at 2,000 ms, while no document is shown.
    await advance(3_000)
    await expect(fake.recovery.list()).resolves.toEqual([])
    await fake.app.restartCore()
    await settle()
    await expect(fake.recovery.list()).resolves.toEqual([
      {
        id: currentDocument(fake.app).recoveryId,
        name: 'cube',
        text: JSON.stringify(R1, null, 2),
        savedAt: expect.any(Number),
        handle,
        reason: 'autosave',
      },
    ])
  })
})

describe('restartCore (4.4)', () => {
  it('U-84 reopens the last recipe in the new core, keeping the name and handle and setting localUnsaved', async () => {
    const handle = createFakeFileHandle('cube.ptsm')
    const fake = await startWithDocument('cube.ptsm', CUBE_DOCUMENT, { handle })
    await applyEdits(fake.app, 'Rename', RENAME_TO_BOX)
    await settle()
    const recipe = currentDocument(fake.app).recipe
    fake.transport.die('The worker crashed.')
    await settle()
    expect(fake.app.store.getState().core.status).toBe('failed')
    fake.transport.clearRequests()
    await fake.app.restartCore()
    await settle()
    expect(fake.transport.starts).toBe(2)
    const reopened = fake.transport.commandsOfType('open_document')
    expect(reopened).toHaveLength(1)
    expect(reopened[0]?.text).toBe(JSON.stringify(recipe, null, 2))
    const document = currentDocument(fake.app)
    expect(document.name).toBe('cube')
    expect(document.file?.handle).toBe(handle)
    expect(document.localUnsaved).toBe(true)
    expect(document.recipe).toEqual(recipe)
    expect(fake.app.store.getState().core.status).toBe('ready')
  })

  it('keeps the document when a restart fails, and reopens it on the next attempt', async () => {
    const handle = createFakeFileHandle('cube.ptsm')
    const fake = await startWithDocument('cube.ptsm', CUBE_DOCUMENT, { handle })
    await applyEdits(fake.app, 'Rename', RENAME_TO_BOX)
    await settle()
    const recipe = currentDocument(fake.app).recipe
    fake.transport.die('The worker crashed.')
    await settle()
    fake.transport.failNextStart(new CoreFatalError('The geometry core did not start.'))
    await fake.app.restartCore()
    await settle()
    expect(fake.app.store.getState().core).toMatchObject({
      status: 'failed',
      fatal: 'The geometry core did not start.',
    })
    fake.transport.clearRequests()
    await fake.app.restartCore()
    await settle()
    expect(fake.transport.commandsOfType('open_document').map((command) => command.text)).toEqual([
      JSON.stringify(recipe, null, 2),
    ])
    expect(fake.transport.commandTypes()).not.toContain('new_document')
    const document = currentDocument(fake.app)
    expect(document).toMatchObject({ name: 'cube', localUnsaved: true, recipe })
    expect(document.file?.handle).toBe(handle)
    expect(fake.app.store.getState().core.status).toBe('ready')
  })

  it('keeps the document when the new core refuses to open it, for the next attempt', async () => {
    const fake = await startWithDocument('cube.ptsm', CUBE_DOCUMENT)
    const recipe = currentDocument(fake.app).recipe
    fake.transport.die('The worker crashed.')
    await settle()
    fake.transport.handle('open_document', () => ({
      error: { code: 'internal_error', message: 'The core is not ready.' },
    }))
    await fake.app.restartCore()
    await settle()
    expect(fake.app.store.getState().core.status).toBe('failed')
    fake.transport.handle('open_document', null)
    fake.transport.clearRequests()
    await fake.app.restartCore()
    await settle()
    expect(fake.transport.commandsOfType('open_document').map((command) => command.text)).toEqual([
      JSON.stringify(recipe, null, 2),
    ])
    expect(currentDocument(fake.app).name).toBe('cube')
  })

  it('opens a new Untitled document when no document was open, with the origin hidden', async () => {
    const fake = createFakeApp()
    fake.transport.failNextStart(new CoreFatalError('HTTP 404'))
    await startCore(fake.app)
    await settle()
    expect(fake.app.store.getState().document).toBeNull()
    await fake.app.restartCore()
    await settle()
    expect(fake.transport.commandTypes()).toContain('new_document')
    expect(currentDocument(fake.app)).toMatchObject({ name: 'Untitled', localUnsaved: false })
    expect(selectOriginVisible(fake.app.store.getState(), 'origin:XY')).toBe(false)
    expect(fake.app.store.getState().core.status).toBe('ready')
  })

  it('reopens the recipe from before an open command, so its preview never becomes permanent', async () => {
    const fake = await startWithDocument('cube.ptsm', CUBE_DOCUMENT)
    const confirmed = currentDocument(fake.app).recipe
    await startFeatureCommand(fake.app, 'box')
    await settle()
    const previewed = currentDocument(fake.app)
    expect(previewed.transactionOpen).toBe(true)
    expect(previewed.recipe.features?.map((feature) => feature.type)).toContain('box')
    fake.transport.die('The core trapped.')
    await settle()
    fake.transport.clearRequests()
    await fake.app.restartCore()
    await settle()
    expect(fake.transport.commandsOfType('open_document').map((command) => command.text)).toEqual([
      JSON.stringify(confirmed, null, 2),
    ])
    expect(currentDocument(fake.app).recipe).toEqual(confirmed)
  })

  it('reloads the catalog and printers and clears selection, command and sketch session', async () => {
    const fake = await startWithDocument('cube.ptsm', CUBE_DOCUMENT)
    select(fake.app, [elementItem(CUBE_TOP_FACE, 'Cube')], 'replace')
    fake.transport.die('The worker crashed.')
    await settle()
    fake.transport.clearRequests()
    await fake.app.restartCore()
    await settle()
    expect(fake.transport.commandTypes()).toEqual(
      expect.arrayContaining(['feature_catalog', 'list_printer_profiles', 'open_document']),
    )
    const state = fake.app.store.getState()
    expect(state.catalog?.byType['extrude']).toBeDefined()
    expect(state.printers.map((profile) => profile.id)).toEqual(['generic-fdm', 'generic-resin'])
    expect(state.selection).toEqual([])
    expect(state.command).toBeNull()
    expect(state.sketch).toBeNull()
  })
})
