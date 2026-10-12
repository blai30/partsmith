// Test support: an in-memory IndexedDB for jsdom, which has none. It covers what the recent files
// and recovery stores use (ui.md 2.15): opening and upgrading a database, object stores with a key
// path, get, getAll, put and delete requests, transactions that commit once their requests have
// settled, and the versionchange, blocked and close events. Requests run one at a time in the
// order they are made, each in its own task. Values are kept as given rather than
// structured-cloned, so test file handles keep their methods.

/** The key of a record: the value its key path names. */
type Key = string | number

type StoreData = { readonly keyPath: string; readonly records: Map<Key, unknown> }

type DatabaseData = {
  version: number
  openCount: number
  readonly stores: Map<string, StoreData>
  readonly connections: Set<FakeDatabase>
  /** Upgrades waiting for the other connections to close. */
  readonly waiting: (() => void)[]
}

export type FakeIndexedDb = {
  readonly factory: IDBFactory
  /** The key path of each object store of a database, or null when the database does not exist. */
  keyPaths(databaseName: string): Readonly<Record<string, string>> | null
  /** The version of a database, or null when it does not exist. */
  version(databaseName: string): number | null
  /** The records of an object store in key order. */
  records(databaseName: string, storeName: string): readonly unknown[]
  /** Stores a record directly, as an earlier session would have. */
  putRecord(databaseName: string, storeName: string, record: unknown): void
  /** How many connections to a database have been opened. */
  openCount(databaseName: string): number
  /**
   * Closes every open connection to a database as the browser does when site data is cleared:
   * with a close event, or without one as some browsers do.
   */
  closeConnections(databaseName: string, options: { readonly event: boolean }): void
  /** Opens a database with a higher version, as a newer tab would; resolves once it is open. */
  upgradeFromAnotherTab(databaseName: string): Promise<void>
}

function domError(name: string, message: string): DOMException {
  return new DOMException(message, name)
}

function isKey(value: unknown): value is Key {
  return typeof value === 'string' || (typeof value === 'number' && !Number.isNaN(value))
}

function compareKeys(first: Key, second: Key): number {
  if (typeof first !== typeof second) {
    // IndexedDB orders numbers before strings.
    return typeof first === 'number' ? -1 : 1
  }
  if (first === second) {
    return 0
  }
  return first < second ? -1 : 1
}

function keyOf(record: unknown, keyPath: string): unknown {
  return typeof record === 'object' && record !== null ? Reflect.get(record, keyPath) : undefined
}

function sortedRecords(store: StoreData): unknown[] {
  return Array.from(store.records.entries())
    .toSorted(([first], [second]) => compareKeys(first, second))
    .map(([, record]) => record)
}

/** Runs a task in a later macrotask, after the current one and its promise reactions. */
function later(task: () => void): void {
  setTimeout(task, 0)
}

class FakeRequest extends EventTarget {
  result: unknown = undefined
  error: DOMException | null = null
  readyState: 'pending' | 'done' = 'pending'

  succeed(result: unknown): void {
    this.result = result
    this.readyState = 'done'
    this.dispatchEvent(new Event('success'))
  }

  fail(error: DOMException): void {
    this.error = error
    this.readyState = 'done'
    this.dispatchEvent(new Event('error'))
  }
}

class FakeObjectStore {
  readonly name: string
  readonly keyPath: string
  readonly #data: StoreData
  readonly #transaction: FakeTransaction

  constructor(name: string, data: StoreData, transaction: FakeTransaction) {
    this.name = name
    this.keyPath = data.keyPath
    this.#data = data
    this.#transaction = transaction
  }

  get(key: unknown): FakeRequest {
    this.#transaction.requireActive()
    const validKey = validated(key)
    return this.#transaction.request(() => this.#data.records.get(validKey))
  }

  getAll(): FakeRequest {
    this.#transaction.requireActive()
    return this.#transaction.request(() => sortedRecords(this.#data))
  }

