import { CoreError } from '../../core/client'
import { assetKindOf, namedAssets, type AssetFile } from '../../files/assets'
import {
  ASSETS_FOLDER,
  FileServiceError,
  folderHolds,
  isSameHandle,
  type AssetReading,
  type DocumentPlace,
  type FileHandle,
  type OpenedFile,
} from '../../files/file-service'
import type { AddedAsset, AssetKind, AssetUse, DocHandle } from '../../generated/protocol'
import { MESSAGES } from '../../messages'
import { pendingMissingAssets } from '../../state/core-events'
import { selectFeatureLabel } from '../../state/selectors'
import type {
  DocumentState,
  FileReference,
  Notification,
  NotificationAction,
} from '../../state/store'
import { settledRecipeOf } from '../connection'
import type { App } from '../createApp'
import { sessionGoesOn } from '../recovery-copy'
import { documentOf, showCoreError } from './edits'
import { showFileError } from './files'
import { copyDetails } from './fixes'
import { notify } from './notifications'

// A document's assets folder in the UI (ui.md 3.19, ADR 0037, ADR 0064): the files Open reads for
// the names the core reports missing and adds to the opened document, why imports are missing, the
// folder the web asks for ("Choose folder"), which adds its files to the open document, the files
// a user picks, and the actions offered for one document only.

/** The file name the messages of 3.19 give a document: its file's name, else its name and `.ptsm`. */
function fileNameOf(document: {
  readonly name: string
  readonly file: FileReference | null
}): string {
  return document.file?.name ?? `${document.name}.ptsm`
}

/** The document session and file handle an action was offered for (3.19). */
export type DocumentOffer = { readonly recoveryId: string; readonly handle: FileHandle | null }

export function offerOf(document: DocumentState): DocumentOffer {
  return { recoveryId: document.recoveryId, handle: fileHandleOf(document) }
}

/** The handle of a document's file; null without one. */
export function fileHandleOf(document: DocumentState): FileHandle | null {
  return document.file?.handle ?? null
}

/**
 * Whether the shown document is still the session an action was offered for, with the file
 * handle it had then, so a click after New, Open or a Save as to another file never saves one
 * document into another one's file or reopens the wrong document (3.19).
 */
export function isOffered(app: App, offer: DocumentOffer): boolean {
  const document = documentOf(app)
  return (
    document !== null &&
    document.recoveryId === offer.recoveryId &&
    fileHandleOf(document) === offer.handle
  )
}

/** The place of a file reference with a handle; null without one. */
function placeOf(file: FileReference | null): DocumentPlace | null {
  return file === null || file.handle === null ? null : { handle: file.handle, folder: file.folder }
}

/**
 * The folder a file is opened with (openFile step 3): the given one, else, for a web handle, the
 * folder of its recent entry when that folder holds it, so a file opened again with the picker
 * keeps the folder an earlier session chose.
 */
export async function folderOfFile(
  app: App,
  handle: FileHandle | null,
  folder: FileSystemDirectoryHandle | null,
): Promise<FileSystemDirectoryHandle | null> {
  if (folder !== null || handle === null || handle.kind === 'desktop') {
    return folder
  }
  const folders = await Promise.all(
    (await app.recent.list()).map(async (entry) =>
      entry.handle !== null &&
      entry.folder !== null &&
      (await isSameHandle(entry.handle, handle)) &&
      (await folderHolds(entry.folder, handle))
        ? entry.folder
        : null,
    ),
  )
  return folders.find((recentFolder) => recentFolder !== null) ?? null
}

/** A reading of a folder that was reached but had no name to read (openFile step 4). */
const NOTHING_READ: AssetReading = { assets: [], failure: null }

/**
 * The names of `missing` that a host reads from a folder: those whose name has an asset kind, in
 * order (3.19).
 */
function namesToRead(missing: readonly AssetUse[]): readonly string[] {
  return missing.map((use) => use.name).filter((name) => assetKindOf(name) !== null)
}

/** What one `addAssets` of files read from a folder did: the files the core stored, and the missing names it refused, with the features that read them, in the order read. */
type AddedFiles = { readonly stored: readonly AssetFile[]; readonly refused: readonly AssetUse[] }

/** Nothing added. */
const NOTHING_ADDED: AddedFiles = { stored: [], refused: [] }

/** Sorts the files given to one `addAssets` by its result, which lists one entry per file in order. */
function sortAdded(
  files: readonly AssetFile[],
  added: readonly AddedAsset[],
  missing: readonly AssetUse[],
): AddedFiles {
  const refused = new Set(
    files.filter((_, index) => added[index]?.stored === false).map((file) => file.name),
  )
  return {
    stored: files.filter((_, index) => added[index]?.stored === true),
    refused: missing.filter((use) => refused.has(use.name)),
  }
}

