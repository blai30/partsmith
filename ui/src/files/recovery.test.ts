import { afterEach, describe, expect, it, vi } from 'vitest'
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
})
