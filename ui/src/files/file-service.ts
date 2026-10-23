import { errorMessage } from '../core/errors'
import type { AssetFile } from './assets'

export type FileKind = 'ptsm' | '3mf' | 'stl' | 'obj' | 'step'

/** A file the desktop host granted through a native dialog. Plain data, so IndexedDB stores it. */
export type DesktopFileHandle = {
  readonly kind: 'desktop'
  readonly name: string
  readonly grant: string
}

/** A file the user chose, which Save and Open recent reach again without a dialog. */
export type FileHandle = FileSystemFileHandle | DesktopFileHandle

export type OpenedFile = {
  readonly name: string
  readonly bytes: Uint8Array
  readonly handle: FileHandle | null
}

export type SaveTarget =
  | { readonly kind: 'handle'; readonly name: string; readonly handle: FileHandle }
  | { readonly kind: 'download'; readonly name: string }

export type FileServiceErrorReason = 'permission' | 'not_found' | 'io'

/** The options of a FileServiceError: its cause and the entry of the document's folder it is about. */
export type FileServiceErrorOptions = ErrorOptions & { readonly entry?: string }

/** A file operation that failed for a reason the UI explains (ui.md 4.3). */
export class FileServiceError extends Error {
  readonly reason: FileServiceErrorReason
  /** The entry of the document's folder a failure is about, `assets` or `assets/{name}`, for Copy details (3.19); null for the file itself. */
  readonly entry: string | null

  /** `entry` is null unless given. */
  constructor(reason: FileServiceErrorReason, message: string, options?: FileServiceErrorOptions) {
    super(message, options)
    this.name = 'FileServiceError'
    this.reason = reason
    this.entry = options?.entry ?? null
  }
}

/** A document's file and, on the web, the folder the user chose that holds it (3.19); the desktop host finds the folder from the grant. */
export type DocumentPlace = {
  readonly handle: FileHandle
  readonly folder: FileSystemDirectoryHandle | null
}

/** What `readAssets` read, in the order asked, and its first failure (null when none). */
export type AssetReading = {
  readonly assets: readonly AssetFile[]
  readonly failure: FileServiceError | null
}

/** The answer to "choose the folder that holds this file" (3.19): `other` is a folder that does not hold it directly, by its name. */
export type FolderChoice =
  | { readonly kind: 'chosen'; readonly folder: FileSystemDirectoryHandle }
  | { readonly kind: 'canceled' }
  | { readonly kind: 'other'; readonly name: string }

/** The permission a read asks the web for (3.19): `readwrite` for Save's change check. */
export type PermissionMode = 'read' | 'readwrite'

export type FileService = {
  readonly fileSystemAccess: boolean
  /** Null when the user cancels. */
  pickToOpen(): Promise<OpenedFile | null>
  /**
   * `mode` is the permission the web asks for: `read` unless given; Save's change check asks
   * `readwrite` (3.19). `folder` is the web folder that holds the file, whose permission is asked
   * for first; null unless given.
   */
  readHandle(
    handle: FileHandle,
    mode?: PermissionMode,
    folder?: FileSystemDirectoryHandle | null,
  ): Promise<OpenedFile>
  /** Asks where to save. Null when the user cancels. Without the File System Access API it returns a download target at once. */
  chooseSaveTarget(suggestedName: string, kind: FileKind): Promise<SaveTarget | null>
  /** Writes the bytes to the target (a download for download targets). */
  write(target: SaveTarget, kind: FileKind, bytes: Uint8Array): Promise<void>
  /** Whether `readAssets` and `writeAssets` reach the `assets/` folder next to the file of `place`. */
  reachesFolder(place: DocumentPlace): boolean
  /** Asks for the folder that holds the file (the web; the desktop never needs it). */
  chooseFolder(handle: FileHandle): Promise<FolderChoice>
  /** Reads the named assets from the `assets/` folder next to the file, leaving out those that are not there. */
  readAssets(place: DocumentPlace, names: readonly string[]): Promise<AssetReading>
  /** Writes the assets, in order, to the `assets/` folder next to the file with the entry check of file-format.md 3.11 step 1; throws at the first failure. */
  writeAssets(place: DocumentPlace, assets: readonly AssetFile[]): Promise<void>
}

