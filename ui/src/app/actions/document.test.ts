import { afterEach, describe, expect, it, vi } from 'vitest'
import { CoreFatalError } from '../../core/transport'
import { FileServiceError } from '../../files/file-service'
import { createMemoryRecovery } from '../../files/recovery'
import type { Document, Edit } from '../../generated/protocol'
import { MESSAGES } from '../../messages'
import { selectOriginVisible } from '../../state/selectors'
import {
  actionLabels,
  createFakeApp,
  currentDocument,
  type FakeApp,
  notificationWith,
  openTestDocument,
  runAction,
  startFakeApp,
  startWithDocument,
} from '../../test/fakes/app'
import {
  CUBE_DOCUMENT,
  CUBE_TOP_FACE,
  documentFile,
  documentText,
  bodiesOf,
  featuresOf,
  NC09_DOCUMENT,
} from '../../test/fakes/documents'
import { createFakeFileHandle } from '../../test/fakes/files'
import { elementItem } from '../../test/fakes/selection'
import { settle } from '../../test/fakes/settle'
import { startFeatureCommand } from './command'
import {
  applyEdits,
  newDocument,
  openFile,
  openRecent,
  openWithPicker,
  redo,
  save,
  saveAs,
  setKeepSizes,
  setUnits,
  startCore,
  undo,
} from './document'
import { select } from './selection'

const RENAME_TO_BOX: readonly Edit[] = [
  { type: 'update_body', body: { id: 'b-cube', name: 'Box' } },
]

function renamed(name: string): Document {
  return { ...CUBE_DOCUMENT, bodies: [{ id: 'b-cube', name }] }
}

/** cube.ptsm opened from a file handle, then renamed to "Box" so it is dirty. */
async function dirtyCubeWithHandle(): Promise<{ fake: FakeApp; handle: FileSystemFileHandle }> {
  const handle = createFakeFileHandle('cube.ptsm')
  const fake = await startWithDocument('cube.ptsm', CUBE_DOCUMENT, { handle })
  await applyEdits(fake.app, 'Rename', RENAME_TO_BOX)
  await settle()
  fake.transport.clearRequests()
  fake.journal.length = 0
  return { fake, handle }
}

function errorMessages(fake: FakeApp): string[] {
  return fake.app.store
    .getState()
    .notifications.filter((notification) => notification.severity === 'error')
    .map((notification) => notification.message)
}

afterEach(() => {
  vi.useRealTimers()
  vi.restoreAllMocks()
})

