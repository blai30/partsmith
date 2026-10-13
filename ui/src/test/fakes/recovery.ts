import { createMemoryRecovery, type RecoveryEntry, type RecoveryStore } from '../../files/recovery'

// Test support: a recovery store in memory (ui.md 2.15) that also keeps every entry written to
// it, so tests can tell when a write happened and what it held.

export type RecordingRecovery = RecoveryStore & {
  /** Every entry passed to `write`, oldest first. */
  readonly writes: readonly RecoveryEntry[]
}

/** A memory recovery store that records its writes. */
export function createRecordingRecovery(): RecordingRecovery {
  const memory = createMemoryRecovery()
  const writes: RecoveryEntry[] = []
  return {
    writes,
    list: () => memory.list(),
    write: (entry) => {
      writes.push(entry)
      return memory.write(entry)
    },
    remove: (id) => memory.remove(id),
  }
}