/** The folder next to a document that holds its assets (file-format.md 3.11). */
export const ASSETS_FOLDER = 'assets'

/** The entry of the document's folder an asset is: `assets/{name}`. */
export function assetEntry(name: string): string {
  return `${ASSETS_FOLDER}/${name}`
}

/**
 * The reading of assets read all at once (2.15): each read resolves its asset, null when it is not
 * there, or the failure that leaves it out; the assets keep the order asked, with the first failure.
 */
export async function readingOf(
  reads: readonly Promise<AssetFile | FileServiceError | null>[],
): Promise<AssetReading> {
  const results = await Promise.all(reads)
  return {
    assets: results.filter(
      (result): result is AssetFile => result !== null && !(result instanceof FileServiceError),
    ),
    failure:
      results.find((result): result is FileServiceError => result instanceof FileServiceError) ??
      null,
  }
}

export const FILE_TYPES: Readonly<
  Record<
    FileKind,
    {
      readonly description: string
      readonly mediaType: string
      readonly extensions: readonly string[]
    }
  >
> = {
  ptsm: { description: 'Partsmith model', mediaType: 'application/json', extensions: ['.ptsm'] },
  '3mf': { description: '3MF model', mediaType: 'model/3mf', extensions: ['.3mf'] },
  stl: { description: 'STL mesh', mediaType: 'model/stl', extensions: ['.stl'] },
  obj: { description: 'OBJ mesh', mediaType: 'model/obj', extensions: ['.obj'] },
  step: { description: 'STEP model', mediaType: 'model/step', extensions: ['.step', '.stp'] },
}

/** How long a download's object URL stays valid after the click, so the browser can fetch it. */
const DOWNLOAD_URL_LIFETIME_MILLISECONDS = 60_000

// The File System Access API, which TypeScript's DOM library does not declare (Chromium only).

type FilePickerType = {
  readonly description: string
  readonly accept: Readonly<Record<string, readonly string[]>>
}

type OpenFilePicker = (options: {
  readonly types: readonly FilePickerType[]
  readonly excludeAcceptAllOption: boolean
  readonly multiple: boolean
}) => Promise<readonly FileSystemFileHandle[]>

type SaveFilePicker = (options: {
  readonly suggestedName: string
  readonly types: readonly FilePickerType[]
}) => Promise<FileSystemFileHandle>

type DirectoryPicker = (options: {
  readonly mode: PermissionMode
  readonly startIn: FileSystemFileHandle
}) => Promise<FileSystemDirectoryHandle>

type PermissionMethods = {
  queryPermission?: (descriptor: { readonly mode: PermissionMode }) => Promise<PermissionState>
  requestPermission?: (descriptor: { readonly mode: PermissionMode }) => Promise<PermissionState>
}

type Pickers = { readonly open: OpenFilePicker; readonly save: SaveFilePicker }

function isFunction(value: unknown): value is (...parameters: never[]) => unknown {
  return typeof value === 'function'
}

/** The pickers when both are functions, bound to the environment. */
function pickersOf(environment: Window & typeof globalThis): Pickers | null {
  const open: unknown = Reflect.get(environment, 'showOpenFilePicker')
  const save: unknown = Reflect.get(environment, 'showSaveFilePicker')
  if (!isFunction(open) || !isFunction(save)) {
    return null
  }
  return {
    open: (options) => Reflect.apply(open, environment, [options]) as ReturnType<OpenFilePicker>,
    save: (options) => Reflect.apply(save, environment, [options]) as ReturnType<SaveFilePicker>,
  }
}

/** Whether a value is a file handle of either host, such as one read back from IndexedDB. */
export function isFileHandle(value: unknown): value is FileHandle {
  if (typeof value !== 'object' || value === null) {
    return false
  }
  const kind: unknown = Reflect.get(value, 'kind')
  return (
    kind === 'file' ||
    (kind === 'desktop' &&
      typeof Reflect.get(value, 'name') === 'string' &&
      typeof Reflect.get(value, 'grant') === 'string')
  )
}

/** Whether a value is a web folder handle, such as one read back from IndexedDB. */
export function isFolderHandle(value: unknown): value is FileSystemDirectoryHandle {
  return typeof value === 'object' && value !== null && Reflect.get(value, 'kind') === 'directory'
}