describe('startCore (3.1)', () => {
  it('starts the transport, loads the catalog and printers, and sets the core ready', async () => {
    const fake = createFakeApp()
    await startCore(fake.app)
    await settle()
    const state = fake.app.store.getState()
    expect(fake.transport.starts).toBe(1)
    expect(fake.transport.commandTypes()).toEqual(
      expect.arrayContaining(['feature_catalog', 'list_printer_profiles']),
    )
    expect(state.core.status).toBe('ready')
    expect(state.core.engineVersion).toBe('0.1.0')
    expect(state.printers.map((profile) => profile.id)).toEqual(['generic-fdm', 'generic-resin'])
  })

  it('registers one command per catalog feature', async () => {
    const fake = await startFakeApp()
    expect(fake.app.commands.get('feature.extrude')).toBeDefined()
    expect(fake.app.commands.get('feature.fillet')).toBeDefined()
  })

  it('opens a new mm document named "Untitled"', async () => {
    const fake = await startFakeApp()
    expect(fake.transport.commandsOfType('new_document')).toEqual([
      { type: 'new_document', units: 'mm' },
    ])
    const document = currentDocument(fake.app)
    expect(document.name).toBe('Untitled')
    expect(document.file).toBeNull()
    expect(document.dirty).toBe(false)
    expect(document.recipe.units).toBe('mm')
  })

  it('starts with the origin axes and point shown and the planes hidden, as New does (3.10)', async () => {
    const fake = await startFakeApp()
    const expectOriginOfEmptyDocument = (): void => {
      const state = fake.app.store.getState()
      for (const name of ['X', 'Y', 'Z', 'O']) {
        expect(selectOriginVisible(state, `origin:${name}`)).toBe(true)
      }
      for (const name of ['XY', 'XZ', 'YZ']) {
        expect(selectOriginVisible(state, `origin:${name}`)).toBe(false)
      }
    }
    expectOriginOfEmptyDocument()
    // Origin items get no session entry until their eye button or V writes one.
    const visible = fake.app.store.getState().view.sessionVisible
    expect(Object.keys(visible).some((key) => key.startsWith('origin:'))).toBe(false)
    await newDocument(fake.app)
    await settle()
    expect(fake.app.store.getState().view.sessionVisible).toEqual(visible)
    expectOriginOfEmptyDocument()
  })

  it('a transport that fails to start sets the core status to failed with the reason', async () => {
    const fake = createFakeApp()
    fake.transport.failNextStart(new CoreFatalError('HTTP 404'))
    await startCore(fake.app).catch(() => undefined)
    await settle()
    const { core, document } = fake.app.store.getState()
    expect(core.status).toBe('failed')
    expect(core.fatal).toContain('HTTP 404')
    expect(document).toBeNull()
  })

  it('a recovery entry shows the recovery notification and is not opened automatically', async () => {
    const recovery = createMemoryRecovery()
    await recovery.write({
      id: 'kept',
      name: 'cube',
      text: JSON.stringify(renamed('Box'), null, 2),
      savedAt: Date.UTC(2026, 9, 12, 9, 30),
      handle: null,
      reason: 'autosave',
    })
    const fake = await startFakeApp({ recovery })
    const notification = notificationWith(
      fake.app,
      /^Partsmith kept unsaved changes to cube from .+\.$/,
    )
    expect(notification.severity).toBe('info')
    expect(actionLabels(notification)).toEqual(['Open', 'Discard'])
    expect(currentDocument(fake.app).name).toBe('Untitled')
  })

  it("the recovery notification's Open opens the text named after the entry, with its handle, as locally unsaved", async () => {
    const handle = createFakeFileHandle('cube.ptsm')
    const recovery = createMemoryRecovery()
    await recovery.write({
      id: 'kept',
      name: 'cube',
      text: JSON.stringify(renamed('Box'), null, 2),
      savedAt: Date.UTC(2026, 9, 12, 9, 30),
      handle,
      reason: 'autosave',
    })
    const fake = await startFakeApp({ recovery })
    runAction(notificationWith(fake.app, /^Partsmith kept unsaved changes to cube/), 'Open')
    await settle()
    const document = currentDocument(fake.app)
    expect(document.name).toBe('cube')
    expect(bodiesOf(document.recipe)[0]?.name).toBe('Box')
    expect(document.file?.handle).toBe(handle)
    expect(document.localUnsaved).toBe(true)
  })

  it("the recovery notification's Discard removes that entry", async () => {
    const recovery = createMemoryRecovery()
    await recovery.write({
      id: 'kept',
      name: 'cube',
      text: JSON.stringify(renamed('Box'), null, 2),
      savedAt: Date.UTC(2026, 9, 12, 9, 30),
      handle: null,
      reason: 'autosave',
    })
    const fake = await startFakeApp({ recovery })
    runAction(notificationWith(fake.app, /^Partsmith kept unsaved changes to cube/), 'Discard')
    await settle()
    await expect(recovery.list()).resolves.toEqual([])
    expect(currentDocument(fake.app).name).toBe('Untitled')
  })
})

describe('newDocument (3.19)', () => {
  it('closes the current document and opens a new mm document named "Untitled" in the Model context', async () => {
    const fake = await startWithDocument('cube.ptsm', CUBE_DOCUMENT)
    const previous = currentDocument(fake.app).documentHandle
    select(fake.app, [elementItem(CUBE_TOP_FACE, 'Cube')], 'replace')
    fake.app.store.setState({ context: 'print' })
    await newDocument(fake.app)
    await settle()
    expect(fake.transport.commandsOfType('close_document')).toEqual([
      { type: 'close_document', doc: previous },
    ])
    expect(fake.transport.commandsOfType('new_document')).toEqual([
      { type: 'new_document', units: 'mm' },
    ])
    const state = fake.app.store.getState()
    expect(state.document?.name).toBe('Untitled')
    expect(state.document?.file).toBeNull()
    expect(state.document?.documentHandle).not.toBe(previous)
    expect(state.selection).toEqual([])
    expect(state.command).toBeNull()
    expect(state.sketch).toBeNull()
    expect(state.context).toBe('model')
  })

  it("a New without changes removes the session's leftover entry and keeps every other entry", async () => {
    const fake = await startWithDocument('cube.ptsm', CUBE_DOCUMENT)
    const other = {
      id: 'other',
      name: 'older',
      text: '{}',
      savedAt: 0,
      handle: null,
      reason: 'autosave',
    } as const
    await fake.recovery.write(other)
    await fake.recovery.write({
      id: currentDocument(fake.app).recoveryId,
      name: 'cube',
      text: '{}',
      savedAt: 1,
      handle: null,
      reason: 'autosave',
    })
    await newDocument(fake.app)
    await settle()
    await expect(fake.recovery.list()).resolves.toEqual([other])
  })

  it('replacing a dirty document stashes it under its session id before closing it and offers to reopen it', async () => {
    const { fake, handle } = await dirtyCubeWithHandle()
    const { recipe, recoveryId } = currentDocument(fake.app)
    await newDocument(fake.app)
    await settle()
    const write = fake.journal.indexOf('recovery.write')
    expect(write).toBeGreaterThanOrEqual(0)
    expect(write).toBeLessThan(fake.journal.indexOf('command:close_document'))
    await expect(fake.recovery.list()).resolves.toEqual([
      {
        id: recoveryId,
        name: 'cube',
        text: JSON.stringify(recipe, null, 2),
        savedAt: expect.any(Number),
        handle,
        reason: 'replaced',
      },
    ])
    expect(currentDocument(fake.app).recoveryId).not.toBe(recoveryId)
    const stashed = notificationWith(fake.app, MESSAGES.stashed('cube'))
    expect(stashed.severity).toBe('info')
    expect(actionLabels(stashed)).toEqual(['Reopen'])
  })

  it('Reopen opens the stashed copy again', async () => {
    const { fake } = await dirtyCubeWithHandle()
    await newDocument(fake.app)
    await settle()
    runAction(notificationWith(fake.app, MESSAGES.stashed('cube')), 'Reopen')
    await settle()
    const document = currentDocument(fake.app)
    expect(document.name).toBe('cube')
    expect(bodiesOf(document.recipe)[0]?.name).toBe('Box')
    expect(document.localUnsaved).toBe(true)
  })
})

