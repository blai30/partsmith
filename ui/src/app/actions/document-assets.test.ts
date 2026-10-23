import { afterEach, describe, expect, it, vi, type Mock } from 'vitest'
import type { AssetFile } from '../../files/assets'
import { fileDigest } from '../../files/digest'
import {
  FileServiceError,
  type DocumentPlace,
  type FileHandle,
  type SaveTarget,
} from '../../files/file-service'
import { createMemoryRecovery, type RecoveryEntry } from '../../files/recovery'
import type { Edit, FeatureRecord } from '../../generated/protocol'
import { isUnsaved } from '../../state/selectors'
import type { Notification } from '../../state/store'
import type { App } from '../createApp'
import {
  actionLabels,
  currentDocument,
  type FakeApp,
  openTestDocument,
  runAction,
  startFakeApp,
} from '../../test/fakes/app'
import {
  ASSET_A,
  ASSET_B,
  ASSET_MISSING_MESSAGE,
  assetMismatchMessage,
  assetValues,
  BLOCK_DOCUMENT,
} from '../../test/fakes/assets'
import { bodiesOf, CUBE_DOCUMENT, documentFile, documentText } from '../../test/fakes/documents'
import {
  callsOf,
  createDesktopHandle,
  createFakeFileHandle,
  createFakeFolderHandle,
} from '../../test/fakes/files'
import { deferred, type Deferred } from '../../test/fakes/deferred'
import { settle } from '../../test/fakes/settle'
import { startFeatureCommand } from './command'
import { applyEdits, newDocument, openFile, save, saveAs } from './document'
import { dismiss } from './notifications'

// Documents with imports and their assets folder (ui.md 3.19, 4.4, ADR 0037): Open reads the
// assets the text names, says why imports are missing, "Choose folder" reopens with the assets,
// Save writes them before the document, and a core restart gives them to the new core again.
// U-219 to U-222 and U-226 to U-229 of ui.md 6.4.

const A = ASSET_A.name

const RENAME_TO_BLOCK: readonly Edit[] = [
  { type: 'update_body', body: { id: 'b-1', name: 'Block' } },
]

/** The import record of `block.ptsm`, suppressed. */
const SUPPRESSED_IMPORT: FeatureRecord = {
  id: 'f-imp1',
  type: 'import',
  body: 'b-1',
  asset: A,
  op: 'new',
  suppressed: true,
}

const FOLDER_NEEDED =
  'The imported geometry of block.ptsm is missing, because the browser has not given Partsmith the folder that holds it and its assets folder. Choose the folder block.ptsm is in.'

afterEach(() => {
  Reflect.deleteProperty(navigator, 'clipboard')
  vi.useRealTimers()
  vi.restoreAllMocks()
})

/** Copied into this realm's Uint8Array: under jsdom, TextEncoder returns Node's. */
function utf8(text: string): Uint8Array {
  return Uint8Array.from(new TextEncoder().encode(text))
}

/** The digest of the text `block.ptsm` holds on disk. */
function blockDigest(): Promise<string> {
  return fileDigest(utf8(documentText(BLOCK_DOCUMENT)))
}

/** The shown notification with this code; throws when there is none. */
function notificationCoded(app: App, code: string): Notification {
  const found = app.store
    .getState()
    .notifications.find((notification) => notification.code === code)
  if (found === undefined) {
    const shown = app.store
      .getState()
      .notifications.map((notification) => notification.code ?? notification.message)
    throw new Error(`No ${code} notification is shown; shown: ${shown.join(', ') || 'none'}.`)
  }
  return found
}

/** The codes of the shown notifications, oldest first. */
function shownCodes(app: App): readonly (string | undefined)[] {
  return app.store.getState().notifications.map((notification) => notification.code)
}

/** Clicks a notification button as the notification list does: dismiss, then run (3.22). */
async function click(app: App, notification: Notification, label: string): Promise<void> {
  dismiss(app, notification.id)
  runAction(notification, label)
  await settle()
}

/** The status of "Import 1" in the shown document. */
function importStatus(app: App): string | undefined {
  return currentDocument(app).featureStatus['f-imp1']?.status
}

/** The assets of each `open_document` request, as plain names and bytes. */
function openedAssets(fake: FakeApp): readonly (readonly { name: string; bytes: number[] }[])[] {
  return fake.transport.commandsOfType('open_document').map((command) => {
    const buffers = fake.transport.buffersOf(command)
    return (command.assets ?? []).map((asset) => ({
      name: asset.name,
      bytes: Array.from(buffers[asset.data] ?? []),
    }))
  })
}

/** Gives `navigator` a clipboard whose `writeText` records the text. */
function stubClipboard(): Mock<(text: string) => Promise<void>> {
  const writeText = vi.fn<(text: string) => Promise<void>>(async () => undefined)
  Object.defineProperty(navigator, 'clipboard', { value: { writeText }, configurable: true })
  return writeText
}

/** The journal without recovery writes, which the session schedules on its own. */
function steps(fake: FakeApp): readonly string[] {
  return fake.journal.filter((entry) => !entry.startsWith('recovery.'))
}

/** Forgets what was sent and called so far. */
function forget(fake: FakeApp): void {
  fake.transport.clearRequests()
  fake.files.clearCalls()
  fake.journal.length = 0
}

/** A web handle of `block.ptsm` with the folder that holds it (and `siblings`) and `A`. */
type WebBlock = {
  readonly fake: FakeApp
  readonly handle: FileSystemFileHandle
  readonly folder: FileSystemDirectoryHandle
}

/** The web handle `H` of `block.ptsm` and the folder `F` that holds it, `siblings` and `A`. */
function webPlace(
  fake: FakeApp,
  siblings: readonly FileHandle[] = [],
): { readonly handle: FileSystemFileHandle; readonly folder: FileSystemDirectoryHandle } {
  const handle = createFakeFileHandle('block.ptsm')
  const folder = createFakeFolderHandle('work', [handle, ...siblings])
  fake.files.putAssets(folder, [ASSET_A])
  return { handle, folder }
}

