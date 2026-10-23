import type { AssetFile } from '../../files/assets'
import {
  FileServiceError,
  type DesktopFileHandle,
  type DocumentPlace,
  type FileHandle,
  type FileKind,
  type FileService,
  type FolderChoice,
  type OpenedFile,
  type SaveTarget,
} from '../../files/file-service'

// A FileService for app tests: no pickers, no downloads, every call recorded. It keeps the text of
// each handle, which a test changes to stand for a change made elsewhere, and the asset files of
// fake web folders and of the folders of desktop handles (ui.md 6.4, assets and files changed
// elsewhere).

/** One `write` call. */
export type FileWrite = {
  readonly target: SaveTarget
  readonly kind: FileKind
  readonly bytes: Uint8Array
}

/** The methods of a FileService. */
export type FileServiceMethod = Exclude<keyof FileService, 'fileSystemAccess'>

/** One call of the fake file service with its arguments. */
export type FileServiceCall = {
  readonly method: FileServiceMethod
  readonly args: readonly unknown[]
}

/** Where the fake keeps asset files: a fake web folder, or the folder of a desktop handle. */
export type FakeAssetLocation = FileSystemDirectoryHandle | DesktopFileHandle

export type FakeFileService = FileService & {
  readonly writes: readonly FileWrite[]
  /** The arguments of every `chooseSaveTarget` call. */
  readonly saveRequests: readonly { readonly suggestedName: string; readonly kind: FileKind }[]
  /** The handles passed to `readHandle`. */
  readonly reads: readonly FileHandle[]
  /** Every call, oldest first, with its arguments. */
  readonly calls: readonly FileServiceCall[]
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
  /** When set, `readAssets` of a folder it reaches reads nothing and answers it as the failure. */
  readAssetsFailure: FileServiceError | null
  /** When set, `writeAssets` to a folder it reaches throws it and writes nothing. */
  writeAssetsError: FileServiceError | null
  /** The answers of `chooseFolder`, taken in order; with none left it answers `canceled`, as a dismissed dialog. */
  readonly folderChoices: FolderChoice[]
  /** The text `readHandle` returns for a handle; set by `write` to a handle too. */
  readonly contents: Map<FileHandle, string>
  /** Puts asset files into a fake folder or the folder of a desktop handle, replacing those of the same name. */
  putAssets(location: FakeAssetLocation, assets: readonly AssetFile[]): void
  /** The asset files of a fake folder or of a desktop handle's folder, sorted by name. */
  assetsIn(location: FakeAssetLocation): readonly AssetFile[]
  /** Forgets the recorded `calls`. */
  clearCalls(): void
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

/**
 * A folder handle for app tests: `resolve` answers `[name]` for the file handles it holds directly
 * and null for any other, as `folderHolds` reads it (ui.md 2.15); it grants every permission.
 */
export function createFakeFolderHandle(
  name: string,
  holds: readonly FileHandle[] = [],
): FileSystemDirectoryHandle {
  const held = new Set<unknown>(holds)
  const folder = {
    kind: 'directory',
    name,
    isSameEntry: async (other: unknown) => other === folder,
    resolve: async (handle: { readonly name: string }) => (held.has(handle) ? [handle.name] : null),
    queryPermission: async () => 'granted',
    requestPermission: async () => 'granted',
  }
  return folder as unknown as FileSystemDirectoryHandle
}

/** A desktop file handle, as the desktop host's dialogs grant it. */
export function createDesktopHandle(name: string, grant: string): DesktopFileHandle {
  return { kind: 'desktop', name, grant }
}

function isDesktopLocation(location: FakeAssetLocation): location is DesktopFileHandle {
  return location.kind === 'desktop'
}

/** Whether a place reaches its assets folder: a desktop handle, or a web handle with a folder (2.15). */
function reaches(place: DocumentPlace): boolean {
  return place.handle.kind === 'desktop' || place.folder !== null
}

/** Where the fake keeps the asset files of a place that reaches its folder. */
function locationOf(place: DocumentPlace): FakeAssetLocation {
  if (place.handle.kind === 'desktop') {
    return place.handle
  }
  if (place.folder === null) {
    throw new Error('A web place without a folder reaches no assets.')
  }
  return place.folder
}

export function createFakeFileService(
  options: { readonly fileSystemAccess?: boolean; readonly journal?: string[] } = {},
): FakeFileService {
  const fileSystemAccess = options.fileSystemAccess ?? true
  const writes: FileWrite[] = []
  const saveRequests: { suggestedName: string; kind: FileKind }[] = []
  const reads: FileHandle[] = []
  const calls: FileServiceCall[] = []
  const contents = new Map<FileHandle, string>()
  // Desktop folders are keyed by grant, web folders by their handle.
  const folders = new Map<unknown, Map<string, Uint8Array>>()

  function record(method: FileServiceMethod, args: readonly unknown[]): void {
    options.journal?.push(`files.${method}`)
    calls.push({ method, args })
  }

  function folderOf(location: FakeAssetLocation): Map<string, Uint8Array> {
    const key = isDesktopLocation(location) ? location.grant : location
    let folder = folders.get(key)
    if (folder === undefined) {
      folder = new Map()
      folders.set(key, folder)
    }
    return folder
  }

  const service: FakeFileService = {
    fileSystemAccess,
    writes,
    saveRequests,
    reads,
    calls,
    nextOpen: null,
    nextSaveTarget: 'default',
    writeError: null,
    readError: null,
    readAssetsFailure: null,
    writeAssetsError: null,
    folderChoices: [],
    contents,
    putAssets(location, assets) {
      const folder = folderOf(location)
      for (const asset of assets) {
        folder.set(asset.name, Uint8Array.from(asset.bytes))
      }
    },
    clearCalls() {
      calls.length = 0
    },
    assetsIn(location) {
      return [...folderOf(location).entries()]
        .map(([name, bytes]) => ({ name, bytes }))
        .toSorted((first, second) => (first.name < second.name ? -1 : 1))
    },
    async pickToOpen() {
      record('pickToOpen', [])
      return service.nextOpen
    },
    async readHandle(handle, ...rest) {
      record('readHandle', [handle, ...rest])
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
      record('chooseSaveTarget', [suggestedName, kind])
      saveRequests.push({ suggestedName, kind })
      if (service.nextSaveTarget !== 'default') {
        return service.nextSaveTarget
      }
      return fileSystemAccess
        ? { kind: 'handle', name: suggestedName, handle: createFakeFileHandle(suggestedName) }
        : { kind: 'download', name: suggestedName }
    },
    async write(target, kind, bytes) {
      record('write', [target, kind, bytes])
      if (service.writeError !== null) {
        throw service.writeError
      }
      writes.push({ target, kind, bytes })
      if (target.kind === 'handle') {
        contents.set(target.handle, new TextDecoder().decode(bytes))
      }
    },
    reachesFolder(place) {
      return reaches(place)
    },
    async chooseFolder(handle) {
      record('chooseFolder', [handle])
      return service.folderChoices.shift() ?? { kind: 'canceled' }
    },
    async readAssets(place, names) {
      record('readAssets', [place, names])
      if (!reaches(place)) {
        return { assets: [], failure: null }
      }
      if (service.readAssetsFailure !== null) {
        return { assets: [], failure: service.readAssetsFailure }
      }
      const folder = folderOf(locationOf(place))
      const assets = names.flatMap((name) => {
        const bytes = folder.get(name)
        return bytes === undefined ? [] : [{ name, bytes: Uint8Array.from(bytes) }]
      })
      return { assets, failure: null }
    },
    async writeAssets(place, assets) {
      record('writeAssets', [place, assets])
      if (!reaches(place)) {
        throw new FileServiceError(
          'io',
          `Partsmith has no access to the folder of ${place.handle.name}.`,
          { entry: 'assets' },
        )
      }
      if (service.writeAssetsError !== null) {
        throw service.writeAssetsError
      }
      service.putAssets(locationOf(place), assets)
    },
  }
  return service
}

/** The arguments of each call of one method of the fake file service, oldest first. */
export function callsOf(
  files: FakeFileService,
  method: FileServiceMethod,
): readonly (readonly unknown[])[] {
  return files.calls.filter((call) => call.method === method).map((call) => call.args)
}

/** The names of the methods the fake file service was called with, oldest first. */
export function calledMethods(files: FakeFileService): readonly FileServiceMethod[] {
  return files.calls.map((call) => call.method)
}
