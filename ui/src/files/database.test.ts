import { describe, expect, it, vi } from 'vitest'
import { createFakeIndexedDb, type FakeIndexedDb } from '../test/fakes/indexed-db'
import {
  createConnection,
  createStoreAccess,
  DATABASE_NAME,
  RECENT_STORE,
  RECOVERY_STORE,
  requestResult,
  upgradeDatabase,
  withStore,
} from './database'

// A factory whose first open throws, as a browser that blocks storage until the user allows it.
function failingOnce(fake: FakeIndexedDb): IDBFactory {
  let opened = false
  return {
    ...fake.factory,
    open: (name: string, version?: number) => {
      if (!opened) {
        opened = true
        throw new DOMException('Storage is blocked.', 'SecurityError')
      }
      return fake.factory.open(name, version)
    },
  }
}

describe('upgradeDatabase (2.15)', () => {
  it('U-170 keeps the recovery entries of a version 2 database when a later version upgrades it', async () => {
    const fake = createFakeIndexedDb()
    fake.createDatabase(DATABASE_NAME, 2, { recent: 'id', recovery: 'id' })
    const entry = {
      id: 'a',
      name: 'bracket',
      text: '{}',
      savedAt: 1000,
      handle: null,
      reason: 'autosave',
    }
    fake.putRecord(DATABASE_NAME, RECOVERY_STORE, entry)
    // Version 3 stands in for a later build whose upgrade runs this one's steps first.
    const request = fake.factory.open(DATABASE_NAME, 3)
    request.addEventListener('upgradeneeded', (event) => {
      if (request.transaction !== null) {
        upgradeDatabase(request.result, request.transaction, event.oldVersion)
      }
    })
    const database = await requestResult(request)
    database.close()
    expect(fake.version(DATABASE_NAME)).toBe(3)
    expect(fake.keyPaths(DATABASE_NAME)).toEqual({ recent: 'id', recovery: 'id' })
    expect(fake.records(DATABASE_NAME, RECOVERY_STORE)).toEqual([entry])
  })
})

describe('createConnection', () => {
  it('U-155 opens the database partsmith at version 2 with the stores recent and recovery, both keyed by id', async () => {
    const fake = createFakeIndexedDb()
    await createConnection(fake.factory).transaction(RECENT_STORE, 'readonly')
    expect(DATABASE_NAME).toBe('partsmith')
    expect(fake.version('partsmith')).toBe(2)
    expect(fake.keyPaths('partsmith')).toEqual({ recent: 'id', recovery: 'id' })
  })

  it('opens an existing database without touching its records', async () => {
    const fake = createFakeIndexedDb()
    await createConnection(fake.factory).transaction(RECOVERY_STORE, 'readonly')
    fake.putRecord('partsmith', 'recovery', { id: 'a', name: 'bracket' })
    await createConnection(fake.factory).transaction(RECOVERY_STORE, 'readonly')
    expect(fake.records('partsmith', 'recovery')).toEqual([{ id: 'a', name: 'bracket' }])
  })

  it('shares one connection between transactions', async () => {
    const fake = createFakeIndexedDb()
    const connection = createConnection(fake.factory)
    await connection.transaction(RECENT_STORE, 'readonly')
    await connection.transaction(RECOVERY_STORE, 'readwrite')
    expect(fake.openCount('partsmith')).toBe(1)
  })

  it('opens the database again after a failed open', async () => {
    const fake = createFakeIndexedDb()
    const connection = createConnection(failingOnce(fake))
    await expect(connection.transaction(RECENT_STORE, 'readonly')).rejects.toThrow(
      'Storage is blocked.',
    )
    await expect(connection.transaction(RECENT_STORE, 'readonly')).resolves.toBeDefined()
    expect(fake.openCount('partsmith')).toBe(1)
  })

  it('opens a new connection after the browser closes the connection with a close event', async () => {
    const fake = createFakeIndexedDb()
    const connection = createConnection(fake.factory)
    const closed = (await connection.transaction(RECENT_STORE, 'readonly')).db
    const transaction = vi.spyOn(closed, 'transaction')
    fake.closeConnections('partsmith', { event: true })
    const next = await connection.transaction(RECENT_STORE, 'readonly')
    expect(next.db).not.toBe(closed)
    expect(transaction).not.toHaveBeenCalled()
    expect(fake.openCount('partsmith')).toBe(2)
  })

  it('opens a new connection when the browser closed the connection without a close event', async () => {
    const fake = createFakeIndexedDb()
    const connection = createConnection(fake.factory)
    const closed = (await connection.transaction(RECENT_STORE, 'readonly')).db
    fake.closeConnections('partsmith', { event: false })
    const next = await connection.transaction(RECENT_STORE, 'readonly')
    expect(next.db).not.toBe(closed)
    // The new connection is kept for later transactions.
    expect((await connection.transaction(RECENT_STORE, 'readonly')).db).toBe(next.db)
    expect(fake.openCount('partsmith')).toBe(2)
  })

  it('closes its connection when another tab upgrades the database, so that tab is not blocked', async () => {
    const fake = createFakeIndexedDb()
    const connection = createConnection(fake.factory)
    await connection.transaction(RECENT_STORE, 'readonly')
    await fake.upgradeFromAnotherTab('partsmith')
    expect(fake.version('partsmith')).toBe(3)
    // This tab's version 2 can no longer open the upgraded database.
    await expect(connection.transaction(RECENT_STORE, 'readonly')).rejects.toMatchObject({
      name: 'VersionError',
    })
  })
})

