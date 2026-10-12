import { afterEach, describe, expect, it, vi } from 'vitest'
import type { Document, Edit, Event as ProtocolEvent } from '../generated/protocol'
import {
  createFakeApp,
  currentDocument,
  LINUX_WEB,
  startFakeApp,
  startWithDocument,
} from '../test/fakes/app'
import {
  CUBE_DOCUMENT,
  CUBE_SKETCH_SOLUTION,
  CUBE_TOP_FACE,
  bodiesOf,
} from '../test/fakes/documents'
import { createFakeFileHandle } from '../test/fakes/files'
import { elementItem } from '../test/fakes/selection'
import { settle } from '../test/fakes/settle'
import { createAppStore } from '../state/store'
import { applyEdits, save } from './actions/document'
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
  return { type: 'document_changed', doc, revision, document, dirty: true, transaction_open: false }
}

function navigatorWith(fields: {
  readonly platform: string
  readonly userAgentData?: { readonly platform: string }
}): Navigator {
  return fields as unknown as Navigator
}

/** cube.ptsm opened from a file handle; clean until a test edits it. */
async function cubeWithHandle(): Promise<{ fake: FakeApp; handle: FileSystemFileHandle }> {
  const handle = createFakeFileHandle('cube.ptsm')
  const fake = await startWithDocument('cube.ptsm', CUBE_DOCUMENT, { handle })
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
  it('U-77 a dirty document_changed writes an autosave entry 2 s later', async () => {
    const { fake, handle } = await cubeWithHandle()
    vi.useFakeTimers()
    await applyEdits(fake.app, 'Rename', RENAME_TO_BOX)
    await settle()
    await vi.advanceTimersByTimeAsync(1_999)
    await settle()
    await expect(fake.recovery.read()).resolves.toBeNull()
    await vi.advanceTimersByTimeAsync(1)
    await settle()
    const entry = await fake.recovery.read()
    expect(entry).toMatchObject({
      name: 'cube',
      text: JSON.stringify(currentDocument(fake.app).recipe, null, 2),
      handle,
      reason: 'autosave',
    })
    expect(typeof entry?.savedAt).toBe('number')
  })

  it('U-77 another change within 2 s postpones the write', async () => {
    const { fake } = await cubeWithHandle()
    vi.useFakeTimers()
    await applyEdits(fake.app, 'Rename', RENAME_TO_BOX)
    await settle()
    await vi.advanceTimersByTimeAsync(1_500)
    await applyEdits(fake.app, 'Rename', RENAME_TO_BLOCK)
    await settle()
    await vi.advanceTimersByTimeAsync(1_000)
    await settle()
    await expect(fake.recovery.read()).resolves.toBeNull()
    await vi.advanceTimersByTimeAsync(1_000)
    await settle()
    const entry = await fake.recovery.read()
    expect(entry?.text).toBe(JSON.stringify(renamed('Block'), null, 2))
  })

  it('writes nothing for a clean document', async () => {
    const { fake } = await cubeWithHandle()
    vi.useFakeTimers()
    await vi.advanceTimersByTimeAsync(5_000)
    await settle()
    await expect(fake.recovery.read()).resolves.toBeNull()
  })

  it('U-77 a successful save clears the store', async () => {
    const { fake } = await cubeWithHandle()
    vi.useFakeTimers()
    await applyEdits(fake.app, 'Rename', RENAME_TO_BOX)
    await settle()
    await vi.advanceTimersByTimeAsync(2_000)
    await settle()
    expect(await fake.recovery.read()).not.toBeNull()
    await save(fake.app)
    await settle()
    await expect(fake.recovery.read()).resolves.toBeNull()
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
