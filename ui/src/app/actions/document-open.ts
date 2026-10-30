import type { CoreApi } from '../../core/api'
import { CoreError } from '../../core/client'
import { errorMessage } from '../../core/errors'
import type { AssetFile } from '../../files/assets'
import { fileDigestOrNull } from '../../files/digest'
import { FileServiceError, type OpenedFile } from '../../files/file-service'
import type { RecentFile } from '../../files/recent'
import type { RecoveryEntry } from '../../files/recovery'
import { MESSAGES } from '../../messages'
import { TIMING_NAMES } from '../../performance/marks'
import { adoptDocument } from '../../state/core-events'
import type { AppState, DocumentState, FileReference } from '../../state/store'
import { initialSessionVisibility } from '../../state/visibility'
import { DEFAULT_CAMERA } from '../../viewport/camera'
import { startTransport } from '../connection'
import { loadCoreData, markCoreFailed, markCoreReady, openUntitled } from '../core-data'
import type { App } from '../createApp'
import { adoptRecoveryEntry, endRecoverySession } from '../recovery-copy'
import { measureToNextFrame } from '../timing'
import { cancelCommand } from './command'
import { explainMissingAssets, folderOfFile, readDocumentAssets } from './document-assets'
import { documentOf, showCoreError, type StoppedNotice } from './edits'
import { notify } from './notifications'
import { leaveSketchHistory } from './sketch'

// Starting the core and opening documents (ui.md 3.1, 3.19).

const PTSM_EXTENSION = /\.ptsm$/i

/** The document name of a file name: without the `.ptsm` extension, in any case. */
export function documentName(fileName: string): string {
  return fileName.replace(PTSM_EXTENSION, '')
}

export function textBytes(text: string): Uint8Array {
  return Uint8Array.from(new TextEncoder().encode(text))
}

function recoveryTime(savedAt: number): string {
  return new Date(savedAt).toLocaleString('en-US', { dateStyle: 'medium', timeStyle: 'short' })
}

/** Clears what belonged to the previous document and returns to the Model context. */
function startDocumentSession(app: App): void {
  leaveSketchHistory(app)
  app.store.setState((state): Partial<AppState> => {
    const recipe = state.document?.recipe
    return {
      context: 'model',
      selection: [],
      hover: null,
      command: null,
      sketch: null,
      // The print orientation is session state of the document it was picked in (3.17).
      print: {
        ...state.print,
        report: null,
        reportRevision: null,
        reportOrientation: null,
        focusedFinding: null,
        orientation: null,
        pickingFace: false,
      },
      view: {
        ...state.view,
        sessionVisible: initialSessionVisibility(recipe ?? null),
        locked: [],
      },
    }
  })
}

/**
 * Cancels an open feature command before its document is replaced (3.7): its preview is not part of
 * the document the stash keeps. Explicit, so no Restore is offered for another document.
 */
async function cancelCommandOfReplaced(app: App): Promise<void> {
  if (app.store.getState().command !== null) {
    await cancelCommand(app)
  }
}

/**
 * Closes the document the new one replaces and ends its recovery session: an unsaved one is first
 * kept as a `replaced` recovery entry, and the returned function then offers to reopen it (3.19).
 */
async function closeReplaced(app: App, replaced: DocumentState | null): Promise<() => void> {
  if (replaced === null) {
    return () => undefined
  }
  const stashed = await endRecoverySession(app, replaced)
  try {
    await app.mutations.run(() => app.core.closeDocument(replaced.documentHandle), 'other')
  } catch (error) {
    // A document the core no longer has is closed already.
    if (!(error instanceof CoreError)) {
      throw error
    }
  }
  return () => {
    if (stashed === null) {
      return
    }
    notify(app, {
      severity: 'info',
      message: MESSAGES.stashed(stashed.name),
      code: 'stashed',
      actions: [
        {
          label: 'Reopen',
          run: () => {
            void openRecovered(app, stashed)
          },
        },
      ],
    })
  }
}

type DocumentSource = {
  readonly text: string
  readonly name: string
  readonly file: FileReference | null
  readonly localUnsaved: boolean
  /** The file name used in messages. */
  readonly fileName: string
  /** The warning shown when the core stops before the document is open (4.4). */
  readonly stopped: StoppedNotice
}

/** What the core opened for a text: the document, the assets it accepted, and its refusal of the assets it was given first, if any. */
type OpenedText = {
  readonly opened: Awaited<ReturnType<CoreApi['openDocument']>>
  readonly assets: readonly AssetFile[]
  readonly refusal: CoreError | null
}

