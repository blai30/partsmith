import { CoreError } from '../../core/client'
import { FileServiceError, type OpenedFile, type SaveTarget } from '../../files/file-service'
import type { RecentFile } from '../../files/recent'
import type { RecoveryEntry } from '../../files/recovery'
import type { Document, Edit, Units } from '../../generated/protocol'
import { MESSAGES } from '../../messages'
import { adoptDocument } from '../../state/core-events'
import type { AppState, DocumentState, FileReference } from '../../state/store'
import { DEFAULT_CAMERA } from '../../viewport/camera'
import { loadCoreData } from '../core-data'
import { nowOf, startTransport, type App } from '../createApp'
import { cancelCommand } from './command'
import { commitEdits, documentOf, isUnsaved, mutate, showCoreError } from './edits'
import { notify } from './notifications'
import { leaveSketchHistory, redoInSketch, undoInSketch } from './sketch'

// Starting the core, document files and undo (ui.md 3.1, 3.19, 3.20).

const PTSM_EXTENSION = /\.ptsm$/i
const ORIGIN_KEYS = ['XY', 'XZ', 'YZ', 'X', 'Y', 'Z', 'O'].map((name) => `origin:${name}`)

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

/** Whether some feature of the recipe refers to the sketch (a `sketch` key anywhere in its params). */
function referencesSketch(value: unknown, sketchId: string): boolean {
  if (Array.isArray(value)) {
    return value.some((item) => referencesSketch(item, sketchId))
  }
  if (typeof value !== 'object' || value === null) {
    return false
  }
  return Object.entries(value).some(
    ([key, item]) => (key === 'sketch' && item === sketchId) || referencesSketch(item, sketchId),
  )
}

/** Session visibility for a newly opened recipe (3.10): used sketches and origin items start hidden. */
export function initialSessionVisibility(recipe: Document): Readonly<Record<string, boolean>> {
  const features = recipe.features ?? []
  const visibility: Record<string, boolean> = {}
  for (const feature of features) {
    if (feature.type === 'sketch') {
      visibility[`feature:${feature.id}`] = !features.some(
        (other) => other.id !== feature.id && referencesSketch(other, feature.id),
      )
    } else if (feature.type === 'datum_plane') {
      visibility[`feature:${feature.id}`] = true
    }
  }
  for (const key of ORIGIN_KEYS) {
    visibility[key] = false
  }
  return visibility
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
      print: { ...state.print, report: null, reportRevision: null, focusedFinding: null },
      view: {
        ...state.view,
        sessionVisible: recipe === undefined ? {} : initialSessionVisibility(recipe),
        locked: [],
      },
    }
  })
}

function recoveryEntryFor(app: App, document: DocumentState): RecoveryEntry {
  return {
    name: document.name,
    text: JSON.stringify(document.recipe, null, 2),
    savedAt: nowOf(app),
    handle: document.file?.handle ?? null,
    reason: 'replaced',
  }
}

/**
 * Closes the document the new one replaces: an unsaved one is first kept as a recovery copy, and
 * the returned function then offers to reopen it (3.19). With `clearRecovery` (a New), a clean one
 * clears the recovery store.
 */