/** `block.ptsm` opened from a web handle without a folder: "Import 1" fails and the folder is asked for. */
async function blockWithoutFolder(siblings: readonly FileHandle[] = []): Promise<WebBlock> {
  const fake = await startFakeApp()
  const place = webPlace(fake, siblings)
  await openTestDocument(fake, 'block.ptsm', BLOCK_DOCUMENT, place.handle)
  forget(fake)
  return { fake, ...place }
}

/** `block.ptsm` from `H`, given its folder `F` with "Choose folder", so its assets are `[A]` (U-227). */
async function blockWithFolder(siblings: readonly FileHandle[] = []): Promise<WebBlock> {
  const opened = await blockWithoutFolder(siblings)
  opened.fake.files.folderChoices.push({ kind: 'chosen', folder: opened.folder })
  await click(
    opened.fake.app,
    notificationCoded(opened.fake.app, 'assets_folder_needed'),
    'Choose folder',
  )
  expect(importStatus(opened.fake.app)).toBe('ok')
  forget(opened.fake)
  return opened
}

/** Holds every `readAssets` answer of the fake file service until the returned deferred resolves. */
function holdReadAssets(fake: FakeApp): Deferred<void> {
  const held = deferred<void>()
  const readAssets = fake.files.readAssets
  vi.spyOn(fake.files, 'readAssets').mockImplementation(async (place, names) => {
    await held.promise
    return readAssets(place, names)
  })
  return held
}

/** The arguments of the `writeAssets` calls, with the assets as plain names and bytes. */
function writtenAssets(
  fake: FakeApp,
): readonly { readonly place: unknown; readonly assets: unknown }[] {
  return callsOf(fake.files, 'writeAssets').map(([place, assets]) => ({
    place,
    assets: assetValues(assets as readonly AssetFile[]),
  }))
}

describe('Open reads the assets the text names (3.19 openFile step 3)', () => {
  it('U-219 reads the named assets from a desktop handle before openDocument, which gets only those', async () => {
    const fake = await startFakeApp()
    const handle = createDesktopHandle('block.ptsm', 'g1')
    fake.files.putAssets(handle, [ASSET_A, ASSET_B])
    forget(fake)
    await openTestDocument(fake, 'block.ptsm', BLOCK_DOCUMENT, handle)
    expect(callsOf(fake.files, 'readAssets')).toEqual([[{ handle, folder: null }, [A]]])
    expect(
      steps(fake).filter((step) => step === 'files.readAssets' || step === 'command:open_document'),
    ).toEqual(['files.readAssets', 'command:open_document'])
    expect(openedAssets(fake)).toEqual([[{ name: A, bytes: [1, 2, 3] }]])
    expect(assetValues(currentDocument(fake.app).assets)).toEqual([{ name: A, bytes: [1, 2, 3] }])
    expect(importStatus(fake.app)).toBe('ok')
    expect(fake.app.store.getState().notifications).toEqual([])
  })

  it('U-219 a document that names no asset reads none and sends assets: []', async () => {
    const fake = await startFakeApp()
    const handle = createDesktopHandle('cube.ptsm', 'g1')
    fake.files.putAssets(handle, [ASSET_A, ASSET_B])
    forget(fake)
    await openTestDocument(fake, 'cube.ptsm', CUBE_DOCUMENT, handle)
    expect(callsOf(fake.files, 'readAssets')).toEqual([])
    expect(fake.transport.commandsOfType('open_document')[0]?.assets ?? []).toEqual([])
    expect(currentDocument(fake.app).assets).toEqual([])
  })

  it('a web handle with a folder reads the assets through that folder', async () => {
    const fake = await startFakeApp()
    const { handle, folder } = webPlace(fake)
    forget(fake)
    await openTestDocument(fake, 'block.ptsm', BLOCK_DOCUMENT, handle, folder)
    expect(callsOf(fake.files, 'readAssets')).toEqual([[{ handle, folder }, [A]]])
    expect(currentDocument(fake.app).file).toMatchObject({ name: 'block.ptsm', handle, folder })
    expect(importStatus(fake.app)).toBe('ok')
  })

  it('a dropped file, without a handle, reads no assets', async () => {
    const fake = await startFakeApp()
    forget(fake)
    await openTestDocument(fake, 'block.ptsm', BLOCK_DOCUMENT)
    expect(callsOf(fake.files, 'readAssets')).toEqual([])
    expect(fake.transport.commandsOfType('open_document')[0]?.assets ?? []).toEqual([])
  })
})

