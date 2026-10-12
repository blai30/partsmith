import { createConnection, RECENT_STORE, requestResult, withStore } from './database'
import { isFileHandle, type FileHandle } from './file-service'

export type RecentFile = {
  readonly id: string
  readonly name: string
  readonly openedAt: number
  readonly handle: FileHandle | null
  /** The file's text when there is no handle, so it reopens without file access; else null. */
  readonly text: string | null
}

export type RecentFilesStore = {
  /** The remembered files, most recent first. */
  list(): Promise<readonly RecentFile[]>
  remember(file: {
    readonly name: string
    readonly handle: FileHandle | null
    readonly text: string
  }): Promise<void>
  forget(id: string): Promise<void>
}

type OpenedFile = Parameters<RecentFilesStore['remember']>[0]

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
async function isSameFile(entry: RecentFile, file: OpenedFile): Promise<boolean> {
  if (file.handle === null || entry.handle === null) {
    return file.handle === null && entry.handle === null && entry.name === file.name
  }
  return isSameHandle(file.handle, entry.handle)
}

/** Whether two handles name one file: desktop handles with equal grants, or web handles to the same entry. */
async function isSameHandle(first: FileHandle, second: FileHandle): Promise<boolean> {
  // Each host's handles come only from that host, so a desktop and a web handle are never the same file.
  if (first.kind === 'desktop' || second.kind === 'desktop') {
    return first.kind === 'desktop' && second.kind === 'desktop' && first.grant === second.grant
  }
  try {
    return await first.isSameEntry(second)
  } catch {
    // A handle whose file is gone cannot be compared; it is a different entry.
    return false
  }
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
  file: OpenedFile,
  now: () => number,
): Promise<RememberPlan> {
  const added: RecentFile = {
    id: crypto.randomUUID(),
    name: file.name,
    openedAt: now(),
    handle: file.handle,
    text: file.handle === null ? file.text : null,
  }
  const same = await Promise.all(entries.map((entry) => isSameFile(entry, file)))
  const others = entries.filter((_, index) => same[index] !== true)
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

function isRecentFile(value: unknown): value is RecentFile {
  if (typeof value !== 'object' || value === null) {
    return false
  }
  const field = (name: keyof RecentFile): unknown => Reflect.get(value, name)
  const handle = field('handle')
  const text = field('text')
  return (
    typeof field('id') === 'string' &&
    typeof field('name') === 'string' &&
    typeof field('openedAt') === 'number' &&
    (handle === null || isFileHandle(handle)) &&
    (text === null || typeof text === 'string')
  )
}

function warn(error: unknown): void {
  console.warn('Partsmith could not use the recent files store in IndexedDB.', error)
}

/** Recent files in the IndexedDB object store `recent`; every failure is logged and ignored. */
export function createIndexedDbRecentFiles(
  factory: IDBFactory,
  now: () => number = Date.now,
): RecentFilesStore {
  const connection = createConnection(factory)
  const queue = createQueue()

  async function readAll(): Promise<readonly RecentFile[]> {
    const records = await withStore(connection, RECENT_STORE, 'readonly', (store) =>
      requestResult(store.getAll()),
    )
    return records
      .filter(isRecentFile)
      .toSorted((first, second) => second.openedAt - first.openedAt)
      .slice(0, RECENT_FILES_LIMIT)
  }

  return {
    list: () =>
      queue(async () => {
        try {
          return await readAll()
        } catch (error) {
          warn(error)
          return []
        }
      }),
    remember: (file) =>
      queue(async () => {
        try {
          const plan = await planRemember(await readAll(), file, now)
          await withStore(connection, RECENT_STORE, 'readwrite', async (store) => {
            const requests = plan.removedIds.map((id) => requestResult(store.delete(id)))
            requests.push(requestResult(store.put(plan.added)).then(() => undefined))
            await Promise.all(requests)
          })
        } catch (error) {
          warn(error)
        }
      }),
    forget: (id) =>
      queue(async () => {
        try {
          await withStore(connection, RECENT_STORE, 'readwrite', (store) =>
            requestResult(store.delete(id)),
          )
        } catch (error) {
          warn(error)
        }
      }),
  }
}