describe('withStore', () => {
  it('resolves with the result of the operation once the transaction has committed', async () => {
    const fake = createFakeIndexedDb()
    const connection = createConnection(fake.factory)
    const key = await withStore(connection, RECOVERY_STORE, 'readwrite', (store) =>
      requestResult(store.put({ id: 'a', name: 'bracket' })),
    )
    expect(key).toBe('a')
    expect(fake.records('partsmith', 'recovery')).toEqual([{ id: 'a', name: 'bracket' }])
  })

  it('runs every request the operation places, including those placed after a result', async () => {
    const fake = createFakeIndexedDb()
    const connection = createConnection(fake.factory)
    await withStore(connection, RECENT_STORE, 'readwrite', async (store) => {
      await requestResult(store.put({ id: 'a' }))
      await requestResult(store.put({ id: 'b' }))
    })
    expect(fake.records('partsmith', 'recent')).toEqual([{ id: 'a' }, { id: 'b' }])
  })

  it('rejects with the error of a failing operation', async () => {
    const fake = createFakeIndexedDb()
    const connection = createConnection(fake.factory)
    await expect(
      withStore(connection, RECENT_STORE, 'readonly', (store) =>
        requestResult(store.put({ id: 'a' })),
      ),
    ).rejects.toMatchObject({ name: 'ReadOnlyError' })
  })
})

describe('createStoreAccess', () => {
  it('resolves with the result of the operation', async () => {
    const fake = createFakeIndexedDb()
    const run = createStoreAccess(fake.factory, RECOVERY_STORE, 'The recovery store failed.')
    const key = await run(
      'readwrite',
      (store) => requestResult(store.put({ id: 'a', name: 'bracket' })),
      null,
    )
    expect(key).toBe('a')
  })

  it('warns with its message and resolves with the fallback when the operation fails', async () => {
    const warn = vi.spyOn(console, 'warn').mockImplementation(() => undefined)
    const fake = createFakeIndexedDb()
    const run = createStoreAccess(fake.factory, RECENT_STORE, 'The recent store failed.')
    const result = await run('readonly', (store) => requestResult(store.put({ id: 'a' })), 'none')
    expect(result).toBe('none')
    expect(warn).toHaveBeenCalledWith('The recent store failed.', expect.anything())
    warn.mockRestore()
  })
})