describe('a folder that cannot be read (3.19 openFile steps 4 and 6)', () => {
  it('U-220 (a) a failed read opens without the asset and shows assets_not_read with Copy details', async () => {
    const fake = await startFakeApp()
    const handle = createDesktopHandle('block.ptsm', 'g1')
    fake.files.readAssetsFailure = new FileServiceError('io', 'Disk failure', {
      entry: `assets/${A}`,
    })
    forget(fake)
    await openTestDocument(fake, 'block.ptsm', BLOCK_DOCUMENT, handle)
    expect(fake.transport.commandsOfType('open_document')[0]?.assets ?? []).toEqual([])
    const notification = notificationCoded(fake.app, 'assets_not_read')
    expect(notification.severity).toBe('warning')
    expect(notification.message).toBe(
      'Partsmith could not read some imported files from the assets folder next to block.ptsm, so their geometry is missing.',
    )
    expect(actionLabels(notification)).toEqual(['Copy details'])
    expect(shownCodes(fake.app)).toEqual(['assets_not_read'])
    const writeText = stubClipboard()
    runAction(notification, 'Copy details')
    await settle()
    expect(writeText).toHaveBeenCalledWith(
      [
        'assets_not_read',
        'Partsmith could not read some imported files from the assets folder next to block.ptsm, so their geometry is missing.',
        `assets/${A}: Disk failure`,
      ].join('\n'),
    )
  })

  it('U-220 (b) a refused asset opens the text again without assets and shows assets_mismatch', async () => {
    const fake = await startFakeApp({ core: { mismatchedAssets: [A] } })
    const handle = createDesktopHandle('block.ptsm', 'g1')
    fake.files.putAssets(handle, [ASSET_A])
    forget(fake)
    await openTestDocument(fake, 'block.ptsm', BLOCK_DOCUMENT, handle)
    expect(openedAssets(fake)).toEqual([[{ name: A, bytes: [1, 2, 3] }], []])
    expect(currentDocument(fake.app).name).toBe('block')
    expect(importStatus(fake.app)).toBe('error')
    expect(shownCodes(fake.app)).not.toContain('open_failed')
    const notification = notificationCoded(fake.app, 'assets_mismatch')
    expect(notification.severity).toBe('warning')
    expect(notification.message).toBe(
      'Some files in the assets folder next to block.ptsm are not the files it imported, so Partsmith opened it without them and its imported geometry is missing.',
    )
    expect(actionLabels(notification)).toEqual(['Copy details'])
    const writeText = stubClipboard()
    runAction(notification, 'Copy details')
    await settle()
    expect(writeText.mock.calls[0]?.[0].split('\n').at(-1)).toBe(assetMismatchMessage(A))
  })

  it('U-220 (b) the session keeps no refused asset, so a restart sends assets: [] and no buffer', async () => {
    const fake = await startFakeApp({ core: { mismatchedAssets: [A] } })
    const handle = createDesktopHandle('block.ptsm', 'g1')
    fake.files.putAssets(handle, [ASSET_A])
    await openTestDocument(fake, 'block.ptsm', BLOCK_DOCUMENT, handle)
    expect(currentDocument(fake.app).assets).toEqual([])
    fake.transport.die('The worker crashed.')
    await settle()
    forget(fake)
    await fake.app.restartCore()
    await settle()
    const [reopened] = fake.transport.commandsOfType('open_document')
    if (reopened === undefined) {
      throw new Error('The restart opened nothing.')
    }
    expect(reopened.assets ?? []).toEqual([])
    expect(fake.transport.buffersOf(reopened)).toEqual([])
    expect(callsOf(fake.files, 'readAssets')).toEqual([])
  })

  it('U-220 (c) a folder whose permission the browser refused counts as not reached: assets_folder_needed', async () => {
    const fake = await startFakeApp()
    const { handle, folder } = webPlace(fake)
    fake.files.readAssetsFailure = new FileServiceError(
      'permission',
      'Partsmith has no access to the folder.',
      { entry: 'assets' },
    )
    await openTestDocument(fake, 'block.ptsm', BLOCK_DOCUMENT, handle, folder)
    const notification = notificationCoded(fake.app, 'assets_folder_needed')
    expect(notification.message).toBe(FOLDER_NEEDED)
    expect(actionLabels(notification)).toEqual(['Choose folder'])
    expect(shownCodes(fake.app)).not.toContain('assets_not_read')
  })
})

describe('why imports are missing (3.19 openFile step 6)', () => {
  it('U-221 (a) a web handle without a folder shows the persistent assets_folder_needed for f-imp1', async () => {
    // Fake timers from the start, so the notification's countdown, if it had one, would run on them.
    vi.useFakeTimers()
    const { fake } = await blockWithoutFolder()
    const notification = notificationCoded(fake.app, 'assets_folder_needed')
    expect(notification.severity).toBe('warning')
    expect(notification.message).toBe(FOLDER_NEEDED)
    expect(actionLabels(notification)).toEqual(['Choose folder'])
    expect(notification.features).toEqual(['f-imp1'])
    vi.advanceTimersByTime(60_000)
    expect(shownCodes(fake.app)).toContain('assets_folder_needed')
  })

  it('U-221 (a) the warning goes once f-imp1 is no longer in error', async () => {
    const { fake } = await blockWithoutFolder()
    // Suppressing the import is a recompute in which f-imp1 is not in error.
    await applyEdits(fake.app, 'Suppress', [{ type: 'update_feature', feature: SUPPRESSED_IMPORT }])
    await settle()
    expect(importStatus(fake.app)).toBe('suppressed')
    expect(shownCodes(fake.app)).not.toContain('assets_folder_needed')
  })

  it('U-221 (b) a dropped file with the File System Access API shows assets_without_folder with "Open..."', async () => {
    const fake = await startFakeApp()
    await openTestDocument(fake, 'block.ptsm', BLOCK_DOCUMENT)
    const notification = notificationCoded(fake.app, 'assets_without_folder')
    expect(notification.severity).toBe('warning')
    expect(notification.message).toBe(
      'The imported geometry of block.ptsm is missing, because Partsmith has no file of it to find its assets folder by. Open the file with Open... instead.',
    )
    expect(actionLabels(notification)).toEqual(['Open...'])
    expect(notification.features).toEqual(['f-imp1'])
    forget(fake)
    await click(fake.app, notification, 'Open...')
    expect(callsOf(fake.files, 'pickToOpen')).toEqual([[]])
  })

  it('U-221 (c) a dropped file without the File System Access API shows assets_unsupported with no action', async () => {
    const fake = await startFakeApp({ fileSystemAccess: false })
    await openTestDocument(fake, 'block.ptsm', BLOCK_DOCUMENT)
    const notification = notificationCoded(fake.app, 'assets_unsupported')
    expect(notification.severity).toBe('warning')
    expect(notification.message).toBe(
      'The imported geometry of block.ptsm is missing, because this browser does not let Partsmith reach files next to the one you chose. Open it in Chrome or Edge, or in the Partsmith desktop app.',
    )
    expect(actionLabels(notification)).toEqual([])
  })

  it('U-221 (d) a desktop folder that lacks the asset adds no warning of its own', async () => {
    const fake = await startFakeApp()
    await openTestDocument(
      fake,
      'block.ptsm',
      BLOCK_DOCUMENT,
      createDesktopHandle('block.ptsm', 'g1'),
    )
    expect(importStatus(fake.app)).toBe('error')
    expect(currentDocument(fake.app).featureStatus['f-imp1']?.messages[0]?.message).toBe(
      ASSET_MISSING_MESSAGE,
    )
    for (const code of [
      'assets_folder_needed',
      'assets_without_folder',
      'assets_unsupported',
      'assets_not_read',
    ]) {
      expect(shownCodes(fake.app)).not.toContain(code)
    }
  })

  it('U-221 a document without imports opened from a web handle without a folder shows none', async () => {
    const fake = await startFakeApp()
    await openTestDocument(fake, 'cube.ptsm', CUBE_DOCUMENT, createFakeFileHandle('cube.ptsm'))
    expect(fake.app.store.getState().notifications).toEqual([])
  })

  it('U-221 (e) a recovery entry without a handle names its file after the document: block.ptsm', async () => {
    const recovery = createMemoryRecovery()
    const entry: RecoveryEntry = {
      id: 'kept',
      name: 'block',
      text: JSON.stringify(BLOCK_DOCUMENT, null, 2),
      savedAt: Date.UTC(2026, 9, 12, 9, 30),
      handle: null,
      folder: null,
      digest: null,
      reason: 'autosave',
    }
    await recovery.write(entry)
    const fake = await startFakeApp({ recovery })
    runAction(notificationCoded(fake.app, 'recovered'), 'Open')
    await settle()
    expect(currentDocument(fake.app).name).toBe('block')
    expect(notificationCoded(fake.app, 'assets_without_folder').message).toBe(
      'The imported geometry of block.ptsm is missing, because Partsmith has no file of it to find its assets folder by. Open the file with Open... instead.',
    )
  })
})

