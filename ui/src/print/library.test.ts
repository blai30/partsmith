import { afterEach, describe, expect, it, vi } from 'vitest'
import { startCore } from '../app/actions/document'
import { DATABASE_NAME } from '../files/database'
import type { RecoveryEnvironment } from '../files/recovery'
import type { Document, PrinterProfile } from '../generated/protocol'
import {
  actionLabels,
  clickAction,
  createFakeApp,
  labeledEdits,
  notificationCoded,
  notificationCodes,
  openTestDocument,
  printerLibraryHolding,
  startFakeApp,
  type FakeApp,
} from '../test/fakes/app'
import { CUBE_DOCUMENT } from '../test/fakes/documents'
import { createFakeIndexedDb } from '../test/fakes/indexed-db'
import { GENERIC_FDM, GENERIC_RESIN, MK4, MK4B, MY_MARS } from '../test/fakes/printers'
import { settle } from '../test/fakes/settle'
import { createFakeStorage } from '../test/fakes/storage'
import { createFakeTauri } from '../test/fakes/tauri'
import { createIndexedDbPrinterLibrary, createMemoryPrinterLibrary } from './library'

// The printer library of ui.md 2.15 and 3.17 (ADR 0062): written forms in the IndexedDB store
// `printers`, put into every new core at startup and after a restart, and the offers a document
// that stores a profile gets when it opens.

/** No Web Locks and no storage manager. */
const BARE: RecoveryEnvironment = { locks: null, storage: null }

/** A profile the fake core refuses with `invalid_profile` (U-257). */
const A_BAD: PrinterProfile = { ...MK4, id: 'a-bad', name: 'A bad', layer_height: 0.5 }

const TALL_LAYER =
  'Layer height 0.5 mm is larger than the 0.4 mm nozzle. Use a layer height of 0.4 mm or less.'

/** `cube.ptsm` at format version 3, naming `my-mk4` and storing `MK4`. */
const STORES_MK4: Document = {
  ...CUBE_DOCUMENT,
  format_version: 3,
  printer: 'my-mk4',
  printer_profile: MK4,
}

function blocked(): never {
  throw new DOMException('The user denied permission to access the database.', 'SecurityError')
}

/** An IDBFactory whose every call fails, as in a private window that blocks IndexedDB. */
function blockedFactory(): IDBFactory {
  return {
    open: blocked,
    deleteDatabase: blocked,
    cmp: blocked,
    databases: () => Promise.reject(new DOMException('Blocked.', 'SecurityError')),
  }
}

/** The ids of the profiles that `put_printer_profile` requests carried, in order. */
function putIds(fake: FakeApp): (string | undefined)[] {
  return fake.transport.commandsOfType('put_printer_profile').map((command) => command.profile?.id)
}

/** The commands of the printer step and of opening a document, in the order the core got them. */
function printerSteps(fake: FakeApp): string[] {
  return fake.transport.commands.flatMap((command) => {
    switch (command.type) {
      case 'put_printer_profile':
        return [`put ${command.profile?.id ?? ''}`]
      case 'list_printer_profiles':
      case 'open_document':
        return [command.type]
      default:
        return []
    }
  })
}

afterEach(() => {
  vi.restoreAllMocks()
  vi.unstubAllGlobals()
})

