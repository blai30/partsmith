import { afterEach, describe, expect, it, vi } from 'vitest'
import {
  createIndexedDbRecovery,
  createMemoryRecovery,
  type RecoveryEntry,
  type RecoveryStore,
} from '../../files/recovery'
import type { Document, Edit } from '../../generated/protocol'
import { MESSAGES } from '../../messages'
import type { App } from '../createApp'
import {
  actionLabels,
  clickAction,
  currentDocument,
  notificationCoded,
  notificationCodes,
  notificationWith,
  openTestDocument,
  startFakeApp,
  startWithDocument,
} from '../../test/fakes/app'
import { CUBE_DOCUMENT, TWO_BODIES_DOCUMENT, bodiesOf, cubeNamed } from '../../test/fakes/documents'
import { createFakeFileHandle } from '../../test/fakes/files'
import { createFakeIndexedDb, type FakeIndexedDb } from '../../test/fakes/indexed-db'
import { createFakeLocks, type FakeLockPage } from '../../test/fakes/locks'
import { createRecordingRecovery } from '../../test/fakes/recovery'
import { settle } from '../../test/fakes/settle'
import { applyEdits, newDocument, save } from './document'
import { dismiss } from './notifications'

// Recovery entries per document session (ui.md 2.15 and 3.19): a replaced document's stash that
// later autosaves and saves leave alone, two running sessions that never touch each other's
// copies, and the startup offer of every entry.

const OFFER = /^Partsmith kept unsaved changes/

function renameBody(body: string, name: string): readonly Edit[] {
  return [{ type: 'update_body', body: { id: body, name } }]
}

/** The text of a recipe as the recovery copy stores it (3.19). */
function stored(recipe: Document): string {
  return JSON.stringify(recipe, null, 2)
}

/** `savedAt` as the recovery notifications show it (3.19). */
function shownTime(savedAt: number): string {
  return new Date(savedAt).toLocaleString('en-US', { dateStyle: 'medium', timeStyle: 'short' })
}

/** The messages of the shown recovery offers. */
function offers(app: App): string[] {
  return app.store
    .getState()
    .notifications.map((notification) => notification.message)
    .filter((message) => OFFER.test(message))
}

/** Advances the fake clock, then lets the writes it starts finish, IndexedDB ones included. */
async function advance(milliseconds: number): Promise<void> {
  await vi.advanceTimersByTimeAsync(milliseconds)
  await settle(30)
}

/** Writes every entry, one after another. */
async function writeAll(store: RecoveryStore, entries: readonly RecoveryEntry[]): Promise<void> {
  for (const entry of entries) {
    // The order of the writes is part of the setup.
    // oxlint-disable-next-line no-await-in-loop
    await store.write(entry)
  }
}

/** One field of a stored record, or undefined when the record is not an object. */
function recordField(record: unknown, name: string): unknown {
  return typeof record === 'object' && record !== null ? Reflect.get(record, name) : undefined
}

/** The text of every record in the recovery object store, by entry id. */
function storedTexts(database: FakeIndexedDb): Record<string, unknown> {
  return Object.fromEntries(
    database
      .records('partsmith', 'recovery')
      .map((record) => [String(recordField(record, 'id')), recordField(record, 'text')]),
  )
}

afterEach(() => {
  vi.useRealTimers()
  vi.restoreAllMocks()
})