async function closeReplaced(
  app: App,
  replaced: DocumentState | null,
  clearRecovery: boolean,
): Promise<() => void> {
  if (replaced === null) {
    return () => undefined
  }
  let stashed: RecoveryEntry | null = null
  if (isUnsaved(replaced)) {
    stashed = recoveryEntryFor(app, replaced)
    await app.recovery.write(stashed)
  } else if (clearRecovery) {
    await app.recovery.clear()
  }
  try {
    await app.mutations.run(() => app.core.closeDocument(replaced.documentHandle))
  } catch (error) {
    // A document the core no longer has is closed already.
    if (!(error instanceof CoreError)) {
      throw error
    }
  }
  const entry = stashed
  return () => {
    if (entry === null) {
      return
    }
    notify(app, {
      severity: 'info',
      message: MESSAGES.stashed(entry.name),
      actions: [
        {
          label: 'Reopen',
          run: () => {
            void openRecovered(app, entry)
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
}

/** Opens a recipe text in place of the current document (3.19 openFile step 3 and 4). */
async function replaceDocument(app: App, source: DocumentSource): Promise<boolean> {
  let opened: { documentHandle: number; migratedFrom: number | null }
  try {
    opened = await app.mutations.run(() => app.core.openDocument(source.text))
  } catch (error) {
    if (error instanceof CoreError) {
      notify(app, {
        severity: 'error',
        message: MESSAGES.open_failed(source.fileName, error.message),
        actions: [],
      })
      return false
    }
    showCoreError(app, error)
    return false
  }
  const offerReopen = await closeReplaced(app, documentOf(app), false)
  adoptDocument(app.store, opened.documentHandle, {
    name: source.name,
    file: source.file,
    localUnsaved: source.localUnsaved,
  })
  startDocumentSession(app)
  const viewport = app.viewport()
  viewport?.setStandardView('iso')
  viewport?.fit()
  if (opened.migratedFrom !== null) {
    notify(app, {
      severity: 'info',
      message: MESSAGES.migrated(source.fileName, opened.migratedFrom),
      actions: [],
    })
  }
  offerReopen()
  return true
}

/** Opens a recovery copy as a locally unsaved document named after it (3.19). */
async function openRecovered(app: App, entry: RecoveryEntry): Promise<void> {
  await replaceDocument(app, {
    text: entry.text,
    name: entry.name,
    file: entry.handle === null ? null : { name: entry.handle.name, handle: entry.handle },
    localUnsaved: true,
    fileName: entry.name,
  })
}

/** Offers the recovery copy kept from an earlier session, without opening it (3.1 step 5). */
async function offerRecovery(app: App): Promise<void> {
  const entry = await app.recovery.read()
  if (entry === null) {
    return
  }
  notify(
    app,
    {
      severity: 'info',
      message: MESSAGES.recovered(entry.name, recoveryTime(entry.savedAt)),
      actions: [
        {
          label: 'Open',
          run: () => {
            void openRecovered(app, entry)
          },
        },
        {
          label: 'Discard',
          run: () => {
            void app.recovery.clear()
          },
        },
      ],
    },
    { persistent: true },
  )
}

/** Opens a new empty mm document named "Untitled". */
async function openUntitled(app: App): Promise<void> {
  const documentHandle = await app.mutations.run(() => app.core.newDocument('mm'))
  adoptDocument(app.store, documentHandle, { name: 'Untitled', file: null, localUnsaved: false })
}

/** Starts the core, loads the catalog and printers, offers recovery and opens "Untitled" (3.1). */
export async function startCore(app: App): Promise<void> {
  try {
    const ready = await startTransport(app)
    await loadCoreData(app)
    app.store.setState((state) => ({
      core: { ...state.core, status: 'ready', engineVersion: ready.engineVersion, fatal: null },
    }))
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error)
    app.store.setState((state) => ({ core: { ...state.core, status: 'failed', fatal: reason } }))
    return
  }
  await offerRecovery(app)
  try {
    await openUntitled(app)
  } catch (error) {
    showCoreError(app, error)
  }
}

/** File > New (3.19). */
export async function newDocument(app: App): Promise<void> {
  try {
    const offerReopen = await closeReplaced(app, documentOf(app), true)
    await openUntitled(app)
    startDocumentSession(app)
    app.viewport()?.setCamera(DEFAULT_CAMERA)
    offerReopen()
  } catch (error) {
    showCoreError(app, error)
  }
}

/** Opens a chosen or dropped file (3.19). */
export async function openFile(app: App, file: OpenedFile): Promise<void> {
  if (!PTSM_EXTENSION.test(file.name)) {
    notify(app, { severity: 'error', message: MESSAGES.not_ptsm(file.name), actions: [] })
    return
  }
  let text: string
  try {
    text = new TextDecoder('utf-8', { fatal: true }).decode(file.bytes)
  } catch {
    notify(app, { severity: 'error', message: MESSAGES.not_text(file.name), actions: [] })
    return
  }
  const opened = await replaceDocument(app, {
    text,
    name: documentName(file.name),
    file: { name: file.name, handle: file.handle },
    localUnsaved: false,
    fileName: file.name,
  })
  if (opened) {
    await app.recent.remember({ name: file.name, handle: file.handle, text })
  }
}

/** Open... with the platform's file picker; a canceled picker does nothing. */
export async function openWithPicker(app: App): Promise<void> {
  const file = await app.files.pickToOpen()
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
        actions: [],
      })
      await app.recent.forget(entry.id)
    } else {
      notify(app, {
        severity: 'error',
        message: MESSAGES.open_failed(entry.name, error.message),
        actions: [],
      })
    }
    return
  }
  await openFile(app, file)
}

function saveAsAction(app: App): { readonly label: string; readonly run: () => void } {
  return {
    label: 'Save as',
    run: () => {
      void saveAs(app)
    },
  }
}

/** The message of a failed write of the document file (3.19). */
function showWriteError(app: App, fileName: string, error: FileServiceError): void {
  const message =
    error.reason === 'permission'
      ? MESSAGES.save_permission(fileName)
      : error.reason === 'not_found'
        ? MESSAGES.save_not_found(fileName)
        : MESSAGES.save_io(fileName, error.message)
  notify(app, { severity: 'error', message, actions: [saveAsAction(app)] })
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
  const saved = await mutate(app, (current) => app.core.saveDocument(current.documentHandle))
  if (saved === null) {
    return false
  }
  try {
    await app.files.write(target, 'ptsm', textBytes(saved.text))
  } catch (error) {
    if (error instanceof FileServiceError) {
      showWriteError(app, target.name, error)
      return false
    }
    throw error
  }
  const marked = await mutate(app, (current) =>
    app.core.markSaved(current.documentHandle, saved.revision),
  )
  if (marked === null) {
    return false
  }
  app.store.setState((state) =>
    state.document === null ? {} : { document: { ...state.document, localUnsaved: false } },
  )
  await app.recovery.clear()
  return true
}

/** Save (Mod+S): to the document's file, or Save as without one. */
export async function save(app: App): Promise<void> {
  const document = documentOf(app)
  if (document === null) {
    return
  }
  const file = document.file
  if (file?.handle === null || file?.handle === undefined) {
    await saveAs(app)
    return
  }
  await writeDocument(app, { kind: 'handle', name: file.name, handle: file.handle })
}

/** Save as (Mod+Shift+S): asks where first; a canceled picker changes nothing. */
export async function saveAs(app: App): Promise<void> {
  const document = documentOf(app)
  if (document === null) {
    return
  }
  const target = await app.files.chooseSaveTarget(`${document.name}.ptsm`, 'ptsm')
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
    notify(app, { severity: 'info', message: MESSAGES.download_hint(target.name), actions: [] })
  }
}

