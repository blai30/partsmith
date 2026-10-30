import { afterEach, describe, expect, it, vi } from 'vitest'
import { fileDigest } from '../../files/digest'
import { FileServiceError } from '../../files/file-service'
import { createMemoryRecovery, type RecoveryEntry } from '../../files/recovery'
import type { Edit } from '../../generated/protocol'
import type { Notification } from '../../state/store'
import {
  actionLabels,
  clickAction,
  currentDocument,
  type FakeApp,
  forgetCalls,
  journalSteps,
  notificationCoded,
  notificationCodes,
  runAction,
  startFakeApp,
  startWithDocument,
} from '../../test/fakes/app'
import { CUBE_DOCUMENT, bodiesOf, cubeNamed, documentText, utf8 } from '../../test/fakes/documents'
import { callsOf, calledMethods, createFakeFileHandle } from '../../test/fakes/files'
import { deferred } from '../../test/fakes/deferred'
import { settle } from '../../test/fakes/settle'
import { applyEdits, newDocument, save, saveAs } from './document'

// Save stops when the file was changed elsewhere (ui.md 3.19 step 2, ADR 0037 decision 1): the
// digest of what the session last read or wrote against the file read again right before
// anything is written. U-223 to U-225 of ui.md 6.4.

const CONFLICT =
  'cube.ptsm was changed elsewhere after you opened or saved it, so Partsmith did not save over it.'

function rename(name: string): readonly Edit[] {
  return [{ type: 'update_body', body: { id: 'b-cube', name } }]
}

/** `T`: the bytes of `cube.ptsm` as it was opened. */
const ORIGINAL = documentText(CUBE_DOCUMENT)

/** `T2`: the same file after another program renamed its body to "Lid". */
const CHANGED = documentText(cubeNamed('Lid'))

afterEach(() => {
  vi.restoreAllMocks()
})

/** The text of the last write. */
function writtenText(fake: FakeApp): string {
  return new TextDecoder().decode(fake.files.writes.at(-1)?.bytes)
}

/**
 * Holds the first `write` of the fake file service until `release`, as a slow desktop write or a
 * permission prompt does; `started` counts the writes begun so far.
 */
function holdFirstWrite(fake: FakeApp): {
  readonly started: () => number
  readonly release: () => void
} {
  const held = deferred<void>()
  const write = fake.files.write
  const spy = vi.spyOn(fake.files, 'write').mockImplementationOnce(async (...args) => {
    await held.promise
    return write(...args)
  })
  return { started: () => spy.mock.calls.length, release: () => held.resolve() }
}

/** `cube.ptsm` opened from the web handle `H` holding `T`, then its body renamed to "Box". */
async function renamedCube(): Promise<{
  readonly fake: FakeApp
  readonly handle: FileSystemFileHandle
}> {
  const handle = createFakeFileHandle('cube.ptsm')
  const fake = await startWithDocument('cube.ptsm', CUBE_DOCUMENT, { handle })
  await applyEdits(fake.app, 'Rename', rename('Box'))
  await settle()
  forgetCalls(fake)
  return { fake, handle }
}

/** U-223 (c): the save that found `H` holding `T2` and stopped with `save_conflict`. */
async function conflicted(): Promise<{
  readonly fake: FakeApp
  readonly handle: FileSystemFileHandle
  readonly notification: Notification
}> {
  const { fake, handle } = await renamedCube()
  fake.files.contents.set(handle, CHANGED)
  await save(fake.app)
  await settle()
  const notification = notificationCoded(fake.app, 'save_conflict')
  forgetCalls(fake)
  return { fake, handle, notification }
}

