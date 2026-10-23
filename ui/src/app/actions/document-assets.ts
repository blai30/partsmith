import { CoreError } from '../../core/client'
import { referencedAssetNames, type AssetFile } from '../../files/assets'
import {
  ASSETS_FOLDER,
  FileServiceError,
  folderHolds,
  type AssetReading,
  type DocumentPlace,
  type FileHandle,
} from '../../files/file-service'
import { isSameHandle } from '../../files/recent'
import type { DocHandle } from '../../generated/protocol'
import { MESSAGES } from '../../messages'
import { adoptDocument } from '../../state/core-events'
import { isUnsaved } from '../../state/selectors'
import type { AppState, DocumentState, FileReference, Notification } from '../../state/store'
import { settledRecipeOf } from '../connection'
import type { App } from '../createApp'
import { documentOf, refusedDuringCommand, showCoreError } from './edits'
import { copyDetails } from './fixes'
import { notify } from './notifications'
import { leaveSketchHistory } from './sketch'

// A document's assets folder in the UI (ui.md 3.19, ADR 0037): which assets Open reads and why
// imports are missing, the folder the web asks for ("Choose folder"), which reopens the document
// with its assets, and the actions offered for one document only.

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
  return { recoveryId: document.recoveryId, handle: document.file?.handle ?? null }
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
    (document.file?.handle ?? null) === offer.handle
  )
}