describe('Choose folder (3.19)', () => {
  it('U-222 another folder shows assets_wrong_folder, whose Choose folder reopens with the assets once the right one is chosen', async () => {
    const { fake, handle, folder } = await blockWithoutFolder()
    const { recoveryId, documentHandle } = currentDocument(fake.app)
    await applyEdits(fake.app, 'Rename', RENAME_TO_BLOCK)
    await settle()
    const settled = currentDocument(fake.app).recipe
    forget(fake)
    fake.files.folderChoices.push({ kind: 'other', name: 'parts' })
    await click(fake.app, notificationCoded(fake.app, 'assets_folder_needed'), 'Choose folder')
    expect(callsOf(fake.files, 'chooseFolder')).toEqual([[handle]])
    const wrong = notificationCoded(fake.app, 'assets_wrong_folder')
    expect(wrong.severity).toBe('error')
    expect(wrong.message).toBe(
      'parts is not the folder that holds block.ptsm. Choose the folder block.ptsm is in.',
    )
    expect(actionLabels(wrong)).toEqual(['Choose folder'])

    forget(fake)
    fake.files.folderChoices.push({ kind: 'chosen', folder })
    await click(fake.app, wrong, 'Choose folder')
    expect(callsOf(fake.files, 'readAssets')).toEqual([[{ handle, folder }, [A]]])
    expect(
      steps(fake).filter((step) =>
        ['files.readAssets', 'command:open_document', 'command:close_document'].includes(step),
      ),
    ).toEqual(['files.readAssets', 'command:open_document', 'command:close_document'])
    const [reopened] = fake.transport.commandsOfType('open_document')
    expect(reopened?.text).toBe(JSON.stringify(settled, null, 2))
    expect(openedAssets(fake)).toEqual([[{ name: A, bytes: [1, 2, 3] }]])
    expect(fake.transport.commandsOfType('close_document')).toEqual([
      { type: 'close_document', doc: documentHandle },
    ])
    const document = currentDocument(fake.app)
    expect(document.documentHandle).not.toBe(documentHandle)
    expect(document.name).toBe('block')
    expect(document.recoveryId).toBe(recoveryId)
    expect(document.file).toMatchObject({ name: 'block.ptsm', handle, folder })
    expect(assetValues(document.assets)).toEqual([{ name: A, bytes: [1, 2, 3] }])
    expect(document.localUnsaved).toBe(true)
    expect(importStatus(fake.app)).toBe('ok')
    const [recent] = await fake.recent.list()
    expect(recent).toMatchObject({ handle, folder })
    expect(shownCodes(fake.app)).not.toContain('assets_folder_needed')
    expect(shownCodes(fake.app)).not.toContain('assets_wrong_folder')
  })

  it('U-222 without unsaved changes the reopened document is not unsaved', async () => {
    const { fake, folder } = await blockWithoutFolder()
    fake.files.folderChoices.push({ kind: 'chosen', folder })
    await click(fake.app, notificationCoded(fake.app, 'assets_folder_needed'), 'Choose folder')
    const document = currentDocument(fake.app)
    expect(document.localUnsaved).toBe(false)
    expect(isUnsaved(document)).toBe(false)
  })

  it('U-222 a canceled dialog sends nothing and shows assets_folder_needed again for f-imp1', async () => {
    const { fake, handle } = await blockWithoutFolder()
    fake.files.folderChoices.push({ kind: 'canceled' })
    await click(fake.app, notificationCoded(fake.app, 'assets_folder_needed'), 'Choose folder')
    expect(callsOf(fake.files, 'chooseFolder')).toEqual([[handle]])
    expect(fake.transport.commandTypes()).toEqual([])
    expect(callsOf(fake.files, 'readAssets')).toEqual([])
    const again = notificationCoded(fake.app, 'assets_folder_needed')
    expect(again.message).toBe(FOLDER_NEEDED)
    expect(again.features).toEqual(['f-imp1'])
    expect(actionLabels(again)).toEqual(['Choose folder'])
  })

  it('U-222 a chosen folder without the asset reads it, sends no open_document and shows nothing', async () => {
    const { fake, handle } = await blockWithoutFolder()
    const empty = createFakeFolderHandle('empty', [handle])
    fake.files.folderChoices.push({ kind: 'chosen', folder: empty })
    await click(fake.app, notificationCoded(fake.app, 'assets_folder_needed'), 'Choose folder')
    expect(callsOf(fake.files, 'readAssets')).toEqual([[{ handle, folder: empty }, [A]]])
    expect(fake.transport.commandsOfType('open_document')).toEqual([])
    expect(fake.app.store.getState().notifications).toEqual([])
  })

  it('U-222 the action waits during a feature command and refuses with finish_command_first', async () => {
    const { fake } = await blockWithoutFolder()
    const notification = notificationCoded(fake.app, 'assets_folder_needed')
    expect(
      notification.actions.find((action) => action.label === 'Choose folder')?.editsDocument,
    ).toBe(true)
    await startFeatureCommand(fake.app, 'fillet')
    await settle()
    forget(fake)
    runAction(notification, 'Choose folder')
    await settle()
    expect(callsOf(fake.files, 'chooseFolder')).toEqual([])
    expect(shownCodes(fake.app)).toContain('finish_command_first')
  })

  it('a folder whose asset the core refuses leaves the document as it is and shows assets_mismatch', async () => {
    const fake = await startFakeApp({ core: { mismatchedAssets: [A] } })
    const { handle, folder } = webPlace(fake)
    await openTestDocument(fake, 'block.ptsm', BLOCK_DOCUMENT, handle)
    const { documentHandle } = currentDocument(fake.app)
    forget(fake)
    fake.files.folderChoices.push({ kind: 'chosen', folder })
    await click(fake.app, notificationCoded(fake.app, 'assets_folder_needed'), 'Choose folder')
    expect(openedAssets(fake)).toEqual([[{ name: A, bytes: [1, 2, 3] }]])
    expect(fake.transport.commandsOfType('close_document')).toEqual([])
    expect(currentDocument(fake.app).documentHandle).toBe(documentHandle)
    expect(currentDocument(fake.app).assets).toEqual([])
    expect(importStatus(fake.app)).toBe('error')
    expect(notificationCoded(fake.app, 'assets_mismatch').severity).toBe('warning')
  })

  it('a folder dialog that fails shows its message as an error and reads nothing (3.19)', async () => {
    const { fake } = await blockWithoutFolder()
    const message = 'Must be handling a user gesture to show a file picker.'
    vi.spyOn(fake.files, 'chooseFolder').mockRejectedValue(new FileServiceError('io', message))
    await click(fake.app, notificationCoded(fake.app, 'assets_folder_needed'), 'Choose folder')
    const shown = fake.app.store
      .getState()
      .notifications.find((notification) => notification.message === message)
    expect(shown?.severity).toBe('error')
    expect(callsOf(fake.files, 'readAssets')).toEqual([])
  })

  it('U-222 an edit committed while the folder is read is in the reopened recipe', async () => {
    const { fake, folder } = await blockWithoutFolder()
    const reading = holdReadAssets(fake)
    fake.files.folderChoices.push({ kind: 'chosen', folder })
    await click(fake.app, notificationCoded(fake.app, 'assets_folder_needed'), 'Choose folder')
    await applyEdits(fake.app, 'Rename', RENAME_TO_BLOCK)
    await settle()
    const settled = currentDocument(fake.app).recipe
    reading.resolve()
    await settle()
    const [reopened] = fake.transport.commandsOfType('open_document')
    expect(reopened?.text).toBe(JSON.stringify(settled, null, 2))
    expect(bodiesOf(currentDocument(fake.app).recipe)[0]?.name).toBe('Block')
    expect(currentDocument(fake.app).localUnsaved).toBe(true)
    expect(importStatus(fake.app)).toBe('ok')
  })

  it('U-222 an edit still waiting for the core when the folder is chosen is in the reopened recipe', async () => {
    const { fake, folder } = await blockWithoutFolder()
    fake.transport.pause()
    const renamed = applyEdits(fake.app, 'Rename', RENAME_TO_BLOCK)
    fake.files.folderChoices.push({ kind: 'chosen', folder })
    await click(fake.app, notificationCoded(fake.app, 'assets_folder_needed'), 'Choose folder')
    fake.transport.resume()
    await renamed
    await settle()
    expect(fake.transport.commandTypes()).toEqual([
      'apply_edits',
      'open_document',
      'close_document',
    ])
    const [reopened] = fake.transport.commandsOfType('open_document')
    expect(JSON.parse(reopened?.text ?? '{}').bodies[0].name).toBe('Block')
    expect(bodiesOf(currentDocument(fake.app).recipe)[0]?.name).toBe('Block')
  })

  it('U-222 an edit made while the document is reopened goes to the reopened document', async () => {
    const { fake, folder } = await blockWithoutFolder()
    const { documentHandle } = currentDocument(fake.app)
    const opening = deferred<void>()
    fake.transport.handle('open_document', async (command, core, buffers) => {
      await opening.promise
      return core.reply(command, buffers)
    })
    fake.files.folderChoices.push({ kind: 'chosen', folder })
    await click(fake.app, notificationCoded(fake.app, 'assets_folder_needed'), 'Choose folder')
    expect(fake.transport.commandsOfType('open_document')).toHaveLength(1)
    const renamed = applyEdits(fake.app, 'Rename', RENAME_TO_BLOCK)
    opening.resolve()
    expect(await renamed).toBe(true)
    await settle()
    const document = currentDocument(fake.app)
    expect(document.documentHandle).not.toBe(documentHandle)
    expect(fake.transport.commandsOfType('apply_edits').map((command) => command.doc)).toEqual([
      document.documentHandle,
    ])
    expect(bodiesOf(document.recipe)[0]?.name).toBe('Block')
    expect(isUnsaved(document)).toBe(true)
  })

  it('U-222 a feature command opened while the folder is read keeps the document and asks again', async () => {
    const { fake, folder } = await blockWithoutFolder()
    const { documentHandle } = currentDocument(fake.app)
    const reading = holdReadAssets(fake)
    fake.files.folderChoices.push({ kind: 'chosen', folder })
    await click(fake.app, notificationCoded(fake.app, 'assets_folder_needed'), 'Choose folder')
    await startFeatureCommand(fake.app, 'fillet')
    await settle()
    reading.resolve()
    await settle()
    expect(fake.transport.commandsOfType('open_document')).toEqual([])
    expect(currentDocument(fake.app).documentHandle).toBe(documentHandle)
    expect(fake.app.store.getState().command?.featureType).toBe('fillet')
    expect(shownCodes(fake.app)).toContain('finish_command_first')
    const again = notificationCoded(fake.app, 'assets_folder_needed')
    expect(again.features).toEqual(['f-imp1'])
    expect(actionLabels(again)).toEqual(['Choose folder'])
  })

  it('U-222 a document that replaced this one while the folder was read is not reopened', async () => {
    const { fake, folder } = await blockWithoutFolder()
    const reading = holdReadAssets(fake)
    fake.files.folderChoices.push({ kind: 'chosen', folder })
    await click(fake.app, notificationCoded(fake.app, 'assets_folder_needed'), 'Choose folder')
    await newDocument(fake.app)
    await settle()
    reading.resolve()
    await settle()
    expect(fake.transport.commandsOfType('open_document')).toEqual([])
    expect(currentDocument(fake.app).name).toBe('Untitled')
  })

  it('U-222 a Choose folder clicked after New replaced the document calls no chooseFolder', async () => {
    const { fake, folder } = await blockWithoutFolder()
    const notification = notificationCoded(fake.app, 'assets_folder_needed')
    await newDocument(fake.app)
    await settle()
    forget(fake)
    fake.files.folderChoices.push({ kind: 'chosen', folder })
    runAction(notification, 'Choose folder')
    await settle()
    expect(callsOf(fake.files, 'chooseFolder')).toEqual([])
    expect(fake.transport.commandTypes()).toEqual([])
    expect(currentDocument(fake.app).name).toBe('Untitled')
  })
})

