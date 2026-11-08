import { afterEach, describe, expect, it, vi, type Mock } from 'vitest'
import type { AssetFile } from '../../files/assets'
import { fileDigest } from '../../files/digest'
import {
  FileServiceError,
  type DocumentPlace,
  type FileHandle,
  type SaveTarget,
} from '../../files/file-service'
import type { TaskKind } from '../../core/queue'
import { CoreFatalError } from '../../core/transport'
import { createMemoryRecovery, type RecoveryEntry } from '../../files/recovery'
import type { Document, Edit, FeatureRecord } from '../../generated/protocol'
import { isUnsaved } from '../../state/selectors'
import type { App } from '../createApp'
import {
  actionLabels,
  clickAction,
  currentDocument,
  type FakeApp,
  forgetCalls,
  journalSteps,
  notificationCoded,
  notificationCodes,
  openTestDocument,
  runAction,
  startFakeApp,
} from '../../test/fakes/app'
import {
  ASSET_A,
  ASSET_B,
  ASSET_MISSING_MESSAGE,
  assetValues,
  BLOCK_DOCUMENT,
} from '../../test/fakes/assets'
import {
  bodiesOf,
  CUBE_DOCUMENT,
  documentFile,
  documentText,
  utf8,
} from '../../test/fakes/documents'
import {
  callsOf,
  createDesktopHandle,
  createFakeFileHandle,
  createFakeFolderHandle,
} from '../../test/fakes/files'
import { deferred, type Deferred } from '../../test/fakes/deferred'
import { settle } from '../../test/fakes/settle'
import { startFeatureCommand } from './command'
import { applyEdits, newDocument, openFile, save, saveAs, undo } from './document'
import { addPickedAsset } from './document-assets'

// Documents with imports and their assets folder (ui.md 3.19, 4.4, ADR 0037, ADR 0064): Open
// opens without assets and adds the files the core reports missing, says why imports are missing,
// "Choose folder" adds the assets to the open document, Save writes them before the document, a
// core restart gives them to the new core again, and a picked file is added by its kind. U-219 to
// U-222, U-226 to U-229 and U-279 to U-283 of ui.md 6.4.

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

