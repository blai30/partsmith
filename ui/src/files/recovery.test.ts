import { afterEach, describe, expect, it, vi } from 'vitest'
import { createFakeIndexedDb, type FakeIndexedDb } from '../test/fakes/indexed-db'
import { createFakeLocks } from '../test/fakes/locks'
import { settle } from '../test/fakes/settle'
import { createFakeStorage } from '../test/fakes/storage'
import type { DesktopFileHandle } from './file-service'
import { createIndexedDbRecentFiles } from './recent'
import {
  createIndexedDbRecovery,
  createMemoryRecovery,
  type RecoveryEnvironment,
  type RecoveryEntry,
  type RecoveryStore,
} from './recovery'

// The recovery store of ui.md 2.15: one entry per document session in IndexedDB version 2, the
// migration of the version 1 slot, the Web Locks of running sessions and persistent storage.

const ENTRY: RecoveryEntry = {
  id: 'a',
  name: 'bracket',
  text: '{"format": 1}',
  savedAt: 1000,
  handle: null,
  reason: 'autosave',
}

const SECOND: RecoveryEntry = {
  id: 'b',
  name: 'lid',
  text: '{"format": 2}',
  savedAt: 2000,
  handle: null,
  reason: 'autosave',
}

/** No Web Locks and no storage manager: every entry is listed. */
const BARE: RecoveryEnvironment = { locks: null, storage: null }

const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/

function blocked(): never {
  throw new DOMException('The user denied permission to access the database.', 'SecurityError')
}

// An IDBFactory whose every call fails, as in a private window that blocks IndexedDB.
function blockedFactory(): IDBFactory {
  return {
    open: blocked,
    deleteDatabase: blocked,
    cmp: blocked,
    databases: () => Promise.reject(new DOMException('Blocked.', 'SecurityError')),
  }
}

/** A version 1 database as the one-slot build left it: `recent` and the `recovery` slot store. */
function versionOneDatabase(slotRecord: unknown): FakeIndexedDb {
  const fake = createFakeIndexedDb()
  fake.createDatabase('partsmith', 1, { recent: 'id', recovery: 'slot' })
  fake.putRecord('partsmith', 'recent', {
    id: 'r-1',
    name: 'cube.ptsm',
    openedAt: 500,
    handle: null,
    text: '{}',
  })
  fake.putRecord('partsmith', 'recovery', slotRecord)
  return fake
}

/** Writes every entry, one after another. */
async function writeAll(store: RecoveryStore, entries: readonly RecoveryEntry[]): Promise<void> {
  for (const entry of entries) {
    // The order of the writes is part of the setup.
    // oxlint-disable-next-line no-await-in-loop
    await store.write(entry)
  }
}

afterEach(() => {
  vi.restoreAllMocks()
})

describe('createMemoryRecovery', () => {
  it('lists nothing before anything is written', async () => {
    await expect(createMemoryRecovery().list()).resolves.toEqual([])
  })

  it('U-156 lists every entry it holds, newest savedAt first', async () => {
    const recovery = createMemoryRecovery()
    await writeAll(recovery, [ENTRY, SECOND])
    await expect(recovery.list()).resolves.toEqual([SECOND, ENTRY])
  })

  it('replaces only the entry with the same id, and removes only the entry with the id', async () => {
    const recovery = createMemoryRecovery()
    await writeAll(recovery, [ENTRY, SECOND])
    const replaced: RecoveryEntry = { ...ENTRY, text: '{"format": 3}', reason: 'replaced' }
    await recovery.write(replaced)
    await expect(recovery.list()).resolves.toEqual([SECOND, replaced])
    await recovery.remove('a')
    await expect(recovery.list()).resolves.toEqual([SECOND])
  })
})