describe('the change check (3.19 step 2)', () => {
  it('U-223 the file reference keeps the digest of the bytes the file was opened from', async () => {
    const { fake, handle } = await renamedCube()
    expect(currentDocument(fake.app).file).toEqual({
      name: 'cube.ptsm',
      handle,
      folder: null,
      digest: await fileDigest(utf8(ORIGINAL)),
    })
  })

  it('U-223 (a) an unchanged file is read with readwrite, then written and marked saved', async () => {
    const { fake, handle } = await renamedCube()
    await save(fake.app)
    await settle()
    expect(journalSteps(fake)).toEqual([
      'command:save_document',
      'files.readHandle',
      'files.write',
      'command:mark_saved',
    ])
    const [read] = callsOf(fake.files, 'readHandle')
    expect(read?.[0]).toBe(handle)
    expect(read?.[1]).toBe('readwrite')
    expect(currentDocument(fake.app).file?.digest).toBe(await fileDigest(utf8(writtenText(fake))))
    expect(currentDocument(fake.app).dirty).toBe(false)
  })

  it('U-223 (b) a file rewritten elsewhere with the same bytes counts as unchanged', async () => {
    const { fake, handle } = await renamedCube()
    fake.files.contents.set(handle, `${ORIGINAL}`)
    await save(fake.app)
    await settle()
    expect(journalSteps(fake)).toEqual([
      'command:save_document',
      'files.readHandle',
      'files.write',
      'command:mark_saved',
    ])
    expect(notificationCodes(fake.app)).not.toContain('save_conflict')
  })

  it('U-223 (c) a file holding other bytes stops the save before anything is written', async () => {
    const { fake, handle } = await renamedCube()
    fake.files.contents.set(handle, CHANGED)
    await save(fake.app)
    await settle()
    expect(calledMethods(fake.files)).toEqual(['readHandle'])
    expect(fake.transport.commandTypes()).toEqual(['save_document'])
    expect(currentDocument(fake.app).dirty).toBe(true)
    expect(fake.files.contents.get(handle)).toBe(CHANGED)
    const notification = notificationCoded(fake.app, 'save_conflict')
    expect(notification.severity).toBe('error')
    expect(notification.message).toBe(CONFLICT)
    expect(actionLabels(notification)).toEqual([
      'Overwrite cube.ptsm',
      'Save as',
      'Open the newer file',
    ])
  })

  it('U-223 (d) a file that is gone shows save_not_found and writes nothing', async () => {
    const { fake } = await renamedCube()
    fake.files.readError = new FileServiceError('not_found', 'cube.ptsm was not found.')
    await save(fake.app)
    await settle()
    expect(callsOf(fake.files, 'write')).toEqual([])
    expect(notificationCoded(fake.app, 'save_not_found').message).toBe(
      'cube.ptsm was moved or deleted. Save it again to choose a location.',
    )
  })

  it('U-223 (d) a refused permission shows save_permission and writes nothing', async () => {
    const { fake } = await renamedCube()
    fake.files.readError = new FileServiceError(
      'permission',
      'Partsmith has no access to cube.ptsm.',
    )
    await save(fake.app)
    await settle()
    expect(callsOf(fake.files, 'write')).toEqual([])
    expect(notificationCoded(fake.app, 'save_permission').message).toBe(
      'Could not save cube.ptsm because Partsmith no longer has permission to write it.',
    )
  })

  it('U-223 (e) without Web Crypto the digest is null with a warning, and Save writes without the check', async () => {
    const warn = vi.spyOn(console, 'warn').mockImplementation(() => undefined)
    vi.spyOn(crypto.subtle, 'digest').mockRejectedValue(
      new DOMException('Web Crypto needs a secure context.', 'NotSupportedError'),
    )
    const handle = createFakeFileHandle('cube.ptsm')
    const fake = await startWithDocument('cube.ptsm', CUBE_DOCUMENT, { handle })
    expect(currentDocument(fake.app).file?.digest).toBeNull()
    expect(warn).toHaveBeenCalledTimes(1)
    await applyEdits(fake.app, 'Rename', rename('Box'))
    await settle()
    forgetCalls(fake)
    await save(fake.app)
    await settle()
    expect(journalSteps(fake)).toEqual([
      'command:save_document',
      'files.write',
      'command:mark_saved',
    ])
  })

  it('U-223 (f) a second Save pressed while the first writes waits for it and shows no save_conflict', async () => {
    const { fake, handle } = await renamedCube()
    const writing = holdFirstWrite(fake)
    const first = save(fake.app)
    await settle()
    expect(writing.started()).toBe(1)
    await applyEdits(fake.app, 'Rename', rename('Crate'))
    const second = save(fake.app)
    await settle()
    expect(fake.transport.commandsOfType('save_document')).toHaveLength(1)
    writing.release()
    await Promise.all([first, second])
    await settle()
    expect(notificationCodes(fake.app)).not.toContain('save_conflict')
    expect(journalSteps(fake)).toEqual([
      'command:save_document',
      'files.readHandle',
      'command:apply_edits',
      'files.write',
      'command:mark_saved',
      'command:save_document',
      'files.readHandle',
      'files.write',
      'command:mark_saved',
    ])
    expect(fake.files.writes.map((write) => write.target)).toEqual([
      { kind: 'handle', name: 'cube.ptsm', handle },
      { kind: 'handle', name: 'cube.ptsm', handle },
    ])
    expect(writtenText(fake)).toContain('"Crate"')
    expect(currentDocument(fake.app).file?.digest).toBe(await fileDigest(utf8(writtenText(fake))))
    expect(currentDocument(fake.app).dirty).toBe(false)
  })

  it('U-223 (f) a Save pressed while a Save as writes goes to the file the Save as chose', async () => {
    const { fake, handle } = await renamedCube()
    const copy = createFakeFileHandle('copy.ptsm')
    fake.files.nextSaveTarget = { kind: 'handle', name: 'copy.ptsm', handle: copy }
    const writing = holdFirstWrite(fake)
    const savedAs = saveAs(fake.app)
    await settle()
    expect(writing.started()).toBe(1)
    const saved = save(fake.app)
    writing.release()
    await Promise.all([savedAs, saved])
    await settle()
    expect(fake.files.writes.map((write) => write.target)).toEqual([
      { kind: 'handle', name: 'copy.ptsm', handle: copy },
      { kind: 'handle', name: 'copy.ptsm', handle: copy },
    ])
    expect(callsOf(fake.files, 'readHandle').map(([read]) => read)).toEqual([copy])
    expect(fake.files.contents.get(handle)).toBe(ORIGINAL)
    expect(notificationCodes(fake.app)).not.toContain('save_conflict')
    expect(currentDocument(fake.app).file?.handle).toBe(copy)
  })

  it('U-223 (f) a Save waiting behind another write saves nothing once New replaced the document', async () => {
    const { fake } = await renamedCube()
    const writing = holdFirstWrite(fake)
    const first = save(fake.app)
    await settle()
    const second = save(fake.app)
    await newDocument(fake.app)
    writing.release()
    await Promise.all([first, second])
    await settle()
    expect(fake.transport.commandsOfType('save_document')).toHaveLength(1)
    expect(fake.files.writes).toHaveLength(1)
  })
})

