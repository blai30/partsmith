import { errorMessage } from '../core/errors'
import type { AssetFile } from './assets'
import {
  assetEntry,
  ASSETS_FOLDER,
  FileServiceError,
  noAccessError,
  readingOf,
  type DesktopFileHandle,
  type DocumentPlace,
  type FileService,
  type FileServiceErrorReason,
  type OpenedFile,
} from './file-service'

// The desktop file service (desktop.md 3.7): native dialogs, the files the user chose and the assets
// next to a chosen document, through the desktop host's commands on `window.__TAURI__`, without a
// Tauri npm package.

/** The part of `window.__TAURI__` the desktop modules use (requires `app.withGlobalTauri`, 2.8). */
export type TauriInvokeGlobal = {
  readonly core: {
    invoke(
      command: string,
      payload?: Readonly<Record<string, unknown>> | ArrayBuffer,
      options?: { readonly headers: Readonly<Record<string, string>> },
    ): Promise<unknown>
  }
}

/** The header that carries the grant of write_file and write_asset. */
export const GRANT_HEADER = 'Partsmith-Grant'

/** The header that carries the asset name of write_asset. */
export const ASSET_HEADER = 'Partsmith-Asset'

/** The Tauri global of an environment when its `__TAURI__.core.invoke` is a function, else null. */
export function tauriGlobalOf(environment: object): TauriInvokeGlobal | null {
  const tauri: unknown = Reflect.get(environment, '__TAURI__')
  if (typeof tauri !== 'object' || tauri === null) {
    return null
  }
  const core: unknown = Reflect.get(tauri, 'core')
  if (typeof core !== 'object' || core === null) {
    return null
  }
  return typeof Reflect.get(core, 'invoke') === 'function' ? (tauri as TauriInvokeGlobal) : null
}

function isReason(value: unknown): value is FileServiceErrorReason {
  return value === 'permission' || value === 'not_found' || value === 'io'
}

function isNonEmptyString(value: unknown): value is string {
  return typeof value === 'string' && value !== ''
}

/** A rejected invoke as a FileServiceError: the host's `{reason, message}` keeps both, anything else is `io`. */
function hostRejectionError(rejection: unknown): FileServiceError {
  if (typeof rejection === 'object' && rejection !== null) {
    const reason: unknown = Reflect.get(rejection, 'reason')
    const message: unknown = Reflect.get(rejection, 'message')
    if (isReason(reason) && typeof message === 'string') {
      return new FileServiceError(reason, message, { cause: rejection })
    }
  }
  return new FileServiceError('io', errorMessage(rejection), { cause: rejection })
}

/** A failure of the host's asset command for one asset, with the entry `assets/{name}` (3.5). */
function assetError(error: unknown, name: string): FileServiceError {
  const failure = error instanceof FileServiceError ? error : hostRejectionError(error)
  return new FileServiceError(failure.reason, failure.message, {
    cause: failure.cause,
    entry: assetEntry(name),
  })
}

function unexpectedReply(command: string): FileServiceError {
  return new FileServiceError('io', `The desktop app sent an unexpected reply to ${command}.`)
}

/** The grant of a place's desktop handle, or null for a web handle, which reaches no assets here (3.7). */
function grantOf(place: DocumentPlace): string | null {
  return place.handle.kind === 'desktop' ? place.handle.grant : null
}

/** The failure for a web handle's assets on the desktop (3.7 steps 11 and 12). */
function noAssetsAccess(place: DocumentPlace): FileServiceError {
  return noAccessError(place.handle.name, { entry: ASSETS_FOLDER })
}

/** The handle of a pick reply, or null when the user canceled. */
function pickedHandle(reply: unknown, command: string): DesktopFileHandle | null {
  if (reply === null) {
    return null
  }
  if (typeof reply === 'object') {
    const name: unknown = Reflect.get(reply, 'name')
    const grant: unknown = Reflect.get(reply, 'grant')
    if (isNonEmptyString(name) && isNonEmptyString(grant)) {
      return { kind: 'desktop', name, grant }
    }
  }
  throw unexpectedReply(command)
}