/** The place of a file reference with a handle; null without one. */
function placeOf(file: FileReference | null): DocumentPlace | null {
  return file?.handle === null || file?.handle === undefined
    ? null
    : { handle: file.handle, folder: file.folder }
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

/**
 * Reads the assets a document's text names from the folder next to its file (openFile step 3);
 * null when it names none, has no file, or the host does not reach the folder.
 */
export async function readDocumentAssets(
  app: App,
  text: string,
  file: FileReference | null,
): Promise<AssetReading | null> {
  const names = referencedAssetNames(text)
  const place = placeOf(file)
  if (names.length === 0 || place === null || !app.files.reachesFolder(place)) {
    return null
  }
  return app.files.readAssets(place, names)
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

/** `assets_mismatch`: the core refused the folder's files, so the document opened without them (openFile step 4). */
function showAssetsMismatch(app: App, fileName: string, refusal: CoreError): void {
  notifyWithDetails(
    app,
    {
      severity: 'warning',
      message: MESSAGES.assets_mismatch(fileName),
      code: 'assets_mismatch',
      actions: [],
    },
    refusal.message,
  )
}

/** `save_assets`: the assets could not be written, so the document was not saved (3.19 writing step 3). */
export function showSaveAssetsError(
  app: App,
  fileName: string,
  failure: FileServiceError,
  saveAs: () => void,
): void {
  notifyWithDetails(
    app,
    {
      severity: 'error',
      message: MESSAGES.save_assets(fileName),
      code: 'save_assets',
      actions: [{ label: 'Save as', run: saveAs }],
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
 * Says why the imports of a document just opened are missing (openFile steps 4 and 6): the core's
 * refusal of the folder's files, a failed read, and, when the folder was not reached, one warning
 * naming the features that miss their asset. A host that reached the folder adds nothing more,
 * since each import's own `asset_missing` message says what to copy. `openWithPicker` is the
 * "Open..." action of `assets_without_folder`.
 */
export function explainMissingAssets(
  app: App,
  document: DocumentState,
  reading: AssetReading | null,
  refusal: CoreError | null,
  openWithPicker: () => void,
): void {
  const fileName = fileNameOf(document)
  if (refusal !== null) {
    showAssetsMismatch(app, fileName, refusal)
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
  if (document.file?.handle !== null && document.file?.handle !== undefined) {
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
      actions: [
        {
          label: 'Choose folder',
          run: () => {
            void chooseDocumentFolder(app, offer, fileName, features)
          },
          editsDocument: true,
        },
      ],
    },
    { persistent: true },
  )
}

/**
 * Asks for the folder that holds `handle` (3.19 Choose folder) and resolves it. Another folder
 * shows `assets_wrong_folder`, whose "Choose folder" runs `again`, and a canceled dialog runs
 * `canceled`, since running an action dismissed the notification it came from; both resolve null.
 */
export async function chooseFolderOf(
  app: App,
  handle: FileHandle,
  fileName: string,
  again: () => void,
  canceled: () => void,
): Promise<FileSystemDirectoryHandle | null> {
  let choice: Awaited<ReturnType<App['files']['chooseFolder']>>
  try {
    choice = await app.files.chooseFolder(handle)
  } catch (error) {
    if (!(error instanceof FileServiceError)) {
      throw error
    }
    notify(app, { severity: 'error', message: error.message, actions: [] })
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
    actions: [{ label: 'Choose folder', run: again, editsDocument: true }],
  })
  return null
}

/**
 * "Choose folder" of `assets_folder_needed` (3.19): keeps the chosen folder in the document's file
 * reference and recent files, reads the assets the settled recipe names from it, and reopens the
 * document with them when it read any. Nothing happens unless the document is the one it was
 * offered for, and it is refused during a feature command.
 */
async function chooseDocumentFolder(
  app: App,
  offer: DocumentOffer,
  fileName: string,
  features: readonly string[],
): Promise<void> {
  const { handle } = offer
  if (handle === null || !isOffered(app, offer) || refusedDuringCommand(app)) {
    return
  }
  const offerAgain = (): void => {
    showFolderNeeded(app, offer, fileName, features)
  }
  const folder = await chooseFolderOf(
    app,
    handle,
    fileName,
    () => {
      void chooseDocumentFolder(app, offer, fileName, features)
    },
    offerAgain,
  )
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
  const reading = await app.files.readAssets({ handle, folder }, referencedAssetNames(text))
  if (reading.failure !== null) {
    showAssetsNotRead(app, fileName, reading.failure)
  }
  if (reading.assets.length > 0) {
    await reopenWithAssets(app, offer, fileName, reading.assets, offerAgain)
  }
}

/**
 * Reopens the shown document's settled recipe with assets, as a core restart does but without
 * stopping the core (3.19, 4.4): the session keeps its name, file reference, recovery entry and
 * unsaved state, while sketch mode, the selection and the undo history start again. The recipe is
 * taken when the task's turn in the mutation queue comes, so every edit queued before it is kept.
 * A document that is no longer the one offered changes nothing, a feature command opened meanwhile
 * refuses with `finish_command_first` and runs `refused`, and a refusal of the assets leaves the
 * document as it is.
 */
async function reopenWithAssets(
  app: App,
  offer: DocumentOffer,
  fileName: string,
  assets: readonly AssetFile[],
  refused: () => void,
): Promise<void> {
  try {
    // One task, so no queued edit lands on the old document between the open and the close.
    await app.mutations.run(async () => {
      const document = documentOf(app)
      if (document === null || !isOffered(app, offer)) {
        return
      }
      if (refusedDuringCommand(app) || document.transactionOpen) {
        refused()
        return
      }
      const text = JSON.stringify(settledRecipeOf(app, document), null, 2)
      const opened = await app.core.openDocument(text, assets)
      await app.core.closeDocument(document.documentHandle)
      adoptReopened(app, documentOf(app) ?? document, opened.documentHandle, assets)
    }, 'other')
  } catch (error) {
    if (error instanceof CoreError && error.code === 'asset_mismatch') {
      showAssetsMismatch(app, fileName, error)
      return
    }
    showCoreError(app, error, {
      stopped: { code: 'open_stopped', message: MESSAGES.open_stopped(fileName) },
    })
  }
}

/** Shows the reopened document in place of `shown`, in the same document session (3.19 Choose folder). */
function adoptReopened(
  app: App,
  shown: DocumentState,
  documentHandle: DocHandle,
  assets: readonly AssetFile[],
): void {
  leaveSketchHistory(app)
  app.store.setState((state): Partial<AppState> => ({
    context: state.context === 'sketch' ? 'model' : state.context,
    selection: [],
    hover: null,
    command: null,
    sketch: null,
  }))
  adoptDocument(app.store, documentHandle, {
    name: shown.name,
    file: shown.file,
    localUnsaved: isUnsaved(shown),
    recoveryId: shown.recoveryId,
    assets,
  })
}
