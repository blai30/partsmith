import { createCommandRegistry, type CommandRegistry } from '../commands/registry'
import { createCoreApi, rebuildsDocument, rebuiltDocument, type CoreApi } from '../core/api'
import { createCoreClient, type CoreClient, type Reply } from '../core/client'
import type { CoreEvent } from '../core/events'
import { createMutationQueue, type MutationQueue } from '../core/queue'
import { CoreFatalError, type Transport } from '../core/transport'
import type { FileService } from '../files/file-service'
import type { RecentFilesStore } from '../files/recent'
import type { RecoveryStore } from '../files/recovery'
import type { Command, DocHandle, Document } from '../generated/protocol'
import { TIMING_NAMES } from '../performance/marks'
import {
  adoptDocument,
  applyCoreEvent,
  recordRebuildTime,
  releaseDocuments,
} from '../state/core-events'
import { createAppStore, type AppStore, type FileReference } from '../state/store'
import { initialSessionVisibility } from '../state/visibility'
import type { ViewportHandle } from '../viewport/controller'
import { isUnsaved } from './actions/edits'
import { installFailureDismissal } from './actions/notifications'
import { installPrintAnalysis } from './actions/print'
import { connectApp } from './connection'
import { loadCoreData } from './core-data'
import { installSelectionUpkeep } from './selection-upkeep'
import { measureToNextFrame } from './timing'

// The app object every action takes (ui.md 2.9): the store, the core API, the mutation queue, the
// command registry and the host services. createApp wires core events into the store, counts and
// times the requests that rebuild a document, keeps a recovery copy of unsaved work, and restarts
// the core after a fatal error (4.4).

export { onCoreEvent } from './connection'

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

/** What a core restart opens again: the document shown when the core stopped (4.4). */
type Reopening = {
  readonly recipe: Document
  readonly name: string
  readonly file: FileReference | null
}

/** Delay after the last document_changed before a dirty document's recovery copy is written (3.19). */
const RECOVERY_DELAY_MILLISECONDS = 2_000

function reasonOf(error: unknown): string {
  return error instanceof Error ? error.message : String(error)
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
    if (!isUnsaved(document)) {
      return
    }
    timer = setTimeout(() => {
      timer = null
      const current = app.store.getState().document
      if (current === null || !isUnsaved(current)) {
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

/**
 * Follows the recipe of the shown document as it was last without an open transaction, which is
 * what the user has confirmed: a restart reopens it, so a previewed feature is never made
 * permanent (protocol.md 2.7, 4.4). Returns the reader of that recipe for a document handle.
 */
function followSettledRecipe(store: AppStore): (documentHandle: DocHandle) => Document | null {
  let settled: { readonly documentHandle: DocHandle; readonly recipe: Document } | null = null
  function follow(): void {
    const document = store.getState().document
    if (document !== null && !document.transactionOpen && settled?.recipe !== document.recipe) {
      settled = { documentHandle: document.documentHandle, recipe: document.recipe }
    }
  }
  follow()
  store.subscribe(follow)
  return (documentHandle) => (settled?.documentHandle === documentHandle ? settled.recipe : null)
}

export function createApp(options: CreateAppOptions): App {
  const store = options.store ?? createAppStore()
  const now = options.now ?? Date.now
  const mutations = createMutationQueue()
  const eventListeners = new Set<(event: CoreEvent) => void>()
  const settledRecipe = followSettledRecipe(store)
  let viewport: ViewportHandle | null = null
  /** Set by the first restart attempt and kept until a restart has opened it again. */
  let reopening: Reopening | null = null

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

  function changePending(change: number): void {
    store.setState((state) => ({
      core: { ...state.core, pendingMutations: state.core.pendingMutations + change },
    }))
  }

  /**
   * Sends a command to the running core. A command that rebuilds a document counts in
   * `pendingMutations` until it is answered, and its time from sending to the applied response is
   * the document's rebuild time (3.21) and `ps:rebuild` once the viewport drew it (2.18).
   */
  async function request(command: Command): Promise<Reply> {
    if (!rebuildsDocument(command)) {
      return client.request(command)
    }
    const started = performance.now()
    changePending(1)
    try {
      const reply = await client.request(command)
      const documentHandle = rebuiltDocument(command, reply.result)
      if (documentHandle !== null) {
        recordRebuildTime(store, documentHandle, performance.now() - started)
      }
      measureToNextFrame(TIMING_NAMES.rebuild, started)
      return reply
    } finally {
      changePending(-1)
    }
  }

  // The API's client follows restarts: each request goes to the client of the running core.
  const core = createCoreApi({
    request,
    onEvent: (listener) => client.onEvent(listener),
    onFatal: (listener) => client.onFatal(listener),
    close: () => {
      client.close()
    },
  })

  /** What a restart opens: kept from the first attempt, else the shown document's settled recipe. */
  function reopeningOf(): Reopening | null {
    const shown = store.getState().document
    if (reopening !== null || shown === null) {
      return reopening
    }
    return {
      recipe: settledRecipe(shown.documentHandle) ?? shown.recipe,
      name: shown.name,
      file: shown.file,
    }
  }

  /** Opens the document a restart keeps, or a new "Untitled" when there is none (4.4). */
  async function reopen(): Promise<void> {
    const kept = reopening
    if (kept === null) {
      const documentHandle = await mutations.run(() => core.newDocument('mm'))
      adoptDocument(store, documentHandle, { name: 'Untitled', file: null, localUnsaved: false })
      store.setState((state) => ({
        view: {
          ...state.view,
          sessionVisible: initialSessionVisibility(state.document?.recipe ?? null),
        },
      }))
      return
    }
    const opened = await mutations.run(() =>
      core.openDocument(JSON.stringify(kept.recipe, null, 2)),
    )
    adoptDocument(store, opened.documentHandle, {
      name: kept.name,
      file: kept.file,
      localUnsaved: true,
    })
    reopening = null
  }

  const app: App = {
    store,
    core,
    mutations,
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
      // Taken before the store forgets the document, and kept if this attempt fails.
      reopening = reopeningOf()
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
        await reopen()
        store.setState((state) => ({
          core: { ...state.core, status: 'ready', engineVersion: ready.engineVersion, fatal: null },
        }))
      } catch (error) {
        failed(error instanceof CoreFatalError ? error : new CoreFatalError(reasonOf(error)))
      }
    },
  }
  connectApp(app, { transport: options.transport, now, eventListeners })
  eventListeners.add(scheduleRecovery(app, now))
  installPrintAnalysis(app)
  installSelectionUpkeep(app)
  installFailureDismissal(app)
  return app
}

/** Makes the browser ask before leaving the page while the document has unsaved changes (3.19). */
export function installUnloadGuard(app: App, target: Window): () => void {
  function onBeforeUnload(event: BeforeUnloadEvent): void {
    const document = app.store.getState().document
    if (document !== null && isUnsaved(document)) {
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