/** An ArrayBuffer holding exactly the bytes: the view's own buffer when it covers all of one, else a copy. */
function exactBuffer(bytes: Uint8Array): ArrayBuffer {
  const { buffer } = bytes
  if (
    buffer instanceof ArrayBuffer &&
    bytes.byteOffset === 0 &&
    bytes.byteLength === buffer.byteLength
  ) {
    return buffer
  }
  return bytes.slice().buffer
}

/** The desktop file service: native dialogs and granted files through the desktop host's commands. */
export function createDesktopFileService(tauri: TauriInvokeGlobal): FileService {
  async function invoke(
    command: string,
    payload?: Parameters<TauriInvokeGlobal['core']['invoke']>[1],
    options?: Parameters<TauriInvokeGlobal['core']['invoke']>[2],
  ): Promise<unknown> {
    try {
      return await tauri.core.invoke(command, payload, options)
    } catch (error) {
      throw hostRejectionError(error)
    }
  }

  /** The bytes a read command answers with; any other reply is unexpected. */
  async function readBytes(
    command: 'read_file' | 'read_asset',
    payload: Readonly<Record<string, string>>,
  ): Promise<Uint8Array> {
    const reply = await invoke(command, payload)
    if (!(reply instanceof ArrayBuffer)) {
      throw unexpectedReply(command)
    }
    return new Uint8Array(reply)
  }

  async function read(handle: DesktopFileHandle): Promise<OpenedFile> {
    return {
      name: handle.name,
      bytes: await readBytes('read_file', { grant: handle.grant }),
      handle,
    }
  }

  return {
    fileSystemAccess: true,

    async pickToOpen() {
      const handle = pickedHandle(await invoke('pick_open_file'), 'pick_open_file')
      return handle === null ? null : read(handle)
    },

    // A grant reads and writes alike, and the host finds the folder by itself (3.7 step 9).
    async readHandle(handle) {
      // A web handle reaches this service only if IndexedDB data moved between hosts.
      if (handle.kind !== 'desktop') {
        throw noAccessError(handle.name)
      }
      return read(handle)
    },

    async chooseSaveTarget(suggestedName, kind) {
      const reply = await invoke('pick_save_file', { suggestedName, kind })
      const handle = pickedHandle(reply, 'pick_save_file')
      return handle === null ? null : { kind: 'handle', name: handle.name, handle }
    },

    async write(target, _kind, bytes) {
      if (target.kind === 'download') {
        throw new FileServiceError('io', 'The desktop app saves to files, not downloads.')
      }
      const { handle } = target
      if (handle.kind !== 'desktop') {
        throw noAccessError(handle.name)
      }
      await invoke('write_file', exactBuffer(bytes), {
        headers: { [GRANT_HEADER]: handle.grant },
      })
    },

    reachesFolder(place) {
      return place.handle.kind === 'desktop'
    },

    async chooseFolder() {
      throw new FileServiceError('io', 'The desktop app finds the folder of a file by itself.')
    },

    async readAssets(place, names) {
      const grant = grantOf(place)
      if (grant === null) {
        return { assets: [], failure: noAssetsAccess(place) }
      }
      return readingOf(
        names.map(async (name): Promise<AssetFile | FileServiceError | null> => {
          try {
            return { name, bytes: await readBytes('read_asset', { grant, name }) }
          } catch (error) {
            // An asset the host does not find is left out; any other failure leaves it out too.
            return error instanceof FileServiceError && error.reason === 'not_found'
              ? null
              : assetError(error, name)
          }
        }),
      )
    },

    async writeAssets(place, assets) {
      const grant = grantOf(place)
      if (grant === null) {
        throw noAssetsAccess(place)
      }
      for (const asset of assets) {
        try {
          // In order, so no asset after the first failure is sent (file-format.md 3.11).
          // oxlint-disable-next-line no-await-in-loop
          await invoke('write_asset', exactBuffer(asset.bytes), {
            headers: { [GRANT_HEADER]: grant, [ASSET_HEADER]: asset.name },
          })
        } catch (error) {
          throw assetError(error, asset.name)
        }
      }
    },
  }
}