/** Applies labeled edits as one undo entry; resolves false when the core refused them. */
export function applyEdits(app: App, label: string, edits: readonly Edit[]): Promise<boolean> {
  return commitEdits(app, label, edits)
}

/** Undo (Mod+Z, 3.20): cancels an active feature command instead; in sketch mode views undo first. */
export async function undo(app: App): Promise<void> {
  const state = app.store.getState()
  if (state.command !== null) {
    await cancelCommand(app)
    return
  }
  if (await undoInSketch(app)) {
    return
  }
  await coreUndo(app)
}

/** Redo (Mod+Shift+Z, 3.20); does nothing while a feature command is active. */
export async function redo(app: App): Promise<void> {
  if (app.store.getState().command !== null) {
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

/** The core's undo through the mutation queue. */
export async function coreUndo(app: App): Promise<void> {
  const document = documentOf(app)
  if (document === null) {
    return
  }
  try {
    await app.mutations.run(() => app.core.undo(document.documentHandle))
  } catch (error) {
    quietWhenNothingToDo(app, error)
  }
}

/** The core's redo through the mutation queue. */
export async function coreRedo(app: App): Promise<void> {
  const document = documentOf(app)
  if (document === null) {
    return
  }
  try {
    await app.mutations.run(() => app.core.redo(document.documentHandle))
  } catch (error) {
    quietWhenNothingToDo(app, error)
  }
}

/** Changes the document units and explains what that means for typed numbers (3.21). */
export async function setUnits(app: App, units: Units): Promise<void> {
  if (await commitEdits(app, `Change units to ${units}`, [{ type: 'set_units', units }])) {
    notify(app, { severity: 'info', message: MESSAGES.units_changed(units), actions: [] })
  }
}
