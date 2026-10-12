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

/** A file operation that failed for a reason the UI explains (ui.md 4.3). */
export class FileServiceError extends Error {
  readonly reason: FileServiceErrorReason

  constructor(reason: FileServiceErrorReason, message: string, options?: ErrorOptions) {
    super(message, options)
    this.name = 'FileServiceError'
    this.reason = reason
  }
}

export type FileService = {
  readonly fileSystemAccess: boolean
  /** Null when the user cancels. */
  pickToOpen(): Promise<OpenedFile | null>
  readHandle(handle: FileHandle): Promise<OpenedFile>
  /** Asks where to save. Null when the user cancels. Without the File System Access API it returns a download target at once. */
  chooseSaveTarget(suggestedName: string, kind: FileKind): Promise<SaveTarget | null>
  /** Writes the bytes to the target (a download for download targets). */
  write(target: SaveTarget, kind: FileKind, bytes: Uint8Array): Promise<void>
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

type PermissionMode = 'read' | 'readwrite'

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

/** The error for a file Partsmith cannot reach, such as a handle that belongs to the other host. */
export function noAccessError(name: string, options?: ErrorOptions): FileServiceError {
  return new FileServiceError('permission', `Partsmith has no access to ${name}.`, options)
}

function pickerType(kind: FileKind): FilePickerType {
  const type = FILE_TYPES[kind]
  return { description: type.description, accept: { [type.mediaType]: type.extensions } }
}

function isDomError(error: unknown, name: string): boolean {
  return typeof error === 'object' && error !== null && Reflect.get(error, 'name') === name
}

function messageOf(error: unknown): string {
  return error instanceof Error ? error.message : String(error)
}

/** A failed read or write as a FileServiceError: a missing file is `not_found`, anything else `io`. */
function fileServiceError(error: unknown, name: string): FileServiceError {
  if (error instanceof FileServiceError) {
    return error
  }
  if (isDomError(error, 'NotFoundError')) {
    return new FileServiceError('not_found', `${name} was not found.`, { cause: error })
  }
  return new FileServiceError('io', messageOf(error), { cause: error })
}

/** Asks for permission when the browser has not granted it yet; throws `permission` when it is refused. */
async function ensurePermission(handle: FileSystemFileHandle, mode: PermissionMode): Promise<void> {
  const methods = handle as FileSystemFileHandle & PermissionMethods
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

/** The bytes as a view on an ordinary ArrayBuffer: a view on shared memory is copied. */
function writableBytes(bytes: Uint8Array): Uint8Array<ArrayBuffer> {
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

    async readHandle(handle) {
      // A desktop handle reaches this service only if IndexedDB data moved between hosts.
      if (handle.kind === 'desktop') {
        throw noAccessError(handle.name)
      }
      await ensurePermission(handle, 'read')
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
  }
}
