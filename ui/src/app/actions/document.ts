import type { CoreApi } from '../../core/api'
import { CoreError } from '../../core/client'
import { errorMessage } from '../../core/errors'
import { createOneAtATime } from '../../core/queue'
import type { AssetFile } from '../../files/assets'
import { fileDigestOrNull } from '../../files/digest'
import {
  FileServiceError,
  folderHolds,
  type FileHandle,
  type OpenedFile,
  type SaveTarget,
} from '../../files/file-service'
import type { RecentFile } from '../../files/recent'
import type { RecoveryEntry } from '../../files/recovery'
import type { Edit, Units } from '../../generated/protocol'
import { MESSAGES } from '../../messages'
import { TIMING_NAMES } from '../../performance/marks'
import { adoptDocument } from '../../state/core-events'
import { isUnsaved } from '../../state/selectors'
import type { AppState, DocumentState, FileReference } from '../../state/store'
import { initialSessionVisibility } from '../../state/visibility'
import { DEFAULT_CAMERA } from '../../viewport/camera'
import { startTransport } from '../connection'
import { loadCoreData, markCoreFailed, markCoreReady, openUntitled } from '../core-data'
import type { App } from '../createApp'
import { adoptRecoveryEntry, endRecoverySession, removeRecoveryEntry } from '../recovery-copy'
import { measureToNextFrame } from '../timing'
import {
  cancelCommand,
  confirmRunning,
  confirmSettled,
  redoCommandChange,
  undoCommandChange,
} from './command'
import {
  chooseFolderOf,
  explainMissingAssets,
  folderOfFile,
  isOffered,
  offerOf,
  readDocumentAssets,
  showSaveAssetsError,
  type DocumentOffer,
} from './document-assets'
import {
  commitEdits,
  documentOf,
  mutate,
  refusedDuringCommand,
  runChange,
  showCoreError,
  type StoppedNotice,
} from './edits'
import { showSaveError } from './files'
import { announce, notify } from './notifications'
import { leaveSketchHistory, redoInSketch, undoInSketch } from './sketch'

// Starting the core, document files and undo (ui.md 3.1, 3.19, 3.20).

const PTSM_EXTENSION = /\.ptsm$/i

/** Stores that already showed the download hint this session (3.19). */
const downloadHintShown = new WeakSet<App['store']>()

/** The document name of a file name: without the `.ptsm` extension, in any case. */
export function documentName(fileName: string): string {
  return fileName.replace(PTSM_EXTENSION, '')
}

