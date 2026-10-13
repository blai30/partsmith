import { startFeatureCommand } from '../../app/actions/command'
import { openFile, startCore } from '../../app/actions/document'
import { createApp, type App, type Platform } from '../../app/createApp'
import type { FileHandle } from '../../files/file-service'
import { createMemoryRecentFiles, type RecentFilesStore } from '../../files/recent'
import { createMemoryRecovery, type RecoveryStore } from '../../files/recovery'
import type { Document, Edit } from '../../generated/protocol'
import type { AppStore, DocumentState } from '../../state/store'
import { currentDocument, LINUX_WEB } from './test-app'
import { CUBE_DOCUMENT, documentFile, featuresOf } from './documents'
import { createFakeFileService, type FakeFileService } from './files'
import { editsOf, withoutNulls } from './protocol'
import { settle } from './settle'
import { createFakeTransport, type FakeTransport, type FakeTransportOptions } from './transport'

// Two ways to get an App in tests: createTestApp (from ./test-app, re-exported here) assembles one by
// hand around a FakeCoreApi, and createFakeApp builds the real one with createApp over a FakeTransport
// (the in-memory core of ./core) and a FakeFileService, so actions, the core client and event
// handling run as in the product.

export * from './test-app'

/** The real app over fakes, with the fakes for inspection. */
export type FakeApp = {
  readonly app: App
  readonly transport: FakeTransport
  readonly files: FakeFileService
  readonly recent: RecentFilesStore
  readonly recovery: RecoveryStore
  /** "command:<type>" per request, "files.<method>" per file service call and "recovery.<method>" per recovery store call, in order. */
  readonly journal: string[]
}

export type FakeAppOptions = {
  readonly core?: FakeCoreOptions
  readonly fileSystemAccess?: boolean
  readonly platform?: Platform
  readonly store?: AppStore
  readonly recovery?: RecoveryStore
  readonly now?: () => number
}

type FakeCoreOptions = Omit<FakeTransportOptions, 'journal'>

/** A recovery store that also logs "recovery.<method>" to the journal. */
function journaledRecovery(recovery: RecoveryStore, journal: string[]): RecoveryStore {
  return {
    list: () => {
      journal.push('recovery.list')
      return recovery.list()
    },
    write: (entry) => {
      journal.push('recovery.write')
      return recovery.write(entry)
    },
    remove: (id) => {
      journal.push('recovery.remove')
      return recovery.remove(id)
    },
  }
}

/** createApp over a FakeTransport and a FakeFileService; the core is not started. */
export function createFakeApp(options: FakeAppOptions = {}): FakeApp {
  const journal: string[] = []
  const transport = createFakeTransport({ ...options.core, journal })
  const files = createFakeFileService({
    fileSystemAccess: options.fileSystemAccess ?? true,
    journal,
  })
  const recent = createMemoryRecentFiles()
  const recovery = journaledRecovery(options.recovery ?? createMemoryRecovery(), journal)
  const app = createApp({
    transport,
    files,
    recent,
    recovery,
    platform: options.platform ?? LINUX_WEB,
    ...(options.store === undefined ? {} : { store: options.store }),
    ...(options.now === undefined ? {} : { now: options.now }),
  })
  return { app, transport, files, recent, recovery, journal }
}

/** createFakeApp, then startCore, which ends with the new "Untitled" document open (ui.md 3.1). */
export async function startFakeApp(options: FakeAppOptions = {}): Promise<FakeApp> {
  const fake = createFakeApp(options)
  await startCore(fake.app)
  await settle()
  currentDocument(fake.app)
  return fake
}

/** Opens a recipe through `openFile` as if the user had picked `name`, and waits until it is applied. */
export async function openTestDocument(
  fake: FakeApp,
  name: string,
  document: Document,
  handle: FileHandle | null = null,
): Promise<DocumentState> {
  await openFile(fake.app, documentFile(name, document, handle))
  await settle()
  const opened = currentDocument(fake.app)
  if (featuresOf(opened.recipe).length !== featuresOf(document).length) {
    throw new Error(`${name} did not open.`)
  }
  return opened
}

/** startFakeApp, then openTestDocument, then forgets the requests made so far. */
export async function startWithDocument(
  name: string,
  document: Document,
  options: FakeAppOptions & { readonly handle?: FileHandle | null } = {},
): Promise<FakeApp> {
  const fake = await startFakeApp(options)
  await openTestDocument(fake, name, document, options.handle ?? null)
  fake.transport.clearRequests()
  fake.journal.length = 0
  return fake
}

/** cube.ptsm open with a Fillet command started on it. */
export async function filletOnCube(options: FakeAppOptions = {}): Promise<FakeApp> {
  const fake = await startWithDocument('cube.ptsm', CUBE_DOCUMENT, options)
  await startFeatureCommand(fake.app, 'fillet')
  await settle()
  return fake
}

/** The edits of each apply_edits request, in order. */
export function appliedEdits(fake: FakeApp): readonly (readonly Edit[])[] {
  return fake.transport.commandsOfType('apply_edits').map((command) => editsOf(command))
}

/** The apply_edits requests as { label, edits }, null fields removed. */
export function labeledEdits(fake: FakeApp): unknown[] {
  return fake.transport
    .commandsOfType('apply_edits')
    .map((command) => withoutNulls({ label: command.label, edits: editsOf(command) }))
}