/** cube.ptsm, clean, with a Box command whose preview made the document dirty. */
async function cubeWithBoxPreview(): Promise<FakeApp> {
  const fake = await startWithDocument('cube.ptsm', CUBE_DOCUMENT)
  await startFeatureCommand(fake.app, 'box')
  await settle()
  expect(currentDocument(fake.app).dirty).toBe(true)
  fake.transport.clearRequests()
  fake.journal.length = 0
  return fake
}

describe('replacing a document during a feature command (3.7)', () => {
  it('U-109 New cancels the command before closing the document, so the preview is not kept as changes', async () => {
    const fake = await cubeWithBoxPreview()
    await newDocument(fake.app)
    await settle()
    const cancel = fake.journal.indexOf('command:cancel_transaction')
    expect(cancel).toBeGreaterThanOrEqual(0)
    expect(cancel).toBeLessThan(fake.journal.indexOf('command:close_document'))
    expect(fake.journal).not.toContain('recovery.write')
    expect(fake.app.store.getState().notifications).toEqual([])
    expect(fake.app.store.getState().command).toBeNull()
  })

  it('U-109 Open cancels the command before closing the document, so nothing is stashed', async () => {
    const fake = await cubeWithBoxPreview()
    await openFile(fake.app, documentFile('other.ptsm', renamed('Other')))
    await settle()
    const cancel = fake.journal.indexOf('command:cancel_transaction')
    expect(cancel).toBeGreaterThanOrEqual(0)
    expect(cancel).toBeLessThan(fake.journal.indexOf('command:close_document'))
    expect(fake.journal).not.toContain('recovery.write')
    expect(currentDocument(fake.app).name).toBe('other')
    expect(fake.app.store.getState().notifications).toEqual([])
  })
})

describe('document edits during a feature command (3.7)', () => {
  it('Save, Save as and units are refused with finish_command_first and send nothing', async () => {
    const fake = await startWithDocument('cube.ptsm', CUBE_DOCUMENT, {
      handle: createFakeFileHandle('cube.ptsm'),
    })
    await startFeatureCommand(fake.app, 'box')
    await settle()
    fake.transport.clearRequests()
    fake.journal.length = 0
    await save(fake.app)
    await saveAs(fake.app)
    await setUnits(fake.app, 'in')
    await settle()
    expect(fake.transport.commandTypes()).toEqual([])
    expect(fake.journal).toEqual([])
    expect(fake.app.store.getState().notifications).toMatchObject([
      {
        severity: 'info',
        message: MESSAGES.finish_command_first('Box'),
        code: 'finish_command_first',
      },
    ])
  })
})

