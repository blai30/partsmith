// The IndexedDB database behind recent files and recovery (ui.md 2.15).

import { isDomError } from './file-service'
import { recoveryEntryFrom } from './recovery-entry'

export const DATABASE_NAME = 'partsmith'
export const DATABASE_VERSION = 2
export const RECENT_STORE = 'recent'
export const RECOVERY_STORE = 'recovery'

/** The key of the one recovery record of version 1, whose `recovery` store had the key path `slot`. */
const VERSION_1_SLOT = 'current'

export type StoreName = typeof RECENT_STORE | typeof RECOVERY_STORE

/** The result of a request, or its error. */
export function requestResult<Result>(request: IDBRequest<Result>): Promise<Result> {
  return new Promise((resolve, reject) => {
    request.addEventListener('success', () => {
      resolve(request.result)
    })
    request.addEventListener('error', () => {
      reject(request.error ?? new Error('An IndexedDB request failed.'))
    })
  })
}

/** Resolves when a transaction commits; rejects when it fails or aborts. */
export function transactionDone(transaction: IDBTransaction): Promise<void> {
  return new Promise((resolve, reject) => {
    transaction.addEventListener('complete', () => {
      resolve()
    })
    const failed = (): void => {
      reject(transaction.error ?? new Error('An IndexedDB transaction was aborted.'))
    }
    transaction.addEventListener('error', failed)
    transaction.addEventListener('abort', failed)
  })
}

/**
 * Moves the recovery copy of version 1 into the version 2 store, inside the upgrade transaction:
 * the one record under the slot `current` becomes an entry with a new id, and an invalid record is
 * dropped (2.15).
 */
function upgradeRecoveryFromVersion1(database: IDBDatabase, upgrade: IDBTransaction): void {
  const slot = upgrade.objectStore(RECOVERY_STORE).get(VERSION_1_SLOT)
  slot.addEventListener('success', () => {
    const record: unknown = slot.result
    database.deleteObjectStore(RECOVERY_STORE)
    const recovery = database.createObjectStore(RECOVERY_STORE, { keyPath: 'id' })
    const entry =
      typeof record === 'object' && record !== null
        ? recoveryEntryFrom({ ...record, id: crypto.randomUUID() })
        : null
    if (entry !== null) {
      // Version 1 kept no folder or digest, so the record keeps its own fields; both read as null.
      const { folder: _folder, digest: _digest, ...moved } = entry
      recovery.put(moved)
    }
  })
}

/**
 * Brings a database at `oldVersion` to the stores of `DATABASE_VERSION`, inside the open request's
 * versionchange transaction `upgrade`: a new database gets them at once, and each later step runs
 * only for a database older than the version that made it, so no upgrade redoes an earlier one.
 */
export function upgradeDatabase(
  database: IDBDatabase,
  upgrade: IDBTransaction,
  oldVersion: number,
): void {
  if (oldVersion < 1) {
    database.createObjectStore(RECENT_STORE, { keyPath: 'id' })
    database.createObjectStore(RECOVERY_STORE, { keyPath: 'id' })
    return
  }
  if (oldVersion < 2) {
    upgradeRecoveryFromVersion1(database, upgrade)
  }
}

/** Opens the database, creating or upgrading its object stores on first use. */
function openDatabase(factory: IDBFactory): Promise<IDBDatabase> {
  return new Promise((resolve, reject) => {
    let rejected = false
    const fail = (error: Error): void => {
      rejected = true
      reject(error)
    }
    const request = factory.open(DATABASE_NAME, DATABASE_VERSION)
    request.addEventListener('upgradeneeded', (event) => {
      // An upgrade runs in its versionchange transaction, which the open request holds meanwhile.
      const upgrade = request.transaction
      if (upgrade !== null) {
        upgradeDatabase(request.result, upgrade, event.oldVersion)
      }
    })
    request.addEventListener('success', () => {
      // A connection that arrives after a blocked open was given up is closed at once.
      if (rejected) {
        request.result.close()
      } else {
        resolve(request.result)
      }
    })
    request.addEventListener('error', () => {
      fail(request.error ?? new Error('The IndexedDB database could not be opened.'))
    })
    request.addEventListener('blocked', () => {
      fail(new Error('The IndexedDB database is blocked by another tab.'))
    })
  })
}

/** A lazily opened database connection shared by one store's operations. */
export type Connection = {
  /**
   * A transaction on one object store. The database is opened on first use, and opened again
   * after a failed open, after a version change from another tab and after the browser closed the
   * connection (cleared site data, evicted storage).
   */
  transaction(storeName: StoreName, mode: IDBTransactionMode): Promise<IDBTransaction>
}

/** The connection one store (recent files or recovery) shares between its operations. */
export function createConnection(factory: IDBFactory): Connection {
  let opening: Promise<IDBDatabase> | null = null

  // Forgets a connection, unless a newer one has replaced it already.
  function forget(attempt: Promise<IDBDatabase>): void {
    if (opening === attempt) {
      opening = null
    }
  }

  function connect(): Promise<IDBDatabase> {
    if (opening !== null) {
      return opening
    }
    const attempt: Promise<IDBDatabase> = openDatabase(factory).then((database) => {
      database.addEventListener('versionchange', () => {
        database.close()
        forget(attempt)
      })
      database.addEventListener('close', () => {
        forget(attempt)
      })
      return database
    })
    attempt.catch(() => {
      forget(attempt)
    })
    opening = attempt
    return attempt
  }

  return {
    async transaction(storeName, mode) {
      const attempt = connect()
      const database = await attempt
      try {
        return database.transaction(storeName, mode)
      } catch (error) {
        if (!isDomError(error, 'InvalidStateError')) {
          throw error
        }
        // The browser closed the connection without a close event: open a new one, once.
        forget(attempt)
        return (await connect()).transaction(storeName, mode)
      }
    },
  }
}

/**
 * Runs an operation against one object store and resolves with its result once the transaction
 * commits.
 */
export async function withStore<Result>(
  connection: Connection,
  storeName: StoreName,
  mode: IDBTransactionMode,
  operation: (store: IDBObjectStore) => Promise<Result>,
): Promise<Result> {
  const transaction = await connection.transaction(storeName, mode)
  const done = transactionDone(transaction)
  const [result] = await Promise.all([operation(transaction.objectStore(storeName)), done])
  return result
}

/**
 * Runs an operation against one object store in a transaction of `mode`. A failure is logged as a
 * warning and yields `fallback`, so the app never depends on IndexedDB working.
 */
export type StoreAccess = <Result, Fallback>(
  mode: IDBTransactionMode,
  operation: (store: IDBObjectStore) => Promise<Result>,
  fallback: Fallback,
) => Promise<Result | Fallback>

/** Access to one object store over its own lazily opened connection; failures warn with `failureMessage`. */
export function createStoreAccess(
  factory: IDBFactory,
  storeName: StoreName,
  failureMessage: string,
): StoreAccess {
  const connection = createConnection(factory)
  return async (mode, operation, fallback) => {
    try {
      return await withStore(connection, storeName, mode, operation)
    } catch (error) {
      console.warn(failureMessage, error)
      return fallback
    }
  }
}
