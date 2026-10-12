import { startCore, openFile } from '../../app/actions/document'
import { createApp, type App, type Platform } from '../../app/createApp'
import { createCommandRegistry, type CommandRegistry } from '../../commands/registry'
import type { CoreApi } from '../../core/api'
import { createMutationQueue } from '../../core/queue'
import type { FileService } from '../../files/file-service'
import { createMemoryRecentFiles, type RecentFilesStore } from '../../files/recent'
import { createMemoryRecovery, type RecoveryStore } from '../../files/recovery'
import type { Document } from '../../generated/protocol'
import { createAppStore, type AppStore, type DocumentState } from '../../state/store'
import type { ViewportHandle } from '../../viewport/controller'
import { createFakeCoreApi } from './core-api'
import { documentFile } from './documents'
import { createFakeFileService, type FakeFileService } from './files'
import { createFakeTransport, type FakeTransport, type FakeTransportOptions } from './transport'

// Two ways to get an App in tests: createTestApp assembles one by hand around a FakeCoreApi (no
// events reach the store), and createFakeApp builds the real one with createApp over a FakeTransport,
// so actions, the core client and event handling run as in the product.

export const LINUX_WEB: Platform = { operatingSystem: 'linux', host: 'web' }
export const MAC_WEB: Platform = { operatingSystem: 'mac', host: 'web' }
export const WINDOWS_DESKTOP: Platform = { operatingSystem: 'windows', host: 'desktop' }

// Captured before any test installs fake timers, so settle() always waits on the real clock.
const realSetTimeout = globalThis.setTimeout

/**
 * Lets pending promise chains (requests through the fake transport, their events and the actions
 * awaiting them) run to completion. Works with real and fake timers alike: it waits for real
 * macrotasks and never advances fake time.
 */
export async function settle(): Promise<void> {
  for (let round = 0; round < 10; round += 1) {
    await new Promise<void>((resolve) => {
      realSetTimeout(resolve, 0)
    })
  }
}

export type TestAppOptions = {
  readonly store?: AppStore
  readonly core?: CoreApi
  readonly commands?: CommandRegistry
  readonly files?: FileService
  readonly recent?: RecentFilesStore
  readonly recovery?: RecoveryStore
  readonly platform?: Platform
  readonly viewport?: ViewportHandle | null
}

/** An App assembled by hand around a FakeCoreApi, a real mutation queue and in-memory stores. */
export function createTestApp(options: TestAppOptions = {}): App {
  let viewport = options.viewport ?? null
  return {
    store: options.store ?? createAppStore(),
    core: options.core ?? createFakeCoreApi(),
    mutations: createMutationQueue(),
    commands: options.commands ?? createCommandRegistry(),
    files: options.files ?? createFakeFileService(),
    recent: options.recent ?? createMemoryRecentFiles(),
    recovery: options.recovery ?? createMemoryRecovery(),
    platform: options.platform ?? LINUX_WEB,
    viewport: () => viewport,
    attachViewport: (handle) => {
      viewport = handle
    },
    restartCore: () => Promise.resolve(),
  }
}

/** The real app over fakes, with the fakes for inspection. */
export type FakeApp = {
  readonly app: App
  readonly transport: FakeTransport
  readonly files: FakeFileService
  readonly recent: RecentFilesStore
  readonly recovery: RecoveryStore
  /** "command:<type>" per request and "files.<method>" per file service call, in order. */
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

/** createApp over a FakeTransport and a FakeFileService; the core is not started. */
export function createFakeApp(options: FakeAppOptions = {}): FakeApp {
  const journal: string[] = []
  const transport = createFakeTransport({ ...options.core, journal })
  const files = createFakeFileService({
    fileSystemAccess: options.fileSystemAccess ?? true,
    journal,
  })
  const recent = createMemoryRecentFiles()
  const recovery = options.recovery ?? createMemoryRecovery()
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

/** The open document; throws when there is none. */
export function currentDocument(app: App): DocumentState {
  const document = app.store.getState().document
  if (document === null) {
    throw new Error('The app has no open document.')
  }
  return document
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
  handle: FileSystemFileHandle | null = null,
): Promise<DocumentState> {
  await openFile(fake.app, documentFile(name, document, handle))
  await settle()
  const opened = currentDocument(fake.app)
  if (opened.recipe.features.length !== document.features.length) {
    throw new Error(`${name} did not open.`)
  }
  return opened
}

/** startFakeApp, then openTestDocument, then forgets the requests made so far. */
export async function startWithDocument(
  name: string,
  document: Document,
  options: FakeAppOptions & { readonly handle?: FileSystemFileHandle | null } = {},
): Promise<FakeApp> {
  const fake = await startFakeApp(options)
  await openTestDocument(fake, name, document, options.handle ?? null)
  fake.transport.clearRequests()
  fake.journal.length = 0
  return fake
}
