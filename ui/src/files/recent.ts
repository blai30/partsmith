import { createStoreAccess, RECENT_STORE, requestResult } from './database'
import { isFileHandle, isFolderHandle, isSameHandle, type FileHandle } from './file-service'

export type RecentFile = {
  readonly id: string
  readonly name: string
  readonly openedAt: number
  readonly handle: FileHandle | null
  /** The web folder that holds the file (ui.md 3.19); null on the desktop and without one. */
  readonly folder: FileSystemDirectoryHandle | null
  /** The file's text when there is no handle, so it reopens without file access; else null. */
  readonly text: string | null
}

/** A file to remember: its name, handle and web folder, and its text for when it has no handle. */
export type RememberedFile = {
  readonly name: string
  readonly handle: FileHandle | null
  readonly folder: FileSystemDirectoryHandle | null
  readonly text: string
}

export type RecentFilesStore = {
  /** The remembered files, most recent first. */
  list(): Promise<readonly RecentFile[]>
  remember(file: RememberedFile): Promise<void>
  forget(id: string): Promise<void>
}

/** The most files the list keeps. */
export const RECENT_FILES_LIMIT = 10

/** Runs tasks one at a time in call order, so a remember finishes before the next one reads. */
function createQueue(): <Result>(task: () => Promise<Result>) => Promise<Result> {
  let tail: Promise<unknown> = Promise.resolve()
  return (task) => {
    const run = tail.then(task, task)
    tail = run.catch(() => undefined)
    return run
  }
}

/** Whether a remembered entry is the same file: the same handle, or for entries without one, the same name. */
async function isSameFile(entry: RecentFile, file: RememberedFile): Promise<boolean> {
  if (file.handle === null || entry.handle === null) {
    return file.handle === null && entry.handle === null && entry.name === file.name
  }
  return isSameHandle(file.handle, entry.handle)
}

type RememberPlan = {
  /** The list after remembering, most recent first. */
  readonly entries: readonly RecentFile[]
  readonly added: RecentFile
  /** Ids of the entries the new one replaces or pushes past the limit. */
  readonly removedIds: readonly string[]
}

async function planRemember(
  entries: readonly RecentFile[],
  file: RememberedFile,
  now: () => number,
): Promise<RememberPlan> {
  const same = await Promise.all(entries.map((entry) => isSameFile(entry, file)))
  const others = entries.filter((_, index) => same[index] !== true)
  // Opening a file again with the picker keeps the folder an earlier session chose (3.19).
  const replacedFolder =
    entries.find((entry, index) => same[index] === true && entry.folder !== null)?.folder ?? null
  const added: RecentFile = {
    id: crypto.randomUUID(),
    name: file.name,
    openedAt: now(),
    handle: file.handle,
    folder: file.folder ?? replacedFolder,
    text: file.handle === null ? file.text : null,
  }
  const kept = [added, ...others].slice(0, RECENT_FILES_LIMIT)
  const keptIds = new Set(kept.map((entry) => entry.id))
  return {
    entries: kept,
    added,
    removedIds: entries.filter((entry) => !keptIds.has(entry.id)).map((entry) => entry.id),
  }
}

/** Recent files kept in memory, for tests and for hosts without IndexedDB. */
export function createMemoryRecentFiles(now: () => number = Date.now): RecentFilesStore {
  const queue = createQueue()
  let entries: readonly RecentFile[] = []
  return {
    list: () => queue(async () => entries),
    remember: (file) =>
      queue(async () => {
        entries = (await planRemember(entries, file, now)).entries
      }),
    forget: (id) =>
      queue(async () => {
        entries = entries.filter((entry) => entry.id !== id)
      }),
  }
}

/** The recent file a stored record holds, or null when it is not one; a record without `folder` has none. */
function recentFileFrom(record: unknown): RecentFile | null {
  if (typeof record !== 'object' || record === null) {
    return null
  }
  const field = (name: keyof RecentFile): unknown => Reflect.get(record, name)
  const id = field('id')
  const name = field('name')
  const openedAt = field('openedAt')
  const handle = field('handle')
  const folder = field('folder') ?? null
  const text = field('text')
  if (
    typeof id !== 'string' ||
    typeof name !== 'string' ||
    typeof openedAt !== 'number' ||
    (handle !== null && !isFileHandle(handle)) ||
    (folder !== null && !isFolderHandle(folder)) ||
    (text !== null && typeof text !== 'string')
  ) {
    return null
  }
  return { id, name, openedAt, handle, folder, text }
}

/** Every stored record of the `recent` object store. */
function readRecords(store: IDBObjectStore): Promise<readonly unknown[]> {
  return requestResult(store.getAll())
}

/** The remembered files among stored records, most recent first, at most the limit. */
function recentFilesOf(records: readonly unknown[]): readonly RecentFile[] {
  return records
    .flatMap((record) => recentFileFrom(record) ?? [])
    .toSorted((first, second) => second.openedAt - first.openedAt)
    .slice(0, RECENT_FILES_LIMIT)
}

/** Recent files in the IndexedDB object store `recent`; every failure is logged and ignored. */
export function createIndexedDbRecentFiles(
  factory: IDBFactory,
  now: () => number = Date.now,
): RecentFilesStore {
  const run = createStoreAccess(
    factory,
    RECENT_STORE,
    'Partsmith could not use the recent files store in IndexedDB.',
  )
  const queue = createQueue()
  return {
    list: () => queue(async () => recentFilesOf(await run('readonly', readRecords, []))),
    remember: (file) =>
      queue(async () => {
        const records = await run('readonly', readRecords, null)
        if (records === null) {
          return
        }
        const plan = await planRemember(recentFilesOf(records), file, now)
        await run(
          'readwrite',
          async (store) => {
            const requests = plan.removedIds.map((id) => requestResult(store.delete(id)))
            requests.push(requestResult(store.put(plan.added)).then(() => undefined))
            await Promise.all(requests)
          },
          null,
        )
      }),
    forget: (id) =>
      queue(async () => {
        await run('readwrite', (store) => requestResult(store.delete(id)), null)
      }),
  }
}
