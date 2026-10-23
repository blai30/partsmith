import { isFileHandle, isFolderHandle, type FileHandle } from './file-service'

// The recovery entry of ui.md 2.15 and its check, shared by the recovery store and the database
// upgrade that moves the version 1 copy into an entry.

/**
 * One document session's recovery copy (3.19); `id` is the session's `DocumentState.recoveryId`,
 * and `folder` and `digest` are those of the session's file reference (2.8).
 */
export type RecoveryEntry = {
  readonly id: string
  readonly name: string
  readonly text: string
  readonly savedAt: number
  readonly handle: FileHandle | null
  readonly folder: FileSystemDirectoryHandle | null
  readonly digest: string | null
  readonly reason: 'autosave' | 'replaced'
}

/**
 * The entry a stored record holds, or null when the record is not a valid entry. A record written
 * before `folder` and `digest` existed reads them as null.
 */
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
  const folder = field('folder') ?? null
  const digest = field('digest') ?? null
  const reason = field('reason')
  if (
    typeof id !== 'string' ||
    typeof name !== 'string' ||
    typeof text !== 'string' ||
    typeof savedAt !== 'number' ||
    (reason !== 'autosave' && reason !== 'replaced') ||
    (handle !== null && !isFileHandle(handle)) ||
    (folder !== null && !isFolderHandle(folder)) ||
    (digest !== null && typeof digest !== 'string')
  ) {
    return null
  }
  return { id, name, text, savedAt, handle, folder, digest, reason }
}
