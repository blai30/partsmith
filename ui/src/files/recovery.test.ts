import { afterEach, describe, expect, it, vi } from 'vitest'
import { createFakeIndexedDb } from '../test/fakes/indexed-db'
import { createIndexedDbRecovery, createMemoryRecovery, type RecoveryEntry } from './recovery'

const ENTRY: RecoveryEntry = {
  name: 'bracket',
  text: '{"format": 1}',
  savedAt: 1000,
  handle: null,
  reason: 'autosave',
}

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

afterEach(() => {
  vi.restoreAllMocks()
})

describe('createMemoryRecovery', () => {
  it('reads null before anything is written', async () => {
    expect(await createMemoryRecovery().read()).toBeNull()
  })

  it('reads back the entry it wrote', async () => {
    const recovery = createMemoryRecovery()
    await recovery.write(ENTRY)
    expect(await recovery.read()).toEqual(ENTRY)
  })

  it('keeps one entry: a second write replaces the first', async () => {
    const recovery = createMemoryRecovery()
    await recovery.write(ENTRY)
    const replaced: RecoveryEntry = {
      ...ENTRY,
      text: '{"format": 2}',
      savedAt: 2000,
      reason: 'replaced',
    }
    await recovery.write(replaced)
    expect(await recovery.read()).toEqual(replaced)
  })

  it('reads null after clear', async () => {
    const recovery = createMemoryRecovery()
    await recovery.write(ENTRY)
    await recovery.clear()
    expect(await recovery.read()).toBeNull()
  })
})

describe('createIndexedDbRecovery', () => {
  it('resolves empty and without effect when IndexedDB fails, and warns naming the store', async () => {
    const warn = vi.spyOn(console, 'warn').mockImplementation(() => undefined)
    const recovery = createIndexedDbRecovery(blockedFactory())
    expect(await recovery.read()).toBeNull()
    await expect(recovery.write(ENTRY)).resolves.toBeUndefined()
    await expect(recovery.clear()).resolves.toBeUndefined()
    expect(warn).toHaveBeenCalled()
    expect(warn.mock.calls.flat().map(String).join(' ')).toMatch(/recovery/i)
  })

  it('reads null before anything is written', async () => {
    expect(await createIndexedDbRecovery(createFakeIndexedDb().factory).read()).toBeNull()
  })

  it('writes the entry as the one record with slot current and reads it back', async () => {
    const fake = createFakeIndexedDb()
    const recovery = createIndexedDbRecovery(fake.factory)
    await recovery.write(ENTRY)
    expect(fake.records('partsmith', 'recovery')).toEqual([{ ...ENTRY, slot: 'current' }])
    expect(await recovery.read()).toEqual(ENTRY)
  })

  it('keeps one record: a second write replaces the first', async () => {
    const fake = createFakeIndexedDb()
    const recovery = createIndexedDbRecovery(fake.factory)
    const handle = { kind: 'file', name: 'bracket.ptsm' } as unknown as FileSystemFileHandle
    const replaced: RecoveryEntry = { ...ENTRY, savedAt: 2000, handle, reason: 'replaced' }
    await recovery.write(ENTRY)
    await recovery.write(replaced)
    expect(fake.records('partsmith', 'recovery')).toHaveLength(1)
    expect(await recovery.read()).toEqual(replaced)
  })

  it('clear deletes the record', async () => {
    const fake = createFakeIndexedDb()
    const recovery = createIndexedDbRecovery(fake.factory)
    await recovery.write(ENTRY)
    await recovery.clear()
    expect(fake.records('partsmith', 'recovery')).toEqual([])
    expect(await recovery.read()).toBeNull()
  })

  it('keeps the entry for a later session', async () => {
    const fake = createFakeIndexedDb()
    await createIndexedDbRecovery(fake.factory).write(ENTRY)
    expect(await createIndexedDbRecovery(fake.factory).read()).toEqual(ENTRY)
  })

  it.each([
    ['a reason that is not autosave or replaced', { reason: 'crash' }],
    ['text that is not a string', { text: 42 }],
    ['a handle that is not a file handle', { handle: { kind: 'directory' } }],
    ['no savedAt', { savedAt: undefined }],
  ])('reads null for a stored record with %s', async (_, change) => {
    const fake = createFakeIndexedDb()
    const recovery = createIndexedDbRecovery(fake.factory)
    await recovery.clear()
    fake.putRecord('partsmith', 'recovery', { ...ENTRY, ...change, slot: 'current' })
    expect(await recovery.read()).toBeNull()
  })
})