describe('createIndexedDbPrinterLibrary (2.15)', () => {
  it('U-254 keeps written forms, sorted by id, and replaces or removes only the one with an id', async () => {
    const fake = createFakeIndexedDb()
    const library = createIndexedDbPrinterLibrary(fake.factory, BARE)
    await expect(library.put(MK4)).resolves.toBe(true)
    await expect(library.put(MY_MARS)).resolves.toBe(true)
    await expect(library.list()).resolves.toEqual([MY_MARS, MK4])
    await library.put(MK4B)
    await expect(library.list()).resolves.toEqual([MY_MARS, MK4B])
    await library.remove('my-mars')
    await expect(library.list()).resolves.toEqual([MK4B])
  })

  it('U-254 stores each profile as its written form alone, keyed by its id', async () => {
    const fake = createFakeIndexedDb()
    await createIndexedDbPrinterLibrary(fake.factory, BARE).put(MK4)
    expect(fake.keyPaths(DATABASE_NAME)).toMatchObject({ printers: 'id' })
    expect(fake.records(DATABASE_NAME, 'printers')).toEqual([MK4])
  })

  it('U-255 skips a record that is not a profile and keeps it in the store', async () => {
    const fake = createFakeIndexedDb()
    const library = createIndexedDbPrinterLibrary(fake.factory, BARE)
    await library.put(MK4)
    fake.putRecord(DATABASE_NAME, 'printers', { id: 'x', name: 'X' })
    await expect(library.list()).resolves.toEqual([MK4])
    expect(fake.records(DATABASE_NAME, 'printers')).toContainEqual({ id: 'x', name: 'X' })
  })

  it('skips a record whose member has the wrong JSON type', async () => {
    const fake = createFakeIndexedDb()
    const library = createIndexedDbPrinterLibrary(fake.factory, BARE)
    // Listing opens the database, so the store exists for the record.
    await library.list()
    fake.putRecord(DATABASE_NAME, 'printers', { ...MK4, layer_height: '0.15' })
    await expect(library.list()).resolves.toEqual([])
  })

  it('lists a resin profile, which has no nozzle', async () => {
    const fake = createFakeIndexedDb()
    const library = createIndexedDbPrinterLibrary(fake.factory, BARE)
    await library.put(MY_MARS)
    await expect(library.list()).resolves.toEqual([MY_MARS])
  })

  it('U-256 never throws: a failing IndexedDB lists nothing, keeps nothing and warns about the printer store', async () => {
    const warn = vi.spyOn(console, 'warn').mockImplementation(() => undefined)
    const library = createIndexedDbPrinterLibrary(blockedFactory(), BARE)
    await expect(library.list()).resolves.toEqual([])
    await expect(library.put(MK4)).resolves.toBe(false)
    await expect(library.remove('my-mk4')).resolves.toBeUndefined()
    expect(warn).toHaveBeenCalled()
    expect(warn.mock.calls.flat().map(String).join(' ')).toMatch(/printer/i)
  })

  it("asks once for persistence on the page's first put, as the recovery store's first write does", async () => {
    const fake = createFakeIndexedDb()
    const storage = createFakeStorage({ persisted: false })
    const library = createIndexedDbPrinterLibrary(fake.factory, {
      locks: null,
      storage: storage.storage,
    })
    await library.put(MK4)
    await settle()
    expect(storage.calls).toEqual({ persisted: 1, persist: 1 })
    await library.put(MY_MARS)
    await settle()
    expect(storage.calls).toEqual({ persisted: 1, persist: 1 })
  })

  it('desktop 74 needs no host command: the library works over IndexedDB and invokes nothing', async () => {
    const tauri = createFakeTauri()
    vi.stubGlobal('__TAURI__', tauri.tauri)
    const fake = createFakeIndexedDb()
    const library = createIndexedDbPrinterLibrary(fake.factory, BARE)
    await library.put(MK4)
    await expect(library.list()).resolves.toEqual([MK4])
    await library.remove('my-mk4')
    await expect(library.list()).resolves.toEqual([])
    expect(tauri.invocations).toEqual([])
  })
})

describe('createMemoryPrinterLibrary (2.15)', () => {
  it('keeps profiles by id, sorted by id, and removes only the one with an id', async () => {
    const library = createMemoryPrinterLibrary()
    await expect(library.put(MK4)).resolves.toBe(true)
    await library.put(MY_MARS)
    await library.put(MK4B)
    await expect(library.list()).resolves.toEqual([MY_MARS, MK4B])
    await library.remove('my-mk4')
    await expect(library.list()).resolves.toEqual([MY_MARS])
  })
})

