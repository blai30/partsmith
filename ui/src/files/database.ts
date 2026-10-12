// The IndexedDB database behind recent files and recovery (ui.md 2.15).

export const DATABASE_NAME = 'partsmith'
export const DATABASE_VERSION = 1
export const RECENT_STORE = 'recent'
export const RECOVERY_STORE = 'recovery'

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

/** Opens the database, creating its object stores on first use. */
function openDatabase(factory: IDBFactory): Promise<IDBDatabase> {
  return new Promise((resolve, reject) => {
    let rejected = false
    const fail = (error: Error): void => {
      rejected = true
      reject(error)
    }
    const request = factory.open(DATABASE_NAME, DATABASE_VERSION)
    request.addEventListener('upgradeneeded', () => {
      const database = request.result
      if (!database.objectStoreNames.contains(RECENT_STORE)) {
        database.createObjectStore(RECENT_STORE, { keyPath: 'id' })
      }
      if (!database.objectStoreNames.contains(RECOVERY_STORE)) {
        database.createObjectStore(RECOVERY_STORE, { keyPath: 'slot' })
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

function isInvalidState(error: unknown): boolean {
  return error instanceof DOMException && error.name === 'InvalidStateError'
}

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
        if (!isInvalidState(error)) {
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