describe('openFile (3.19)', () => {
  it('U-79 a file that is not .ptsm shows not_ptsm and opens nothing', async () => {
    const fake = await startFakeApp()
    fake.transport.clearRequests()
    await openFile(fake.app, documentFile('model.stl', CUBE_DOCUMENT))
    await settle()
    expect(errorMessages(fake)).toEqual([MESSAGES.not_ptsm('model.stl')])
    expect(fake.transport.commandTypes()).not.toContain('open_document')
    expect(currentDocument(fake.app).name).toBe('Untitled')
  })

  it('U-79 bytes that are not UTF-8 show not_text and open nothing', async () => {
    const fake = await startFakeApp()
    fake.transport.clearRequests()
    await openFile(fake.app, {
      name: 'bad.ptsm',
      bytes: Uint8Array.of(0x7b, 0xff, 0xfe, 0x7d),
      handle: null,
    })
    await settle()
    expect(errorMessages(fake)).toEqual([MESSAGES.not_text('bad.ptsm')])
    expect(fake.transport.commandTypes()).not.toContain('open_document')
  })

  it('U-79 a parse_error shows open_failed with the core message and keeps the current document', async () => {
    const fake = await startWithDocument('cube.ptsm', CUBE_DOCUMENT)
    const before = currentDocument(fake.app)
    const message = 'The file is not a valid Partsmith document: expected value at line 1 column 1.'
    fake.transport.handle('open_document', () => ({ error: { code: 'parse_error', message } }))
    await openFile(fake.app, {
      name: 'broken.ptsm',
      bytes: Uint8Array.from(new TextEncoder().encode('not json')),
      handle: null,
    })
    await settle()
    expect(errorMessages(fake)).toEqual([MESSAGES.open_failed('broken.ptsm', message)])
    expect(fake.transport.commandTypes()).not.toContain('close_document')
    expect(currentDocument(fake.app).documentHandle).toBe(before.documentHandle)
    expect(currentDocument(fake.app).name).toBe('cube')
  })

  it('U-79 with a dirty current document the stash is written before closeDocument', async () => {
    const { fake } = await dirtyCubeWithHandle()
    await openFile(fake.app, documentFile('other.ptsm', renamed('Other')))
    await settle()
    const write = fake.journal.indexOf('recovery.write')
    expect(write).toBeGreaterThanOrEqual(0)
    expect(write).toBeLessThan(fake.journal.indexOf('command:close_document'))
    await expect(fake.recovery.list()).resolves.toMatchObject([
      { name: 'cube', reason: 'replaced' },
    ])
    expect(currentDocument(fake.app).name).toBe('other')
    notificationWith(fake.app, MESSAGES.stashed('cube'))
  })

  it('opens the text, adopts the handle and the name without the extension, and closes the old document', async () => {
    const fake = await startFakeApp()
    const untitled = currentDocument(fake.app).documentHandle
    fake.app.store.setState({ context: 'print' })
    const handle = createFakeFileHandle('Cube.PTSM')
    await openFile(fake.app, documentFile('Cube.PTSM', CUBE_DOCUMENT, handle))
    await settle()
    expect(fake.transport.commandsOfType('open_document')[0]?.text).toBe(
      documentText(CUBE_DOCUMENT),
    )
    expect(fake.transport.commandsOfType('close_document')).toEqual([
      { type: 'close_document', doc: untitled },
    ])
    const document = currentDocument(fake.app)
    expect(document.name).toBe('Cube')
    expect(document.file).toEqual({ name: 'Cube.PTSM', handle })
    expect(document.recipe).toEqual(CUBE_DOCUMENT)
    expect(document.localUnsaved).toBe(false)
    expect(fake.app.store.getState().context).toBe('model')
  })

  it('remembers a file with a handle in recent files by its handle', async () => {
    const fake = await startFakeApp()
    const handle = createFakeFileHandle('cube.ptsm')
    await openTestDocument(fake, 'cube.ptsm', CUBE_DOCUMENT, handle)
    const [entry] = await fake.recent.list()
    expect(entry).toMatchObject({ name: 'cube.ptsm', handle, text: null })
  })

  it('remembers a file without a handle in recent files by its text', async () => {
    const fake = await startFakeApp()
    await openTestDocument(fake, 'cube.ptsm', CUBE_DOCUMENT)
    const [entry] = await fake.recent.list()
    expect(entry).toMatchObject({
      name: 'cube.ptsm',
      handle: null,
      text: documentText(CUBE_DOCUMENT),
    })
  })

  it('a migrated file shows the migrated notification', async () => {
    const fake = await startFakeApp()
    fake.transport.handle('open_document', (command, core) => {
      const reply = core.reply(command)
      return 'result' in reply && reply.result.type === 'open_document'
        ? { ...reply, result: { ...reply.result, migrated_from: 0 } }
        : reply
    })
    await openTestDocument(fake, 'cube.ptsm', CUBE_DOCUMENT)
    const notification = notificationWith(fake.app, MESSAGES.migrated('cube.ptsm', 0))
    expect(notification.severity).toBe('info')
  })
})

