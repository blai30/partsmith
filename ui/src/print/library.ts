// The printer library: the written forms of the user's printers, kept in the IndexedDB store
// `printers`, and what a document that stores a profile offers when it opens (ui.md 2.15 and 3.17).
import { PRINTERS_STORE, createStoreAccess, requestResult } from '../files/database'
import { askForPersistence, type RecoveryEnvironment } from '../files/recovery'
import type { Document, PrinterProfile } from '../generated/protocol'

export type PrinterLibrary = {
  /** Every record that has the members of a written form, sorted by id; other records are skipped and kept. */
  list(): Promise<readonly PrinterProfile[]>
  /** Writes the profile under its id and replaces only a record with that id; resolves whether it was stored. */
  put(profile: PrinterProfile): Promise<boolean>
  /** Removes the record with this id; every other record stays. */
  remove(id: string): Promise<void>
}

/** What opening a document that stores a profile offers (3.17), or null for nothing. */
export type LibraryOffer =
  | { readonly kind: 'add'; readonly profile: PrinterProfile }
  | { readonly kind: 'differs'; readonly profile: PrinterProfile }
  | null

function byId(profiles: Iterable<PrinterProfile>): PrinterProfile[] {
  return [...profiles].toSorted((first, second) =>
    first.id < second.id ? -1 : first.id > second.id ? 1 : 0,
  )
}

/** A library in memory, for tests and hosts without IndexedDB. */
export function createMemoryPrinterLibrary(): PrinterLibrary {
  const profiles = new Map<string, PrinterProfile>()
  return {
    list: () => Promise.resolve(byId(profiles.values())),
    put: (profile) => {
      profiles.set(profile.id, profile)
      return Promise.resolve(true)
    },
    remove: (id) => {
      profiles.delete(id)
      return Promise.resolve()
    },
  }
}

/**
 * The library in the IndexedDB object store `printers`; every database failure is logged and
 * changes nothing, and a put it fails resolves false (2.15).
 */
export function createIndexedDbPrinterLibrary(
  factory: IDBFactory,
  environment: RecoveryEnvironment,
): PrinterLibrary {
  const run = createStoreAccess(
    factory,
    PRINTERS_STORE,
    'Partsmith could not use the printer store in IndexedDB.',
  )
  let persistenceAsked = false
  return {
    list: async () => {
      const records = await run('readonly', (store) => requestResult<unknown[]>(store.getAll()), [])
      return byId(records.flatMap((record) => printerProfileFrom(record) ?? []))
    },
    put: (profile) => {
      // Only the page's first put asks.
      if (!persistenceAsked && environment.storage !== null) {
        askForPersistence(environment.storage)
      }
      persistenceAsked = true
      return run(
        'readwrite',
        async (store) => {
          await requestResult(store.put(profile))
          return true
        },
        false,
      )
    },
    remove: async (id) => {
      await run('readwrite', (store) => requestResult(store.delete(id)), null)
    },
  }
}

function isObject(value: unknown): value is Readonly<Record<string, unknown>> {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
}

function areNumbers(record: Readonly<Record<string, unknown>>, keys: readonly string[]): boolean {
  return keys.every((key) => typeof record[key] === 'number')
}

function isBed(value: unknown): boolean {
  if (!isObject(value)) {
    return false
  }
  switch (value['kind']) {
    case 'rectangle':
      return areNumbers(value, ['width', 'depth', 'height'])
    case 'circle':
      return areNumbers(value, ['diameter', 'height'])
    default:
      return false
  }
}

/**
 * The record as a written form when it has every required member of the generated
 * `PrinterProfile` with the right JSON type, else null (2.15). The core checks the values.
 */
export function printerProfileFrom(record: unknown): PrinterProfile | null {
  if (
    !isObject(record) ||
    typeof record['id'] !== 'string' ||
    typeof record['name'] !== 'string' ||
    (record['technology'] !== 'fdm' && record['technology'] !== 'resin') ||
    !isBed(record['bed']) ||
    !(
      record['nozzle_diameter'] === undefined ||
      record['nozzle_diameter'] === null ||
      typeof record['nozzle_diameter'] === 'number'
    ) ||
    !areNumbers(record, ['layer_height', 'max_overhang_angle', 'max_bridge_length']) ||
    !isObject(record['fit']) ||
    !areNumbers(record['fit'], ['press', 'snug', 'slip', 'loose'])
  ) {
    return null
  }
  return record as unknown as PrinterProfile
}

/** Whether two JSON values are equal, member for member and whatever the order of members. */
function sameJson(first: unknown, second: unknown): boolean {
  if (Array.isArray(first) || Array.isArray(second)) {
    return (
      Array.isArray(first) &&
      Array.isArray(second) &&
      first.length === second.length &&
      first.every((item, index) => sameJson(item, second[index]))
    )
  }
  if (isObject(first) && isObject(second)) {
    const keys = Object.keys(first)
    return (
      keys.length === Object.keys(second).length &&
      keys.every((key) => key in second && sameJson(first[key], second[key]))
    )
  }
  return first === second
}

/** Whether two profiles have equal written forms: every member equal as JSON values (3.17). */
export function sameProfile(first: PrinterProfile, second: PrinterProfile): boolean {
  return sameJson(first, second)
}

/**
 * What opening `document` offers (3.17): adding the profile it stores when no listed profile has
 * its id, using the listed one when that one has other values, else nothing.
 */
export function libraryOffer(
  document: Document,
  printers: readonly PrinterProfile[],
): LibraryOffer {
  const stored = document.printer_profile
  if (stored === undefined || stored === null) {
    return null
  }
  const listed = printers.find((profile) => profile.id === stored.id)
  if (listed === undefined) {
    return { kind: 'add', profile: stored }
  }
  return sameProfile(listed, stored) ? null : { kind: 'differs', profile: stored }
}
