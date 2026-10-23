import { vi, type Mock } from 'vitest'

// File System Access handles for the web file service's folder tests (ui.md 2.15, U-230 to
// U-232): folders that hold files and folders, files whose writable streams replace their bytes on
// close() and keep them on abort(), the DOMException names Chromium gives, and permission answers
// a test sets. Every permission call is logged in order, so a test can tell which was asked first.

type PermissionDescriptor = { readonly mode: 'read' | 'readwrite' }

/** The permission methods of a handle, as mocks. */
export type FakePermissions = {
  readonly queryPermission: Mock<(descriptor: PermissionDescriptor) => Promise<PermissionState>>
  readonly requestPermission: Mock<(descriptor: PermissionDescriptor) => Promise<PermissionState>>
}

/** What a fake handle answers to permission queries and requests; both `granted` unless set. */
export type PermissionAnswers = {
  readonly query?: PermissionState
  readonly request?: PermissionState
}

/** A file of a fake folder. */
export type FakeWebFile = FakePermissions & {
  readonly kind: 'file'
  readonly handle: FileSystemFileHandle
  readonly name: string
  /** The file's bytes; a closed writable replaces them. */
  bytes: Uint8Array
  /** When set, `write` of this file's writable streams rejects with it. */
  writeError: Error | null
  readonly getFile: Mock<() => Promise<File>>
  readonly createWritable: Mock<() => Promise<FakeWritable>>
  /** The `File` objects `getFile` returned, each with `arrayBuffer` spied on. */
  readonly files: File[]
  readonly writables: FakeWritable[]
}

/** A writable stream of a fake file. */
export type FakeWritable = {
  readonly write: Mock<(data: unknown) => Promise<void>>
  readonly close: Mock<() => Promise<void>>
  readonly abort: Mock<() => Promise<void>>
}

/** `getDirectoryHandle` or `getFileHandle`: the entry of a folder by name, created when asked. */
type EntryLookup<Handle> = (
  name: string,
  options?: { readonly create?: boolean },
) => Promise<Handle>

/** A fake folder: its entries by name, and the handle methods the file service uses. */
export type FakeWebFolder = FakePermissions & {
  readonly kind: 'directory'
  readonly handle: FileSystemDirectoryHandle
  readonly name: string
  readonly entries: Map<string, FakeWebFile | FakeWebFolder>
  /** Names whose files, once created through `getFileHandle(name, { create: true })`, fail to write. */
  readonly failingNewFiles: Set<string>
  readonly getDirectoryHandle: Mock<EntryLookup<FileSystemDirectoryHandle>>
  readonly getFileHandle: Mock<EntryLookup<FileSystemFileHandle>>
  readonly removeEntry: Mock<(name: string) => Promise<void>>
  /** What `resolve` answers for a handle; by default the path of a file it holds, else null. */
  readonly resolve: Mock<(handle: unknown) => Promise<readonly string[] | null>>
}

export function domError(
  name: string,
  message = `${name} from the fake file system.`,
): DOMException {
  return new DOMException(message, name)
}

function permissions(name: string, answers: PermissionAnswers, log: string[]): FakePermissions {
  return {
    queryPermission: vi.fn<(descriptor: PermissionDescriptor) => Promise<PermissionState>>(
      async (descriptor) => {
        log.push(`${name}.queryPermission:${descriptor.mode}`)
        return answers.query ?? 'granted'
      },
    ),
    requestPermission: vi.fn<(descriptor: PermissionDescriptor) => Promise<PermissionState>>(
      async (descriptor) => {
        log.push(`${name}.requestPermission:${descriptor.mode}`)
        return answers.request ?? 'granted'
      },
    ),
  }
}