describe('openWithPicker', () => {
  it('opens the picked file', async () => {
    const fake = await startFakeApp()
    fake.files.nextOpen = documentFile('cube.ptsm', CUBE_DOCUMENT)
    await openWithPicker(fake.app)
    await settle()
    expect(currentDocument(fake.app).name).toBe('cube')
  })

  it('does nothing when the picker is canceled', async () => {
    const fake = await startFakeApp()
    fake.transport.clearRequests()
    fake.files.nextOpen = null
    await openWithPicker(fake.app)
    await settle()
    expect(fake.transport.commandTypes()).toEqual([])
    expect(currentDocument(fake.app).name).toBe('Untitled')
  })

  it('a picker that fails shows its message as an error and opens nothing', async () => {
    const fake = await startFakeApp()
    fake.transport.clearRequests()
    vi.spyOn(fake.files, 'pickToOpen').mockRejectedValue(
      new FileServiceError('not_found', 'part.ptsm was not found.'),
    )
    await openWithPicker(fake.app)
    await settle()
    const notification = notificationWith(fake.app, 'part.ptsm was not found.')
    expect(notification.severity).toBe('error')
    expect(actionLabels(notification)).toEqual([])
    expect(fake.transport.commandTypes()).toEqual([])
    expect(currentDocument(fake.app).name).toBe('Untitled')
  })
})

describe('openRecent (3.19)', () => {
  it('opens an entry with text without a handle', async () => {
    const fake = await startFakeApp()
    await openRecent(fake.app, {
      id: 'recent-1',
      name: 'cube.ptsm',
      openedAt: 0,
      handle: null,
      text: documentText(CUBE_DOCUMENT),
    })
    await settle()
    const document = currentDocument(fake.app)
    expect(document.name).toBe('cube')
    expect(document.file?.handle ?? null).toBeNull()
  })

  it('reads an entry with a handle through the file service', async () => {
    const fake = await startFakeApp()
    const handle = createFakeFileHandle('cube.ptsm')
    fake.files.contents.set(handle, documentText(CUBE_DOCUMENT))
    await openRecent(fake.app, {
      id: 'recent-1',
      name: 'cube.ptsm',
      openedAt: 0,
      handle,
      text: null,
    })
    await settle()
    expect(fake.files.reads).toEqual([handle])
    expect(currentDocument(fake.app).file?.handle).toBe(handle)
    expect(currentDocument(fake.app).recipe).toEqual(CUBE_DOCUMENT)
  })

  it('a denied permission shows recent_permission with the action "Open..."', async () => {
    const fake = await startFakeApp()
    const handle = createFakeFileHandle('cube.ptsm')
    fake.files.readError = new FileServiceError('permission', 'Permission denied.')
    await openRecent(fake.app, {
      id: 'recent-1',
      name: 'cube.ptsm',
      openedAt: 0,
      handle,
      text: null,
    })
    await settle()
    const notification = notificationWith(fake.app, MESSAGES.recent_permission('cube.ptsm'))
    expect(notification.severity).toBe('error')
    expect(actionLabels(notification)).toEqual(['Open...'])
    expect(currentDocument(fake.app).name).toBe('Untitled')
  })

  it('a missing file shows recent_missing and forgets the entry', async () => {
    const fake = await startFakeApp()
    const handle = createFakeFileHandle('cube.ptsm')
    await fake.recent.remember({ name: 'cube.ptsm', handle, text: documentText(CUBE_DOCUMENT) })
    const [entry] = await fake.recent.list()
    if (entry === undefined) {
      throw new Error('The recent entry was not remembered.')
    }
    fake.files.readError = new FileServiceError('not_found', 'The file is gone.')
    await openRecent(fake.app, entry)
    await settle()
    expect(notificationWith(fake.app, MESSAGES.recent_missing('cube.ptsm')).severity).toBe(
      'warning',
    )
    await expect(fake.recent.list()).resolves.toEqual([])
  })
})

