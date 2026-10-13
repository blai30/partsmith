import { isFileHandle, type FileHandle } from './file-service'

// The recovery entry of ui.md 2.15 and its check, shared by the recovery store and the database
// upgrade that moves the version 1 copy into an entry.

/** One document session's recovery copy (3.19); `id` is the session's `DocumentState.recoveryId`. */
export type RecoveryEntry = {
  readonly id: string
  readonly name: string
  readonly text: string
  readonly savedAt: number
  readonly handle: FileHandle | null
  readonly reason: 'autosave' | 'replaced'
}

/** The entry a stored record holds, or null when the record is not a valid entry. */
export function recoveryEntryFrom(record: unknown): RecoveryEntry | null {
  if (typeof record !== 'object' || record === null) {
    return null
  }
  const field = (name: keyof RecoveryEntry): unknown => Reflect.get(record, name)
  const id = field('id')
  const name = field('name')
  const text = field('text')
  const savedAt = field('savedAt')
  const handle = field('handle')
  const reason = field('reason')
  if (
    typeof id !== 'string' ||
    typeof name !== 'string' ||
    typeof text !== 'string' ||
    typeof savedAt !== 'number' ||
    (reason !== 'autosave' && reason !== 'replaced') ||
    (handle !== null && !isFileHandle(handle))
  ) {
    return null
  }
  return { id, name, text, savedAt, handle, reason }
}