/**
 * Has the core open a text with its assets (3.19 openFile step 4). When a file under an asset's
 * name holds other bytes (`asset_mismatch`), the text opens again without assets, so the document
 * can still be edited. Any other refusal shows `open_failed`; resolves null when nothing opened.
 */
async function openText(
  app: App,
  source: DocumentSource,
  assets: readonly AssetFile[],
): Promise<OpenedText | null> {
  try {
    const opened = await app.mutations.run(
      () => app.core.openDocument(source.text, assets),
      'other',
    )
    return { opened, assets, refusal: null }
  } catch (error) {
    if (error instanceof CoreError && error.code === 'asset_mismatch' && assets.length > 0) {
      const retried = await openText(app, source, [])
      return retried === null ? null : { ...retried, refusal: error }
    }
    if (error instanceof CoreError) {
      notify(app, {
        severity: 'error',
        message: MESSAGES.open_failed(source.fileName, error.message),
        code: 'open_failed',
        actions: [],
      })
      return null
    }
    showCoreError(app, error, { stopped: source.stopped })
    return null
  }
}

/**
 * Opens a recipe text in place of the current document (3.19 openFile steps 3 to 6) in a new
 * document session, with the assets its file's folder holds. Resolves the session's recovery id,
 * or null when the text was not opened.
 */
async function replaceDocument(app: App, source: DocumentSource): Promise<string | null> {
  const reading = await readDocumentAssets(app, source.text, source.file)
  const result = await openText(app, source, reading?.assets ?? [])
  if (result === null) {
    return null
  }
  const { opened } = result
  let offerReopen: () => void
  try {
    await cancelCommandOfReplaced(app)
    offerReopen = await closeReplaced(app, documentOf(app))
  } catch (error) {
    // The core stopped before the replaced document was closed, so the opened one is not shown.
    showCoreError(app, error, { stopped: source.stopped })
    return null
  }
  const recoveryId = crypto.randomUUID()
  adoptDocument(app.store, opened.documentHandle, {
    name: source.name,
    file: source.file,
    localUnsaved: source.localUnsaved,
    recoveryId,
    assets: result.assets,
  })
  startDocumentSession(app)
  const viewport = app.viewport()
  viewport?.setStandardView('iso')
  viewport?.fit()
  if (opened.migratedFrom !== null) {
    notify(app, {
      severity: 'info',
      message: MESSAGES.migrated(source.fileName, opened.migratedFrom),
      code: 'migrated',
      actions: [],
    })
  }
  offerReopen()
  const shown = documentOf(app)
  if (shown !== null) {
    explainMissingAssets(app, shown, reading, result.refusal, () => {
      void openWithPicker(app)
    })
  }
  return recoveryId
}

/**
 * Opens a recovery entry as a locally unsaved document named after it, in a new document session
 * that takes the entry over (3.19). An entry the core refuses stays.
 */
async function openRecovered(app: App, entry: RecoveryEntry): Promise<void> {
  const recoveryId = await replaceDocument(app, {
    text: entry.text,
    name: entry.name,
    file:
      entry.handle === null
        ? null
        : {
            name: entry.handle.name,
            handle: entry.handle,
            folder: await folderOfFile(app, entry.handle, entry.folder),
            digest: entry.digest,
          },
    localUnsaved: true,
    fileName: entry.name,
    stopped: {
      code: 'recovery_open_stopped',
      message: MESSAGES.recovery_open_stopped(entry.name),
    },
  })
  if (recoveryId !== null) {
    await adoptRecoveryEntry(app, entry, recoveryId)
  }
}

/**
 * Offers recovery entries one at a time, newest first, each with Open and Discard; once either
 * has finished, the next one is offered. Dismissing an offer leaves the rest for the next startup.
 */
function offerRecoveries(app: App, entries: readonly RecoveryEntry[]): void {
  const [entry, ...waiting] = entries
  if (entry === undefined) {
    return
  }
  const time = recoveryTime(entry.savedAt)
  const offerNext = (): void => {
    offerRecoveries(app, waiting)
  }
  notify(
    app,
    {
      severity: 'info',
      ...(waiting.length === 0
        ? { message: MESSAGES.recovered(entry.name, time), code: 'recovered' }
        : {
            message: MESSAGES.recovered_more(entry.name, time, waiting.length),
            code: 'recovered_more',
          }),
      actions: [
        {
          label: 'Open',
          run: () => {
            void openRecovered(app, entry).then(offerNext)
          },
        },
        {
          label: 'Discard',
          run: () => {
            void app.recovery.remove(entry.id).then(offerNext)
          },
        },
      ],
    },
    { persistent: true },
  )
}

