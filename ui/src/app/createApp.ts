import { createCommandRegistry, type CommandRegistry } from '../commands/registry'
import { createCoreApi, type CoreApi } from '../core/api'
import { createCoreClient, type CoreClient } from '../core/client'
import type { CoreEvent } from '../core/events'
import { createMutationQueue, type MutationQueue } from '../core/queue'
import { CoreFatalError, type CoreReady, type Transport } from '../core/transport'
import type { FileService } from '../files/file-service'
import type { RecentFilesStore } from '../files/recent'
import type { RecoveryStore } from '../files/recovery'
import { adoptDocument, applyCoreEvent, releaseDocuments } from '../state/core-events'
import { createAppStore, type AppStore } from '../state/store'
import type { ViewportHandle } from '../viewport/controller'
import { installPrintAnalysis } from './actions/print'
import { loadCoreData } from './core-data'
import { installSelectionUpkeep } from './selection-upkeep'

// The app object every action takes (ui.md 2.9): the store, the core API, the mutation queue, the
// command registry and the host services. createApp wires core events into the store, keeps a
// recovery copy of unsaved work, and restarts the core after a fatal error (4.4).

export type Platform = {
  readonly operatingSystem: 'mac' | 'windows' | 'linux' | 'other'
  readonly host: 'web' | 'desktop'
}

export type App = {
  readonly store: AppStore
  readonly core: CoreApi
  readonly mutations: MutationQueue
  readonly commands: CommandRegistry
  readonly files: FileService
  readonly recent: RecentFilesStore
  readonly recovery: RecoveryStore
  readonly platform: Platform
  /** The mounted viewport, or null before the viewport mounts. */
  viewport(): ViewportHandle | null
  attachViewport(handle: ViewportHandle | null): void
  /** Restarts the core after a fatal error and reopens the last recipe (4.4). */
  restartCore(): Promise<void>
}

export type CreateAppOptions = {
  readonly transport: Transport
  readonly files: FileService
  readonly recent: RecentFilesStore
  readonly recovery: RecoveryStore
  readonly platform: Platform
  readonly store?: AppStore
  readonly now?: () => number
}

/** What createApp keeps about an app beyond the App type. */
type AppConnection = {
  readonly transport: Transport
  readonly now: () => number
  readonly eventListeners: Set<(event: CoreEvent) => void>
}

const connections = new WeakMap<App, AppConnection>()

/** Delay after the last document_changed before a dirty document's recovery copy is written (3.19). */
const RECOVERY_DELAY_MILLISECONDS = 2_000

function reasonOf(error: unknown): string {
  return error instanceof Error ? error.message : String(error)
}

/**
 * Starts the app's transport (3.1 step 4). An app assembled without createApp has no transport of
 * its own: its core is taken to be running, and its versions come from engine_info.
 */
export async function startTransport(app: App): Promise<CoreReady> {
  const connection = connections.get(app)
  if (connection === undefined) {
    const info = await app.core.engineInfo()
    return { protocolVersion: info.protocolVersion, engineVersion: info.engineVersion }
  }
  return connection.transport.start()
}

/** The app's clock, in milliseconds since the epoch. */
export function nowOf(app: App): number {
  return (connections.get(app)?.now ?? Date.now)()
}

/**
 * Hears every core event after it reached the store, for the parts of the UI that keep data
 * outside the store, such as the scene's mesh buffers. Returns the unsubscriber.
 */
export function onCoreEvent(app: App, listener: (event: CoreEvent) => void): () => void {
  const listeners = connections.get(app)?.eventListeners
  if (listeners === undefined) {
    return () => undefined
  }
  listeners.add(listener)
  return () => {
    listeners.delete(listener)
  }
}

/** Counts mutations in the store and records how long the last one took. */
function trackedQueue(store: AppStore, now: () => number): MutationQueue {
  const queue = createMutationQueue()
  function setPending(): void {
    store.setState((state) => ({ core: { ...state.core, pendingMutations: queue.pending } }))
  }
  return {
    run(task) {
      const result = queue.run(async () => {
        const started = now()
        try {
          return await task()
        } finally {
          const elapsed = now() - started
          store.setState((state) =>
            state.document === null
              ? {}
              : { document: { ...state.document, lastRebuildMilliseconds: elapsed } },
          )
        }
      })
      setPending()
      result.then(setPending, setPending)
      return result
    },
    get pending() {
      return queue.pending
    },
  }
}