describe('a replaced document (3.19)', () => {
  it("U-157 its stash survives the next document's autosave, and saving that document removes only its own entry", async () => {
    const recovery = createMemoryRecovery()
    const cubeHandle = createFakeFileHandle('cube.ptsm')
    const fake = await startWithDocument('cube.ptsm', CUBE_DOCUMENT, {
      recovery,
      handle: cubeHandle,
    })
    vi.useFakeTimers()
    const cubeSession = currentDocument(fake.app).recoveryId
    await applyEdits(fake.app, 'Rename', renameBody('b-cube', 'Box'))
    await advance(2_000)
    const cubeRecipe = currentDocument(fake.app).recipe
    expect(await recovery.list()).toEqual([
      {
        id: cubeSession,
        name: 'cube',
        text: stored(cubeRecipe),
        savedAt: expect.any(Number),
        handle: cubeHandle,
        folder: null,
        digest: expect.any(String),
        reason: 'autosave',
      },
    ])

    const pegHandle = createFakeFileHandle('two-bodies.ptsm')
    await openTestDocument(fake, 'two-bodies.ptsm', TWO_BODIES_DOCUMENT, pegHandle)
    const stashes = await recovery.list()
    expect(stashes).toEqual([
      {
        id: cubeSession,
        name: 'cube',
        text: stored(cubeRecipe),
        savedAt: expect.any(Number),
        handle: cubeHandle,
        folder: null,
        digest: expect.any(String),
        reason: 'replaced',
      },
    ])
    notificationWith(fake.app, MESSAGES.stashed('cube'))
    const pegSession = currentDocument(fake.app).recoveryId
    expect(pegSession).not.toBe(cubeSession)

    await applyEdits(fake.app, 'Rename', renameBody('b-peg', 'Pin'))
    await advance(2_000)
    const entries = await recovery.list()
    expect(entries).toHaveLength(2)
    expect(entries).toContainEqual(stashes[0])
    expect(entries).toContainEqual({
      id: pegSession,
      name: 'two-bodies',
      text: stored(currentDocument(fake.app).recipe),
      savedAt: expect.any(Number),
      handle: pegHandle,
      folder: null,
      digest: expect.any(String),
      reason: 'autosave',
    })

    await save(fake.app)
    await settle()
    expect(currentDocument(fake.app).dirty).toBe(false)
    await expect(recovery.list()).resolves.toEqual(stashes)
  })

  it('U-157 a document replaced before its autosave is due is written once, as replaced, and never again', async () => {
    const recovery = createRecordingRecovery()
    const fake = await startWithDocument('cube.ptsm', CUBE_DOCUMENT, { recovery })
    vi.useFakeTimers()
    const cubeSession = currentDocument(fake.app).recoveryId
    await applyEdits(fake.app, 'Rename', renameBody('b-cube', 'Box'))
    await advance(1_000)
    await openTestDocument(fake, 'two-bodies.ptsm', TWO_BODIES_DOCUMENT)
    await advance(1_000)
    await advance(20_000)
    const cubeWrites = recovery.writes.filter((entry) => entry.id === cubeSession)
    expect(cubeWrites.map((entry) => entry.reason)).toEqual(['replaced'])
    expect(cubeWrites[0]?.text).toBe(stored(cubeNamed('Box')))
  })
})

describe('two sessions at once (2.15, 3.19)', () => {
  it("U-158 two pages never offer, overwrite or discard each other's live copy", async () => {
    const database = createFakeIndexedDb()
    const locks = createFakeLocks()
    const storeOn = (page: FakeLockPage): RecoveryStore =>
      createIndexedDbRecovery(database.factory, { locks: page.locks, storage: null })
    const pageA = locks.page()
    const pageB = locks.page()

    const first = await startWithDocument('cube.ptsm', CUBE_DOCUMENT, {
      recovery: storeOn(pageA),
      handle: createFakeFileHandle('cube.ptsm'),
    })
    vi.useFakeTimers()
    await applyEdits(first.app, 'Rename', renameBody('b-cube', 'Box'))
    await advance(2_000)
    const firstSession = currentDocument(first.app).recoveryId
    expect(storedTexts(database)).toEqual({
      [firstSession]: stored(currentDocument(first.app).recipe),
    })

    const second = await startWithDocument('two-bodies.ptsm', TWO_BODIES_DOCUMENT, {
      recovery: storeOn(pageB),
    })
    expect(offers(second.app)).toEqual([])
    const codes = notificationCodes(second.app)
    expect(codes).not.toContain('recovered')
    expect(codes).not.toContain('recovered_more')

    await applyEdits(second.app, 'Rename', renameBody('b-peg', 'Pin'))
    await advance(2_000)
    const secondSession = currentDocument(second.app).recoveryId
    expect(secondSession).not.toBe(firstSession)
    await applyEdits(first.app, 'Rename', renameBody('b-cube', 'Block'))
    await advance(2_000)
    expect(storedTexts(database)).toEqual({
      [firstSession]: stored(cubeNamed('Block')),
      [secondSession]: stored(currentDocument(second.app).recipe),
    })

    await save(first.app)
    await settle(30)
    expect(storedTexts(database)).toEqual({
      [secondSession]: stored(currentDocument(second.app).recipe),
    })

    pageB.end()
    await settle(30)
    const third = await startFakeApp({ recovery: storeOn(locks.page()) })
    const savedAt = Number(recordField(database.records('partsmith', 'recovery')[0], 'savedAt'))
    expect(offers(third.app)).toEqual([
      `Partsmith kept unsaved changes to two-bodies from ${shownTime(savedAt)}.`,
    ])
    await clickAction(third.app, notificationWith(third.app, OFFER), 'Discard')
    await settle(30)
    expect(database.records('partsmith', 'recovery')).toEqual([])
  })
})

/** An entry whose document has its cube body named after the entry, in capitals. */
function entryNamed(name: string, savedAt: number): RecoveryEntry {
  return {
    id: `session-${name}`,
    name,
    text: stored(cubeNamed(name.toUpperCase())),
    savedAt,
    handle: null,
    folder: null,
    digest: null,
    reason: 'autosave',
  }
}

