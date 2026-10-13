// Test support: the `navigator.storage` persistence calls the recovery store makes (ui.md 2.15),
// counted, with the answers a test chooses.

export type FakeStorage = {
  /** The page's `navigator.storage`. */
  readonly storage: StorageManager
  /** How many times `persisted()` and `persist()` were called. */
  readonly calls: { readonly persisted: number; readonly persist: number }
}

export type FakeStorageOptions = {
  /** What `persisted()` resolves to until a `persist()` succeeds. */
  readonly persisted: boolean
  /** When true, `persist()` rejects instead of granting persistence. */
  readonly persistFails?: boolean
}

/** A StorageManager whose persistence answers the test sets. */
export function createFakeStorage(options: FakeStorageOptions): FakeStorage {
  const calls = { persisted: 0, persist: 0 }
  let persisted = options.persisted
  const storage = {
    persisted: async (): Promise<boolean> => {
      calls.persisted += 1
      return persisted
    },
    persist: async (): Promise<boolean> => {
      calls.persist += 1
      if (options.persistFails === true) {
        throw new DOMException('Persistence is not allowed here.', 'NotAllowedError')
      }
      persisted = true
      return true
    },
    estimate: () => Promise.reject(new DOMException('No estimate.', 'NotSupportedError')),
    getDirectory: () => Promise.reject(new DOMException('No directory.', 'NotSupportedError')),
  }
  // The fake has the members the app uses.
  return { storage: storage as unknown as StorageManager, calls }
}
