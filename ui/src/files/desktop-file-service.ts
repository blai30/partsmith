import { errorMessage } from '../core/errors'
import {
  FileServiceError,
  noAccessError,
  type DesktopFileHandle,
  type FileService,
  type FileServiceErrorReason,
  type OpenedFile,
} from './file-service'

// The desktop file service (desktop.md 3.7): native dialogs and the files the user chose, through the
// desktop host's commands on `window.__TAURI__`, without a Tauri npm package.

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

/** The header that carries a write_file grant. */
export const GRANT_HEADER = 'Partsmith-Grant'

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

function unexpectedReply(command: string): FileServiceError {
  return new FileServiceError('io', `The desktop app sent an unexpected reply to ${command}.`)
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

  async function read(handle: DesktopFileHandle): Promise<OpenedFile> {
    const reply = await invoke('read_file', { grant: handle.grant })
    if (!(reply instanceof ArrayBuffer)) {
      throw unexpectedReply('read_file')
    }
    return { name: handle.name, bytes: new Uint8Array(reply), handle }
  }

  return {
    fileSystemAccess: true,

    async pickToOpen() {
      const handle = pickedHandle(await invoke('pick_open_file'), 'pick_open_file')
      return handle === null ? null : read(handle)
    },

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
  }
}