/** `block.ptsm` from a desktop handle whose folder holds `A`, with the body renamed. */
async function renamedDesktopBlock(): Promise<{
  readonly fake: FakeApp
  readonly handle: FileHandle
}> {
  const fake = await startFakeApp()
  const handle = createDesktopHandle('block.ptsm', 'g1')
  fake.files.putAssets(handle, [ASSET_A])
  await openTestDocument(fake, 'block.ptsm', BLOCK_DOCUMENT, handle)
  await applyEdits(fake.app, 'Rename', RENAME_TO_BLOCK)
  await settle()
  forget(fake)
  return { fake, handle }
}

describe('Save writes the assets first (3.19 writing the document)', () => {
  it('U-226 Save calls saveDocument, readHandle, writeAssets, write and markSaved, in that order', async () => {
    const { fake, handle } = await renamedDesktopBlock()
    await save(fake.app)
    await settle()
    expect(steps(fake)).toEqual([
      'command:save_document',
      'files.readHandle',
      'files.writeAssets',
      'files.write',
      'command:mark_saved',
    ])
    expect(writtenAssets(fake)).toEqual([
      { place: { handle, folder: null }, assets: [{ name: A, bytes: [1, 2, 3] }] },
    ])
    expect(currentDocument(fake.app).dirty).toBe(false)
  })

  it('U-226 a document whose save_document reply has no assets writes none', async () => {
    const fake = await startFakeApp()
    const handle = createDesktopHandle('cube.ptsm', 'g1')
    await openTestDocument(fake, 'cube.ptsm', CUBE_DOCUMENT, handle)
    await applyEdits(fake.app, 'Rename', [
      { type: 'update_body', body: { id: 'b-cube', name: 'Box' } },
    ])
    await settle()
    forget(fake)
    await save(fake.app)
    await settle()
    expect(callsOf(fake.files, 'writeAssets')).toEqual([])
    expect(fake.files.writes).toHaveLength(1)
  })

  it('U-226 a failed asset write shows save_assets with Save as and Copy details and writes no document', async () => {
    const { fake } = await renamedDesktopBlock()
    fake.files.writeAssetsError = new FileServiceError(
      'permission',
      `Partsmith has no access to ${A}: denied`,
      { entry: `assets/${A}` },
    )
    await save(fake.app)
    await settle()
    expect(callsOf(fake.files, 'write')).toEqual([])
    expect(fake.transport.commandTypes()).not.toContain('mark_saved')
    expect(currentDocument(fake.app).dirty).toBe(true)
    const notification = notificationCoded(fake.app, 'save_assets')
    expect(notification.severity).toBe('error')
    expect(notification.message).toBe(
      'block.ptsm was not saved, because Partsmith could not write its imported files to the assets folder next to it.',
    )
    expect(actionLabels(notification)).toEqual(['Save as', 'Copy details'])
    const writeText = stubClipboard()
    runAction(notification, 'Copy details')
    await settle()
    expect(writeText.mock.calls[0]?.[0].split('\n').at(-1)).toBe(
      `assets/${A}: Partsmith has no access to ${A}: denied`,
    )
  })
})