describe('the startup offer (3.1 step 5, 3.19)', () => {
  it('U-159 offers every entry one at a time, newest first, each with Open and Discard', async () => {
    vi.useFakeTimers()
    const recovery = createMemoryRecovery()
    const [a, b, c] = [entryNamed('a', 1000), entryNamed('b', 2000), entryNamed('c', 3000)]
    await writeAll(recovery, [a, b, c])
    const fake = await startFakeApp({ recovery })

    expect(offers(fake.app)).toEqual([
      `Partsmith kept unsaved changes to c from ${shownTime(3000)}, and to 2 other documents.`,
    ])
    const first = notificationWith(fake.app, OFFER)
    expect(first).toMatchObject({ severity: 'info', code: 'recovered_more' })
    expect(actionLabels(first)).toEqual(['Open', 'Discard'])
    // Persistent: it outlives the 5 s of other info notifications.
    await advance(10_000)
    expect(offers(fake.app)).toHaveLength(1)

    await clickAction(fake.app, first, 'Discard')
    await expect(recovery.list()).resolves.toEqual([b, a])
    expect(offers(fake.app)).toEqual([
      `Partsmith kept unsaved changes to b from ${shownTime(2000)}, and to 1 other document.`,
    ])

    await clickAction(fake.app, notificationWith(fake.app, OFFER), 'Open')
    const opened = currentDocument(fake.app)
    expect(opened).toMatchObject({ name: 'b', localUnsaved: true })
    expect(bodiesOf(opened.recipe)[0]?.name).toBe('B')
    expect(opened.recoveryId).not.toBe(b.id)
    await expect(recovery.list()).resolves.toEqual([{ ...b, id: opened.recoveryId }, a])
    expect(offers(fake.app)).toEqual([
      `Partsmith kept unsaved changes to a from ${shownTime(1000)}.`,
    ])
    const last = notificationWith(fake.app, OFFER)
    expect(last).toMatchObject({ severity: 'info', code: 'recovered' })
    expect(actionLabels(last)).toEqual(['Open', 'Discard'])

    dismiss(fake.app, last.id)
    await advance(10_000)
    expect(offers(fake.app)).toEqual([])
    expect(await recovery.list()).toContainEqual(a)
  })

  it('U-159 Reopen of a stashed entry moves it to a new session id', async () => {
    const recovery = createMemoryRecovery()
    const handle = createFakeFileHandle('cube.ptsm')
    const fake = await startWithDocument('cube.ptsm', CUBE_DOCUMENT, { recovery, handle })
    await applyEdits(fake.app, 'Rename', renameBody('b-cube', 'Box'))
    await settle()
    const stashedSession = currentDocument(fake.app).recoveryId
    await newDocument(fake.app)
    await settle()
    const [stash] = await recovery.list()
    expect(stash).toMatchObject({ id: stashedSession, reason: 'replaced', handle })

    await clickAction(fake.app, notificationWith(fake.app, MESSAGES.stashed('cube')), 'Reopen')
    const reopened = currentDocument(fake.app)
    expect(reopened).toMatchObject({ name: 'cube', localUnsaved: true })
    expect(reopened.file?.handle).toBe(handle)
    expect(reopened.recoveryId).not.toBe(stashedSession)
    await expect(recovery.list()).resolves.toEqual([
      { ...stash, id: reopened.recoveryId, reason: 'autosave' },
    ])
  })

  it('U-168 an opened entry stays when its new copy is not stored', async () => {
    const memory = createMemoryRecovery()
    const kept = entryNamed('b', 2000)
    await memory.write(kept)
    // A full IndexedDB: nothing new is stored.
    const full: RecoveryStore = { ...memory, write: async () => false }
    const fake = await startFakeApp({ recovery: full })
    await clickAction(fake.app, notificationWith(fake.app, OFFER), 'Open')
    expect(currentDocument(fake.app)).toMatchObject({ name: 'b', localUnsaved: true })
    await expect(full.list()).resolves.toEqual([kept])
  })

  it('U-159 an Open the core refuses keeps the entry it tried to open', async () => {
    const recovery = createMemoryRecovery()
    const kept: RecoveryEntry = { ...entryNamed('broken', 1000), text: 'not json' }
    await recovery.write(kept)
    const fake = await startFakeApp({ recovery })
    fake.transport.handle('open_document', () => ({
      error: { code: 'parse_error', message: 'The file is not valid JSON: line 1, column 1.' },
    }))
    await clickAction(fake.app, notificationWith(fake.app, OFFER), 'Open')
    expect(notificationCoded(fake.app, 'open_failed').severity).toBe('error')
    expect(currentDocument(fake.app).name).toBe('Untitled')
    await expect(recovery.list()).resolves.toEqual([kept])
  })
})