/** Starts the core, loads the catalog and printers, offers recovery and opens "Untitled" (3.1). */
export async function startCore(app: App): Promise<void> {
  try {
    const ready = await startTransport(app)
    await loadCoreData(app)
    markCoreReady(app.store, ready.engineVersion)
  } catch (error) {
    markCoreFailed(app.store, errorMessage(error))
    return
  }
  // The entries of sessions that no running page holds; none is opened without being asked.
  offerRecoveries(app, await app.recovery.list())
  try {
    await openUntitled(app)
    startDocumentSession(app)
  } catch (error) {
    showCoreError(app, error)
  }
}

/** File > New (3.19). */
export async function newDocument(app: App): Promise<void> {
  try {
    await cancelCommandOfReplaced(app)
    const offerReopen = await closeReplaced(app, documentOf(app))
    await openUntitled(app)
    startDocumentSession(app)
    app.viewport()?.setCamera(DEFAULT_CAMERA)
    offerReopen()
  } catch (error) {
    showCoreError(app, error, {
      stopped: { code: 'new_stopped', message: MESSAGES.new_stopped() },
    })
  }
}

/**
 * Opens a chosen or dropped file (3.19), with the web folder that holds it when one is known
 * (given, or kept in the file's recent entry), through which its assets are read.
 */
export async function openFile(
  app: App,
  file: OpenedFile,
  folder: FileSystemDirectoryHandle | null = null,
): Promise<void> {
  // ps:open runs from the read file bytes to the first frame with the opened document (2.18).
  const started = performance.now()
  if (!PTSM_EXTENSION.test(file.name)) {
    notify(app, {
      severity: 'error',
      message: MESSAGES.not_ptsm(file.name),
      actions: [],
      code: 'not_ptsm',
    })
    return
  }
  let text: string
  try {
    text = new TextDecoder('utf-8', { fatal: true }).decode(file.bytes)
  } catch {
    notify(app, {
      severity: 'error',
      message: MESSAGES.not_text(file.name),
      actions: [],
      code: 'not_text',
    })
    return
  }
  const knownFolder = await folderOfFile(app, file.handle, folder)
  const opened = await replaceDocument(app, {
    text,
    name: documentName(file.name),
    file: {
      name: file.name,
      handle: file.handle,
      folder: knownFolder,
      digest: file.handle === null ? null : await fileDigestOrNull(file.bytes, file.name),
    },
    localUnsaved: false,
    fileName: file.name,
    stopped: { code: 'open_stopped', message: MESSAGES.open_stopped(file.name) },
  })
  if (opened !== null) {
    measureToNextFrame(TIMING_NAMES.open, started)
    await app.recent.remember({ name: file.name, handle: file.handle, folder: knownFolder, text })
  }
}

/**
 * Open... with the platform's file picker; a canceled picker does nothing. A picker that fails (the
 * desktop host could not read the chosen file, desktop.md 3.7) shows its message as an error (3.19).
 */
export async function openWithPicker(app: App): Promise<void> {
  let file: OpenedFile | null
  try {
    file = await app.files.pickToOpen()
  } catch (error) {
    if (!(error instanceof FileServiceError)) {
      throw error
    }
    notify(app, { severity: 'error', message: error.message, actions: [] })
    return
  }
  if (file !== null) {
    await openFile(app, file)
  }
}

/** Opens a recent file: from its handle (which may ask for permission) or from its stored text. */
export async function openRecent(app: App, entry: RecentFile): Promise<void> {
  if (entry.handle === null) {
    await openFile(app, { name: entry.name, bytes: textBytes(entry.text ?? ''), handle: null })
    return
  }
  let file: OpenedFile
  try {
    // With the entry's folder, the one prompt this click may show is the folder's (2.15).
    file = await app.files.readHandle(entry.handle, 'read', entry.folder)
  } catch (error) {
    if (!(error instanceof FileServiceError)) {
      throw error
    }
    if (error.reason === 'permission') {
      notify(app, {
        severity: 'error',
        message: MESSAGES.recent_permission(entry.name),
        code: 'recent_permission',
        actions: [
          {
            label: 'Open...',
            run: () => {
              void openWithPicker(app)
            },
          },
        ],
      })
    } else if (error.reason === 'not_found') {
      notify(app, {
        severity: 'warning',
        message: MESSAGES.recent_missing(entry.name),
        code: 'recent_missing',
        actions: [],
      })
      await app.recent.forget(entry.id)
    } else {
      notify(app, {
        severity: 'error',
        message: MESSAGES.open_failed(entry.name, error.message),
        code: 'open_failed',
        actions: [],
      })
    }
    return
  }
  await openFile(app, file, entry.folder)
}