/** The digest of the text `block.ptsm` holds on disk. */
function blockDigest(): Promise<string> {
  return fileDigest(utf8(documentText(BLOCK_DOCUMENT)))
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

/** The entries of each `add_assets` request, as plain kinds, names and bytes (a picked file has no name). */
function addedAssets(
  fake: FakeApp,
): readonly (readonly { kind: string; name: string | null; bytes: number[] }[])[] {
  return fake.transport.commandsOfType('add_assets').map((command) => {
    const buffers = fake.transport.buffersOf(command)
    return (command.assets ?? []).map((asset) => ({
      kind: asset.kind,
      name: asset.name ?? null,
      bytes: Array.from(buffers[asset.data] ?? []),
    }))
  })
}

/** The steps of the journal that open, read assets and add them, in order. */
function assetSteps(fake: FakeApp): readonly string[] {
  return journalSteps(fake).filter((step) =>
    ['command:open_document', 'files.readAssets', 'command:add_assets'].includes(step),
  )
}

/** The words of `assets_mismatch` for "Import 1" of `block.ptsm` (4.3, since M2). */
const BLOCK_MISMATCH =
  "The geometry of 'Import 1' is missing, because the assets folder next to block.ptsm holds another file in place of the one it uses. Copy the assets folder that came with block.ptsm next to it."

/** Gives `navigator` a clipboard whose `writeText` records the text. */
function stubClipboard(): Mock<(text: string) => Promise<void>> {
  const writeText = vi.fn<(text: string) => Promise<void>>(async () => undefined)
  Object.defineProperty(navigator, 'clipboard', { value: { writeText }, configurable: true })
  return writeText
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
  forgetCalls(fake)
  return { fake, ...place }
}

/** `block.ptsm` from `H`, given its folder `F` with "Choose folder", so its assets are `[A]` (U-227). */
async function blockWithFolder(siblings: readonly FileHandle[] = []): Promise<WebBlock> {
  const opened = await blockWithoutFolder(siblings)
  opened.fake.files.folderChoices.push({ kind: 'chosen', folder: opened.folder })
  await clickAction(
    opened.fake.app,
    notificationCoded(opened.fake.app, 'assets_folder_needed'),
    'Choose folder',
  )
  expect(importStatus(opened.fake.app)).toBe('ok')
  forgetCalls(opened.fake)
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

/** Records the kind of each mutation-queue task that sends `add_assets` (2.6: an `other` task). */
function addAssetsTaskKinds(fake: FakeApp): readonly TaskKind[] {
  const kinds: TaskKind[] = []
  const queue = fake.app.mutations
  const run = queue.run.bind(queue)
  vi.spyOn(queue, 'run').mockImplementation((task, kind = 'change') =>
    run(async () => {
      const sent = fake.transport.commandsOfType('add_assets').length
      const result = await task()
      if (fake.transport.commandsOfType('add_assets').length > sent) {
        kinds.push(kind)
      }
      return result
    }, kind),
  )
  return kinds
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
  it('U-219 opens without assets, then reads the missing names from a desktop handle and adds them', async () => {
    const fake = await startFakeApp()
    const handle = createDesktopHandle('block.ptsm', 'g1')
    fake.files.putAssets(handle, [ASSET_A, ASSET_B])
    forgetCalls(fake)
    const taskKinds = addAssetsTaskKinds(fake)
    await openTestDocument(fake, 'block.ptsm', BLOCK_DOCUMENT, handle)
    expect(openedAssets(fake)).toEqual([[]])
    expect(taskKinds).toEqual(['other'])
    expect(callsOf(fake.files, 'readAssets')).toEqual([[{ handle, folder: null }, [A]]])
    expect(assetSteps(fake)).toEqual([
      'command:open_document',
      'files.readAssets',
      'command:add_assets',
    ])
    expect(addedAssets(fake)).toEqual([[{ kind: 'step', name: A, bytes: [1, 2, 3] }]])
    expect(assetValues(currentDocument(fake.app).assets)).toEqual([{ name: A, bytes: [1, 2, 3] }])
    expect(currentDocument(fake.app).missingAssets).toEqual([])
    expect(importStatus(fake.app)).toBe('ok')
    expect(fake.app.store.getState().notifications).toEqual([])
  })

  it('U-219 a document that reads no asset reads none and sends no add_assets', async () => {
    const fake = await startFakeApp()
    const handle = createDesktopHandle('cube.ptsm', 'g1')
    fake.files.putAssets(handle, [ASSET_A, ASSET_B])
    forgetCalls(fake)
    await openTestDocument(fake, 'cube.ptsm', CUBE_DOCUMENT, handle)
    expect(callsOf(fake.files, 'readAssets')).toEqual([])
    expect(fake.transport.commandsOfType('open_document')[0]?.assets ?? []).toEqual([])
    expect(fake.transport.commandsOfType('add_assets')).toEqual([])
    expect(currentDocument(fake.app).assets).toEqual([])
  })

  it('a web handle with a folder reads the assets through that folder', async () => {
    const fake = await startFakeApp()
    const { handle, folder } = webPlace(fake)
    forgetCalls(fake)
    await openTestDocument(fake, 'block.ptsm', BLOCK_DOCUMENT, handle, folder)
    expect(callsOf(fake.files, 'readAssets')).toEqual([[{ handle, folder }, [A]]])
    expect(currentDocument(fake.app).file).toMatchObject({ name: 'block.ptsm', handle, folder })
    expect(importStatus(fake.app)).toBe('ok')
  })

  it('a dropped file, without a handle, reads no assets', async () => {
    const fake = await startFakeApp()
    forgetCalls(fake)
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
    forgetCalls(fake)
    await openTestDocument(fake, 'block.ptsm', BLOCK_DOCUMENT, handle)
    expect(fake.transport.commandsOfType('open_document')[0]?.assets ?? []).toEqual([])
    expect(fake.transport.commandsOfType('add_assets')).toEqual([])
    const notification = notificationCoded(fake.app, 'assets_not_read')
    expect(notification.severity).toBe('warning')
    expect(notification.message).toBe(
      'Partsmith could not read some imported files from the assets folder next to block.ptsm, so their geometry is missing.',
    )
    expect(actionLabels(notification)).toEqual(['Copy details'])
    expect(notificationCodes(fake.app)).toEqual(['assets_not_read'])
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

  it('U-220 (b) a file the core does not store opens once without assets and shows assets_mismatch', async () => {
    const fake = await startFakeApp({ core: { mismatchedAssets: [A] } })
    const handle = createDesktopHandle('block.ptsm', 'g1')
    fake.files.putAssets(handle, [ASSET_A])
    forgetCalls(fake)
    await openTestDocument(fake, 'block.ptsm', BLOCK_DOCUMENT, handle)
    expect(openedAssets(fake)).toEqual([[]])
    expect(addedAssets(fake)).toEqual([[{ kind: 'step', name: A, bytes: [1, 2, 3] }]])
    expect(currentDocument(fake.app).name).toBe('block')
    expect(importStatus(fake.app)).toBe('error')
    expect(notificationCodes(fake.app)).not.toContain('open_failed')
    const notification = notificationCoded(fake.app, 'assets_mismatch')
    expect(notification.severity).toBe('warning')
    expect(notification.message).toBe(BLOCK_MISMATCH)
    expect(actionLabels(notification)).toEqual(['Copy details'])
    const writeText = stubClipboard()
    runAction(notification, 'Copy details')
    await settle()
    expect(writeText).toHaveBeenCalledWith(
      ['assets_mismatch', BLOCK_MISMATCH, `assets/${A}: its contents do not match its name`].join(
        '\n',
      ),
    )
  })

  it('U-220 (b) the session keeps no file the core did not store: a restart sends assets: [] and no buffer', async () => {
    const fake = await startFakeApp({ core: { mismatchedAssets: [A] } })
    const handle = createDesktopHandle('block.ptsm', 'g1')
    fake.files.putAssets(handle, [ASSET_A])
    await openTestDocument(fake, 'block.ptsm', BLOCK_DOCUMENT, handle)
    expect(currentDocument(fake.app).assets).toEqual([])
    fake.transport.die('The worker crashed.')
    await settle()
    forgetCalls(fake)
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
    expect(notificationCodes(fake.app)).not.toContain('assets_not_read')
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
    expect(notificationCodes(fake.app)).toContain('assets_folder_needed')
  })

  it('U-221 (a) the warning goes once f-imp1 is no longer in error', async () => {
    const { fake } = await blockWithoutFolder()
    // Suppressing the import is a recompute in which f-imp1 is not in error.
    await applyEdits(fake.app, 'Suppress', [{ type: 'update_feature', feature: SUPPRESSED_IMPORT }])
    await settle()
    expect(importStatus(fake.app)).toBe('suppressed')
    expect(notificationCodes(fake.app)).not.toContain('assets_folder_needed')
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
    forgetCalls(fake)
    await clickAction(fake.app, notification, 'Open...')
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
      expect(notificationCodes(fake.app)).not.toContain(code)
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

/** Clicks "Choose folder" of the shown `assets_folder_needed`. */
async function chooseFolder(fake: FakeApp): Promise<void> {
  await clickAction(fake.app, notificationCoded(fake.app, 'assets_folder_needed'), 'Choose folder')
}

describe('Choose folder adds the assets to the open document (3.19, since M2, ADR 0064)', () => {
  it('U-222 another folder shows assets_wrong_folder, whose Choose folder adds the assets once the right one is chosen', async () => {
    const { fake, handle, folder } = await blockWithoutFolder()
    const { recoveryId, documentHandle } = currentDocument(fake.app)
    await applyEdits(fake.app, 'Rename', RENAME_TO_BLOCK)
    await settle()
    forgetCalls(fake)
    fake.files.folderChoices.push({ kind: 'other', name: 'parts' })
    await chooseFolder(fake)
    expect(callsOf(fake.files, 'chooseFolder')).toEqual([[handle]])
    const wrong = notificationCoded(fake.app, 'assets_wrong_folder')
    expect(wrong.severity).toBe('error')
    expect(wrong.message).toBe(
      'parts is not the folder that holds block.ptsm. Choose the folder block.ptsm is in.',
    )
    expect(actionLabels(wrong)).toEqual(['Choose folder'])

    forgetCalls(fake)
    const taskKinds = addAssetsTaskKinds(fake)
    fake.files.folderChoices.push({ kind: 'chosen', folder })
    await clickAction(fake.app, wrong, 'Choose folder')
    expect(callsOf(fake.files, 'readAssets')).toEqual([[{ handle, folder }, [A]]])
    expect(taskKinds).toEqual(['other'])
    expect(assetSteps(fake)).toEqual(['files.readAssets', 'command:add_assets'])
    expect(addedAssets(fake)).toEqual([[{ kind: 'step', name: A, bytes: [1, 2, 3] }]])
    expect(fake.transport.commandsOfType('add_assets')[0]?.doc).toBe(documentHandle)
    expect(fake.transport.commandsOfType('open_document')).toEqual([])
    expect(fake.transport.commandsOfType('close_document')).toEqual([])
    const document = currentDocument(fake.app)
    expect(document.documentHandle).toBe(documentHandle)
    expect(document.name).toBe('block')
    expect(document.recoveryId).toBe(recoveryId)
    expect(document.file).toMatchObject({ name: 'block.ptsm', handle, folder })
    expect(assetValues(document.assets)).toEqual([{ name: A, bytes: [1, 2, 3] }])
    expect(document.localUnsaved).toBe(false)
    expect(document.dirty).toBe(true)
    expect(importStatus(fake.app)).toBe('ok')
    const [recent] = await fake.recent.list()
    expect(recent).toMatchObject({ handle, folder })
    expect(notificationCodes(fake.app)).not.toContain('assets_folder_needed')
    expect(notificationCodes(fake.app)).not.toContain('assets_wrong_folder')
  })

  it('U-222 without unsaved changes the document stays saved', async () => {
    const { fake, folder } = await blockWithoutFolder()
    fake.files.folderChoices.push({ kind: 'chosen', folder })
    await chooseFolder(fake)
    const document = currentDocument(fake.app)
    expect(document.localUnsaved).toBe(false)
    expect(isUnsaved(document)).toBe(false)
  })

  it('U-222 a canceled dialog sends nothing and shows assets_folder_needed again for f-imp1', async () => {
    const { fake, handle } = await blockWithoutFolder()
    fake.files.folderChoices.push({ kind: 'canceled' })
    await chooseFolder(fake)
    expect(callsOf(fake.files, 'chooseFolder')).toEqual([[handle]])
    expect(fake.transport.commandTypes()).toEqual([])
    expect(callsOf(fake.files, 'readAssets')).toEqual([])
    const again = notificationCoded(fake.app, 'assets_folder_needed')
    expect(again.message).toBe(FOLDER_NEEDED)
    expect(again.features).toEqual(['f-imp1'])
    expect(actionLabels(again)).toEqual(['Choose folder'])
  })

  it('U-222 a chosen folder without the asset reads it, sends no add_assets and shows nothing', async () => {
    const { fake, handle } = await blockWithoutFolder()
    const empty = createFakeFolderHandle('empty', [handle])
    fake.files.folderChoices.push({ kind: 'chosen', folder: empty })
    await chooseFolder(fake)
    expect(callsOf(fake.files, 'readAssets')).toEqual([[{ handle, folder: empty }, [A]]])
    expect(fake.transport.commandsOfType('add_assets')).toEqual([])
    expect(fake.app.store.getState().notifications).toEqual([])
  })

  it('U-222 a Choose folder clicked after New replaced the document calls no chooseFolder', async () => {
    const { fake } = await blockWithoutFolder()
    const notification = notificationCoded(fake.app, 'assets_folder_needed')
    await newDocument(fake.app)
    await settle()
    forgetCalls(fake)
    await clickAction(fake.app, notification, 'Choose folder')
    expect(callsOf(fake.files, 'chooseFolder')).toEqual([])
    expect(fake.transport.commandsOfType('add_assets')).toEqual([])
  })

  it('U-222 an edit committed while the folder is read is sent before add_assets, to the same document', async () => {
    const { fake, folder } = await blockWithoutFolder()
    const { documentHandle } = currentDocument(fake.app)
    const held = holdReadAssets(fake)
    fake.files.folderChoices.push({ kind: 'chosen', folder })
    const choosing = chooseFolder(fake)
    await settle()
    await applyEdits(fake.app, 'Rename', RENAME_TO_BLOCK)
    held.resolve()
    await choosing
    await settle()
    const types = fake.transport.commandTypes()
    expect(types.indexOf('apply_edits')).toBeLessThan(types.indexOf('add_assets'))
    expect(fake.transport.commandsOfType('add_assets')[0]?.doc).toBe(documentHandle)
    expect(bodiesOf(currentDocument(fake.app).recipe)[0]?.name).toBe('Block')
  })

  it('U-222 a rename still waiting for the core when the folder is chosen is sent before add_assets, to the same document', async () => {
    const { fake, folder } = await blockWithoutFolder()
    const { documentHandle } = currentDocument(fake.app)
    fake.transport.pause()
    const renaming = applyEdits(fake.app, 'Rename', RENAME_TO_BLOCK)
    fake.files.folderChoices.push({ kind: 'chosen', folder })
    await chooseFolder(fake)
    expect(callsOf(fake.files, 'readAssets')).toHaveLength(1)
    expect(fake.transport.commandTypes()).toEqual(['apply_edits'])
    fake.transport.resume()
    await renaming
    await settle()
    expect(fake.transport.commandTypes()).toEqual(['apply_edits', 'add_assets'])
    expect(fake.transport.commandsOfType('add_assets')[0]?.doc).toBe(documentHandle)
    expect(bodiesOf(currentDocument(fake.app).recipe)[0]?.name).toBe('Block')
  })

  it('U-222 a rename made while add_assets has not answered is sent once it has, with the same handle', async () => {
    const { fake, folder } = await blockWithoutFolder()
    const { documentHandle } = currentDocument(fake.app)
    const held = deferred<void>()
    fake.transport.handle('add_assets', async (command, core, buffers) => {
      await held.promise
      return core.reply(command, buffers)
    })
    fake.files.folderChoices.push({ kind: 'chosen', folder })
    const choosing = chooseFolder(fake)
    await settle()
    const renaming = applyEdits(fake.app, 'Rename', RENAME_TO_BLOCK)
    await settle()
    expect(fake.transport.commandsOfType('apply_edits')).toEqual([])
    held.resolve()
    await choosing
    await renaming
    await settle()
    expect(fake.transport.commandTypes()).toEqual(['add_assets', 'apply_edits'])
    expect(fake.transport.commandsOfType('apply_edits')[0]?.doc).toBe(documentHandle)
    expect(bodiesOf(currentDocument(fake.app).recipe)[0]?.name).toBe('Block')
  })

  it('U-222 a New while the folder is read sends no add_assets', async () => {
    const { fake, folder } = await blockWithoutFolder()
    const held = holdReadAssets(fake)
    fake.files.folderChoices.push({ kind: 'chosen', folder })
    const choosing = chooseFolder(fake)
    await settle()
    await newDocument(fake.app)
    held.resolve()
    await choosing
    await settle()
    expect(fake.transport.commandsOfType('add_assets')).toEqual([])
  })

  it('U-222 a New that has ended the session but not yet shown its document sends no add_assets', async () => {
    const { fake, folder } = await blockWithoutFolder()
    const held = holdReadAssets(fake)
    fake.files.folderChoices.push({ kind: 'chosen', folder })
    const choosing = chooseFolder(fake)
    await settle()
    fake.transport.pause()
    const replacing = newDocument(fake.app)
    await settle()
    held.resolve()
    await settle()
    fake.transport.resume()
    await replacing
    await choosing
    await settle()
    expect(fake.transport.commandsOfType('add_assets')).toEqual([])
    expect(notificationCodes(fake.app)).not.toContain('unknown_document')
  })

  it('a folder whose asset the core does not store leaves the document as it is and shows assets_mismatch', async () => {
    const fake = await startFakeApp({ core: { mismatchedAssets: [A] } })
    const { handle, folder } = webPlace(fake)
    await openTestDocument(fake, 'block.ptsm', BLOCK_DOCUMENT, handle)
    const { documentHandle } = currentDocument(fake.app)
    fake.files.folderChoices.push({ kind: 'chosen', folder })
    await chooseFolder(fake)
    expect(currentDocument(fake.app).documentHandle).toBe(documentHandle)
    expect(currentDocument(fake.app).assets).toEqual([])
    expect(importStatus(fake.app)).toBe('error')
    expect(notificationCoded(fake.app, 'assets_mismatch').message).toBe(BLOCK_MISMATCH)
  })

  it('a folder dialog that fails shows its message as an error and reads nothing (3.19)', async () => {
    const { fake } = await blockWithoutFolder()
    vi.spyOn(fake.files, 'chooseFolder').mockRejectedValue(
      new FileServiceError('io', 'The folder dialog failed.'),
    )
    await chooseFolder(fake)
    expect(callsOf(fake.files, 'readAssets')).toEqual([])
    expect(fake.app.store.getState().notifications.map((shown) => shown.message)).toContain(
      'The folder dialog failed.',
    )
  })

  it('U-280 Choose folder keeps the undo history: undo goes to the same document', async () => {
    const { fake, folder } = await blockWithoutFolder()
    await applyEdits(fake.app, 'Rename', RENAME_TO_BLOCK)
    await settle()
    const before = currentDocument(fake.app)
    forgetCalls(fake)
    fake.files.folderChoices.push({ kind: 'chosen', folder })
    await chooseFolder(fake)
    for (const type of [
      'open_document',
      'close_document',
      'commit_transaction',
      'cancel_transaction',
    ] as const) {
      expect(fake.transport.commandsOfType(type)).toEqual([])
    }
    const after = currentDocument(fake.app)
    expect(after.documentHandle).toBe(before.documentHandle)
    expect(after.revision).toBe(before.revision)
    expect(after.recoveryId).toBe(before.recoveryId)
    expect(after.undoLabel).toBe(before.undoLabel)
    expect(after.localUnsaved).toBe(false)
    expect(after.dirty).toBe(true)
    expect(after.missingAssets).toEqual([])
    expect(fake.app.store.getState().notifications).toEqual([])

    await undo(fake.app)
    await settle()

    expect(fake.transport.commandsOfType('undo')).toEqual([
      { type: 'undo', doc: before.documentHandle },
    ])
    expect(bodiesOf(currentDocument(fake.app).recipe)[0]?.name).toBe('Body 1')
  })

  it('U-281 Choose folder during a feature command adds the files at once and keeps the command open', async () => {
    const { fake, folder } = await blockWithoutFolder()
    const notification = notificationCoded(fake.app, 'assets_folder_needed')
    expect(
      notification.actions.find((action) => action.label === 'Choose folder')?.editsDocument,
    ).toBe(false)
    await startFeatureCommand(fake.app, 'fillet')
    await settle()
    const command = fake.app.store.getState().command
    forgetCalls(fake)
    fake.files.folderChoices.push({ kind: 'chosen', folder })
    await clickAction(fake.app, notification, 'Choose folder')
    expect(callsOf(fake.files, 'chooseFolder')).toHaveLength(1)
    expect(fake.transport.commandsOfType('add_assets')).toHaveLength(1)
    expect(fake.transport.commandsOfType('commit_transaction')).toEqual([])
    expect(fake.transport.commandsOfType('cancel_transaction')).toEqual([])
    expect(currentDocument(fake.app).transactionOpen).toBe(true)
    expect(notificationCodes(fake.app)).not.toContain('finish_command_first')
    expect(fake.app.store.getState().command).toEqual(command)
  })

  it('U-282 an add that the stopped core never took shows assets_stopped with Choose folder', async () => {
    const { fake, folder } = await blockWithoutFolder()
    fake.transport.handle('add_assets', () =>
      Promise.reject(new CoreFatalError('The geometry core was stopped.')),
    )
    fake.files.folderChoices.push({ kind: 'chosen', folder })
    await chooseFolder(fake)
    const stopped = notificationCoded(fake.app, 'assets_stopped')
    expect(stopped.severity).toBe('warning')
    expect(stopped.message).toBe(
      'The geometry that block.ptsm reads from its assets folder is still missing, because the geometry core stopped before it took the files. Choose the folder again.',
    )
    expect(actionLabels(stopped)).toEqual(['Choose folder'])
    expect(notificationCodes(fake.app)).not.toContain('open_stopped')
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
  forgetCalls(fake)
  return { fake, handle }
}

describe('Save writes the assets first (3.19 writing the document)', () => {
  it('U-226 Save calls saveDocument, readHandle, writeAssets, write and markSaved, in that order', async () => {
    const { fake, handle } = await renamedDesktopBlock()
    await save(fake.app)
    await settle()
    expect(journalSteps(fake)).toEqual([
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
    forgetCalls(fake)
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
    forgetCalls(fake)
    fake.files.folderChoices.push({ kind: 'chosen', folder: copy })
    await clickAction(fake.app, notificationCoded(fake.app, 'save_folder_needed'), 'Choose folder')
    expect(callsOf(fake.files, 'chooseFolder')).toEqual([[target]])
    expect(journalSteps(fake).filter((step) => step !== 'files.chooseFolder')).toEqual([
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
    await clickAction(fake.app, notificationCoded(fake.app, 'save_folder_needed'), 'Choose folder')
    const wrong = notificationCoded(fake.app, 'assets_wrong_folder')
    expect(wrong.message).toBe(
      'parts is not the folder that holds block.ptsm. Choose the folder block.ptsm is in.',
    )
    forgetCalls(fake)
    fake.files.folderChoices.push({ kind: 'canceled' })
    await clickAction(fake.app, wrong, 'Choose folder')
    expect(callsOf(fake.files, 'chooseFolder')).toEqual([[target]])
  })

  it('U-227 a canceled dialog writes nothing, keeps the file and shows save_folder_needed again for the target', async () => {
    const { fake, handle } = await blockWithFolder()
    const target = createFakeFileHandle('block.ptsm')
    await saveTo(fake, target)
    fake.files.folderChoices.push({ kind: 'canceled' })
    await clickAction(fake.app, notificationCoded(fake.app, 'save_folder_needed'), 'Choose folder')
    expect(callsOf(fake.files, 'writeAssets')).toEqual([])
    expect(callsOf(fake.files, 'write')).toEqual([])
    expect(currentDocument(fake.app).file?.handle).toBe(handle)
    const again = notificationCoded(fake.app, 'save_folder_needed')
    forgetCalls(fake)
    await clickAction(fake.app, again, 'Choose folder')
    expect(callsOf(fake.files, 'chooseFolder')).toEqual([[target]])
  })

  it('U-227 its Choose folder clicked after New calls no chooseFolder, writeAssets or write', async () => {
    const { fake } = await blockWithFolder()
    const target = createFakeFileHandle('block.ptsm')
    await saveTo(fake, target)
    const notification = notificationCoded(fake.app, 'save_folder_needed')
    await newDocument(fake.app)
    await settle()
    forgetCalls(fake)
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
    forgetCalls(fake)
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
    expect(notificationCodes(fake.app)).not.toContain('save_folder_needed')
  })
})

describe('a core restart keeps the assets (4.4)', () => {
  it("U-228 reopens with the session's assets, reads no folder and keeps the file reference", async () => {
    const { fake, handle, folder } = await blockWithFolder()
    const before = currentDocument(fake.app)
    expect(before.file?.digest).toBe(await blockDigest())
    fake.transport.die('The worker crashed.')
    await settle()
    forgetCalls(fake)
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

  it('U-228 the restart then adds the stored files with one add_assets (since M2)', async () => {
    const { fake } = await blockWithFolder()
    fake.transport.die('The worker crashed.')
    await settle()
    forgetCalls(fake)
    await fake.app.restartCore()
    await settle()
    expect(assetSteps(fake)).toEqual(['command:open_document', 'command:add_assets'])
    expect(addedAssets(fake)).toEqual([[{ kind: 'step', name: A, bytes: [1, 2, 3] }]])
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
  forgetCalls(opened.fake)
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
    expect(assetSteps(fake)).toEqual([
      'command:open_document',
      'files.readAssets',
      'command:add_assets',
    ])
    expect(callsOf(fake.files, 'readAssets')).toEqual([[{ handle, folder }, [A]]])
    expect(openedAssets(fake)).toEqual([[]])
    expect(addedAssets(fake)).toEqual([[{ kind: 'step', name: A, bytes: [1, 2, 3] }]])
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
    forgetCalls(fake)
    runAction(notificationCoded(fake.app, 'recovered'), 'Open')
    await settle()
    expect(assetSteps(fake)).toEqual([
      'command:open_document',
      'files.readAssets',
      'command:add_assets',
    ])
    expect(callsOf(fake.files, 'readAssets')).toEqual([[{ handle, folder }, [A]]])
    expect(openedAssets(fake)).toEqual([[]])
    expect(addedAssets(fake)).toEqual([[{ kind: 'step', name: A, bytes: [1, 2, 3] }]])
    const document = currentDocument(fake.app)
    expect(document.file).toEqual({ name: 'block.ptsm', handle, folder, digest: entry.digest })
    expect(document.localUnsaved).toBe(true)
  })

  it('U-229 opening the file again without a folder takes the folder of its recent entry', async () => {
    const fake = await startFakeApp()
    const { handle, folder } = webPlace(fake)
    await fake.recent.remember({ name: 'block.ptsm', handle, folder, text: '' })
    forgetCalls(fake)
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
    forgetCalls(fake)
    await openFile(fake.app, documentFile('block.ptsm', BLOCK_DOCUMENT, handle))
    await settle()
    const places = callsOf(fake.files, 'readAssets').map(([place]) => place as DocumentPlace)
    expect(places.every((place) => place.folder !== elsewhere)).toBe(true)
    expect(currentDocument(fake.app).file?.folder ?? null).toBeNull()
  })
})

/** `two-imports.ptsm`: `f-imp1` "Import 1", an import of `A` into `b-1`, then `f-imp2` "Import 2", an import of `B` into `b-2`. */
const TWO_IMPORTS: Document = {
  ...BLOCK_DOCUMENT,
  bodies: [
    { id: 'b-1', name: 'Body 1' },
    { id: 'b-2', name: 'Body 2' },
  ],
  features: [
    { id: 'f-imp1', type: 'import', body: 'b-1', asset: A, op: 'new' },
    { id: 'f-imp2', type: 'import', body: 'b-2', asset: ASSET_B.name, op: 'new' },
  ],
}

/** `two-imports.ptsm` opened from a desktop handle whose folder holds `A` and `B`. */
async function twoImportsOpened(mismatchedAssets: readonly string[]): Promise<FakeApp> {
  const fake = await startFakeApp({ core: { mismatchedAssets } })
  const handle = createDesktopHandle('two-imports.ptsm', 'g1')
  fake.files.putAssets(handle, [ASSET_A, ASSET_B])
  await openTestDocument(fake, 'two-imports.ptsm', TWO_IMPORTS, handle)
  return fake
}

describe('the words of files the core does not store (3.19 openFile step 6, since M2)', () => {
  it('U-279 (a) two refused files name the first feature and count the other', async () => {
    const fake = await twoImportsOpened([A, ASSET_B.name])
    const message =
      "The geometry of 'Import 1' and 1 other feature is missing, because the assets folder next to two-imports.ptsm holds other files in place of the ones they use. Copy the assets folder that came with two-imports.ptsm next to it."
    const notification = notificationCoded(fake.app, 'assets_mismatch_more')
    expect(notification.severity).toBe('warning')
    expect(notification.message).toBe(message)
    const writeText = stubClipboard()
    runAction(notification, 'Copy details')
    await settle()
    expect(writeText).toHaveBeenCalledWith(
      [
        'assets_mismatch_more',
        message,
        `assets/${A}: its contents do not match its name`,
        `assets/${ASSET_B.name}: its contents do not match its name`,
      ].join('\n'),
    )
    expect(currentDocument(fake.app).assets).toEqual([])
  })

  it('U-279 (b) one refused file names the feature that reads it', async () => {
    const fake = await twoImportsOpened([ASSET_B.name])
    expect(notificationCoded(fake.app, 'assets_mismatch').message).toBe(
      "The geometry of 'Import 2' is missing, because the assets folder next to two-imports.ptsm holds another file in place of the one it uses. Copy the assets folder that came with two-imports.ptsm next to it.",
    )
    expect(assetValues(currentDocument(fake.app).assets)).toEqual([{ name: A, bytes: [1, 2, 3] }])
    expect(currentDocument(fake.app).featureStatus['f-imp1']?.status).toBe('ok')
    expect(currentDocument(fake.app).featureStatus['f-imp2']?.status).toBe('error')
  })
})

describe('an add the stopped core never took (4.4, since M2)', () => {
  it('U-282 an Open whose add_assets the stopped core never answered shows open_stopped and not the document', async () => {
    const fake = await startFakeApp()
    const before = currentDocument(fake.app).documentHandle
    const handle = createDesktopHandle('block.ptsm', 'g1')
    fake.files.putAssets(handle, [ASSET_A])
    fake.transport.handle('add_assets', () =>
      Promise.reject(new CoreFatalError('The geometry core was stopped.')),
    )
    await openFile(fake.app, documentFile('block.ptsm', BLOCK_DOCUMENT, handle))
    await settle()
    expect(notificationCodes(fake.app)).toContain('open_stopped')
    expect(currentDocument(fake.app).name).not.toBe('block')
    expect(currentDocument(fake.app).documentHandle).toBe(before)
  })
})

/** `C`: the name the fake core gives a file of `kind` without a name. */
function pickedName(kind: string): string {
  return `${'c'.repeat(64)}.${kind}`
}

/** A file the user picked, without a handle. */
function picked(name: string, ...bytes: number[]) {
  return { name, bytes: Uint8Array.from(bytes), handle: null }
}

/** `block.ptsm` dropped: it has no handle and no assets. */
async function droppedBlock(): Promise<FakeApp> {
  const fake = await startFakeApp()
  await openTestDocument(fake, 'block.ptsm', BLOCK_DOCUMENT)
  forgetCalls(fake)
  return fake
}

describe('addPickedAsset (3.19 picked files, since M2, ADR 0064 decision 5)', () => {
  it('U-283 (a) a picked STEP file is added without a name as an other task and named by the core', async () => {
    const fake = await droppedBlock()
    const run = vi.spyOn(fake.app.mutations, 'run')

    const name = await addPickedAsset(fake.app, picked('part.STEP', 7, 8), 'step')

    expect(name).toBe(pickedName('step'))
    expect(addedAssets(fake)).toEqual([[{ kind: 'step', name: null, bytes: [7, 8] }]])
    expect(fake.transport.commandsOfType('add_assets')[0]?.assets?.[0]).not.toHaveProperty('name')
    expect(run).toHaveBeenCalledWith(expect.any(Function), 'other')
    expect(assetValues(currentDocument(fake.app).assets).at(-1)).toEqual({
      name: pickedName('step'),
      bytes: [7, 8],
    })
  })

  it('U-283 (a) the same file again resolves with its name and adds no second entry', async () => {
    const fake = await droppedBlock()
    await addPickedAsset(fake.app, picked('part.STEP', 7, 8), 'step')

    const again = await addPickedAsset(fake.app, picked('part.STEP', 7, 8), 'step')

    expect(again).toBe(pickedName('step'))
    expect(
      currentDocument(fake.app).assets.filter((asset) => asset.name === pickedName('step')),
    ).toHaveLength(1)
  })

  it.each([
    ['logo.otf', 'otf'],
    ['logo.ttf', 'ttf'],
  ])('U-283 (b) %s picked as a font is sent with the kind %s', async (fileName, kind) => {
    const fake = await droppedBlock()

    await addPickedAsset(fake.app, picked(fileName, 1), 'font')

    expect(addedAssets(fake)).toEqual([[{ kind, name: null, bytes: [1] }]])
  })

  it('U-283 (c) a file of another kind shows asset_not_step or asset_not_font and sends nothing', async () => {
    const fake = await droppedBlock()

    const notStep = await addPickedAsset(fake.app, picked('notes.txt', 1), 'step')
    const notFont = await addPickedAsset(fake.app, picked('part.step', 1), 'font')

    expect(notStep).toBeNull()
    expect(notFont).toBeNull()
    expect(fake.transport.commandsOfType('add_assets')).toEqual([])
    expect(notificationCoded(fake.app, 'asset_not_step').message).toBe(
      'notes.txt is not a STEP file. Partsmith imports .step and .stp files.',
    )
    expect(notificationCoded(fake.app, 'asset_not_step').severity).toBe('error')
    expect(notificationCoded(fake.app, 'asset_not_font').message).toBe(
      'part.step is not a font file. Partsmith uses .ttf and .otf fonts.',
    )
  })

  it('U-283 (d) during a feature command it sends at once and the command stays open', async () => {
    const fake = await droppedBlock()
    await startFeatureCommand(fake.app, 'fillet')
    await settle()
    const command = fake.app.store.getState().command

    const name = await addPickedAsset(fake.app, picked('part.STEP', 7, 8), 'step')

    expect(name).toBe(pickedName('step'))
    expect(fake.transport.commandsOfType('add_assets')).toHaveLength(1)
    expect(fake.app.store.getState().command).toEqual(command)
  })

  it('U-283 (e) a document that New replaced before its turn gets nothing', async () => {
    const fake = await droppedBlock()
    fake.transport.pause()
    const renaming = applyEdits(fake.app, 'Rename', RENAME_TO_BLOCK)
    const replacing = newDocument(fake.app)
    const adding = addPickedAsset(fake.app, picked('part.STEP', 7, 8), 'step')
    fake.transport.resume()
    await Promise.allSettled([renaming, replacing])

    expect(await adding).toBeNull()
    expect(fake.transport.commandsOfType('add_assets')).toEqual([])
  })

  it('U-283 (f) a core error shows asset_add_failed with the core message', async () => {
    const fake = await droppedBlock()
    fake.transport.handle('add_assets', () => ({
      error: { code: 'document_poisoned', message: 'The document is broken.' },
    }))

    const name = await addPickedAsset(fake.app, picked('part.STEP', 7, 8), 'step')

    expect(name).toBeNull()
    expect(notificationCoded(fake.app, 'asset_add_failed').message).toBe(
      'Could not add part.STEP. The document is broken.',
    )
  })

  it('U-283 (f) a stopped core shows asset_add_stopped', async () => {
    const fake = await droppedBlock()
    fake.transport.handle('add_assets', () =>
      Promise.reject(new CoreFatalError('The geometry core was stopped.')),
    )

    const name = await addPickedAsset(fake.app, picked('part.STEP', 7, 8), 'step')

    expect(name).toBeNull()
    expect(notificationCoded(fake.app, 'asset_add_stopped').message).toBe(
      'part.STEP was not added, because the geometry core stopped before adding it. Add it again.',
    )
  })

  it('U-283 (g) a restart gives the new core the picked file again', async () => {
    const fake = await droppedBlock()
    await addPickedAsset(fake.app, picked('part.STEP', 7, 8), 'step')
    fake.transport.die('The worker crashed.')
    await settle()
    forgetCalls(fake)

    await fake.app.restartCore()
    await settle()

    expect(openedAssets(fake)).toEqual([[{ name: pickedName('step'), bytes: [7, 8] }]])
    expect(assetSteps(fake)).toEqual(['command:open_document', 'command:add_assets'])
    expect(addedAssets(fake)).toEqual([[{ kind: 'step', name: pickedName('step'), bytes: [7, 8] }]])
  })
})