describe('save (3.19)', () => {
  it('U-86 calls saveDocument, files.write, then markSaved with the returned revision, in that order', async () => {
    const { fake, handle } = await dirtyCubeWithHandle()
    const revision = currentDocument(fake.app).revision
    await save(fake.app)
    await settle()
    expect(fake.journal.filter((entry) => !entry.startsWith('recovery.'))).toEqual([
      'command:save_document',
      'files.write',
      'command:mark_saved',
    ])
    const [write] = fake.files.writes
    expect(write?.kind).toBe('ptsm')
    expect(write?.target).toEqual({ kind: 'handle', name: 'cube.ptsm', handle })
    expect(new TextDecoder().decode(write?.bytes)).toBe(documentText(renamed('Box')))
    expect(fake.transport.commandsOfType('mark_saved')).toEqual([
      { type: 'mark_saved', doc: currentDocument(fake.app).documentHandle, revision },
    ])
  })

  it("a successful save leaves the document clean and not locally unsaved, and removes the session's entry", async () => {
    const { fake } = await dirtyCubeWithHandle()
    await fake.recovery.write({
      id: currentDocument(fake.app).recoveryId,
      name: 'cube',
      text: '{}',
      savedAt: 0,
      handle: null,
      reason: 'autosave',
    })
    await save(fake.app)
    await settle()
    const document = currentDocument(fake.app)
    expect(document.dirty).toBe(false)
    expect(document.localUnsaved).toBe(false)
    await expect(fake.recovery.list()).resolves.toEqual([])
  })

  it('U-78 a write refused for permission sends no markSaved, keeps the document dirty and offers Save as', async () => {
    const { fake } = await dirtyCubeWithHandle()
    fake.files.writeError = new FileServiceError('permission', 'Write permission was denied.')
    await save(fake.app)
    await settle()
    expect(fake.transport.commandTypes()).not.toContain('mark_saved')
    expect(currentDocument(fake.app).dirty).toBe(true)
    const notification = notificationWith(fake.app, MESSAGES.save_permission('cube.ptsm'))
    expect(notification.severity).toBe('error')
    expect(actionLabels(notification)).toEqual(['Save as'])
  })

  it('a file that was moved or deleted shows save_not_found with Save as', async () => {
    const { fake } = await dirtyCubeWithHandle()
    fake.files.writeError = new FileServiceError('not_found', 'The file is gone.')
    await save(fake.app)
    await settle()
    expect(actionLabels(notificationWith(fake.app, MESSAGES.save_not_found('cube.ptsm')))).toEqual([
      'Save as',
    ])
    expect(fake.transport.commandTypes()).not.toContain('mark_saved')
  })

  it('any other write failure shows save_io with the reason and Save as', async () => {
    const { fake } = await dirtyCubeWithHandle()
    fake.files.writeError = new FileServiceError('io', 'The disk is full.')
    await save(fake.app)
    await settle()
    expect(
      actionLabels(notificationWith(fake.app, MESSAGES.save_io('cube.ptsm', 'The disk is full.'))),
    ).toEqual(['Save as'])
  })

  it('without a file handle, Save asks where to save, as Save as does', async () => {
    const fake = await startWithDocument('cube.ptsm', CUBE_DOCUMENT)
    await save(fake.app)
    await settle()
    expect(fake.files.saveRequests).toEqual([{ suggestedName: 'cube.ptsm', kind: 'ptsm' }])
    expect(fake.files.writes).toHaveLength(1)
  })
})

describe('saveAs (3.19)', () => {
  it('U-78 canceled at the picker, it never calls saveDocument', async () => {
    const fake = await startWithDocument('cube.ptsm', CUBE_DOCUMENT)
    fake.files.nextSaveTarget = null
    await saveAs(fake.app)
    await settle()
    expect(fake.files.saveRequests).toEqual([{ suggestedName: 'cube.ptsm', kind: 'ptsm' }])
    expect(fake.transport.commandTypes()).not.toContain('save_document')
    expect(fake.files.writes).toEqual([])
  })

  it('a picker that fails shows the Save message with Save as and never calls saveDocument', async () => {
    const fake = await startWithDocument('cube.ptsm', CUBE_DOCUMENT)
    const reason = 'cube.ptsm already exists. Choose it in the dialog to replace it.'
    vi.spyOn(fake.files, 'chooseSaveTarget').mockRejectedValue(new FileServiceError('io', reason))
    await saveAs(fake.app)
    await settle()
    const notification = notificationWith(fake.app, MESSAGES.save_io('cube.ptsm', reason))
    expect(notification.severity).toBe('error')
    expect(actionLabels(notification)).toEqual(['Save as'])
    expect(fake.transport.commandTypes()).not.toContain('save_document')
    expect(fake.files.writes).toEqual([])
  })

  it('a picker refused for permission shows save_permission with Save as', async () => {
    const fake = await startWithDocument('cube.ptsm', CUBE_DOCUMENT)
    vi.spyOn(fake.files, 'chooseSaveTarget').mockRejectedValue(
      new FileServiceError('permission', 'Partsmith has no access to cube.ptsm: denied'),
    )
    await saveAs(fake.app)
    await settle()
    expect(actionLabels(notificationWith(fake.app, MESSAGES.save_permission('cube.ptsm')))).toEqual(
      ['Save as'],
    )
  })

  it('asks for the target before saving, writes it, then marks the revision saved', async () => {
    const { fake } = await dirtyCubeWithHandle()
    await saveAs(fake.app)
    await settle()
    expect(fake.journal.filter((entry) => !entry.startsWith('recovery.'))).toEqual([
      'files.chooseSaveTarget',
      'command:save_document',
      'files.write',
      'command:mark_saved',
    ])
  })

  it('a handle target becomes the document file, named after the handle, and is remembered', async () => {
    const fake = await startWithDocument('cube.ptsm', CUBE_DOCUMENT)
    const handle = createFakeFileHandle('bracket.ptsm')
    fake.files.nextSaveTarget = { kind: 'handle', name: 'bracket.ptsm', handle }
    await saveAs(fake.app)
    await settle()
    const document = currentDocument(fake.app)
    expect(document.file).toEqual({ name: 'bracket.ptsm', handle })
    expect(document.name).toBe('bracket')
    const [entry] = await fake.recent.list()
    expect(entry).toMatchObject({ name: 'bracket.ptsm', handle })
  })

  it('a failed write sends no markSaved', async () => {
    const fake = await startWithDocument('cube.ptsm', CUBE_DOCUMENT)
    fake.files.writeError = new FileServiceError('io', 'The disk is full.')
    await saveAs(fake.app)
    await settle()
    expect(fake.transport.commandTypes()).not.toContain('mark_saved')
  })

  it('a download target clears the handle and the first download in a session shows download_hint once', async () => {
    const fake = await startWithDocument('cube.ptsm', CUBE_DOCUMENT, {
      fileSystemAccess: false,
      handle: createFakeFileHandle('cube.ptsm'),
    })
    await saveAs(fake.app)
    await settle()
    expect(fake.files.writes[0]?.target).toEqual({ kind: 'download', name: 'cube.ptsm' })
    expect(currentDocument(fake.app).file?.handle ?? null).toBeNull()
    expect(notificationWith(fake.app, MESSAGES.download_hint('cube.ptsm')).severity).toBe('info')
    await saveAs(fake.app)
    await settle()
    const hints = fake.app.store
      .getState()
      .notifications.filter(
        (notification) => notification.message === MESSAGES.download_hint('cube.ptsm'),
      )
    expect(hints).toHaveLength(1)
    expect(fake.transport.commandTypes().filter((type) => type === 'mark_saved')).toHaveLength(2)
  })
})

