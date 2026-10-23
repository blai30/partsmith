import type { CoreEvent } from '../core/events'
import type { RecoveryEntry } from '../files/recovery'
import { isUnsaved } from '../state/selectors'
import type { DocumentState } from '../state/store'
import { nowOf, settledRecipeOf } from './connection'
import type { App } from './createApp'

// The recovery copy of each document session (ui.md 3.19): one entry under the session's
// `recoveryId` holding the settled recipe of an unsaved document, written 2 s after its last
// settled change and at most 10 s after the first change no write has stored yet. A replaced
// document's session ends with a `replaced` entry that is never written again, unless a core
// restart reopens that document because the stop dropped what was replacing it (4.4).

/** Delay after the last settled document_changed before the entry is written (3.19). */
const RECOVERY_DELAY_MILLISECONDS = 2_000

/** The longest a settled change waits for its write while further changes keep coming (3.19). */
const RECOVERY_MAXIMUM_DELAY_MILLISECONDS = 10_000

type RecoveryCopy = {
  /** Hears a core event after the store applied it. */
  hear(event: CoreEvent): void
  /** Drops the session's pending write; an ended session is not written again until it resumes. */
  stop(recoveryId: string, ending: 'continues' | 'ends'): void
  /** Goes on with the session of the shown document, ended or not, and writes its entry now. */
  resume(recoveryId: string): void
}

const copies = new WeakMap<App, RecoveryCopy>()

/** The session's entry for the shown document: its settled recipe, never a preview (1.3). */
function entryOf(
  app: App,
  document: DocumentState,
  reason: RecoveryEntry['reason'],
): RecoveryEntry {
  return {
    id: document.recoveryId,
    name: document.name,
    text: JSON.stringify(settledRecipeOf(app, document), null, 2),
    savedAt: nowOf(app),
    handle: document.file?.handle ?? null,
    folder: document.file?.folder ?? null,
    digest: document.file?.digest ?? null,
    reason,
  }
}

function createRecoveryCopy(app: App): RecoveryCopy {
  const ended = new Set<string>()
  let pending: {
    readonly recoveryId: string
    latest: ReturnType<typeof setTimeout>
    readonly deadline: ReturnType<typeof setTimeout>
  } | null = null

  function drop(): void {
    if (pending !== null) {
      clearTimeout(pending.latest)
      clearTimeout(pending.deadline)
      pending = null
    }
  }

  // Writes the session's entry if its document is still shown and unsaved.
  function writeDue(recoveryId: string): void {
    drop()
    const document = app.store.getState().document
    if (document?.recoveryId !== recoveryId || !isUnsaved(document)) {
      return
    }
    void app.recovery.write(entryOf(app, document, 'autosave'))
  }

  function writeAfter(recoveryId: string, delay: number): ReturnType<typeof setTimeout> {
    return setTimeout(() => {
      writeDue(recoveryId)
    }, delay)
  }

  function schedule(recoveryId: string): void {
    if (pending?.recoveryId === recoveryId) {
      clearTimeout(pending.latest)
      pending.latest = writeAfter(recoveryId, RECOVERY_DELAY_MILLISECONDS)
      return
    }
    drop()
    pending = {
      recoveryId,
      latest: writeAfter(recoveryId, RECOVERY_DELAY_MILLISECONDS),
      deadline: writeAfter(recoveryId, RECOVERY_MAXIMUM_DELAY_MILLISECONDS),
    }
  }

  return {
    hear(event) {
      // The edits of an open feature command neither write the copy nor delay it.
      if (event.type !== 'document_changed' || event.transactionOpen) {
        return
      }
      const document = app.store.getState().document
      // A replaced document's last changes may arrive before the new one is shown.
      if (document?.documentHandle !== event.documentHandle || ended.has(document.recoveryId)) {
        return
      }
      if (isUnsaved(document)) {
        schedule(document.recoveryId)
        return
      }
      // Saved, or undone back to the saved revision: there is nothing left to recover.
      drop()
      void app.recovery.remove(document.recoveryId)
    },
    stop(recoveryId, ending) {
      if (pending?.recoveryId === recoveryId) {
        drop()
      }
      if (ending === 'ends') {
        ended.add(recoveryId)
      }
    },
    resume(recoveryId) {
      ended.delete(recoveryId)
      writeDue(recoveryId)
    },
  }
}

/** Keeps the recovery copy of the app's document sessions; returns the listener of core events. */
export function installRecoveryCopy(app: App): (event: CoreEvent) => void {
  const copy = createRecoveryCopy(app)
  copies.set(app, copy)
  return (event) => {
    copy.hear(event)
  }
}

/**
 * Ends the session of a document that another one replaces (3.19): an unsaved document's entry is
 * written as `replaced` and returned, and a saved document's leftover entry is removed.
 */
export async function endRecoverySession(
  app: App,
  document: DocumentState,
): Promise<RecoveryEntry | null> {
  copies.get(app)?.stop(document.recoveryId, 'ends')
  if (!isUnsaved(document)) {
    await app.recovery.remove(document.recoveryId)
    return null
  }
  const stash = entryOf(app, document, 'replaced')
  await app.recovery.write(stash)
  return stash
}

/** Removes a saved document's entry; its session goes on and writes again after its next change. */
export async function removeRecoveryEntry(app: App, document: DocumentState): Promise<void> {
  copies.get(app)?.stop(document.recoveryId, 'continues')
  await app.recovery.remove(document.recoveryId)
}

/**
 * Continues the session of a document a core restart reopened (4.4): even one that an open or a
 * new document had ended before the restart dropped it, and its entry is written at once, so a
 * write that fell due while the core restarted is not lost.
 */
export function continueRecoverySession(app: App, recoveryId: string): void {
  copies.get(app)?.resume(recoveryId)
}

/**
 * Moves an opened entry to the session that opened it (3.19): its text is written at once under
 * the new `recoveryId`, with its time and handle, and the opened entry is removed once that write
 * is stored, so a write that fails keeps it.
 */
export async function adoptRecoveryEntry(
  app: App,
  entry: RecoveryEntry,
  recoveryId: string,
): Promise<void> {
  if (await app.recovery.write({ ...entry, id: recoveryId, reason: 'autosave' })) {
    await app.recovery.remove(entry.id)
  }
}
