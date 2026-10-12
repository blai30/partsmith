import type {
  FileHandle,
  FileKind,
  FileService,
  FileServiceError,
  OpenedFile,
  SaveTarget,
} from '../../files/file-service'

// A FileService for app tests: no pickers, no downloads, every call recorded.

/** One `write` call. */
export type FileWrite = {
  readonly target: SaveTarget
  readonly kind: FileKind
  readonly bytes: Uint8Array
}

export type FakeFileService = FileService & {
  readonly writes: readonly FileWrite[]
  /** The arguments of every `chooseSaveTarget` call. */
  readonly saveRequests: readonly { readonly suggestedName: string; readonly kind: FileKind }[]
  /** The handles passed to `readHandle`. */
  readonly reads: readonly FileHandle[]
  /** The file the next `pickToOpen` returns; null means the user cancels. */
  nextOpen: OpenedFile | null
  /**
   * What the next `chooseSaveTarget` returns: a target, null (canceled), or 'default': a handle named
   * after the suggestion with the File System Access API, else a download target.
   */
  nextSaveTarget: SaveTarget | null | 'default'
  /** When set, every `write` throws it. */
  writeError: FileServiceError | null
  /** When set, every `readHandle` throws it. */
  readError: FileServiceError | null
  /** The text `readHandle` returns for a handle; set by `write` to a handle too. */
  readonly contents: Map<FileHandle, string>
}

/** A file handle with the parts of the File System Access API the app uses. */
export function createFakeFileHandle(name: string): FileSystemFileHandle {
  const handle = {
    kind: 'file',
    name,
    isSameEntry: async (other: unknown) => other === handle,
    queryPermission: async () => 'granted',
    requestPermission: async () => 'granted',
  }
  return handle as unknown as FileSystemFileHandle
}

export function createFakeFileService(
  options: { readonly fileSystemAccess?: boolean; readonly journal?: string[] } = {},
): FakeFileService {
  const fileSystemAccess = options.fileSystemAccess ?? true
  const writes: FileWrite[] = []
  const saveRequests: { suggestedName: string; kind: FileKind }[] = []
  const reads: FileHandle[] = []
  const contents = new Map<FileHandle, string>()

  const service: FakeFileService = {
    fileSystemAccess,
    writes,
    saveRequests,
    reads,
    nextOpen: null,
    nextSaveTarget: 'default',
    writeError: null,
    readError: null,
    contents,
    async pickToOpen() {
      options.journal?.push('files.pickToOpen')
      return service.nextOpen
    },
    async readHandle(handle) {
      options.journal?.push('files.readHandle')
      reads.push(handle)
      if (service.readError !== null) {
        throw service.readError
      }
      return {
        name: handle.name,
        bytes: Uint8Array.from(new TextEncoder().encode(contents.get(handle) ?? '')),
        handle,
      }
    },
    async chooseSaveTarget(suggestedName, kind) {
      options.journal?.push('files.chooseSaveTarget')
      saveRequests.push({ suggestedName, kind })
      if (service.nextSaveTarget !== 'default') {
        return service.nextSaveTarget
      }
      return fileSystemAccess
        ? { kind: 'handle', name: suggestedName, handle: createFakeFileHandle(suggestedName) }
        : { kind: 'download', name: suggestedName }
    },
    async write(target, kind, bytes) {
      options.journal?.push('files.write')
      if (service.writeError !== null) {
        throw service.writeError
      }
      writes.push({ target, kind, bytes })
      if (target.kind === 'handle') {
        contents.set(target.handle, new TextDecoder().decode(bytes))
      }
    },
  }
  return service
}
