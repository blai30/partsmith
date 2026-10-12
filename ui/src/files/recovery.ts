import { createConnection, RECOVERY_STORE, requestResult, withStore } from './database'
import { isFileHandle, type FileHandle } from './file-service'

export type RecoveryEntry = {
  readonly name: string
  readonly text: string
  readonly savedAt: number
  readonly handle: FileHandle | null
  readonly reason: 'autosave' | 'replaced'
}

/** One recovery copy: writing replaces it. */
export type RecoveryStore = {
  read(): Promise<RecoveryEntry | null>
  write(entry: RecoveryEntry): Promise<void>
  clear(): Promise<void>
}

/** The key of the one record in the `recovery` object store. */
const SLOT = 'current'

/** A recovery copy kept in memory, for tests and for hosts without IndexedDB. */
export function createMemoryRecovery(): RecoveryStore {
  let current: RecoveryEntry | null = null
  return {
    read: async () => current,
    write: async (entry) => {
      current = entry
    },
    clear: async () => {
      current = null
    },
  }
}

function entryFrom(record: unknown): RecoveryEntry | null {
  if (typeof record !== 'object' || record === null) {
    return null
  }
  const field = (name: keyof RecoveryEntry): unknown => Reflect.get(record, name)
  const name = field('name')
  const text = field('text')
  const savedAt = field('savedAt')
  const handle = field('handle')
  const reason = field('reason')
  if (
    typeof name !== 'string' ||
    typeof text !== 'string' ||
    typeof savedAt !== 'number' ||
    (reason !== 'autosave' && reason !== 'replaced') ||
    (handle !== null && !isFileHandle(handle))
  ) {
    return null
  }
  return { name, text, savedAt, handle, reason }
}

function warn(error: unknown): void {
  console.warn('Partsmith could not use the recovery store in IndexedDB.', error)
}

/** The recovery copy in the IndexedDB object store `recovery`; every failure is logged and ignored. */
export function createIndexedDbRecovery(factory: IDBFactory): RecoveryStore {
  const connection = createConnection(factory)
  return {
    read: async () => {
      try {
        const record = await withStore(connection, RECOVERY_STORE, 'readonly', (store) =>
          requestResult<unknown>(store.get(SLOT)),
        )
        return entryFrom(record)
      } catch (error) {
        warn(error)
        return null
      }
    },
    write: async (entry) => {
      try {
        await withStore(connection, RECOVERY_STORE, 'readwrite', (store) =>
          requestResult(store.put({ ...entry, slot: SLOT })),
        )
      } catch (error) {
        warn(error)
      }
    },
    clear: async () => {
      try {
        await withStore(connection, RECOVERY_STORE, 'readwrite', (store) =>
          requestResult(store.delete(SLOT)),
        )
      } catch (error) {
        warn(error)
      }
    },
  }
}
