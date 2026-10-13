import type { CoreApi } from '../../core/api'
import { CoreError } from '../../core/client'
import { errorMessage } from '../../core/errors'
import { FileServiceError, type OpenedFile, type SaveTarget } from '../../files/file-service'
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
import { cancelCommand, redoCommandChange, undoCommandChange } from './command'
import {
  commitEdits,
  documentOf,
  mutate,
  refusedDuringCommand,
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

/**
 * Opens a recipe text in place of the current document (3.19 openFile step 3 and 4) in a new
 * document session. Resolves the session's recovery id, or null when the text was not opened.
 */
async function replaceDocument(app: App, source: DocumentSource): Promise<string | null> {
  let opened: Awaited<ReturnType<CoreApi['openDocument']>>
  try {
    opened = await app.mutations.run(() => app.core.openDocument(source.text), 'other')
  } catch (error) {
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
    file: entry.handle === null ? null : { name: entry.handle.name, handle: entry.handle },
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

/** Opens a chosen or dropped file (3.19). */
export async function openFile(app: App, file: OpenedFile): Promise<void> {
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
  const opened = await replaceDocument(app, {
    text,
    name: documentName(file.name),
    file: { name: file.name, handle: file.handle },
    localUnsaved: false,
    fileName: file.name,
    stopped: { code: 'open_stopped', message: MESSAGES.open_stopped(file.name) },
  })
  if (opened !== null) {
    measureToNextFrame(TIMING_NAMES.open, started)
    await app.recent.remember({ name: file.name, handle: file.handle, text })
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
    file = await app.files.readHandle(entry.handle)
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
  await openFile(app, file)
}

/**
 * Saves the document's canonical text to the target, then tells the core which revision is on
 * disk. Nothing is marked saved when the write fails.
 */
async function writeDocument(app: App, target: SaveTarget): Promise<boolean> {
  const document = documentOf(app)
  if (document === null) {
    return false
  }
  const saved = await mutate(app, (current) => app.core.saveDocument(current.documentHandle), {
    kind: 'other',
    stopped: { code: 'save_stopped', message: MESSAGES.save_stopped(target.name) },
  })
  if (saved === null) {
    return false
  }
  try {
    await app.files.write(target, 'ptsm', textBytes(saved.text))
  } catch (error) {
    if (error instanceof FileServiceError) {
      showSaveError(app, target.name, error, () => saveAs(app))
      return false
    }
    throw error
  }
  // A core that stops now leaves the written document unsaved, which keeps its recovery copy.
  const marked = await mutate(
    app,
    (current) => app.core.markSaved(current.documentHandle, saved.revision),
    { kind: 'other' },
  )
  if (marked === null) {
    return false
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
  return true
}

/** Save (Mod+S): to the document's file, or Save as without one; refused during a feature command (3.7). */
export async function save(app: App): Promise<void> {
  const document = documentOf(app)
  if (document === null || refusedDuringCommand(app)) {
    return
  }
  const file = document.file
  if (file?.handle === null || file?.handle === undefined) {
    await saveAs(app)
    return
  }
  await writeDocument(app, { kind: 'handle', name: file.name, handle: file.handle })
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
  if (target === null || !(await writeDocument(app, target))) {
    return
  }
  if (target.kind === 'handle') {
    app.store.setState((state) =>
      state.document === null
        ? {}
        : {
            document: {
              ...state.document,
              name: documentName(target.name),
              file: { name: target.name, handle: target.handle },
            },
          },
    )
    const current = documentOf(app)
    if (current !== null) {
      await app.recent.remember({
        name: target.name,
        handle: target.handle,
        text: JSON.stringify(current.recipe, null, 2),
      })
    }
    return
  }
  app.store.setState((state) =>
    state.document === null
      ? {}
      : { document: { ...state.document, file: { name: target.name, handle: null } } },
  )
  if (!downloadHintShown.has(app.store)) {
    downloadHintShown.add(app.store)
    notify(app, {
      severity: 'info',
      message: MESSAGES.download_hint(target.name),
      actions: [],
      code: 'download_hint',
    })
  }
}

/** Applies labeled edits as one undo entry; resolves false when the core refused them. */
export function applyEdits(app: App, label: string, edits: readonly Edit[]): Promise<boolean> {
  return commitEdits(app, label, edits)
}

/**
 * Undo (Mod+Z, 3.20): in a feature command it undoes the command's last change, or cancels the
 * command when there is none; in sketch mode views undo first.
 */
export async function undo(app: App): Promise<void> {
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

/** Redo (Mod+Shift+Z, 3.20); in a feature command it redoes the command's last undone change. */
export async function redo(app: App): Promise<void> {
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

/** Changes the document units and explains what that means for typed numbers (3.21); refused during a feature command (3.7). */
export async function setUnits(app: App, units: Units): Promise<void> {
  if (refusedDuringCommand(app)) {
    return
  }
  if (await commitEdits(app, `Change units to ${units}`, [{ type: 'set_units', units }])) {
    notify(app, {
      severity: 'info',
      message: MESSAGES.units_changed(units),
      actions: [],
      code: 'units_changed',
    })
  }
}