/** The assets read from a folder for an opened document (openFile step 4): the reading, null when the host does not reach the folder, and what the core stored. */
export type FolderAssets = { readonly reading: AssetReading | null; readonly added: AddedFiles }

/**
 * Reads the names an opened document misses from the folder next to its file and gives what it
 * read to the core with one `addAssets` in an `other` task (openFile step 4). A `CoreError` of the
 * add is shown as it is and the document opens without assets; a `CoreFatalError` is thrown.
 */
export async function addFolderAssets(
  app: App,
  documentHandle: DocHandle,
  file: FileReference | null,
): Promise<FolderAssets> {
  const place = placeOf(file)
  if (place === null || !app.files.reachesFolder(place)) {
    return { reading: null, added: NOTHING_ADDED }
  }
  const missing = pendingMissingAssets(app.store, documentHandle)
  const names = namesToRead(missing)
  const reading = names.length === 0 ? NOTHING_READ : await app.files.readAssets(place, names)
  if (reading.assets.length === 0) {
    return { reading, added: NOTHING_ADDED }
  }
  try {
    const added = await app.mutations.run(
      () => app.core.addAssets(documentHandle, namedAssets(reading.assets)),
      'other',
    )
    return { reading, added: sortAdded(reading.assets, added, missing) }
  } catch (error) {
    if (!(error instanceof CoreError)) {
      throw error
    }
    showCoreError(app, error)
    return { reading, added: NOTHING_ADDED }
  }
}

/** Whether a reading reached the folder: a refused permission of the folder itself counts as not reached (openFile step 6). */
function reachedFolder(reading: AssetReading | null): reading is AssetReading {
  return (
    reading !== null &&
    !(reading.failure?.reason === 'permission' && reading.failure.entry === ASSETS_FOLDER)
  )
}

/** The detail of a failed asset read or write for Copy details: `{entry}: {message}` (3.19). */
function failureDetail(failure: FileServiceError): string {
  return failure.entry === null ? failure.message : `${failure.entry}: ${failure.message}`
}

/** A warning or error whose only action is Copy details with `detail`. */
function notifyWithDetails(
  app: App,
  notification: Pick<Notification, 'severity' | 'message' | 'actions'> & { readonly code: string },
  detail: string,
): void {
  const { code, message } = notification
  notify(app, {
    ...notification,
    actions: [
      ...notification.actions,
      {
        label: 'Copy details',
        run: () => {
          void copyDetails(app, { code, message, detail })
        },
      },
    ],
  })
}

/** `assets_not_read`: some assets could not be read from the folder (openFile step 6, Choose folder). */
function showAssetsNotRead(app: App, fileName: string, failure: FileServiceError): void {
  notifyWithDetails(
    app,
    {
      severity: 'warning',
      message: MESSAGES.assets_not_read(fileName),
      code: 'assets_not_read',
      actions: [],
    },
    failureDetail(failure),
  )
}

/**
 * `assets_mismatch`, or `assets_mismatch_more` for more than one feature: the core did not store
 * the files of `refused`. The features that read them miss their geometry (3.19 openFile step 6).
 */
function showAssetsMismatch(app: App, fileName: string, refused: readonly AssetUse[]): void {
  const state = app.store.getState()
  const timeline = (state.document?.recipe.features ?? []).map((feature) => feature.id)
  const features = [...new Set(refused.flatMap((use) => use.features))].toSorted(
    (first, second) => timeline.indexOf(first) - timeline.indexOf(second),
  )
  const [first = ''] = features
  const label = selectFeatureLabel(state, first)
  const notification =
    features.length > 1
      ? {
          message: MESSAGES.assets_mismatch_more(label, features.length - 1, fileName),
          code: 'assets_mismatch_more',
        }
      : { message: MESSAGES.assets_mismatch(label, fileName), code: 'assets_mismatch' }
  notifyWithDetails(
    app,
    { severity: 'warning', ...notification, actions: [] },
    refused.map((use) => `assets/${use.name}: its contents do not match its name`).join('\n'),
  )
}

/** `save_assets`: the assets could not be written, so the document was not saved (3.19 writing step 3). */
export function showSaveAssetsError(
  app: App,
  fileName: string,
  failure: FileServiceError,
  saveAs: () => Promise<void>,
): void {
  notifyWithDetails(
    app,
    {
      severity: 'error',
      message: MESSAGES.save_assets(fileName),
      code: 'save_assets',
      actions: [
        {
          label: 'Save as',
          run: () => {
            void saveAs()
          },
        },
      ],
    },
    failureDetail(failure),
  )
}

/** The features of a document that miss their asset (`asset_missing`), in timeline order. */
function featuresMissingAssets(document: DocumentState): readonly string[] {
  return Object.values(document.featureStatus)
    .filter((status) => status.messages.some((message) => message.code === 'asset_missing'))
    .map((status) => status.id)
}