/** Save as of the block to `target`. */
function saveTo(fake: FakeApp, target: FileSystemFileHandle): Promise<void> {
  const saveTarget: SaveTarget = { kind: 'handle', name: target.name, handle: target }
  fake.files.nextSaveTarget = saveTarget
  return saveAs(fake.app).then(() => settle())
}

describe('Save as into another web folder (3.19 writing the document, step 3)', () => {
  it('U-227 a target the folder does not hold sends save_document, writes nothing and asks for its folder', async () => {
    const { fake, handle } = await blockWithFolder()
    const target = createFakeFileHandle('block.ptsm')
    await saveTo(fake, target)
    expect(fake.transport.commandTypes()).toEqual(['save_document'])
    expect(callsOf(fake.files, 'writeAssets')).toEqual([])
    expect(callsOf(fake.files, 'write')).toEqual([])
    const notification = notificationCoded(fake.app, 'save_folder_needed')
    expect(notification.severity).toBe('warning')
    expect(notification.message).toBe(
      'block.ptsm is not saved yet, because its imported files are saved in the folder it goes in and the browser has not given Partsmith that folder. Choose the folder you are saving block.ptsm in.',
    )
    expect(actionLabels(notification)).toEqual(['Choose folder'])
    expect(currentDocument(fake.app).file?.handle).toBe(handle)
  })

  it('U-227 its Choose folder saves again with that folder: assets, document, then markSaved', async () => {
    const { fake } = await blockWithFolder()
    const target = createFakeFileHandle('block.ptsm')
    const copy = createFakeFolderHandle('copy', [target])
    await saveTo(fake, target)
    forget(fake)
    fake.files.folderChoices.push({ kind: 'chosen', folder: copy })
    await click(fake.app, notificationCoded(fake.app, 'save_folder_needed'), 'Choose folder')
    expect(callsOf(fake.files, 'chooseFolder')).toEqual([[target]])
    expect(steps(fake).filter((step) => step !== 'files.chooseFolder')).toEqual([
      'command:save_document',
      'files.writeAssets',
      'files.write',
      'command:mark_saved',
    ])
    expect(writtenAssets(fake)).toEqual([
      { place: { handle: target, folder: copy }, assets: [{ name: A, bytes: [1, 2, 3] }] },
    ])
    const [write] = fake.files.writes
    expect(write?.target).toMatchObject({ kind: 'handle', handle: target })
    const document = currentDocument(fake.app)
    expect(document.file).toEqual({
      name: 'block.ptsm',
      handle: target,
      folder: copy,
      digest: await fileDigest(write?.bytes ?? new Uint8Array(0)),
    })
    const [recent] = await fake.recent.list()
    expect(recent).toMatchObject({ handle: target, folder: copy })
  })

  it("U-227 another folder shows assets_wrong_folder, whose Choose folder asks for the target's folder again", async () => {
    const { fake } = await blockWithFolder()
    const target = createFakeFileHandle('block.ptsm')
    await saveTo(fake, target)
    fake.files.folderChoices.push({ kind: 'other', name: 'parts' })
    await click(fake.app, notificationCoded(fake.app, 'save_folder_needed'), 'Choose folder')
    const wrong = notificationCoded(fake.app, 'assets_wrong_folder')
    expect(wrong.message).toBe(
      'parts is not the folder that holds block.ptsm. Choose the folder block.ptsm is in.',
    )
    forget(fake)
    fake.files.folderChoices.push({ kind: 'canceled' })
    await click(fake.app, wrong, 'Choose folder')
    expect(callsOf(fake.files, 'chooseFolder')).toEqual([[target]])
  })

  it('U-227 a canceled dialog writes nothing, keeps the file and shows save_folder_needed again for the target', async () => {
    const { fake, handle } = await blockWithFolder()
    const target = createFakeFileHandle('block.ptsm')
    await saveTo(fake, target)
    fake.files.folderChoices.push({ kind: 'canceled' })
    await click(fake.app, notificationCoded(fake.app, 'save_folder_needed'), 'Choose folder')
    expect(callsOf(fake.files, 'writeAssets')).toEqual([])
    expect(callsOf(fake.files, 'write')).toEqual([])
    expect(currentDocument(fake.app).file?.handle).toBe(handle)
    const again = notificationCoded(fake.app, 'save_folder_needed')
    forget(fake)
    await click(fake.app, again, 'Choose folder')
    expect(callsOf(fake.files, 'chooseFolder')).toEqual([[target]])
  })

  it('U-227 its Choose folder clicked after New calls no chooseFolder, writeAssets or write', async () => {
    const { fake } = await blockWithFolder()
    const target = createFakeFileHandle('block.ptsm')
    await saveTo(fake, target)
    const notification = notificationCoded(fake.app, 'save_folder_needed')
    await newDocument(fake.app)
    await settle()
    forget(fake)
    fake.files.folderChoices.push({
      kind: 'chosen',
      folder: createFakeFolderHandle('copy', [target]),
    })
    runAction(notification, 'Choose folder')
    await settle()
    expect(callsOf(fake.files, 'chooseFolder')).toEqual([])
    expect(callsOf(fake.files, 'writeAssets')).toEqual([])
    expect(callsOf(fake.files, 'write')).toEqual([])
    expect(fake.transport.commandTypes()).toEqual([])
  })

  it('U-227 its Choose folder clicked after a Save as to another file calls no chooseFolder, writeAssets or write', async () => {
    const other = createFakeFileHandle('other.ptsm')
    const { fake } = await blockWithFolder([other])
    const target = createFakeFileHandle('block.ptsm')
    await saveTo(fake, target)
    const notification = notificationCoded(fake.app, 'save_folder_needed')
    // A Save as into the document's own folder, which holds the new file, writes at once.
    await saveTo(fake, other)
    expect(currentDocument(fake.app).file?.handle).toBe(other)
    forget(fake)
    fake.files.folderChoices.push({
      kind: 'chosen',
      folder: createFakeFolderHandle('copy', [target]),
    })
    runAction(notification, 'Choose folder')
    await settle()
    expect(callsOf(fake.files, 'chooseFolder')).toEqual([])
    expect(callsOf(fake.files, 'writeAssets')).toEqual([])
    expect(callsOf(fake.files, 'write')).toEqual([])
  })

  it("U-227 a target the document's folder holds is written at once with that folder and no warning", async () => {
    const sibling = createFakeFileHandle('block-copy.ptsm')
    const { fake, folder } = await blockWithFolder([sibling])
    await saveTo(fake, sibling)
    expect(writtenAssets(fake)).toEqual([
      { place: { handle: sibling, folder }, assets: [{ name: A, bytes: [1, 2, 3] }] },
    ])
    expect(fake.files.writes).toHaveLength(1)
    expect(shownCodes(fake.app)).not.toContain('save_folder_needed')
  })
})