describe('applyEdits', () => {
  it('sends the edits with the label and resolves true', async () => {
    const fake = await startWithDocument('cube.ptsm', CUBE_DOCUMENT)
    await expect(applyEdits(fake.app, 'Rename', RENAME_TO_BOX)).resolves.toBe(true)
    expect(fake.transport.commandsOfType('apply_edits')).toEqual([
      {
        type: 'apply_edits',
        doc: currentDocument(fake.app).documentHandle,
        label: 'Rename',
        edits: RENAME_TO_BOX,
      },
    ])
  })

  it('a CoreError resolves false and shows the core message as an error notification', async () => {
    const fake = await startWithDocument('cube.ptsm', CUBE_DOCUMENT)
    const applied = await applyEdits(fake.app, 'Delete Widget 1', [
      { type: 'remove_feature', id: 'f-x' },
    ])
    await settle()
    expect(applied).toBe(false)
    expect(errorMessages(fake)).toEqual(['There is no feature with id f-x.'])
  })

  it('an internal_error is shown verbatim and changes nothing else', async () => {
    const fake = await startWithDocument('cube.ptsm', CUBE_DOCUMENT)
    const message =
      'Something went wrong inside Partsmith (kernel panic). Save your work and reload. Please report this.'
    fake.transport.handle('apply_edits', () => ({ error: { code: 'internal_error', message } }))
    await applyEdits(fake.app, 'Rename', RENAME_TO_BOX)
    await settle()
    expect(errorMessages(fake)).toEqual([message])
    expect(currentDocument(fake.app).recipe).toEqual(CUBE_DOCUMENT)
    expect(fake.app.store.getState().core.status).toBe('ready')
  })
})

describe('undo and redo (3.20)', () => {
  it('undo and redo call the core through the mutation queue', async () => {
    const fake = await startWithDocument('cube.ptsm', CUBE_DOCUMENT)
    await applyEdits(fake.app, 'Rename', RENAME_TO_BOX)
    await undo(fake.app)
    await settle()
    expect(bodiesOf(currentDocument(fake.app).recipe)[0]?.name).toBe('Cube')
    expect(currentDocument(fake.app).redoLabel).toBe('Rename')
    await redo(fake.app)
    await settle()
    expect(bodiesOf(currentDocument(fake.app).recipe)[0]?.name).toBe('Box')
    expect(fake.transport.commandTypes()).toEqual(['apply_edits', 'undo', 'redo'])
  })

  it('announces "Undid {label}" and "Redid {label}" in the polite live region', async () => {
    const fake = await startWithDocument('cube.ptsm', CUBE_DOCUMENT)
    await applyEdits(fake.app, 'Rename', RENAME_TO_BOX)
    await undo(fake.app)
    await settle()
    const undone = fake.app.store.getState().announcement
    expect(undone?.message).toBe('Undid Rename')
    await redo(fake.app)
    await settle()
    const redone = fake.app.store.getState().announcement
    expect(redone?.message).toBe('Redid Rename')
    expect(redone?.id).not.toBe(undone?.id)
  })

  it('announces nothing when there is nothing to undo', async () => {
    const fake = await startWithDocument('cube.ptsm', CUBE_DOCUMENT)
    await undo(fake.app)
    await settle()
    expect(fake.app.store.getState().announcement).toBeNull()
  })

  it('U-66 Mod+Z with a feature command active cancels the command and calls no undo', async () => {
    const fake = await startWithDocument('cube.ptsm', CUBE_DOCUMENT)
    await applyEdits(fake.app, 'Rename', RENAME_TO_BOX)
    await startFeatureCommand(fake.app, 'box')
    await settle()
    expect(fake.app.store.getState().command).not.toBeNull()
    fake.transport.clearRequests()
    await undo(fake.app)
    await settle()
    expect(fake.transport.commandTypes()).toContain('cancel_transaction')
    expect(fake.transport.commandTypes()).not.toContain('undo')
    expect(fake.app.store.getState().command).toBeNull()
    expect(bodiesOf(currentDocument(fake.app).recipe).map((body) => body.name)).toEqual(['Box'])
  })
})