/**
 * Says why the imports of a document just opened are missing (openFile step 6): the files the core
 * did not store, a failed read, and, when the folder was not reached, one warning
 * naming the features that miss their asset. A host that reached the folder adds nothing more,
 * since each import's own `asset_missing` message says what to copy. `openWithPicker` is the
 * "Open..." action of `assets_without_folder`.
 */
export function explainMissingAssets(
  app: App,
  document: DocumentState,
  { reading, added }: FolderAssets,
  openWithPicker: () => void,
): void {
  const fileName = fileNameOf(document)
  if (added.refused.length > 0) {
    showAssetsMismatch(app, fileName, added.refused)
  }
  if (reachedFolder(reading)) {
    if (reading.failure !== null) {
      showAssetsNotRead(app, fileName, reading.failure)
    }
    return
  }
  const features = featuresMissingAssets(document)
  if (features.length === 0) {
    return
  }
  if (fileHandleOf(document) !== null) {
    showFolderNeeded(app, offerOf(document), fileName, features)
    return
  }
  notify(app, {
    severity: 'warning',
    features,
    ...(app.files.fileSystemAccess
      ? {
          message: MESSAGES.assets_without_folder(fileName),
          code: 'assets_without_folder',
          actions: [{ label: 'Open...', run: openWithPicker }],
        }
      : {
          message: MESSAGES.assets_unsupported(fileName),
          code: 'assets_unsupported',
          actions: [],
        }),
  })
}

/** `assets_folder_needed`: the persistent warning whose "Choose folder" gives the web the document's folder (3.19). */
function showFolderNeeded(
  app: App,
  offer: DocumentOffer,
  fileName: string,
  features: readonly string[],
): void {
  notify(
    app,
    {
      severity: 'warning',
      message: MESSAGES.assets_folder_needed(fileName),
      code: 'assets_folder_needed',
      features,
      actions: [chooseFolderAction(app, offer, fileName, features)],
    },
    { persistent: true },
  )
}

/** The "Choose folder" that adds the folder's files to the document. It edits nothing and runs during a feature command (3.19). */
function chooseFolderAction(
  app: App,
  offer: DocumentOffer,
  fileName: string,
  features: readonly string[],
): NotificationAction {
  return {
    label: 'Choose folder',
    run: () => {
      void chooseDocumentFolder(app, offer, fileName, features)
    },
    editsDocument: false,
  }
}

/**
 * Asks for the folder that holds `handle` (3.19 Choose folder) and resolves it. Another folder
 * shows `assets_wrong_folder`, whose action is `again`, and a canceled dialog runs `canceled`,
 * since running an action dismissed the notification it came from; both resolve null.
 */
export async function chooseFolderOf(
  app: App,
  handle: FileHandle,
  fileName: string,
  again: NotificationAction,
  canceled: () => void,
): Promise<FileSystemDirectoryHandle | null> {
  let choice: Awaited<ReturnType<App['files']['chooseFolder']>>
  try {
    choice = await app.files.chooseFolder(handle)
  } catch (error) {
    if (!(error instanceof FileServiceError)) {
      throw error
    }
    showFileError(app, error)
    return null
  }
  if (choice.kind === 'chosen') {
    return choice.folder
  }
  if (choice.kind === 'canceled') {
    canceled()
    return null
  }
  notify(app, {
    severity: 'error',
    message: MESSAGES.assets_wrong_folder(choice.name, fileName),
    code: 'assets_wrong_folder',
    actions: [again],
  })
  return null
}

/**
 * "Choose folder" of `assets_folder_needed` (3.19): keeps the chosen folder in the document's file
 * reference and recent files, then adds the files it holds for the document's missing assets.
 * Nothing happens unless the document is the one it was offered for. It edits nothing. A feature
 * command, a transaction or a sketch session stays open.
 */
async function chooseDocumentFolder(
  app: App,
  offer: DocumentOffer,
  fileName: string,
  features: readonly string[],
): Promise<void> {
  const { handle } = offer
  if (handle === null || !isOffered(app, offer)) {
    return
  }
  const again = chooseFolderAction(app, offer, fileName, features)
  const folder = await chooseFolderOf(app, handle, fileName, again, () => {
    showFolderNeeded(app, offer, fileName, features)
  })
  const document = documentOf(app)
  if (folder === null || document === null || document.file === null || !isOffered(app, offer)) {
    return
  }
  const file = { ...document.file, folder }
  app.store.setState((state) =>
    state.document === null ? {} : { document: { ...state.document, file } },
  )
  const text = JSON.stringify(settledRecipeOf(app, document), null, 2)
  await app.recent.remember({ name: file.name, handle, folder, text })
  await addChosenFolderAssets(
    app,
    offer,
    { handle, folder },
    document.missingAssets,
    fileName,
    again,
  )
}