describe('the printer step of startup and restart (3.1 step 4)', () => {
  it('U-257 puts the library into the session in id order, skips a refused profile with a warning and keeps it', async () => {
    const warn = vi.spyOn(console, 'warn').mockImplementation(() => undefined)
    const library = await printerLibraryHolding([MK4, A_BAD])
    const fake = createFakeApp({ printerLibrary: library })
    fake.transport.handle('put_printer_profile', (command, core) =>
      command.profile?.id === 'a-bad'
        ? { error: { code: 'invalid_profile', message: TALL_LAYER } }
        : core.reply(command),
    )
    await startCore(fake.app)
    await settle()
    expect(printerSteps(fake)).toEqual(['put a-bad', 'put my-mk4', 'list_printer_profiles'])
    expect(fake.app.store.getState().printers).toEqual([GENERIC_FDM, GENERIC_RESIN, MK4])
    expect(warn.mock.calls.flat().map(String).join(' ')).toContain('a-bad')
    await expect(library.list()).resolves.toEqual([A_BAD, MK4])
  })

  it('U-294 a restart puts the library into the new core, then lists the profiles, then reopens the document', async () => {
    const fake = await startFakeApp({ printerLibrary: await printerLibraryHolding([MK4, MY_MARS]) })
    await openTestDocument(fake, 'cube.ptsm', CUBE_DOCUMENT)
    fake.transport.die('The worker crashed.')
    await settle()
    fake.transport.clearRequests()
    await fake.app.restartCore()
    await settle()
    expect(printerSteps(fake)).toEqual([
      'put my-mars',
      'put my-mk4',
      'list_printer_profiles',
      'open_document',
    ])
    expect(putIds(fake)).toEqual(['my-mars', 'my-mk4'])
  })
})

describe('offering a stored profile (3.17)', () => {
  it('U-264 offers to add a stored profile no listed profile has the id of', async () => {
    const library = createMemoryPrinterLibrary()
    const fake = await startFakeApp({ printerLibrary: library })
    await openTestDocument(fake, 'custom-printer.ptsm', STORES_MK4)
    const offer = notificationCoded(fake.app, 'printer_offer_add')
    expect(offer.severity).toBe('info')
    expect(offer.message).toBe(
      "custom-printer.ptsm uses the printer 'My MK4', which is not among your printers.",
    )
    expect(actionLabels(offer)).toEqual(["Add 'My MK4' to my printers"])
    fake.transport.clearRequests()
    await clickAction(fake.app, offer, "Add 'My MK4' to my printers")
    expect(
      fake.transport.commandsOfType('put_printer_profile').map((command) => command.profile),
    ).toEqual([MK4])
    await expect(library.list()).resolves.toEqual([MK4])
    expect(labeledEdits(fake)).toEqual([])
    const added = notificationCoded(fake.app, 'printer_added')
    expect(added.severity).toBe('success')
    expect(added.message).toBe("Added 'My MK4' to your printers.")
  })

  it('U-264 offers the listed profile when the stored one differs from it', async () => {
    const fake = await startFakeApp({ printerLibrary: await printerLibraryHolding([MK4B]) })
    await openTestDocument(fake, 'custom-printer.ptsm', STORES_MK4)
    expect(notificationCodes(fake.app)).not.toContain('printer_offer_add')
    const offer = notificationCoded(fake.app, 'printer_offer_differs')
    expect(offer.severity).toBe('info')
    expect(offer.message).toBe(
      "custom-printer.ptsm keeps its own copy of the printer 'My MK4', which differs from yours.",
    )
    expect(actionLabels(offer)).toEqual(["Use my 'My MK4'"])
    fake.transport.clearRequests()
    await clickAction(fake.app, offer, "Use my 'My MK4'")
    expect(labeledEdits(fake)).toEqual([
      { label: 'Change printer', edits: [{ type: 'set_printer', printer: 'my-mk4' }] },
    ])
  })

  it('U-264 offers nothing when the listed profile is equal to the stored one', async () => {
    const fake = await startFakeApp({ printerLibrary: await printerLibraryHolding([MK4]) })
    await openTestDocument(fake, 'custom-printer.ptsm', STORES_MK4)
    expect(notificationCodes(fake.app)).not.toContain('printer_offer_add')
    expect(notificationCodes(fake.app)).not.toContain('printer_offer_differs')
  })

  it('offers nothing for a document that stores no profile, nor after a restart of the core', async () => {
    const fake = await startFakeApp()
    await openTestDocument(fake, 'cube.ptsm', CUBE_DOCUMENT)
    expect(notificationCodes(fake.app)).not.toContain('printer_offer_add')
    const stored = await startFakeApp()
    await openTestDocument(stored, 'custom-printer.ptsm', STORES_MK4)
    stored.app.store.setState({ notifications: [] })
    stored.transport.die('The worker crashed.')
    await settle()
    await stored.app.restartCore()
    await settle()
    expect(notificationCodes(stored.app)).not.toContain('printer_offer_add')
  })
})
