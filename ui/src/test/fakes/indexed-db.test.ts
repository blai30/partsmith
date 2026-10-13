import { afterEach, describe, expect, it, vi } from 'vitest'
import { createFakeIndexedDb } from './indexed-db'

/** Opens a database, creating the store `items` (key path `id`) on upgrade. */
function open(factory: IDBFactory, version?: number): Promise<IDBDatabase> {
  return new Promise((resolve, reject) => {
    const request = factory.open('test', version)
    request.addEventListener('upgradeneeded', () => {
      if (!request.result.objectStoreNames.contains('items')) {
        request.result.createObjectStore('items', { keyPath: 'id' })
      }
    })
    request.addEventListener('success', () => {
      resolve(request.result)
    })
    request.addEventListener('error', () => {
      reject(request.error ?? new Error('The open failed.'))
    })
  })
}

function result<Result>(request: IDBRequest<Result>): Promise<Result> {
  return new Promise((resolve) => {
    request.addEventListener('success', () => {
      resolve(request.result)
    })
  })
}

function completed(transaction: IDBTransaction): Promise<void> {
  return new Promise((resolve) => {
    transaction.addEventListener('complete', () => {
      resolve()
    })
  })
}

afterEach(() => {
  vi.useRealTimers()
})

describe('createFakeIndexedDb', () => {
  it('runs put, get, getAll and delete, with getAll in key order', async () => {
    const fake = createFakeIndexedDb()
    const database = await open(fake.factory)
    const store = database.transaction('items', 'readwrite').objectStore('items')
    await result(store.put({ id: 'b', value: 2 }))
    await result(store.put({ id: 'a', value: 1 }))
    expect(await result(store.get('b'))).toEqual({ id: 'b', value: 2 })
    expect(await result(store.getAll())).toEqual([
      { id: 'a', value: 1 },
      { id: 'b', value: 2 },
    ])
    await result(store.delete('a'))
    expect(fake.records('test', 'items')).toEqual([{ id: 'b', value: 2 }])
  })

  it('commits a transaction only after the requests its success reactions place', async () => {
    const fake = createFakeIndexedDb()
    const database = await open(fake.factory)
    const transaction = database.transaction('items', 'readwrite')
    const done = completed(transaction)
    const store = transaction.objectStore('items')
    await result(store.put({ id: 'a' }))
    await result(store.put({ id: 'b' }))
    await done
    expect(fake.records('test', 'items')).toHaveLength(2)
    expect(() => store.put({ id: 'c' })).toThrow(
      expect.objectContaining({ name: 'TransactionInactiveError' }),
    )
  })

  it('rejects writes in a read-only transaction and records without their key', async () => {
    const fake = createFakeIndexedDb()
    const database = await open(fake.factory)
    const readOnly = database.transaction('items', 'readonly').objectStore('items')
    expect(() => readOnly.put({ id: 'a' })).toThrow(
      expect.objectContaining({ name: 'ReadOnlyError' }),
    )
    const writable = database.transaction('items', 'readwrite').objectStore('items')
    expect(() => writable.put({ name: 'a' })).toThrow(
      expect.objectContaining({ name: 'DataError' }),
    )
  })

  it('creates and deletes object stores only during an upgrade', async () => {
    const fake = createFakeIndexedDb()
    const database = await open(fake.factory)
    expect(() => database.createObjectStore('more', { keyPath: 'id' })).toThrow(
      expect.objectContaining({ name: 'InvalidStateError' }),
    )
    expect(() => {
      database.deleteObjectStore('items')
    }).toThrow(expect.objectContaining({ name: 'InvalidStateError' }))
    expect(fake.keyPaths('test')).toEqual({ items: 'id' })
  })

  it('runs an upgrade in a versionchange transaction that reads, deletes and creates stores before the open succeeds', async () => {
    const fake = createFakeIndexedDb()
    fake.createDatabase('test', 1, { items: 'slot' })
    fake.putRecord('test', 'items', { slot: 'current', value: 7 })
    const versions: [number, number][] = []
    const request = fake.factory.open('test', 2)
    request.addEventListener('upgradeneeded', (event) => {
      // jsdom has no IDBVersionChangeEvent, so the versions are read by name.
      versions.push([
        Number(Reflect.get(event, 'oldVersion')),
        Number(Reflect.get(event, 'newVersion')),
      ])
      const database = request.result
      const old = request.transaction?.objectStore('items').get('current')
      old?.addEventListener('success', () => {
        database.deleteObjectStore('items')
        const items = database.createObjectStore('items', { keyPath: 'id' })
        items.put({ id: 'a', value: Reflect.get(Object(old.result), 'value') })
      })
    })
    const database = await result(request)
    expect(versions).toEqual([[1, 2]])
    expect(request.transaction).toBeNull()
    expect(database.version).toBe(2)
    expect(fake.keyPaths('test')).toEqual({ items: 'id' })
    expect(fake.records('test', 'items')).toEqual([{ id: 'a', value: 7 }])
  })

  it('creates a database as an earlier build left it, with its version and key paths', () => {
    const fake = createFakeIndexedDb()
    fake.createDatabase('test', 1, { recent: 'id', recovery: 'slot' })
    fake.putRecord('test', 'recovery', { slot: 'current' })
    expect(fake.version('test')).toBe(1)
    expect(fake.keyPaths('test')).toEqual({ recent: 'id', recovery: 'slot' })
    expect(fake.records('test', 'recovery')).toEqual([{ slot: 'current' }])
    expect(() => {
      fake.createDatabase('test', 2, {})
    }).toThrow('The database test exists already.')
  })

  it('runs requests on the real clock, so they finish while a test uses fake timers', async () => {
    vi.useFakeTimers()
    const fake = createFakeIndexedDb()
    const database = await open(fake.factory)
    const store = database.transaction('items', 'readwrite').objectStore('items')
    await result(store.put({ id: 'a' }))
    expect(await result(store.get('a'))).toEqual({ id: 'a' })
  })

  it('fails to open a database at a lower version than it has', async () => {
    const fake = createFakeIndexedDb()
    const newer = await open(fake.factory, 2)
    newer.close()
    await expect(open(fake.factory, 1)).rejects.toMatchObject({ name: 'VersionError' })
  })

  it('blocks an upgrade while a connection stays open, and runs it once that connection closes', async () => {
    const fake = createFakeIndexedDb()
    const database = await open(fake.factory, 1)
    const versionChange = vi.fn<() => void>()
    database.addEventListener('versionchange', versionChange)
    const blocked = vi.fn<() => void>()
    const request = fake.factory.open('test', 2)
    request.addEventListener('blocked', blocked)
    const upgraded = result(request)
    await vi.waitFor(() => {
      expect(blocked).toHaveBeenCalledTimes(1)
    })
    expect(versionChange).toHaveBeenCalledTimes(1)
    expect(fake.version('test')).toBe(1)
    database.close()
    await upgraded
    expect(fake.version('test')).toBe(2)
  })

  it('closes connections as the browser does, with or without a close event', async () => {
    const fake = createFakeIndexedDb()
    const first = await open(fake.factory)
    const second = await open(fake.factory)
    const closed = vi.fn<() => void>()
    first.addEventListener('close', closed)
    fake.closeConnections('test', { event: true })
    expect(closed).toHaveBeenCalledTimes(1)
    expect(() => second.transaction('items')).toThrow(
      expect.objectContaining({ name: 'InvalidStateError' }),
    )
    expect(fake.openCount('test')).toBe(2)
  })
})