describe('createIndexedDbRecovery', () => {
  it('resolves empty and without effect when IndexedDB fails, and warns naming the store', async () => {
    const warn = vi.spyOn(console, 'warn').mockImplementation(() => undefined)
    const recovery = createIndexedDbRecovery(blockedFactory(), BARE)
    await expect(recovery.list()).resolves.toEqual([])
    await expect(recovery.write(ENTRY)).resolves.toBeUndefined()
    await expect(recovery.remove('a')).resolves.toBeUndefined()
    expect(warn).toHaveBeenCalled()
    expect(warn.mock.calls.flat().map(String).join(' ')).toMatch(/recovery/i)
  })

  it('U-155 a new database has version 2 and the stores recent and recovery, both keyed by id', async () => {
    const fake = createFakeIndexedDb()
    await expect(createIndexedDbRecovery(fake.factory, BARE).list()).resolves.toEqual([])
    expect(fake.version('partsmith')).toBe(2)
    expect(fake.keyPaths('partsmith')).toEqual({ recent: 'id', recovery: 'id' })
  })

  it('U-155 keeps one record per entry: writing an entry again replaces only it, and remove deletes only it', async () => {
    const fake = createFakeIndexedDb()
    const recovery = createIndexedDbRecovery(fake.factory, BARE)
    await writeAll(recovery, [ENTRY, SECOND])
    expect(fake.records('partsmith', 'recovery')).toEqual([ENTRY, SECOND])
    const rewritten: RecoveryEntry = { ...ENTRY, text: '{"format": 3}' }
    await recovery.write(rewritten)
    await expect(recovery.list()).resolves.toEqual([SECOND, rewritten])
    await recovery.remove('a')
    await expect(recovery.list()).resolves.toEqual([SECOND])
    expect(fake.records('partsmith', 'recovery')).toEqual([SECOND])
  })

  it('U-155 skips a record without text when listing, and leaves it in the store', async () => {
    const fake = createFakeIndexedDb()
    const recovery = createIndexedDbRecovery(fake.factory, BARE)
    await recovery.write(ENTRY)
    const broken = { id: 'c', name: 'broken', savedAt: 3000, handle: null, reason: 'autosave' }
    fake.putRecord('partsmith', 'recovery', broken)
    await expect(recovery.list()).resolves.toEqual([ENTRY])
    expect(fake.records('partsmith', 'recovery')).toEqual([ENTRY, broken])
  })

  it.each([
    ['a reason that is not autosave or replaced', { reason: 'crash' }],
    ['text that is not a string', { text: 42 }],
    ['a handle that is not a file handle', { handle: { kind: 'directory' } }],
    ['no savedAt', { savedAt: undefined }],
    ['no name', { name: undefined }],
  ])('skips a stored record with %s', async (_, change) => {
    const fake = createFakeIndexedDb()
    const recovery = createIndexedDbRecovery(fake.factory, BARE)
    await recovery.write(SECOND)
    fake.putRecord('partsmith', 'recovery', { ...ENTRY, ...change })
    await expect(recovery.list()).resolves.toEqual([SECOND])
  })

  it('keeps the entries for a later session', async () => {
    const fake = createFakeIndexedDb()
    await writeAll(createIndexedDbRecovery(fake.factory, BARE), [ENTRY, SECOND])
    await expect(createIndexedDbRecovery(fake.factory, BARE).list()).resolves.toEqual([
      SECOND,
      ENTRY,
    ])
  })

  it('desktop test 47: an entry with a desktop handle round-trips unchanged', async () => {
    const fake = createFakeIndexedDb()
    const handle: DesktopFileHandle = { kind: 'desktop', name: 'bracket.ptsm', grant: 'g1' }
    const entry: RecoveryEntry = { ...ENTRY, handle, reason: 'replaced' }
    await createIndexedDbRecovery(fake.factory, BARE).write(entry)
    await expect(createIndexedDbRecovery(fake.factory, BARE).list()).resolves.toEqual([
      {
        ...ENTRY,
        handle: { kind: 'desktop', name: 'bracket.ptsm', grant: 'g1' },
        reason: 'replaced',
      },
    ])
  })
})

