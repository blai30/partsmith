import { afterEach, describe, expect, it, vi } from 'vitest'
import { createFakeIndexedDb } from '../test/fakes/indexed-db'
import type { DesktopFileHandle, FileHandle } from './file-service'
import {
  createIndexedDbRecentFiles,
  createMemoryRecentFiles,
  type RecentFile,
  type RecentFilesStore,
} from './recent'

// A handle whose isSameEntry compares the file it points at, like two handles to one file.
function fakeHandle(name: string, file: string): FileSystemFileHandle {
  const handle = {
    kind: 'file',
    name,
    file,
    isSameEntry: async (other: unknown) =>
      typeof other === 'object' && other !== null && Reflect.get(other, 'file') === file,
  }
  return handle as unknown as FileSystemFileHandle
}

function desktopHandle(name: string, grant: string): DesktopFileHandle {
  return { kind: 'desktop', name, grant }
}

function recentFile(id: string, name: string, openedAt: number): RecentFile {
  return { id, name, openedAt, handle: null, text: '{}' }
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

function clock(start: number): { readonly now: () => number; tick(): void } {
  let time = start
  return {
    now: () => time,
    tick: () => {
      time += 1000
    },
  }
}

async function rememberInOrder(
  store: RecentFilesStore,
  entries: readonly {
    readonly name: string
    readonly handle: FileHandle | null
    readonly text: string
  }[],
  tick: () => void,
): Promise<void> {
  await entries.reduce(async (previous, entry) => {
    await previous
    await store.remember(entry)
    tick()
  }, Promise.resolve())
}

afterEach(() => {
  vi.restoreAllMocks()
})

describe('createMemoryRecentFiles', () => {
  it('starts empty', async () => {
    expect(await createMemoryRecentFiles().list()).toEqual([])
  })

  it('U-76 keeps at most 10 entries, newest first', async () => {
    const time = clock(1000)
    const store = createMemoryRecentFiles(time.now)
    const entries = Array.from({ length: 12 }, (_, index) => ({
      name: `part-${String(index + 1)}.ptsm`,
      handle: null,
      text: `{"part": ${String(index + 1)}}`,
    }))
    await rememberInOrder(store, entries, time.tick)
    const list = await store.list()
    expect(list.map((entry) => entry.name)).toEqual([
      'part-12.ptsm',
      'part-11.ptsm',
      'part-10.ptsm',
      'part-9.ptsm',
      'part-8.ptsm',
      'part-7.ptsm',
      'part-6.ptsm',
      'part-5.ptsm',
      'part-4.ptsm',
      'part-3.ptsm',
    ])
  })

  it('stores the text, the time it was opened and a unique id for an entry without a handle', async () => {
    const time = clock(5000)
    const store = createMemoryRecentFiles(time.now)
    await rememberInOrder(
      store,
      [
        { name: 'a.ptsm', handle: null, text: '{"a": 1}' },
        { name: 'b.ptsm', handle: null, text: '{"b": 1}' },
      ],
      time.tick,
    )
    const [newest, oldest] = await store.list()
    expect(newest).toMatchObject({ name: 'b.ptsm', handle: null, text: '{"b": 1}', openedAt: 6000 })
    expect(oldest).toMatchObject({ name: 'a.ptsm', handle: null, text: '{"a": 1}', openedAt: 5000 })
    expect(newest?.id).toEqual(expect.any(String))
    expect(newest?.id).not.toBe('')
    expect(newest?.id).not.toBe(oldest?.id)
  })

  it('U-76 a text entry with an existing name replaces it', async () => {
    const time = clock(1000)
    const store = createMemoryRecentFiles(time.now)
    await rememberInOrder(
      store,
      [
        { name: 'a.ptsm', handle: null, text: 'first' },
        { name: 'b.ptsm', handle: null, text: 'other' },
        { name: 'a.ptsm', handle: null, text: 'second' },
      ],
      time.tick,
    )
    const list = await store.list()
    expect(list.map((entry) => [entry.name, entry.text])).toEqual([
      ['a.ptsm', 'second'],
      ['b.ptsm', 'other'],
    ])
  })

  it('U-76 a handle entry whose handle isSameEntry an existing one replaces it', async () => {
    const time = clock(1000)
    const store = createMemoryRecentFiles(time.now)
    const first = fakeHandle('bracket.ptsm', '/models/bracket.ptsm')
    const again = fakeHandle('bracket.ptsm', '/models/bracket.ptsm')
    await rememberInOrder(
      store,
      [
        { name: 'bracket.ptsm', handle: first, text: '{}' },
        { name: 'other.ptsm', handle: null, text: '{}' },
        { name: 'bracket.ptsm', handle: again, text: '{}' },
      ],
      time.tick,
    )
    const list = await store.list()
    expect(list.map((entry) => entry.name)).toEqual(['bracket.ptsm', 'other.ptsm'])
    expect(list[0]?.handle).toBe(again)
    expect(list[0]?.openedAt).toBe(3000)
  })

  it('keeps two handle entries with the same name that point at different files', async () => {
    const time = clock(1000)
    const store = createMemoryRecentFiles(time.now)
    await rememberInOrder(
      store,
      [
        { name: 'bracket.ptsm', handle: fakeHandle('bracket.ptsm', '/a/bracket.ptsm'), text: '{}' },
        { name: 'bracket.ptsm', handle: fakeHandle('bracket.ptsm', '/b/bracket.ptsm'), text: '{}' },
      ],
      time.tick,
    )
    expect(await store.list()).toHaveLength(2)
  })

  it('stores text null for an entry with a handle', async () => {
    const store = createMemoryRecentFiles()
    await store.remember({
      name: 'bracket.ptsm',
      handle: fakeHandle('bracket.ptsm', '/models/bracket.ptsm'),
      text: '{"format": 1}',
    })
    const [entry] = await store.list()
    expect(entry?.text).toBeNull()
  })

  it('desktop test 46: remembering desktop handles with the same grant twice keeps one entry', async () => {
    const time = clock(1000)
    const store = createMemoryRecentFiles(time.now)
    const again = desktopHandle('bracket.ptsm', 'g1')
    await rememberInOrder(
      store,
      [
        { name: 'bracket.ptsm', handle: desktopHandle('bracket.ptsm', 'g1'), text: '{}' },
        { name: 'bracket.ptsm', handle: again, text: '{}' },
      ],
      time.tick,
    )
    const list = await store.list()
    expect(list).toHaveLength(1)
    expect(list[0]?.handle).toBe(again)
    expect(list[0]?.openedAt).toBe(2000)
  })

  it('desktop test 46: desktop handles with different grants and the same name keep two entries', async () => {
    const time = clock(1000)
    const store = createMemoryRecentFiles(time.now)
    await rememberInOrder(
      store,
      [
        { name: 'bracket.ptsm', handle: desktopHandle('bracket.ptsm', 'g1'), text: '{}' },
        { name: 'bracket.ptsm', handle: desktopHandle('bracket.ptsm', 'g2'), text: '{}' },
      ],
      time.tick,
    )
    expect((await store.list()).map((entry) => entry.handle)).toEqual([
      desktopHandle('bracket.ptsm', 'g2'),
      desktopHandle('bracket.ptsm', 'g1'),
    ])
  })

  it('desktop test 46: an entry with a desktop handle stores text null', async () => {
    const store = createMemoryRecentFiles()
    await store.remember({
      name: 'bracket.ptsm',
      handle: desktopHandle('bracket.ptsm', 'g1'),
      text: '{"format": 1}',
    })
    const [entry] = await store.list()
    expect(entry?.text).toBeNull()
  })

  it('a desktop handle and a web handle are never the same file', async () => {
    const time = clock(1000)
    const store = createMemoryRecentFiles(time.now)
    await rememberInOrder(
      store,
      [
        { name: 'bracket.ptsm', handle: fakeHandle('bracket.ptsm', 'bracket.ptsm'), text: '{}' },
        { name: 'bracket.ptsm', handle: desktopHandle('bracket.ptsm', 'bracket.ptsm'), text: '{}' },
      ],
      time.tick,
    )
    expect(await store.list()).toHaveLength(2)
  })

  it('forget removes the entry with that id', async () => {
    const time = clock(1000)
    const store = createMemoryRecentFiles(time.now)
    await rememberInOrder(
      store,
      [
        { name: 'a.ptsm', handle: null, text: 'a' },
        { name: 'b.ptsm', handle: null, text: 'b' },
      ],
      time.tick,
    )
    const [newest] = await store.list()
    await store.forget(newest?.id ?? '')
    expect((await store.list()).map((entry) => entry.name)).toEqual(['a.ptsm'])
  })
})

describe('createIndexedDbRecentFiles', () => {
  it('resolves empty and without effect when IndexedDB fails, and warns naming the store', async () => {
    const warn = vi.spyOn(console, 'warn').mockImplementation(() => undefined)
    const store = createIndexedDbRecentFiles(blockedFactory(), () => 1000)
    expect(await store.list()).toEqual([])
    await expect(
      store.remember({ name: 'a.ptsm', handle: null, text: '{}' }),
    ).resolves.toBeUndefined()
    await expect(store.forget('missing')).resolves.toBeUndefined()
    expect(warn).toHaveBeenCalled()
    expect(warn.mock.calls.flat().map(String).join(' ')).toMatch(/recent/i)
  })

  it('U-76 keeps the 10 newest of 11 entries, newest first, and deletes the oldest record', async () => {
    const fake = createFakeIndexedDb()
    const time = clock(1000)
    const store = createIndexedDbRecentFiles(fake.factory, time.now)
    const entries = Array.from({ length: 11 }, (_, index) => ({
      name: `part-${String(index + 1)}.ptsm`,
      handle: null,
      text: `{"part": ${String(index + 1)}}`,
    }))
    await rememberInOrder(store, entries, time.tick)
    const names = (await store.list()).map((entry) => entry.name)
    expect(names).toEqual(
      entries
        .map((entry) => entry.name)
        .toReversed()
        .slice(0, 10),
    )
    const stored = fake
      .records('partsmith', 'recent')
      .map((record) => Reflect.get(Object(record), 'name'))
    expect(stored).toHaveLength(10)
    expect(stored).not.toContain('part-1.ptsm')
  })

  it('U-76 a text entry with an existing name replaces its record', async () => {
    const fake = createFakeIndexedDb()
    const time = clock(1000)
    const store = createIndexedDbRecentFiles(fake.factory, time.now)
    await rememberInOrder(
      store,
      [
        { name: 'a.ptsm', handle: null, text: 'first' },
        { name: 'b.ptsm', handle: null, text: 'other' },
        { name: 'a.ptsm', handle: null, text: 'second' },
      ],
      time.tick,
    )
    const list = await store.list()
    expect(list.map((entry) => [entry.name, entry.text, entry.openedAt])).toEqual([
      ['a.ptsm', 'second', 3000],
      ['b.ptsm', 'other', 2000],
    ])
    expect(fake.records('partsmith', 'recent')).toHaveLength(2)
  })

  it('U-76 a handle entry whose handle isSameEntry an existing one replaces it and stores no text', async () => {
    const fake = createFakeIndexedDb()
    const time = clock(1000)
    const store = createIndexedDbRecentFiles(fake.factory, time.now)
    const again = fakeHandle('bracket.ptsm', '/models/bracket.ptsm')
    await rememberInOrder(
      store,
      [
        {
          name: 'bracket.ptsm',
          handle: fakeHandle('bracket.ptsm', '/models/bracket.ptsm'),
          text: '{}',
        },
        { name: 'bracket.ptsm', handle: again, text: '{}' },
      ],
      time.tick,
    )
    const list = await store.list()
    expect(list).toHaveLength(1)
    expect(list[0]?.handle).toBe(again)
    expect(list[0]?.text).toBeNull()
  })

  it('lists the stored entries by openedAt, newest first, whatever their ids', async () => {
    const fake = createFakeIndexedDb()
    const store = createIndexedDbRecentFiles(fake.factory)
    // Creates the database, then stores entries whose key order is not their openedAt order.
    await store.list()
    fake.putRecord('partsmith', 'recent', recentFile('a', 'oldest.ptsm', 1000))
    fake.putRecord('partsmith', 'recent', recentFile('b', 'newest.ptsm', 3000))
    fake.putRecord('partsmith', 'recent', recentFile('c', 'middle.ptsm', 2000))
    expect((await store.list()).map((entry) => entry.name)).toEqual([
      'newest.ptsm',
      'middle.ptsm',
      'oldest.ptsm',
    ])
  })

  it('skips stored records that are not recent files', async () => {
    const fake = createFakeIndexedDb()
    const store = createIndexedDbRecentFiles(fake.factory)
    await store.list()
    fake.putRecord('partsmith', 'recent', {
      id: 'a',
      name: 'good.ptsm',
      openedAt: 1000,
      handle: null,
      text: '{}',
    })
    fake.putRecord('partsmith', 'recent', {
      id: 'b',
      name: 'bad.ptsm',
      openedAt: 'yesterday',
      handle: null,
      text: '{}',
    })
    fake.putRecord('partsmith', 'recent', { id: 'c', name: 'no-handle-field.ptsm', openedAt: 2000 })
    expect((await store.list()).map((entry) => entry.name)).toEqual(['good.ptsm'])
  })

  it('forget deletes the record with that id', async () => {
    const fake = createFakeIndexedDb()
    const time = clock(1000)
    const store = createIndexedDbRecentFiles(fake.factory, time.now)
    await rememberInOrder(
      store,
      [
        { name: 'a.ptsm', handle: null, text: 'a' },
        { name: 'b.ptsm', handle: null, text: 'b' },
      ],
      time.tick,
    )
    const [newest] = await store.list()
    await store.forget(newest?.id ?? '')
    expect((await store.list()).map((entry) => entry.name)).toEqual(['a.ptsm'])
    expect(fake.records('partsmith', 'recent')).toHaveLength(1)
  })

  it('desktop test 47: an entry with a desktop handle round-trips unchanged', async () => {
    const fake = createFakeIndexedDb()
    await createIndexedDbRecentFiles(fake.factory, () => 1000).remember({
      name: 'bracket.ptsm',
      handle: desktopHandle('bracket.ptsm', 'g1'),
      text: '{"format": 1}',
    })
    const later = await createIndexedDbRecentFiles(fake.factory).list()
    expect(later).toEqual([
      {
        id: expect.any(String),
        name: 'bracket.ptsm',
        openedAt: 1000,
        handle: { kind: 'desktop', name: 'bracket.ptsm', grant: 'g1' },
        text: null,
      },
    ])
  })

  it('keeps the entries for a later session', async () => {
    const fake = createFakeIndexedDb()
    await createIndexedDbRecentFiles(fake.factory, () => 1000).remember({
      name: 'bracket.ptsm',
      handle: null,
      text: '{"format": 1}',
    })
    const later = await createIndexedDbRecentFiles(fake.factory).list()
    expect(later).toEqual([
      {
        id: expect.any(String),
        name: 'bracket.ptsm',
        openedAt: 1000,
        handle: null,
        text: '{"format": 1}',
      },
    ])
  })
})