/** A fake file holding `bytes`. `log` receives the permission calls as `{name}.{method}:{mode}`. */
export function fakeWebFile(
  name: string,
  bytes: Uint8Array,
  options: { readonly permission?: PermissionAnswers; readonly log?: string[] } = {},
): FakeWebFile {
  const files: File[] = []
  const writables: FakeWritable[] = []
  const permissionMethods = permissions(name, options.permission ?? {}, options.log ?? [])
  const getFile = vi.fn<() => Promise<File>>(async () => {
    const current = new File([Uint8Array.from(file.bytes)], name)
    vi.spyOn(current, 'arrayBuffer')
    files.push(current)
    return current
  })
  const createWritable = vi.fn<() => Promise<FakeWritable>>(async () => {
    let pending: Uint8Array | null = null
    const writable: FakeWritable = {
      write: vi.fn<(data: unknown) => Promise<void>>(async (data) => {
        if (file.writeError !== null) {
          throw file.writeError
        }
        pending = Uint8Array.from(data instanceof Uint8Array ? data : new Uint8Array(0))
      }),
      close: vi.fn<() => Promise<void>>(async () => {
        if (pending !== null) {
          file.bytes = pending
        }
      }),
      abort: vi.fn<() => Promise<void>>(async () => {
        pending = null
      }),
    }
    writables.push(writable)
    return writable
  })
  const handle = {
    kind: 'file',
    name,
    getFile,
    createWritable,
    ...permissionMethods,
    isSameEntry: async (other: unknown) => other === handle,
  }
  const file: FakeWebFile = {
    kind: 'file',
    handle: handle as unknown as FileSystemFileHandle,
    name,
    bytes: Uint8Array.from(bytes),
    writeError: null,
    getFile,
    createWritable,
    files,
    writables,
    ...permissionMethods,
  }
  return file
}

/** A fake folder holding `entries`. `log` receives the permission calls as `{name}.{method}:{mode}`. */
export function fakeWebFolder(
  name: string,
  entries: readonly (FakeWebFile | FakeWebFolder)[] = [],
  options: { readonly permission?: PermissionAnswers; readonly log?: string[] } = {},
): FakeWebFolder {
  const contents = new Map(entries.map((entry) => [entry.name, entry]))
  const failingNewFiles = new Set<string>()
  const log = options.log ?? []
  const permissionMethods = permissions(name, options.permission ?? {}, log)
  const getDirectoryHandle = vi.fn<EntryLookup<FileSystemDirectoryHandle>>(
    async (entryName, entryOptions) => {
      const entry = contents.get(entryName)
      if (entry === undefined) {
        if (entryOptions?.create !== true) {
          throw domError('NotFoundError')
        }
        const created = fakeWebFolder(entryName, [], { log })
        contents.set(entryName, created)
        return created.handle
      }
      if (entry.kind !== 'directory') {
        throw domError('TypeMismatchError')
      }
      return entry.handle
    },
  )
  const getFileHandle = vi.fn<EntryLookup<FileSystemFileHandle>>(
    async (entryName, entryOptions) => {
      const entry = contents.get(entryName)
      if (entry === undefined) {
        if (entryOptions?.create !== true) {
          throw domError('NotFoundError')
        }
        const created = fakeWebFile(entryName, new Uint8Array(0), { log })
        if (failingNewFiles.has(entryName)) {
          created.writeError = domError('InvalidStateError', 'The disk is full.')
        }
        contents.set(entryName, created)
        return created.handle
      }
      if (entry.kind !== 'file') {
        throw domError('TypeMismatchError')
      }
      return entry.handle
    },
  )
  const removeEntry = vi.fn<(name: string) => Promise<void>>(async (entryName) => {
    if (!contents.delete(entryName)) {
      throw domError('NotFoundError')
    }
  })
  const resolve = vi.fn<(handle: unknown) => Promise<readonly string[] | null>>(async (other) => {
    for (const entry of contents.values()) {
      if (entry.handle === other) {
        return [entry.name]
      }
    }
    return null
  })
  const handle = {
    kind: 'directory',
    name,
    getDirectoryHandle,
    getFileHandle,
    removeEntry,
    resolve,
    ...permissionMethods,
    isSameEntry: async (other: unknown) => other === handle,
  }
  return {
    kind: 'directory',
    handle: handle as unknown as FileSystemDirectoryHandle,
    name,
    entries: contents,
    failingNewFiles,
    getDirectoryHandle,
    getFileHandle,
    removeEntry,
    resolve,
    ...permissionMethods,
  }
}

/** The file of a fake folder under `name`; throws when it holds none. */
export function fileIn(folder: FakeWebFolder, name: string): FakeWebFile {
  const entry = folder.entries.get(name)
  if (entry?.kind !== 'file') {
    throw new Error(`${folder.name} holds no file ${name}.`)
  }
  return entry
}

/** The folder of a fake folder under `name`; throws when it holds none. */
export function folderIn(folder: FakeWebFolder, name: string): FakeWebFolder {
  const entry = folder.entries.get(name)
  if (entry?.kind !== 'directory') {
    throw new Error(`${folder.name} holds no folder ${name}.`)
  }
  return entry
}