/**
 * Reads the names of `missing` from a chosen folder and adds what it read to the document of
 * `offer` with one `addAssets` (3.19 Choose folder). A stopped core shows `assets_stopped`, whose
 * action is `again`.
 */
async function addChosenFolderAssets(
  app: App,
  offer: DocumentOffer,
  place: DocumentPlace,
  missing: readonly AssetUse[],
  fileName: string,
  again: NotificationAction,
): Promise<void> {
  const names = namesToRead(missing)
  if (names.length === 0) {
    return
  }
  const reading = await app.files.readAssets(place, names)
  if (reading.failure !== null) {
    showAssetsNotRead(app, fileName, reading.failure)
  }
  if (reading.assets.length === 0) {
    return
  }
  try {
    // Its turn comes after every edit queued before it, and it goes to the document then shown.
    const added = await app.mutations.run(async () => {
      const shown = sessionDocument(app, offer.recoveryId)
      return shown === null || !isOffered(app, offer)
        ? null
        : app.core.addAssets(shown.documentHandle, namedAssets(reading.assets))
    }, 'other')
    if (added === null) {
      return
    }
    const { stored, refused } = sortAdded(reading.assets, added, missing)
    keepAssets(app, offer, stored)
    if (refused.length > 0) {
      showAssetsMismatch(app, fileName, refused)
    }
  } catch (error) {
    showCoreError(app, error, {
      stopped: {
        code: 'assets_stopped',
        message: MESSAGES.assets_stopped(fileName),
        actions: [again],
      },
    })
  }
}

/**
 * The shown document while it is the session `recoveryId` and no New or Open has ended that
 * session; null otherwise. A New or an Open ends the session before it closes the document, and
 * the document stays shown until the new one is open (ADR 0064 choice 13).
 */
function sessionDocument(app: App, recoveryId: string): DocumentState | null {
  const shown = documentOf(app)
  return shown !== null && shown.recoveryId === recoveryId && sessionGoesOn(app, recoveryId)
    ? shown
    : null
}

/** Adds the files the core stored to the session's assets, each name once, while the shown document is the session they were added for (3.19). */
function keepAssets(
  app: App,
  offer: Pick<DocumentOffer, 'recoveryId'>,
  files: readonly AssetFile[],
): void {
  app.store.setState((state) => {
    const shown = state.document
    if (shown === null || shown.recoveryId !== offer.recoveryId) {
      return {}
    }
    const kept = new Set(shown.assets.map((asset) => asset.name))
    const added = files.filter((file) => !kept.has(file.name))
    return added.length === 0 ? {} : { document: { ...shown, assets: [...shown.assets, ...added] } }
  })
}

/** Whether a picked file of the asset kind `assetKind` is a file of the file kind `kind` (3.19 picked files step 1). */
function isOfKind(assetKind: AssetKind | null, kind: 'step' | 'font'): boolean {
  return kind === 'step' ? assetKind === 'step' : assetKind === 'ttf' || assetKind === 'otf'
}

/**
 * Adds a file the user picked to the shown document's asset store (3.19 picked files, ADR 0064
 * decision 5) and resolves with the asset name the core gave it, or null when it added nothing.
 * A record then names the asset in an edit: wave 2's Import and wave 3's sketch text call it.
 */
export async function addPickedAsset(
  app: App,
  file: OpenedFile,
  kind: 'step' | 'font',
): Promise<string | null> {
  const assetKind = assetKindOf(file.name)
  const document = documentOf(app)
  if (assetKind === null || !isOfKind(assetKind, kind)) {
    notify(app, {
      severity: 'error',
      ...(kind === 'step'
        ? { message: MESSAGES.asset_not_step(file.name), code: 'asset_not_step' }
        : { message: MESSAGES.asset_not_font(file.name), code: 'asset_not_font' }),
      actions: [],
    })
    return null
  }
  if (document === null) {
    return null
  }
  const { recoveryId } = document
  try {
    const added = await app.mutations.run(async () => {
      const shown = sessionDocument(app, recoveryId)
      return shown === null
        ? null
        : app.core.addAssets(shown.documentHandle, [
            { kind: assetKind, bytes: file.bytes, name: null },
          ])
    }, 'other')
    const [entry] = added ?? []
    if (entry === undefined) {
      return null
    }
    keepAssets(app, { recoveryId }, [{ name: entry.name, bytes: file.bytes }])
    return entry.name
  } catch (error) {
    if (error instanceof CoreError) {
      notify(app, {
        severity: 'error',
        message: MESSAGES.asset_add_failed(file.name, error.message),
        code: 'asset_add_failed',
        actions: [],
      })
      return null
    }
    showCoreError(app, error, {
      stopped: { code: 'asset_add_stopped', message: MESSAGES.asset_add_stopped(file.name) },
    })
    return null
  }
}