describe("the conflict's actions (3.19 save_conflict)", () => {
  it('U-224 "Overwrite cube.ptsm" reads again, writes the document and marks it saved', async () => {
    const { fake, handle, notification } = await conflicted()
    await clickAction(fake.app, notification, 'Overwrite cube.ptsm')
    expect(journalSteps(fake)).toEqual([
      'command:save_document',
      'files.readHandle',
      'files.write',
      'command:mark_saved',
    ])
    expect(writtenText(fake)).toContain('"name": "Box"')
    expect(fake.files.contents.get(handle)).toContain('"name": "Box"')
    expect(currentDocument(fake.app).dirty).toBe(false)

    await applyEdits(fake.app, 'Rename', rename('Peg'))
    await settle()
    forgetCalls(fake)
    await save(fake.app)
    await settle()
    expect(notificationCodes(fake.app)).not.toContain('save_conflict')
    expect(writtenText(fake)).toContain('"name": "Peg"')
  })

  it('U-224 "Overwrite cube.ptsm" warns again when the file changed once more, and writes nothing', async () => {
    const { fake, handle, notification } = await conflicted()
    fake.files.contents.set(handle, documentText(cubeNamed('Cap')))
    await clickAction(fake.app, notification, 'Overwrite cube.ptsm')
    expect(callsOf(fake.files, 'write')).toEqual([])
    expect(notificationCoded(fake.app, 'save_conflict').message).toBe(CONFLICT)
    expect(currentDocument(fake.app).dirty).toBe(true)
  })

  it('U-224 "Overwrite cube.ptsm" waits during a feature command', async () => {
    const { notification } = await conflicted()
    expect(
      notification.actions.find((action) => action.label === 'Overwrite cube.ptsm')?.editsDocument,
    ).toBe(true)
  })

  it('U-224 "Open the newer file" opens the file again and keeps the unsaved document as a stash', async () => {
    const { fake, handle, notification } = await conflicted()
    const { recoveryId, recipe } = currentDocument(fake.app)
    await clickAction(fake.app, notification, 'Open the newer file')
    expect(callsOf(fake.files, 'readHandle').map((args) => args[0])).toEqual([handle])
    const document = currentDocument(fake.app)
    expect(bodiesOf(document.recipe)[0]?.name).toBe('Lid')
    expect(document.file?.digest).toBe(await fileDigest(utf8(CHANGED)))
    expect(document.recoveryId).not.toBe(recoveryId)
    const stash = (await fake.recovery.list()).find((entry) => entry.id === recoveryId)
    expect(stash).toMatchObject({
      reason: 'replaced',
      handle,
      text: JSON.stringify(recipe, null, 2),
    })
    const stashed = notificationCoded(fake.app, 'stashed')
    expect(stashed.message).toBe('cube had unsaved changes. They are kept for you.')
    expect(actionLabels(stashed)).toEqual(['Reopen'])
  })

  it('"Open the newer file" whose read fails shows open_failed and keeps the document', async () => {
    const { fake, notification } = await conflicted()
    const { recoveryId } = currentDocument(fake.app)
    fake.files.readError = new FileServiceError('not_found', 'cube.ptsm was not found.')
    await clickAction(fake.app, notification, 'Open the newer file')
    expect(notificationCoded(fake.app, 'open_failed').message).toBe(
      'Could not open cube.ptsm. cube.ptsm was not found.',
    )
    expect(currentDocument(fake.app).recoveryId).toBe(recoveryId)
    expect(fake.transport.commandTypes()).not.toContain('open_document')
  })

  it('U-224 "Save as" asks where and writes there without the change check', async () => {
    const { fake, notification } = await conflicted()
    const target = createFakeFileHandle('cube-copy.ptsm')
    fake.files.nextSaveTarget = { kind: 'handle', name: 'cube-copy.ptsm', handle: target }
    await clickAction(fake.app, notification, 'Save as')
    expect(fake.files.saveRequests).toEqual([{ suggestedName: 'cube.ptsm', kind: 'ptsm' }])
    expect(callsOf(fake.files, 'readHandle')).toEqual([])
    expect(fake.files.writes.at(-1)?.target).toMatchObject({ kind: 'handle', handle: target })
  })

  const ACTIONS = ['Overwrite cube.ptsm', 'Save as', 'Open the newer file'] as const

  it.each(ACTIONS)('U-224 "%s" clicked after New does nothing', async (label) => {
    const { fake, notification } = await conflicted()
    await newDocument(fake.app)
    await settle()
    forgetCalls(fake)
    runAction(notification, label)
    await settle()
    expect(calledMethods(fake.files)).toEqual([])
    expect(fake.transport.commandTypes()).toEqual([])
    expect(currentDocument(fake.app).name).toBe('Untitled')
  })

  it.each(ACTIONS)(
    'U-224 "%s" clicked after a Save as to another file does nothing',
    async (label) => {
      const { fake, notification } = await conflicted()
      const other = createFakeFileHandle('cube-copy.ptsm')
      fake.files.nextSaveTarget = { kind: 'handle', name: 'cube-copy.ptsm', handle: other }
      await saveAs(fake.app)
      await settle()
      expect(currentDocument(fake.app).file?.handle).toBe(other)
      forgetCalls(fake)
      runAction(notification, label)
      await settle()
      expect(calledMethods(fake.files)).toEqual([])
      expect(fake.transport.commandTypes()).toEqual([])
    },
  )
})