function textBytes(text: string): Uint8Array {
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

/** One write of the document (3.19 "Writing the document"). */
type DocumentWrite = {
  readonly target: SaveTarget
  /** The web folder that holds the target, through which its assets are written; null on the desktop and without one. */
  readonly folder: FileSystemDirectoryHandle | null
  /** Whether Save's change check runs: true for Save, false for Save as, whose dialog confirmed replacing the file. */
  readonly checksChange: boolean
  /** The document session and file the save was started for, which its notifications' actions require. */
  readonly offer: DocumentOffer
}

/** Writes of the document, per store: each one waits for the one before it (3.19). */
const writesOneAtATime = createOneAtATime<App['store']>()

/**
 * Runs one write of the document after the write still in progress, if any, so a second Save never
 * checks the file against the digest the first one is about to replace. `write` gives the write
 * when its turn comes, from the document as it is then; null writes nothing. With nothing in
 * progress it starts at once.
 */
function writeInTurn(app: App, write: () => DocumentWrite | null): Promise<void> {
  return writesOneAtATime(app.store, async () => {
    const next = write()
    if (next !== null) {
      await writeDocument(app, next)
    }
  })
}

/** The digest the shown document's file reference knows for `handle`: null for another handle or an unknown digest. */
function knownDigest(app: App, handle: FileHandle): string | null {
  const file = documentOf(app)?.file
  return file?.handle === handle ? file.digest : null
}

/**
 * Save's change check (3.19 writing step 2): whether the target still holds the bytes the
 * session last read or wrote, as the file reference knows them when the check runs, true when
 * there is nothing to check. A file changed elsewhere shows `save_conflict`, and a file that
 * cannot be read the Save message of its reason; a digest that cannot be taken skips the check.
 */
async function unchangedElsewhere(app: App, write: DocumentWrite): Promise<boolean> {
  const { target } = write
  if (!write.checksChange || target.kind !== 'handle') {
    return true
  }
  const expected = knownDigest(app, target.handle)
  if (expected === null) {
    return true
  }
  const fileName = target.name
  let file: OpenedFile
  try {
    // With the document's folder, the one prompt of this click is the folder's (2.15).
    file = await app.files.readHandle(target.handle, 'readwrite', write.folder)
  } catch (error) {
    if (!(error instanceof FileServiceError)) {
      throw error
    }
    showSaveError(app, fileName, error, () => saveAs(app))
    return false
  }
  const digest = await fileDigestOrNull(file.bytes, fileName)
  if (digest === null || digest === expected) {
    return true
  }
  const { offer } = write
  notify(app, {
    severity: 'error',
    message: MESSAGES.save_conflict(fileName),
    code: 'save_conflict',
    actions: [
      {
        label: `Overwrite ${fileName}`,
        run: () => {
          void overwrite(app, offer, digest)
        },
        editsDocument: true,
      },
      {
        label: 'Save as',
        run: () => {
          if (isOffered(app, offer)) {
            void saveAs(app)
          }
        },
      },
      {
        label: 'Open the newer file',
        run: () => {
          void openNewerFile(app, offer)
        },
      },
    ],
  })
  return false
}

/** "Overwrite {file}" of `save_conflict`: takes the newer file's digest as known and saves again, so a file changed once more warns again. */
async function overwrite(app: App, offer: DocumentOffer, digest: string): Promise<void> {
  if (!isOffered(app, offer) || refusedDuringCommand(app)) {
    return
  }
  app.store.setState((state) =>
    state.document?.file === null || state.document?.file === undefined
      ? {}
      : { document: { ...state.document, file: { ...state.document.file, digest } } },
  )
  await save(app)
}

/** "Open the newer file" of `save_conflict`: opens the file again, which keeps the unsaved document as a `replaced` entry (3.19). */
async function openNewerFile(app: App, offer: DocumentOffer): Promise<void> {
  const file = documentOf(app)?.file ?? null
  if (file?.handle === null || file?.handle === undefined || !isOffered(app, offer)) {
    return
  }
  let opened: OpenedFile
  try {
    opened = await app.files.readHandle(file.handle, 'read', file.folder)
  } catch (error) {
    if (!(error instanceof FileServiceError)) {
      throw error
    }
    notify(app, {
      severity: 'error',
      message: MESSAGES.open_failed(file.name, error.message),
      code: 'open_failed',
      actions: [],
    })
    return
  }
  await openFile(app, opened, file.folder)
}

/**
 * The assets step of a write (3.19 writing step 3): writes the assets to the folder next to the
 * target, or, when the page does not reach it (on the web, Save as into another folder), asks for
 * that folder with `save_folder_needed`. Resolves whether the write goes on.
 */
async function wroteAssets(
  app: App,
  write: DocumentWrite,
  assets: readonly AssetFile[],
): Promise<boolean> {
  const { target } = write
  // A download target holds no assets: a page without the File System Access API reads none.
  if (assets.length === 0 || target.kind !== 'handle') {
    return true
  }
  const place = { handle: target.handle, folder: write.folder }
  if (!app.files.reachesFolder(place)) {
    askForSaveFolder(app, write)
    return false
  }
  try {
    await app.files.writeAssets(place, assets)
  } catch (error) {
    if (!(error instanceof FileServiceError)) {
      throw error
    }
    showSaveAssetsError(app, target.name, error, () => {
      void saveAs(app)
    })
    return false
  }
  return true
}

/** `save_folder_needed`: the persistent warning whose "Choose folder" writes the document again with the target's folder (3.19). */
function askForSaveFolder(app: App, write: DocumentWrite): void {
  notify(
    app,
    {
      severity: 'warning',
      message: MESSAGES.save_folder_needed(write.target.name),
      code: 'save_folder_needed',
      actions: [
        {
          label: 'Choose folder',
          run: () => {
            void chooseSaveFolder(app, write)
          },
          editsDocument: true,
        },
      ],
    },
    { persistent: true },
  )
}

/** "Choose folder" of `save_folder_needed`, for the document and target it was offered for. */
async function chooseSaveFolder(app: App, write: DocumentWrite): Promise<void> {
  const { target } = write
  if (target.kind !== 'handle' || !isOffered(app, write.offer) || refusedDuringCommand(app)) {
    return
  }
  const folder = await chooseFolderOf(
    app,
    target.handle,
    target.name,
    () => {
      void chooseSaveFolder(app, write)
    },
    () => {
      askForSaveFolder(app, write)
    },
  )
  if (folder !== null && isOffered(app, write.offer)) {
    await writeInTurn(app, () => (isOffered(app, write.offer) ? { ...write, folder } : null))
  }
}

/**
 * Takes the written target as the document's file (3.19 writing step 5): its digest becomes that
 * of the written bytes, and a new file (Save as) also names the document and goes to recent files
 * with its folder. A download target leaves the document without a file handle.
 */
async function adoptWrittenFile(
  app: App,
  document: DocumentState,
  write: DocumentWrite,
  digest: string | null,
): Promise<void> {
  const { target } = write
  const file: FileReference =
    target.kind === 'handle'
      ? { name: target.name, handle: target.handle, folder: write.folder, digest }
      : { name: target.name, handle: null, folder: null, digest: null }
  const isNewFile = target.kind === 'handle' && target.handle !== document.file?.handle
  app.store.setState((state) =>
    state.document === null || state.document.recoveryId !== document.recoveryId
      ? {}
      : {
          document: {
            ...state.document,
            file,
            ...(isNewFile ? { name: documentName(target.name) } : {}),
          },
        },
  )
  if (isNewFile) {
    await app.recent.remember({
      name: target.name,
      handle: target.handle,
      folder: write.folder,
      text: JSON.stringify((documentOf(app) ?? document).recipe, null, 2),
    })
  }
  if (target.kind === 'download' && !downloadHintShown.has(app.store)) {
    downloadHintShown.add(app.store)
    notify(app, {
      severity: 'info',
      message: MESSAGES.download_hint(target.name),
      actions: [],
      code: 'download_hint',
    })
  }
}

/**
 * Writes the document in the order of file-format.md 3.11 (3.19 "Writing the document"): the
 * core's text and assets, Save's change check, the assets, the document, then `markSaved`. Each
 * step that stops sends no `markSaved`, so the document stays unsaved.
 */
async function writeDocument(app: App, write: DocumentWrite): Promise<void> {
  const document = documentOf(app)
  if (document === null) {
    return
  }
  const { target } = write
  const saved = await mutate(app, (current) => app.core.saveDocument(current.documentHandle), {
    kind: 'other',
    stopped: { code: 'save_stopped', message: MESSAGES.save_stopped(target.name) },
  })
  if (saved === null) {
    return
  }
  const bytes = textBytes(saved.text)
  // Taken alongside the change check, for the file reference once the bytes are written (step 5).
  const writtenDigest =
    target.kind === 'handle' ? fileDigestOrNull(bytes, target.name) : Promise.resolve(null)
  if (!(await unchangedElsewhere(app, write)) || !(await wroteAssets(app, write, saved.assets))) {
    return
  }
  try {
    await app.files.write(target, 'ptsm', bytes)
  } catch (error) {
    if (error instanceof FileServiceError) {
      showSaveError(app, target.name, error, () => saveAs(app))
      return
    }
    throw error
  }
  // The file now holds these bytes, even if the core stops before they are marked saved.
  await adoptWrittenFile(app, document, write, await writtenDigest)
  // A core that stops now leaves the written document unsaved, which keeps its recovery copy.
  const marked = await mutate(
    app,
    (current) => app.core.markSaved(current.documentHandle, saved.revision),
    { kind: 'other' },
  )
  if (marked === null) {
    return
  }
  app.store.setState((state) =>
    state.document === null ? {} : { document: { ...state.document, localUnsaved: false } },
  )
  // Only the saved session's entry goes, and only when nothing is left unsaved: an edit made while
  // the file was written keeps it, and a document that replaced it meanwhile keeps its own.
  const current = documentOf(app)
  if (current?.recoveryId === document.recoveryId && !isUnsaved(current)) {
    await removeRecoveryEntry(app, current)
  }
}

/** Whether the shown document is still the session with this recovery id. */
function isSession(app: App, recoveryId: string): boolean {
  return documentOf(app)?.recoveryId === recoveryId
}

/**
 * Save's write to the document's file as it is when the write's turn comes (3.19), so a Save
 * pressed while another save writes goes to the file and folder that one left; none once another
 * document replaced this session.
 */
function saveToFile(app: App, recoveryId: string): DocumentWrite | null {
  const document = documentOf(app)
  const file = document?.file ?? null
  const handle = file?.handle ?? null
  if (document === null || file === null || handle === null || !isSession(app, recoveryId)) {
    return null
  }
  return {
    target: { kind: 'handle', name: file.name, handle },
    folder: file.folder,
    checksChange: true,
    offer: offerOf(document),
  }
}

/** Save (Mod+S): to the document's file, or Save as without one; refused during a feature command (3.7). */
export async function save(app: App): Promise<void> {
  const document = documentOf(app)
  if (document === null || refusedDuringCommand(app)) {
    return
  }
  if ((document.file?.handle ?? null) === null) {
    await saveAs(app)
    return
  }
  const { recoveryId } = document
  await writeInTurn(app, () => saveToFile(app, recoveryId))
}

/**
 * Save as (Mod+Shift+S): asks where first; a canceled picker changes nothing, and a picker that
 * fails shows the Save messages with "Save as" (3.19). Refused during a feature command (3.7).
 */
export async function saveAs(app: App): Promise<void> {
  const document = documentOf(app)
  if (document === null || refusedDuringCommand(app)) {
    return
  }
  const fileName = `${document.name}.ptsm`
  let target: SaveTarget | null
  try {
    target = await app.files.chooseSaveTarget(fileName, 'ptsm')
  } catch (error) {
    if (!(error instanceof FileServiceError)) {
      throw error
    }
    showSaveError(app, fileName, error, () => saveAs(app))
    return
  }
  if (target === null) {
    return
  }
  // The document's folder goes with a target it holds, so the assets are written there at once.
  const folder = document.file?.folder ?? null
  const holds =
    folder !== null && target.kind === 'handle' && (await folderHolds(folder, target.handle))
  const { recoveryId } = document
  // No change check: the dialog confirmed replacing the file.
  await writeInTurn(app, () => {
    const shown = documentOf(app)
    return shown === null || !isSession(app, recoveryId)
      ? null
      : { target, folder: holds ? folder : null, checksChange: false, offer: offerOf(shown) }
  })
}

/** Applies labeled edits as one undo entry; resolves false when the core refused them. */
export function applyEdits(app: App, label: string, edits: readonly Edit[]): Promise<boolean> {
  return commitEdits(app, label, edits)
}

/**
 * Undo (Mod+Z, 3.20): in a feature command it undoes the command's last change, or cancels the
 * command when there is none; in sketch mode views undo first. While OK runs it waits for it, so
 * Mod+Z right after OK undoes the feature it confirmed (3.7).
 */
export async function undo(app: App): Promise<void> {
  if (confirmRunning(app)) {
    await confirmSettled(app)
  }
  const state = app.store.getState()
  if (state.command !== null) {
    await undoCommandChange(app)
    return
  }
  if (await undoInSketch(app)) {
    return
  }
  await coreUndo(app)
}

/**
 * Redo (Mod+Shift+Z, 3.20); in a feature command it redoes the command's last undone change. While
 * OK runs it waits for it and then redoes in the document (3.7).
 */
export async function redo(app: App): Promise<void> {
  if (confirmRunning(app)) {
    await confirmSettled(app)
  }
  if (app.store.getState().command !== null) {
    await redoCommandChange(app)
    return
  }
  if (await redoInSketch(app)) {
    return
  }
  await coreRedo(app)
}

function quietWhenNothingToDo(app: App, error: unknown): void {
  if (
    error instanceof CoreError &&
    (error.code === 'nothing_to_undo' || error.code === 'nothing_to_redo')
  ) {
    return
  }
  showCoreError(app, error)
}

/** The core's undo through the mutation queue, announced as "Undid {label}" (3.20). */
export async function coreUndo(app: App): Promise<void> {
  const document = documentOf(app)
  if (document === null) {
    return
  }
  try {
    const { label } = await app.mutations.run(() => app.core.undo(document.documentHandle))
    announce(app, `Undid ${label}`)
  } catch (error) {
    quietWhenNothingToDo(app, error)
  }
}

/** The core's redo through the mutation queue, announced as "Redid {label}" (3.20). */
export async function coreRedo(app: App): Promise<void> {
  const document = documentOf(app)
  if (document === null) {
    return
  }
  try {
    const { label } = await app.mutations.run(() => app.core.redo(document.documentHandle))
    announce(app, `Redid ${label}`)
  } catch (error) {
    quietWhenNothingToDo(app, error)
  }
}

/**
 * Changes the document units and explains what that means for typed numbers (3.21): one undo entry
 * through `set_document_units`, keeping sizes while Keep sizes is on and numbers lack a unit.
 * Choosing the current units changes nothing; refused during a feature command (3.7).
 */
export async function setUnits(app: App, units: Units): Promise<void> {
  if (refusedDuringCommand(app)) {
    return
  }
  const { sent } = await runChange(app, async (document) => {
    if (document.recipe.units === units) {
      return false
    }
    const keepSizes = app.store.getState().keepSizes && document.bareNumbers > 0
    await app.core.setDocumentUnits(document.documentHandle, units, keepSizes)
    return true
  })
  if (sent) {
    notify(app, {
      severity: 'info',
      message: MESSAGES.units_changed(units),
      actions: [],
      code: 'units_changed',
    })
  }
}

/** Turns Keep sizes of the units control on or off for the rest of the session (3.21). */
export function setKeepSizes(app: App, keepSizes: boolean): void {
  app.store.setState({ keepSizes })
}