/** Writes a recovery copy of a dirty document 2 s after its last change (3.19). */
function scheduleRecovery(app: App, now: () => number): (event: CoreEvent) => void {
  let timer: ReturnType<typeof setTimeout> | null = null
  return (event) => {
    if (event.type !== 'document_changed') {
      return
    }
    const document = app.store.getState().document
    if (document?.documentHandle !== event.documentHandle) {
      return
    }
    if (timer !== null) {
      clearTimeout(timer)
      timer = null
    }
    if (!document.dirty && !document.localUnsaved) {
      return
    }
    timer = setTimeout(() => {
      timer = null
      const current = app.store.getState().document
      if (current === null || (!current.dirty && !current.localUnsaved)) {
        return
      }
      void app.recovery.write({
        name: current.name,
        text: JSON.stringify(current.recipe, null, 2),
        savedAt: now(),
        handle: current.file?.handle ?? null,
        reason: 'autosave',
      })
    }, RECOVERY_DELAY_MILLISECONDS)
  }
}

export function createApp(options: CreateAppOptions): App {
  const store = options.store ?? createAppStore()
  const now = options.now ?? Date.now
  const eventListeners = new Set<(event: CoreEvent) => void>()
  let viewport: ViewportHandle | null = null

  function dispatch(event: CoreEvent): void {
    applyCoreEvent(store, event)
    for (const listener of Array.from(eventListeners)) {
      try {
        listener(event)
      } catch (error) {
        console.error('A core event listener failed.', error)
      }
    }
  }

  function failed(error: CoreFatalError): void {
    store.setState((state) => ({ core: { ...state.core, status: 'failed', fatal: error.message } }))
  }

  function connect(): CoreClient {
    const connected = createCoreClient(options.transport)
    connected.onEvent(dispatch)
    connected.onFatal(failed)
    return connected
  }

  let client = connect()

  // The API's client follows restarts: each request goes to the client of the running core.
  const core = createCoreApi({
    request: (command) => client.request(command),
    onEvent: (listener) => client.onEvent(listener),
    onFatal: (listener) => client.onFatal(listener),
    close: () => {
      client.close()
    },
  })

  const app: App = {
    store,
    core,
    mutations: trackedQueue(store, now),
    commands: createCommandRegistry(),
    files: options.files,
    recent: options.recent,
    recovery: options.recovery,
    platform: options.platform,
    viewport: () => viewport,
    attachViewport(handle) {
      viewport = handle
    },
    async restartCore() {
      const previous = store.getState().document
      store.setState((state) => ({
        core: { ...state.core, status: 'loading', fatal: null },
        context: state.context === 'sketch' ? 'model' : state.context,
        selection: [],
        hover: null,
        command: null,
        sketch: null,
      }))
      releaseDocuments(store)
      client.close()
      client = connect()
      try {
        const ready = await options.transport.start()
        await loadCoreData(app)
        if (previous !== null) {
          const opened = await core.openDocument(JSON.stringify(previous.recipe, null, 2))
          adoptDocument(store, opened.documentHandle, {
            name: previous.name,
            file: previous.file,
            localUnsaved: true,
          })
        }
        store.setState((state) => ({
          core: { ...state.core, status: 'ready', engineVersion: ready.engineVersion, fatal: null },
        }))
      } catch (error) {
        failed(error instanceof CoreFatalError ? error : new CoreFatalError(reasonOf(error)))
      }
    },
  }
  connections.set(app, { transport: options.transport, now, eventListeners })
  eventListeners.add(scheduleRecovery(app, now))
  installPrintAnalysis(app)
  installSelectionUpkeep(app)
  return app
}

/** Makes the browser ask before leaving the page while the document has unsaved changes (3.19). */
export function installUnloadGuard(app: App, target: Window): () => void {
  function onBeforeUnload(event: BeforeUnloadEvent): void {
    const document = app.store.getState().document
    if (document !== null && (document.dirty || document.localUnsaved)) {
      event.preventDefault()
    }
  }
  target.addEventListener('beforeunload', onBeforeUnload)
  return () => {
    target.removeEventListener('beforeunload', onBeforeUnload)
  }
}

/** The operating system and host the app runs on (2.9). */
export function detectPlatform(
  navigator: Navigator,
  environment: { readonly __TAURI__?: unknown },
): Platform {
  const agentData: unknown = Reflect.get(navigator, 'userAgentData')
  const agentPlatform: unknown =
    typeof agentData === 'object' && agentData !== null
      ? Reflect.get(agentData, 'platform')
      : undefined
  const platform = typeof agentPlatform === 'string' ? agentPlatform : navigator.platform
  // navigator.platform says "MacIntel" or "Win32"; userAgentData says "macOS" or "Windows".
  const operatingSystem = /mac/i.test(platform)
    ? 'mac'
    : /win/i.test(platform)
      ? 'windows'
      : /linux/i.test(platform)
        ? 'linux'
        : 'other'
  return { operatingSystem, host: environment['__TAURI__'] === undefined ? 'web' : 'desktop' }
}