describe('setUnits (3.21, U-138)', () => {
  it('sends set_document_units without keep_sizes when no number lacks a unit, and explains what changed', async () => {
    const fake = await startWithDocument('cube.ptsm', CUBE_DOCUMENT)
    expect(currentDocument(fake.app).bareNumbers).toBe(0)
    await setUnits(fake.app, 'cm')
    await settle()
    expect(fake.transport.commandsOfType('set_document_units')).toEqual([
      {
        type: 'set_document_units',
        doc: currentDocument(fake.app).documentHandle,
        units: 'cm',
        keep_sizes: false,
      },
    ])
    expect(fake.transport.commandsOfType('apply_edits')).toEqual([])
    expect(currentDocument(fake.app).recipe.units).toBe('cm')
    expect(currentDocument(fake.app).undoLabel).toBe('Change units to cm')
    expect(notificationWith(fake.app, MESSAGES.units_changed('cm')).severity).toBe('info')
  })

  it('U-138 keeps sizes while Keep sizes is on and numbers lack a unit: they get the old unit', async () => {
    const fake = await startWithDocument('nc09.ptsm', NC09_DOCUMENT)
    expect(currentDocument(fake.app).bareNumbers).toBe(4)
    expect(fake.app.store.getState().keepSizes).toBe(true)
    await setUnits(fake.app, 'in')
    await settle()
    expect(fake.transport.commandsOfType('set_document_units')).toMatchObject([
      { units: 'in', keep_sizes: true },
    ])
    const recipe = currentDocument(fake.app).recipe
    expect(recipe.units).toBe('in')
    expect(featuresOf(recipe)[0]?.['size']).toEqual(['40 mm', '20 mm', '10 mm'])
    expect(featuresOf(recipe)[1]?.['radius']).toBe('2 mm')
    expect(currentDocument(fake.app).bareNumbers).toBe(0)
    expect(currentDocument(fake.app).undoLabel).toBe('Change units to in')
  })

  it('U-138 sends keep_sizes false once Keep sizes is turned off, and the numbers keep no unit', async () => {
    const fake = await startWithDocument('nc09.ptsm', NC09_DOCUMENT)
    setKeepSizes(fake.app, false)
    expect(fake.app.store.getState().keepSizes).toBe(false)
    await setUnits(fake.app, 'in')
    await settle()
    expect(fake.transport.commandsOfType('set_document_units')).toMatchObject([
      { units: 'in', keep_sizes: false },
    ])
    expect(featuresOf(currentDocument(fake.app).recipe)[1]?.['radius']).toBe('2')
    expect(currentDocument(fake.app).bareNumbers).toBe(4)
  })

  it('U-138 one undo restores the units and the numbers without a unit', async () => {
    const fake = await startWithDocument('nc09.ptsm', NC09_DOCUMENT)
    await setUnits(fake.app, 'in')
    await settle()
    await undo(fake.app)
    await settle()
    const recipe = currentDocument(fake.app).recipe
    expect(recipe.units).toBe('mm')
    expect(featuresOf(recipe)[0]?.['size']).toEqual(['40', '20', '10'])
    expect(currentDocument(fake.app).bareNumbers).toBe(4)
  })

  it('setKeepSizes lasts for the session: a new document keeps the choice', async () => {
    const fake = await startWithDocument('nc09.ptsm', NC09_DOCUMENT)
    setKeepSizes(fake.app, false)
    await newDocument(fake.app)
    await settle()
    expect(fake.app.store.getState().keepSizes).toBe(false)
    setKeepSizes(fake.app, true)
    expect(fake.app.store.getState().keepSizes).toBe(true)
  })
})