describe('a core restart keeps the assets (4.4)', () => {
  it("U-228 reopens with the session's assets, reads no folder and keeps the file reference", async () => {
    const { fake, handle, folder } = await blockWithFolder()
    const before = currentDocument(fake.app)
    expect(before.file?.digest).toBe(await blockDigest())
    fake.transport.die('The worker crashed.')
    await settle()
    forget(fake)
    await fake.app.restartCore()
    await settle()
    const [reopened] = fake.transport.commandsOfType('open_document')
    expect(reopened?.text).toBe(JSON.stringify(before.recipe, null, 2))
    expect(openedAssets(fake)).toEqual([[{ name: A, bytes: [1, 2, 3] }]])
    expect(callsOf(fake.files, 'readAssets')).toEqual([])
    const document = currentDocument(fake.app)
    expect(document.name).toBe('block')
    expect(document.file).toEqual({
      name: 'block.ptsm',
      handle,
      folder,
      digest: await blockDigest(),
    })
    expect(assetValues(document.assets)).toEqual([{ name: A, bytes: [1, 2, 3] }])
    expect(document.recoveryId).toBe(before.recoveryId)
    expect(document.localUnsaved).toBe(true)
    expect(importStatus(fake.app)).toBe('ok')
  })
})

/** The unsaved block from `H` with folder `F`, stashed by New as a `replaced` entry. */
async function stashedBlock(): Promise<{
  readonly fake: FakeApp
  readonly handle: FileSystemFileHandle
  readonly folder: FileSystemDirectoryHandle
  readonly entry: RecoveryEntry
}> {
  const opened = await blockWithFolder()
  await applyEdits(opened.fake.app, 'Rename', RENAME_TO_BLOCK)
  await settle()
  const { recoveryId } = currentDocument(opened.fake.app)
  await newDocument(opened.fake.app)
  await settle()
  const entry = (await opened.fake.recovery.list()).find((candidate) => candidate.id === recoveryId)
  if (entry === undefined) {
    throw new Error('New kept no recovery entry of the block.')
  }
  forget(opened.fake)
  return { ...opened, entry }
}