/** The error for a file Partsmith cannot reach, such as a handle that belongs to the other host. */
export function noAccessError(name: string, options?: FileServiceErrorOptions): FileServiceError {
  return new FileServiceError('permission', `Partsmith has no access to ${name}.`, options)
}

function pickerType(kind: FileKind): FilePickerType {
  const type = FILE_TYPES[kind]
  return { description: type.description, accept: { [type.mediaType]: type.extensions } }
}

/** Whether an error is a DOMException (or a stand-in for one) with this name. */
export function isDomError(error: unknown, name: string): boolean {
  return typeof error === 'object' && error !== null && Reflect.get(error, 'name') === name
}

/**
 * A failed read or write as a FileServiceError about `entry` of the document's folder (none
 * unless given): a missing file is `not_found`, anything else `io`, and a FileServiceError keeps
 * its reason and message.
 */
function fileServiceError(error: unknown, name: string, entry?: string): FileServiceError {
  if (error instanceof FileServiceError) {
    return entry === undefined
      ? error
      : new FileServiceError(error.reason, error.message, { cause: error.cause, entry })
  }
  const options = { cause: error, ...(entry === undefined ? {} : { entry }) }
  if (isDomError(error, 'NotFoundError')) {
    return new FileServiceError('not_found', `${name} was not found.`, options)
  }
  return new FileServiceError('io', errorMessage(error), options)
}

/** Asks for permission when the browser has not granted it yet; throws `permission` when it is refused. */
async function ensurePermission(handle: FileSystemHandle, mode: PermissionMode): Promise<void> {
  const methods = handle as FileSystemHandle & PermissionMethods
  // Browsers without the permission methods grant access through the picker itself.
  if (methods.queryPermission === undefined) {
    return
  }
  let state: PermissionState
  try {
    state = await methods.queryPermission.call(handle, { mode })
    if (state === 'prompt' && methods.requestPermission !== undefined) {
      state = await methods.requestPermission.call(handle, { mode })
    }
  } catch (error) {
    throw noAccessError(handle.name, { cause: error })
  }
  if (state !== 'granted') {
    throw noAccessError(handle.name)
  }
}

/** Whether bytes are a view on an ordinary ArrayBuffer, which Blobs and file streams accept. */
function isOnArrayBuffer(bytes: Uint8Array): bytes is Uint8Array<ArrayBuffer> {
  return bytes.buffer[Symbol.toStringTag] === 'ArrayBuffer'
}

/** The bytes as a view on an ordinary ArrayBuffer, which Blobs, file streams and Web Crypto accept: a view on shared memory is copied. */
export function writableBytes(bytes: Uint8Array): Uint8Array<ArrayBuffer> {
  return isOnArrayBuffer(bytes) ? bytes : bytes.slice()
}

async function bytesOf(file: Blob): Promise<Uint8Array> {
  return new Uint8Array(await file.arrayBuffer())
}

async function readFileHandle(handle: FileSystemFileHandle): Promise<OpenedFile> {
  try {
    const file = await handle.getFile()
    return { name: handle.name, bytes: await bytesOf(file), handle }
  } catch (error) {
    throw fileServiceError(error, handle.name)
  }
}

/** Whether two byte arrays hold the same bytes. */
function sameBytes(first: Uint8Array, second: Uint8Array): boolean {
  return (
    first.byteLength === second.byteLength && first.every((byte, index) => byte === second[index])
  )
}

async function writeFileHandle(handle: FileSystemFileHandle, bytes: Uint8Array): Promise<void> {
  try {
    const writable = await handle.createWritable()
    try {
      await writable.write(writableBytes(bytes))
      await writable.close()
    } catch (error) {
      // Aborting discards the partial write, so the file on disk keeps its earlier content.
      await writable.abort?.().catch(() => undefined)
      throw error
    }
  } catch (error) {
    throw fileServiceError(error, handle.name)
  }
}

/**
 * Whether a web folder holds a file directly: `folder.resolve(handle)` is `[handle.name]`; false
 * for a desktop handle, a file elsewhere or in a subfolder, and a folder that cannot resolve it.
 */