  put(value: unknown): FakeRequest {
    this.#transaction.requireActive()
    this.#transaction.requireWritable()
    const validKey = validated(keyOf(value, this.keyPath))
    return this.#transaction.request(() => {
      this.#data.records.set(validKey, value)
      return validKey
    })
  }

  delete(key: unknown): FakeRequest {
    this.#transaction.requireActive()
    this.#transaction.requireWritable()
    const validKey = validated(key)
    return this.#transaction.request(() => {
      this.#data.records.delete(validKey)
      return undefined
    })
  }
}

function validated(key: unknown): Key {
  if (!isKey(key)) {
    throw domError('DataError', 'The key is not a valid key.')
  }
  return key
}

class FakeTransaction extends EventTarget {
  readonly db: FakeDatabase
  readonly mode: IDBTransactionMode
  error: DOMException | null = null
  readonly #storeNames: readonly string[]
  #pending = 0
  #finished = false

  constructor(db: FakeDatabase, storeNames: readonly string[], mode: IDBTransactionMode) {
    super()
    this.db = db
    this.mode = mode
    this.#storeNames = storeNames
    // A transaction that never gets a request commits after the task that created it.
    this.#commitWhenIdle()
  }

  objectStore(name: string): FakeObjectStore {
    const data = this.db.data.stores.get(name)
    if (!this.#storeNames.includes(name) || data === undefined) {
      throw domError('NotFoundError', `The object store ${name} is not in this transaction.`)
    }
    return new FakeObjectStore(name, data, this)
  }

  requireActive(): void {
    if (this.#finished) {
      throw domError('TransactionInactiveError', 'The transaction has finished.')
    }
  }

  requireWritable(): void {
    if (this.mode === 'readonly') {
      throw domError('ReadOnlyError', 'The transaction is read-only.')
    }
  }

  /** Runs the operation in a later task and succeeds the returned request with its result. */
  request(operation: () => unknown): FakeRequest {
    const request = new FakeRequest()
    this.#pending++
    later(() => {
      this.#pending--
      request.succeed(operation())
      this.#commitWhenIdle()
    })
    return request
  }

  // Commits in a later task if no request is pending then, so requests that success listeners and
  // their promise reactions place still belong to this transaction.
  #commitWhenIdle(): void {
    later(() => {
      if (this.#pending === 0 && !this.#finished) {
        this.#finished = true
        this.dispatchEvent(new Event('complete'))
      }
    })
  }
}

class FakeDatabase extends EventTarget {
  readonly name: string
  readonly data: DatabaseData
  upgrading = false
  #closed = false

  constructor(name: string, data: DatabaseData) {
    super()
    this.name = name
    this.data = data
  }

  get version(): number {
    return this.data.version
  }

  get objectStoreNames(): { readonly length: number; contains(name: string): boolean } {
    const names = Array.from(this.data.stores.keys())
    return { length: names.length, contains: (name) => names.includes(name) }
  }

  createObjectStore(name: string, options: { readonly keyPath: string }): void {
    if (!this.upgrading) {
      throw domError('InvalidStateError', 'Object stores are created only during an upgrade.')
    }
    if (this.data.stores.has(name)) {
      throw domError('ConstraintError', `The object store ${name} already exists.`)
    }
    this.data.stores.set(name, { keyPath: options.keyPath, records: new Map() })
  }

  transaction(
    storeNames: string | readonly string[],
    mode: IDBTransactionMode = 'readonly',
  ): FakeTransaction {
    if (this.#closed) {
      throw domError('InvalidStateError', 'The database connection is closing.')
    }
    const names = typeof storeNames === 'string' ? [storeNames] : storeNames
    for (const name of names) {
      if (!this.data.stores.has(name)) {
        throw domError('NotFoundError', `The object store ${name} does not exist.`)
      }
    }
    return new FakeTransaction(this, names, mode)
  }

