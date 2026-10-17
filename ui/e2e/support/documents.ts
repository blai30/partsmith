/// <reference lib="dom" />
import { expect, type Locator, type Page } from '@playwright/test'

// Helpers for the document, file, recovery, undo, notification and status bar tests (ui.md 6.5
// E-1 to E-9, E-127 to E-129 and the tests of 3.19 to 3.22): the timeline as a whole and its
// statuses, the Problems panel, notifications, the IndexedDB records of recent files and recovery,
// and the Web Locks of running recovery sessions (2.15). The shared helpers of 6.3 are in
// ../helpers.ts.

/** One recovery entry in IndexedDB (ui.md 2.15), without its handle. */
export type StoredRecovery = {
  readonly id: string
  readonly name: string
  readonly text: string
  readonly reason: string
}

/** The prefix of the recovery offers `recovered` and `recovered_more` (ui.md 3.19). */
const RECOVERY_OFFER = /^Partsmith kept unsaved changes/

/** The timeline items. */
export function timelineItems(page: Page): Locator {
  return page.getByTestId('timeline-item')
}

/** Expects the timeline to hold exactly these features, in order, with these `data-status` values. */
export async function expectStatuses(
  page: Page,
  expected: readonly (readonly [label: string, status: string])[],
): Promise<void> {
  const items = timelineItems(page)
  await expect(items).toHaveCount(expected.length)
  await Promise.all(
    expected.flatMap(([label, status], index) => [
      expect(items.nth(index).getByRole('button')).toHaveAccessibleName(label),
      expect(items.nth(index)).toHaveAttribute('data-status', status),
    ]),
  )
}

/** Expects the timeline to hold exactly these features, in order, each with `data-status="ok"`. */
export async function expectTimeline(page: Page, labels: readonly string[]): Promise<void> {
  await expectStatuses(
    page,
    labels.map((label) => [label, 'ok'] as const),
  )
}

/** Opens the Problems panel with the timeline's problems button (ui.md 3.9) and returns it. */
export async function openProblems(page: Page): Promise<Locator> {
  await page.getByTestId('timeline-problems').click()
  const panel = page.getByTestId('problems-panel')
  await expect(panel).toBeVisible()
  return panel
}

/** Expects exactly one notification (ui.md 3.22) and returns it. */
export async function expectOnlyNotification(page: Page): Promise<Locator> {
  const notifications = page.getByTestId('notification')
  await expect(notifications).toHaveCount(1)
  return notifications.first()
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

/** The recovery entries in IndexedDB (ui.md 2.15), in no particular order. */
export async function storedRecoveries(page: Page): Promise<StoredRecovery[]> {
  const records = await readStore(page, 'recovery', null)
  if (!Array.isArray(records)) {
    return []
  }
  return records.flatMap((record: unknown) =>
    typeof record === 'object' && record !== null
      ? [
          {
            id: String(Reflect.get(record, 'id')),
            name: String(Reflect.get(record, 'name')),
            text: String(Reflect.get(record, 'text')),
            reason: String(Reflect.get(record, 'reason')),
          },
        ]
      : [],
  )
}

/**
 * Waits until a recovery entry holds `text`. The copy is written 2 s after the last change
 * (ui.md 3.19), so waiting for it stands in for the spec's waits of 3 s.
 */
export async function expectStoredRecovery(page: Page, text: string): Promise<void> {
  await expect
    .poll(async () => (await storedRecoveries(page)).some((entry) => entry.text.includes(text)))
    .toBe(true)
}

/** The shown recovery offers: the notifications starting with "Partsmith kept unsaved changes". */
export function recoveryOffers(page: Page): Locator {
  return page.getByTestId('notification').filter({ hasText: RECOVERY_OFFER })
}

/** The names of the recovery locks running sessions hold (`partsmith-recovery-{id}`, ui.md 2.15). */
export function heldRecoveryLocks(page: Page): Promise<string[]> {
  return page.evaluate(async () => {
    const { held = [] } = await navigator.locks.query()
    return held.flatMap((lock) =>
      lock.name?.startsWith('partsmith-recovery-') === true ? [lock.name] : [],
    )
  })
}