export async function folderHolds(
  folder: FileSystemDirectoryHandle,
  handle: FileHandle,
): Promise<boolean> {
  if (handle.kind === 'desktop') {
    return false
  }
  try {
    const path = await folder.resolve(handle)
    return path?.length === 1 && path[0] === handle.name
  } catch {
    return false
  }
}

/** The folder of a place the web reaches: a web handle with a folder; null otherwise. */
function webFolderOf(place: DocumentPlace): FileSystemDirectoryHandle | null {
  return place.handle.kind === 'desktop' ? null : place.folder
}

/**
 * The document folder's `assets` folder, after asking for the folder's `readwrite` permission (a
 * refusal throws `permission`); `create` makes a missing one.
 */
async function assetsFolderOf(
  folder: FileSystemDirectoryHandle,
  create: boolean,
): Promise<FileSystemDirectoryHandle> {
  await ensurePermission(folder, 'readwrite')
  return folder.getDirectoryHandle(ASSETS_FOLDER, { create })
}

/**
 * Writes one asset after the entry check of file-format.md 3.11, as far as the API sees it: a
 * file of the asset's size that holds its bytes is kept, any other file is replaced whole (and
 * keeps its bytes when that fails), a folder fails and stays, and a missing entry is created and
 * removed again when its write fails, so no empty file is left under an asset's name.
 */
async function writeAsset(assets: FileSystemDirectoryHandle, asset: AssetFile): Promise<void> {
  const entry = assetEntry(asset.name)
  let file: FileSystemFileHandle
  try {
    file = await assets.getFileHandle(asset.name)
  } catch (error) {
    if (!isDomError(error, 'NotFoundError')) {
      throw fileServiceError(error, asset.name, entry)
    }
    try {
      await writeFileHandle(await assets.getFileHandle(asset.name, { create: true }), asset.bytes)
    } catch (writeError) {
      await assets.removeEntry(asset.name).catch(() => undefined)
      throw fileServiceError(writeError, asset.name, entry)
    }
    return
  }
  try {
    const current = await file.getFile()
    // A file of another size is replaced without being read.
    if (current.size === asset.bytes.byteLength && sameBytes(await bytesOf(current), asset.bytes)) {
      return
    }
    await writeFileHandle(file, asset.bytes)
  } catch (error) {
    throw fileServiceError(error, asset.name, entry)
  }
}

/** Opens a file chosen in a hidden `<input type="file">`; null when the user chooses nothing. */
function pickWithInput(document: Document): Promise<OpenedFile | null> {
  return new Promise((resolve, reject) => {
    const input = document.createElement('input')
    input.type = 'file'
    input.accept = FILE_TYPES.ptsm.extensions.join(',')
    input.addEventListener(
      'change',
      () => {
        const file = input.files?.[0]
        if (file === undefined) {
          resolve(null)
          return
        }
        bytesOf(file).then(
          (bytes) => {
            resolve({ name: file.name, bytes, handle: null })
          },
          (error: unknown) => {
            reject(fileServiceError(error, file.name))
          },
        )
      },
      { once: true },
    )
    input.addEventListener(
      'cancel',
      () => {
        resolve(null)
      },
      { once: true },
    )
    input.click()
  })
}

/** Saves bytes by clicking a download link to an object URL. */
function download(
  environment: Window & typeof globalThis,
  name: string,
  kind: FileKind,
  bytes: Uint8Array,
): void {
  const blob = new environment.Blob([writableBytes(bytes)], { type: FILE_TYPES[kind].mediaType })
  const url = environment.URL.createObjectURL(blob)
  const anchor = environment.document.createElement('a')
  anchor.href = url
  anchor.download = name
  anchor.click()
  environment.setTimeout(() => {
    environment.URL.revokeObjectURL(url)
  }, DOWNLOAD_URL_LIFETIME_MILLISECONDS)
}