  close(): void {
    if (this.#closed) {
      return
    }
    this.#closed = true
    this.data.connections.delete(this)
    if (this.data.connections.size === 0) {
      for (const upgrade of this.data.waiting.splice(0)) {
        later(upgrade)
      }
    }
  }
}

/** Creates an empty in-memory IndexedDB; its `factory` stands in for `window.indexedDB`. */
export function createFakeIndexedDb(): FakeIndexedDb {
  const databases = new Map<string, DatabaseData>()

  function databaseData(name: string): DatabaseData {
    const existing = databases.get(name)
    if (existing !== undefined) {
      return existing
    }
    const created: DatabaseData = {
      version: 0,
      openCount: 0,
      stores: new Map(),
      connections: new Set(),
      waiting: [],
    }
    databases.set(name, created)
    return created
  }

  function connect(request: FakeRequest, name: string, data: DatabaseData, version: number): void {
    const database = new FakeDatabase(name, data)
    data.connections.add(database)
    data.openCount++
    request.result = database
    if (version > data.version) {
      data.version = version
      database.upgrading = true
      request.dispatchEvent(new Event('upgradeneeded'))
      database.upgrading = false
    }
    request.succeed(database)
  }

  function open(name: string, version?: number): FakeRequest {
    const request = new FakeRequest()
    later(() => {
      const data = databaseData(name)
      const requested = version ?? Math.max(data.version, 1)
      if (requested < data.version) {
        request.fail(domError('VersionError', 'The requested version is lower than the current.'))
        return
      }
      if (requested > data.version && data.connections.size > 0) {
        for (const connection of Array.from(data.connections)) {
          connection.dispatchEvent(new Event('versionchange'))
        }
        if (data.connections.size > 0) {
          // The upgrade waits until the last other connection closes.
          data.waiting.push(() => {
            connect(request, name, data, requested)
          })
          request.dispatchEvent(new Event('blocked'))
          return
        }
      }
      connect(request, name, data, requested)
    })
    return request
  }

  function unsupported(): never {
    throw domError('NotSupportedError', 'The fake IndexedDB only opens databases.')
  }

  // The fake has the members the stores use, not all of IDBFactory's.
  const factory = { open, deleteDatabase: unsupported, cmp: unsupported, databases: unsupported }

  return {
    factory: factory as unknown as IDBFactory,
    keyPaths(databaseName) {
      const data = databases.get(databaseName)
      if (data === undefined) {
        return null
      }
      return Object.fromEntries(
        Array.from(data.stores.entries(), ([storeName, store]) => [storeName, store.keyPath]),
      )
    },
    version(databaseName) {
      return databases.get(databaseName)?.version ?? null
    },
    records(databaseName, storeName) {
      const store = databases.get(databaseName)?.stores.get(storeName)
      return store === undefined ? [] : sortedRecords(store)
    },
    putRecord(databaseName, storeName, record) {
      const store = databases.get(databaseName)?.stores.get(storeName)
      if (store === undefined) {
        throw new Error(`The object store ${storeName} of ${databaseName} does not exist.`)
      }
      store.records.set(validated(keyOf(record, store.keyPath)), record)
    },
    openCount(databaseName) {
      return databases.get(databaseName)?.openCount ?? 0
    },
    closeConnections(databaseName, options) {
      for (const connection of Array.from(databases.get(databaseName)?.connections ?? [])) {
        connection.close()
        if (options.event) {
          connection.dispatchEvent(new Event('close'))
        }
      }
    },
    upgradeFromAnotherTab(databaseName) {
      const request = open(databaseName, databaseData(databaseName).version + 1)
      return new Promise((resolve, reject) => {
        request.addEventListener('success', () => {
          if (request.result instanceof FakeDatabase) {
            request.result.close()
          }
          resolve()
        })
        request.addEventListener('error', () => {
          reject(request.error ?? new Error('The upgrade failed.'))
        })
      })
    },
  }
}
