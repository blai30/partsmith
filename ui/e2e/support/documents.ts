/// <reference lib="dom" />
import { expect, type Locator, type Page } from '@playwright/test'

// Helpers for the document, file, recovery, undo, notification and status bar tests (ui.md 6.5
// E-1 to E-9 and the tests of 3.19 to 3.22): the timeline as a whole, notifications by message,
// and the IndexedDB records of recent files and recovery (2.15). The shared helpers of 6.3 are in
// ../helpers.ts.

/** The recovery copy in IndexedDB (ui.md 2.15), without its handle. */
export type StoredRecovery = {
  readonly name: string
  readonly text: string
  readonly reason: string
}

/** The timeline items. */
export function timelineItems(page: Page): Locator {
  return page.getByTestId('timeline-item')
}

/** Expects the timeline to hold exactly these features, in order, each with `data-status="ok"`. */
export async function expectTimeline(page: Page, labels: readonly string[]): Promise<void> {
  const items = timelineItems(page)
  await expect(items).toHaveCount(labels.length)
  await Promise.all(
    labels.flatMap((label, index) => [
      expect(items.nth(index).getByRole('button')).toHaveAccessibleName(label),
      expect(items.nth(index)).toHaveAttribute('data-status', 'ok'),
    ]),
  )
}

/**
 * Runs a read-only IndexedDB request on one object store of the app's database (ui.md 2.15) and
 * returns its result, or null while the database or the record does not exist. The database is
 * never created or upgraded here, so the app's own first open still creates its stores.
 */
function readStore(
  page: Page,
  storeName: 'recent' | 'recovery',
  key: string | null,
): Promise<unknown> {
  return page.evaluate(
    async ({ store, recordKey }) => {
      const databases = await indexedDB.databases()
      if (!databases.some((database) => database.name === 'partsmith')) {
        return null
      }
      const database = await new Promise<IDBDatabase | null>((resolve, reject) => {
        const request = indexedDB.open('partsmith')
        request.addEventListener('upgradeneeded', () => {
          request.transaction?.abort()
        })
        request.addEventListener('success', () => {
          resolve(request.result)
        })
        request.addEventListener('error', () => {
          // An aborted upgrade means the database was deleted meanwhile.
          resolve(null)
        })
        request.addEventListener('blocked', () => {
          reject(new Error('The partsmith database is blocked.'))
        })
      })
      if (database === null) {
        return null
      }
      try {
        if (!database.objectStoreNames.contains(store)) {
          return null
        }
        const objectStore = database.transaction(store, 'readonly').objectStore(store)
        const request = recordKey === null ? objectStore.getAll() : objectStore.get(recordKey)
        return await new Promise<unknown>((resolve, reject) => {
          request.addEventListener('success', () => {
            resolve(request.result ?? null)
          })
          request.addEventListener('error', () => {
            reject(request.error ?? new Error(`Reading ${store} failed.`))
          })
        })
      } finally {
        database.close()
      }
    },
    { store: storeName, recordKey: key },
  )
}

/** The file names in the recent files store (ui.md 2.15), in no particular order. */
export async function recentFileNames(page: Page): Promise<string[]> {
  const records = await readStore(page, 'recent', null)
  if (!Array.isArray(records)) {
    return []
  }
  return records.map((record: unknown) =>
    typeof record === 'object' && record !== null ? String(Reflect.get(record, 'name')) : '',
  )
}

/** The recovery copy in IndexedDB (ui.md 2.15), or null when there is none. */
export async function storedRecovery(page: Page): Promise<StoredRecovery | null> {
  const record = await readStore(page, 'recovery', 'current')
  if (typeof record !== 'object' || record === null) {
    return null
  }
  return {
    name: String(Reflect.get(record, 'name')),
    text: String(Reflect.get(record, 'text')),
    reason: String(Reflect.get(record, 'reason')),
  }
}