describe('the upgrade from version 1 (2.15)', () => {
  /** The fields of the version 1 record that the new entry keeps. */
  const KEPT = { name: 'bracket', text: '{}', savedAt: 1000, handle: null, reason: 'replaced' }

  it('U-155 moves the one slot record to a new UUID id, keyed by id, and leaves recent files alone', async () => {
    const fake = versionOneDatabase({ slot: 'current', ...KEPT })
    const recent = fake.records('partsmith', 'recent')
    const listed = await createIndexedDbRecovery(fake.factory, BARE).list()
    expect(fake.version('partsmith')).toBe(2)
    expect(fake.keyPaths('partsmith')).toEqual({ recent: 'id', recovery: 'id' })
    const records = fake.records('partsmith', 'recovery')
    expect(records).toEqual([{ ...KEPT, id: expect.stringMatching(UUID) }])
    expect(listed).toEqual(records)
    expect(fake.records('partsmith', 'recent')).toEqual(recent)
  })

  it('U-155 drops a version 1 slot record without text, leaving the new store empty', async () => {
    const fake = versionOneDatabase({
      slot: 'current',
      name: 'bracket',
      savedAt: 1000,
      handle: null,
      reason: 'replaced',
    })
    await expect(createIndexedDbRecovery(fake.factory, BARE).list()).resolves.toEqual([])
    expect(fake.version('partsmith')).toBe(2)
    expect(fake.keyPaths('partsmith')).toEqual({ recent: 'id', recovery: 'id' })
    expect(fake.records('partsmith', 'recovery')).toEqual([])
  })

  it('U-155 migrates the slot record whichever store opens the database first', async () => {
    const fake = versionOneDatabase({ slot: 'current', ...KEPT })
    // The upgrade belongs to the database: the recent files store's first use runs it too.
    await createIndexedDbRecentFiles(fake.factory).list()
    expect(fake.version('partsmith')).toBe(2)
    await expect(createIndexedDbRecovery(fake.factory, BARE).list()).resolves.toMatchObject([
      { name: 'bracket', text: '{}', savedAt: 1000, reason: 'replaced' },
    ])
  })
})