/** A recovery entry of cube.ptsm renamed to "Box", from `handle`, with `digest`. */
function entryOf(handle: FileSystemFileHandle, digest: string | null): RecoveryEntry {
  return {
    id: 'kept',
    name: 'cube',
    text: JSON.stringify(cubeNamed('Box'), null, 2),
    savedAt: Date.UTC(2026, 9, 12, 9, 30),
    handle,
    folder: null,
    digest,
    reason: 'autosave',
  }
}

/** Starts with the entry in the recovery store and opens it with the startup "Open". */
async function openedEntry(entry: RecoveryEntry): Promise<FakeApp> {
  const recovery = createMemoryRecovery()
  await recovery.write(entry)
  const fake = await startFakeApp({ recovery })
  runAction(notificationCoded(fake.app, 'recovered'), 'Open')
  await settle()
  forgetCalls(fake)
  return fake
}

describe('no check without a known digest (3.19)', () => {
  it('U-225 an entry without a digest saves to its handle without reading it first', async () => {
    const handle = createFakeFileHandle('cube.ptsm')
    const fake = await openedEntry(entryOf(handle, null))
    fake.files.contents.set(handle, CHANGED)
    expect(currentDocument(fake.app).file?.digest ?? null).toBeNull()
    await save(fake.app)
    await settle()
    expect(callsOf(fake.files, 'readHandle')).toEqual([])
    expect(fake.files.writes.at(-1)?.target).toMatchObject({ kind: 'handle', handle })
  })

  it('U-225 an entry with digest D opens with it, so Save warns when the file no longer hashes to D', async () => {
    const handle = createFakeFileHandle('cube.ptsm')
    const digest = await fileDigest(utf8(ORIGINAL))
    const fake = await openedEntry(entryOf(handle, digest))
    expect(currentDocument(fake.app).file).toEqual({
      name: 'cube.ptsm',
      handle,
      folder: null,
      digest,
    })
    fake.files.contents.set(handle, CHANGED)
    await save(fake.app)
    await settle()
    expect(callsOf(fake.files, 'write')).toEqual([])
    expect(notificationCoded(fake.app, 'save_conflict').message).toBe(CONFLICT)
  })

  it('U-225 an entry with digest D saves when the file still hashes to D', async () => {
    const handle = createFakeFileHandle('cube.ptsm')
    const fake = await openedEntry(entryOf(handle, await fileDigest(utf8(ORIGINAL))))
    fake.files.contents.set(handle, ORIGINAL)
    await save(fake.app)
    await settle()
    expect(journalSteps(fake)).toEqual([
      'command:save_document',
      'files.readHandle',
      'files.write',
      'command:mark_saved',
    ])
  })
})