describe('recovery and recent files keep the place (3.19, 2.15)', () => {
  it('U-229 the recovery entry of the session holds its handle, folder and digest', async () => {
    const { handle, folder, entry } = await stashedBlock()
    expect(entry).toMatchObject({ handle, folder, digest: await blockDigest(), reason: 'replaced' })
  })

  it('U-229 Reopen of the stash reads the assets from the folder and keeps the place', async () => {
    const { fake, handle, folder } = await stashedBlock()
    runAction(notificationCoded(fake.app, 'stashed'), 'Reopen')
    await settle()
    expect(callsOf(fake.files, 'readAssets')).toEqual([[{ handle, folder }, [A]]])
    expect(openedAssets(fake)).toEqual([[{ name: A, bytes: [1, 2, 3] }]])
    expect(currentDocument(fake.app).file).toEqual({
      name: 'block.ptsm',
      handle,
      folder,
      digest: await blockDigest(),
    })
  })

  it('U-229 the startup Open of the entry reads the assets from its folder and keeps the place', async () => {
    const { handle, folder, entry } = await stashedBlock()
    const recovery = createMemoryRecovery()
    await recovery.write(entry)
    const fake = await startFakeApp({ recovery })
    fake.files.putAssets(folder, [ASSET_A])
    forget(fake)
    runAction(notificationCoded(fake.app, 'recovered'), 'Open')
    await settle()
    expect(callsOf(fake.files, 'readAssets')).toEqual([[{ handle, folder }, [A]]])
    expect(openedAssets(fake)).toEqual([[{ name: A, bytes: [1, 2, 3] }]])
    const document = currentDocument(fake.app)
    expect(document.file).toEqual({ name: 'block.ptsm', handle, folder, digest: entry.digest })
    expect(document.localUnsaved).toBe(true)
  })

  it('U-229 opening the file again without a folder takes the folder of its recent entry', async () => {
    const fake = await startFakeApp()
    const { handle, folder } = webPlace(fake)
    await fake.recent.remember({ name: 'block.ptsm', handle, folder, text: '' })
    forget(fake)
    await openFile(fake.app, documentFile('block.ptsm', BLOCK_DOCUMENT, handle))
    await settle()
    expect(callsOf(fake.files, 'readAssets')).toEqual([[{ handle, folder }, [A]]])
    expect(currentDocument(fake.app).file).toMatchObject({ handle, folder })
    expect(importStatus(fake.app)).toBe('ok')
  })

  it("a recent entry's folder that does not hold the file is not taken", async () => {
    const fake = await startFakeApp()
    const handle = createFakeFileHandle('block.ptsm')
    const elsewhere = createFakeFolderHandle('elsewhere')
    fake.files.putAssets(elsewhere, [ASSET_A])
    await fake.recent.remember({ name: 'block.ptsm', handle, folder: elsewhere, text: '' })
    forget(fake)
    await openFile(fake.app, documentFile('block.ptsm', BLOCK_DOCUMENT, handle))
    await settle()
    const places = callsOf(fake.files, 'readAssets').map(([place]) => place as DocumentPlace)
    expect(places.every((place) => place.folder !== elsewhere)).toBe(true)
    expect(currentDocument(fake.app).file?.folder ?? null).toBeNull()
  })
})
