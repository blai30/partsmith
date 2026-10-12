import { describe, expect, it, vi } from 'vitest'
import { createFakeIndexedDb, type FakeIndexedDb } from '../test/fakes/indexed-db'
import {
  createConnection,
  DATABASE_NAME,
  RECENT_STORE,
  RECOVERY_STORE,
  requestResult,
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

describe('createConnection', () => {
  it('opens the database partsmith at version 1 with the stores recent and recovery', async () => {
    const fake = createFakeIndexedDb()
    await createConnection(fake.factory).transaction(RECENT_STORE, 'readonly')
    expect(DATABASE_NAME).toBe('partsmith')
    expect(fake.version('partsmith')).toBe(1)
    expect(fake.keyPaths('partsmith')).toEqual({ recent: 'id', recovery: 'slot' })
  })

  it('opens an existing database without touching its records', async () => {
    const fake = createFakeIndexedDb()
    await createConnection(fake.factory).transaction(RECOVERY_STORE, 'readonly')
    fake.putRecord('partsmith', 'recovery', { slot: 'current', name: 'bracket' })
    await createConnection(fake.factory).transaction(RECOVERY_STORE, 'readonly')
    expect(fake.records('partsmith', 'recovery')).toEqual([{ slot: 'current', name: 'bracket' }])
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
    expect(fake.version('partsmith')).toBe(2)
    // This tab's version 1 can no longer open the upgraded database.
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
      requestResult(store.put({ slot: 'current', name: 'bracket' })),
    )
    expect(key).toBe('current')
    expect(fake.records('partsmith', 'recovery')).toEqual([{ slot: 'current', name: 'bracket' }])
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