describe('running sessions (2.15)', () => {
  it('U-156 another page leaves out an autosave entry while its page holds it, and lists it once it is written as replaced', async () => {
    const fake = createFakeIndexedDb()
    const locks = createFakeLocks()
    const pageA = locks.page()
    const pageB = locks.page()
    const storeA = createIndexedDbRecovery(fake.factory, { locks: pageA.locks, storage: null })
    const storeB = createIndexedDbRecovery(fake.factory, { locks: pageB.locks, storage: null })
    await storeA.write(ENTRY)
    await settle()
    expect(locks.held()).toEqual(['partsmith-recovery-a'])
    await expect(storeB.list()).resolves.toEqual([])

    const replaced: RecoveryEntry = { ...ENTRY, reason: 'replaced' }
    await storeA.write(replaced)
    await settle()
    expect(locks.held()).toEqual([])
    await expect(storeB.list()).resolves.toEqual([replaced])

    await storeA.remove('a')
    await expect(storeB.list()).resolves.toEqual([])
    expect(fake.records('partsmith', 'recovery')).toEqual([])
  })

  it('U-156 a page that ends while it holds an entry releases it to the other pages', async () => {
    const fake = createFakeIndexedDb()
    const locks = createFakeLocks()
    const pageA = locks.page()
    const pageB = locks.page()
    const storeA = createIndexedDbRecovery(fake.factory, { locks: pageA.locks, storage: null })
    const storeB = createIndexedDbRecovery(fake.factory, { locks: pageB.locks, storage: null })
    await storeA.write(ENTRY)
    await settle()
    await expect(storeB.list()).resolves.toEqual([])
    pageA.end()
    await settle()
    await expect(storeB.list()).resolves.toEqual([ENTRY])
  })

  it('U-156 removing an autosave entry releases its lock', async () => {
    const fake = createFakeIndexedDb()
    const locks = createFakeLocks()
    const recovery = createIndexedDbRecovery(fake.factory, {
      locks: locks.page().locks,
      storage: null,
    })
    await recovery.write(ENTRY)
    await settle()
    expect(locks.held()).toEqual(['partsmith-recovery-a'])
    await recovery.remove('a')
    await settle()
    expect(locks.held()).toEqual([])
  })

  it('U-156 writing an entry as autosave twice requests one lock', async () => {
    const fake = createFakeIndexedDb()
    const locks = createFakeLocks()
    const page = locks.page()
    const recovery = createIndexedDbRecovery(fake.factory, { locks: page.locks, storage: null })
    await recovery.write(ENTRY)
    await recovery.write({ ...ENTRY, text: '{"format": 3}', savedAt: 1500 })
    await settle()
    expect(page.requested).toEqual(['partsmith-recovery-a'])
    expect(locks.held()).toEqual(['partsmith-recovery-a'])
  })

  it('U-156 a store without locks lists every entry, including one another page holds', async () => {
    const fake = createFakeIndexedDb()
    const locks = createFakeLocks()
    await createIndexedDbRecovery(fake.factory, { locks: locks.page().locks, storage: null }).write(
      ENTRY,
    )
    await settle()
    await expect(createIndexedDbRecovery(fake.factory, BARE).list()).resolves.toEqual([ENTRY])
  })

  it('U-156 a store whose lock manager cannot be queried lists every entry', async () => {
    const fake = createFakeIndexedDb()
    const locks = createFakeLocks()
    await createIndexedDbRecovery(fake.factory, { locks: locks.page().locks, storage: null }).write(
      ENTRY,
    )
    await settle()
    const failing = locks.page({ failQuery: true })
    const recovery = createIndexedDbRecovery(fake.factory, { locks: failing.locks, storage: null })
    await expect(recovery.list()).resolves.toEqual([ENTRY])
  })
})

describe('persistent storage (2.15)', () => {
  it("U-156 the page's first write asks once for persistence when the storage is not persisted", async () => {
    const fake = createFakeIndexedDb()
    const storage = createFakeStorage({ persisted: false })
    const recovery = createIndexedDbRecovery(fake.factory, {
      locks: null,
      storage: storage.storage,
    })
    await recovery.write(ENTRY)
    await settle()
    expect(storage.calls).toEqual({ persisted: 1, persist: 1 })
    await recovery.write(SECOND)
    await recovery.write({ ...ENTRY, reason: 'replaced' })
    await settle()
    expect(storage.calls).toEqual({ persisted: 1, persist: 1 })
  })

  it('U-156 never asks for persistence when the storage is persisted already', async () => {
    const fake = createFakeIndexedDb()
    const storage = createFakeStorage({ persisted: true })
    const recovery = createIndexedDbRecovery(fake.factory, {
      locks: null,
      storage: storage.storage,
    })
    await writeAll(recovery, [ENTRY, SECOND])
    await settle()
    expect(storage.calls.persist).toBe(0)
  })

  it('U-156 a refused persist() leaves the write done', async () => {
    const fake = createFakeIndexedDb()
    const storage = createFakeStorage({ persisted: false, persistFails: true })
    const recovery = createIndexedDbRecovery(fake.factory, {
      locks: null,
      storage: storage.storage,
    })
    await expect(recovery.write(ENTRY)).resolves.toBeUndefined()
    await settle()
    expect(storage.calls.persist).toBe(1)
    await expect(recovery.list()).resolves.toEqual([ENTRY])
  })

  it('U-156 writes work without a storage manager', async () => {
    const fake = createFakeIndexedDb()
    const recovery = createIndexedDbRecovery(fake.factory, BARE)
    await recovery.write(ENTRY)
    await expect(recovery.list()).resolves.toEqual([ENTRY])
  })
})
