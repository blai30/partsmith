import { createStoreAccess, RECOVERY_STORE, requestResult } from './database'
import { recoveryEntryFrom, type RecoveryEntry } from './recovery-entry'

export type { RecoveryEntry } from './recovery-entry'

// Recovery copies (ui.md 2.15, 3.19): one entry per document session, in memory or in the IndexedDB
// object store `recovery`. The IndexedDB store holds a Web Lock for every autosave entry this page
// writes, so other tabs and windows leave out the copies of sessions that are still running, and
// it asks once for persistent storage.

export type RecoveryStore = {
  /** The entries whose session no running page holds, newest `savedAt` first. */
  list(): Promise<readonly RecoveryEntry[]>
  /** Writes the entry under its `id`, replacing only an entry with the same `id`; resolves whether it was stored. */
  write(entry: RecoveryEntry): Promise<boolean>
  /** Removes the entry with this `id`; every other entry stays. */
  remove(id: string): Promise<void>
}

/** What the IndexedDB store uses besides the database: `navigator.locks` and `navigator.storage`, or null where the host lacks them. */
export type RecoveryEnvironment = {
  readonly locks: LockManager | null
  readonly storage: StorageManager | null
}

function newestFirst(entries: readonly RecoveryEntry[]): RecoveryEntry[] {
  return entries.toSorted((first, second) => second.savedAt - first.savedAt)
}

/** A store in memory, for tests and hosts without IndexedDB: no other page shares it, so `list` returns every entry. */
export function createMemoryRecovery(): RecoveryStore {
  const entries = new Map<string, RecoveryEntry>()
  return {
    list: async () => newestFirst(Array.from(entries.values())),
    write: async (entry) => {
      entries.set(entry.id, entry)
      return true
    },
    remove: async (id) => {
      entries.delete(id)
    },
  }
}

/** The Web Lock a page holds while its session writes the entry `id` (2.15). */
function lockName(id: string): string {
  return `partsmith-recovery-${id}`
}

/** The Web Locks of this page's running sessions. */
type SessionLocks = {
  /** Holds the lock of an entry; resolves once it is held, or once it turned out it cannot be. */
  hold(id: string): Promise<void>
  /** Releases the lock of an entry; nothing happens when this page holds none for it. */
  release(id: string): void
  /** The names of the locks any page holds; none without a lock manager or when it fails. */
  held(): Promise<ReadonlySet<string>>
}

function createSessionLocks(locks: LockManager | null): SessionLocks {
  // The function that releases each lock this page holds, by entry id.
  const releases = new Map<string, () => void>()
  return {
    hold(id) {
      if (locks === null || releases.has(id)) {
        return Promise.resolve()
      }
      const released = new Promise<void>((release) => {
        releases.set(id, release)
      })
      return new Promise<void>((granted) => {
        // With ifAvailable a write never waits: a lock another page holds marks the copy as
        // running just as well.
        locks
          .request(lockName(id), { ifAvailable: true }, (lock) => {
            granted()
            // The browser releases the lock when this page goes away.
            return lock === null ? undefined : released
          })
          .catch((error: unknown) => {
            releases.delete(id)
            console.warn('Partsmith could not mark its recovery copy as in use.', error)
            granted()
          })
      })
    },
    release(id) {
      releases.get(id)?.()
      releases.delete(id)
    },
    async held() {
      if (locks === null) {
        return new Set()
      }
      try {
        const { held = [] } = await locks.query()
        return new Set(held.flatMap((lock) => (lock.name === undefined ? [] : [lock.name])))
      } catch (error) {
        console.warn('Partsmith could not tell which recovery copies are in use.', error)
        return new Set()
      }
    },
  }
}

/**
 * Asks the browser to keep the site's storage under storage pressure. The answer changes
 * nothing, so a refusal is ignored.
 */
function askForPersistence(storage: StorageManager): void {
  void storage
    .persisted()
    .then((persisted) => persisted || storage.persist())
    .catch(() => undefined)
}

/**
 * The recovery copies in the IndexedDB object store `recovery`; every database failure is logged
 * and changes nothing, and a write it fails resolves false (2.15). Operations run one after another in call order, so a write that waits for
 * its lock is never overtaken by a later write or removal of the same entry.
 */
export function createIndexedDbRecovery(
  factory: IDBFactory,
  environment: RecoveryEnvironment,
): RecoveryStore {
  const run = createStoreAccess(
    factory,
    RECOVERY_STORE,
    'Partsmith could not use the recovery store in IndexedDB.',
  )
  const sessions = createSessionLocks(environment.locks)
  let persistenceAsked = false
  let tail: Promise<unknown> = Promise.resolve()

  function inOrder<Result>(operation: () => Promise<Result>): Promise<Result> {
    const result = tail.then(operation)
    tail = result.catch(() => undefined)
    return result
  }

  return {
    list: () =>
      inOrder(async () => {
        const records = await run(
          'readonly',
          (store) => requestResult<unknown[]>(store.getAll()),
          [],
        )
        const running = await sessions.held()
        return newestFirst(
          records.flatMap((record) => {
            const entry = recoveryEntryFrom(record)
            return entry === null || running.has(lockName(entry.id)) ? [] : [entry]
          }),
        )
      }),
    write: (entry) =>
      inOrder(async () => {
        // Only the page's first write asks.
        if (!persistenceAsked && environment.storage !== null) {
          askForPersistence(environment.storage)
        }
        persistenceAsked = true
        // The lock is held before the entry is stored, so no other page offers it meanwhile.
        if (entry.reason === 'autosave') {
          await sessions.hold(entry.id)
        }
        const stored = await run(
          'readwrite',
          async (store) => {
            await requestResult(store.put(entry))
            return true
          },
          false,
        )
        // A replaced document's session has ended: its entry is no longer written.
        if (entry.reason === 'replaced') {
          sessions.release(entry.id)
        }
        return stored
      }),
    remove: (id) =>
      inOrder(async () => {
        await run('readwrite', (store) => requestResult(store.delete(id)), null)
        sessions.release(id)
      }),
  }
}