/** The browser file service: the File System Access API where available, else file inputs and downloads. */
export function createWebFileService(environment: Window & typeof globalThis): FileService {
  const pickers = pickersOf(environment)
  return {
    fileSystemAccess: pickers !== null,

    async pickToOpen() {
      if (pickers === null) {
        return pickWithInput(environment.document)
      }
      let handles: readonly FileSystemFileHandle[]
      try {
        handles = await pickers.open({
          types: [pickerType('ptsm')],
          excludeAcceptAllOption: false,
          multiple: false,
        })
      } catch (error) {
        if (isDomError(error, 'AbortError')) {
          return null
        }
        throw fileServiceError(error, FILE_TYPES.ptsm.description)
      }
      const [handle] = handles
      return handle === undefined ? null : readFileHandle(handle)
    },

    async readHandle(handle, mode = 'read', folder = null) {
      // A desktop handle reaches this service only if IndexedDB data moved between hosts.
      if (handle.kind === 'desktop') {
        throw noAccessError(handle.name)
      }
      // The folder's prompt comes first, so it is the one prompt of the user's click, which in
      // Chromium also covers the file; a refusal here is reported by readAssets and writeAssets.
      if (folder !== null) {
        await ensurePermission(folder, 'readwrite').catch(() => undefined)
      }
      await ensurePermission(handle, mode)
      return readFileHandle(handle)
    },

    async chooseSaveTarget(suggestedName, kind) {
      if (pickers === null) {
        return { kind: 'download', name: suggestedName }
      }
      try {
        const handle = await pickers.save({ suggestedName, types: [pickerType(kind)] })
        return { kind: 'handle', name: handle.name, handle }
      } catch (error) {
        if (isDomError(error, 'AbortError')) {
          return null
        }
        throw fileServiceError(error, suggestedName)
      }
    },

    async write(target, kind, bytes) {
      if (target.kind === 'download') {
        download(environment, target.name, kind, bytes)
        return
      }
      const { handle } = target
      if (handle.kind === 'desktop') {
        throw noAccessError(handle.name)
      }
      await ensurePermission(handle, 'readwrite')
      await writeFileHandle(handle, bytes)
    },

    reachesFolder(place) {
      return webFolderOf(place) !== null
    },

    async chooseFolder(handle) {
      const picker: unknown = Reflect.get(environment, 'showDirectoryPicker')
      // A desktop handle has no web folder, and every browser with the two file pickers has this one (2.15).
      if (handle.kind === 'desktop' || !isFunction(picker)) {
        throw noAccessError(handle.name)
      }
      let folder: FileSystemDirectoryHandle
      try {
        // Opens in the file's folder, and one permission covers reading and writing it.
        folder = await (Reflect.apply(picker, environment, [
          { mode: 'readwrite', startIn: handle },
        ]) as ReturnType<DirectoryPicker>)
      } catch (error) {
        if (isDomError(error, 'AbortError')) {
          return { kind: 'canceled' }
        }
        throw new FileServiceError('io', errorMessage(error), { cause: error })
      }
      return (await folderHolds(folder, handle))
        ? { kind: 'chosen', folder }
        : { kind: 'other', name: folder.name }
    },

    async readAssets(place, names) {
      const folder = webFolderOf(place)
      if (folder === null) {
        return { assets: [], failure: null }
      }
      let assets: FileSystemDirectoryHandle
      try {
        assets = await assetsFolderOf(folder, false)
      } catch (error) {
        // A folder without `assets` holds no asset.
        const failure = fileServiceError(error, ASSETS_FOLDER, ASSETS_FOLDER)
        return { assets: [], failure: failure.reason === 'not_found' ? null : failure }
      }
      return readingOf(
        names.map(async (name) => {
          try {
            const file = await (await assets.getFileHandle(name)).getFile()
            return { name, bytes: await bytesOf(file) }
          } catch (error) {
            // A missing asset is left out; any other failure leaves it out too.
            return isDomError(error, 'NotFoundError')
              ? null
              : fileServiceError(error, name, assetEntry(name))
          }
        }),
      )
    },

    async writeAssets(place, assets) {
      const folder = webFolderOf(place)
      if (folder === null) {
        throw new FileServiceError(
          'io',
          `Partsmith has no access to the folder of ${place.handle.name}.`,
          { entry: ASSETS_FOLDER },
        )
      }
      let assetsFolder: FileSystemDirectoryHandle
      try {
        assetsFolder = await assetsFolderOf(folder, true)
      } catch (error) {
        throw fileServiceError(error, ASSETS_FOLDER, ASSETS_FOLDER)
      }
      for (const asset of assets) {
        // In order, so nothing after the first failure is written (file-format.md 3.11).
        // oxlint-disable-next-line no-await-in-loop
        await writeAsset(assetsFolder, asset)
      }
    },
  }
}
