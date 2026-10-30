import { CoreError } from '../../core/client'
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
import type { Edit, Units } from '../../generated/protocol'
import { MESSAGES } from '../../messages'
import { isUnsaved } from '../../state/selectors'
import type { DocumentState, FileReference } from '../../state/store'
import type { App } from '../createApp'
import { removeRecoveryEntry } from '../recovery-copy'
import { confirmRunning, confirmSettled, redoCommandChange, undoCommandChange } from './command'
import {
  chooseFolderOf,
  isOffered,
  offerOf,
  showSaveAssetsError,
  type DocumentOffer,
} from './document-assets'
import { documentName, openFile, textBytes } from './document-open'
import {
  commitEdits,
  documentOf,
  mutate,
  refusedDuringCommand,
  runChange,
  showCoreError,
} from './edits'
import { showSaveError } from './files'
import { announce, notify } from './notifications'
import { redoInSketch, undoInSketch } from './sketch'

export { newDocument, openFile, openRecent, openWithPicker, startCore } from './document-open'

// Saving document files, undo and document units (ui.md 3.19, 3.20, 3.21); opening them is in
// document-open.ts.

/** Stores that already showed the download hint this session (3.19). */
const downloadHintShown = new WeakSet<App['store']>()

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
