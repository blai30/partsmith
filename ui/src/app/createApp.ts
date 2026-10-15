import { createCommandRegistry, type CommandRegistry } from '../commands/registry'
import { createCoreApi, rebuildsDocument, rebuiltDocument, type CoreApi } from '../core/api'
import { createCoreClient, type CoreClient, type Reply } from '../core/client'
import { errorMessage } from '../core/errors'
import type { CoreEvent } from '../core/events'
import { createMutationQueue, type MutationQueue } from '../core/queue'
import { CoreFatalError, type Transport } from '../core/transport'
import type { FileService } from '../files/file-service'
import type { RecentFilesStore } from '../files/recent'
import type { RecoveryStore } from '../files/recovery'
import type { Command, DocHandle, Document } from '../generated/protocol'
import { MESSAGES } from '../messages'
import { TIMING_NAMES } from '../performance/marks'
import {
  adoptDocument,
  applyCoreEvent,
  recordRebuildTime,
  releaseDocuments,
} from '../state/core-events'
import { selectUnsaved } from '../state/selectors'
import { createAppStore, type AppStore, type FileReference } from '../state/store'
import { initialSessionVisibility } from '../state/visibility'
import type { ViewportHandle } from '../viewport/controller'
import { installFailureDismissal, notify } from './actions/notifications'
import { installPrintAnalysis } from './actions/print'
import { connectApp } from './connection'
import { loadCoreData, markCoreFailed, markCoreReady, openUntitled } from './core-data'
import { continueRecoverySession, installRecoveryCopy } from './recovery-copy'
import { installSelectionUpkeep } from './selection-upkeep'
import { measureToNextFrame } from './timing'

// The app object every action takes (ui.md 2.9): the store, the core API, the mutation queue, the
// command registry and the host services. createApp wires core events into the store, counts and
// times the requests that rebuild a document, notices a core that has answered nothing for a long
// time while the user waits on it, keeps a recovery copy of unsaved work, and restarts the core
// after a fatal error or when the user stops a core that is still working (4.4).

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
  /** Restarts the core after a fatal error, or stops and restarts one that is still working, and reopens the settled recipe (4.4). */
  restartCore(): Promise<void>
  /**
   * Counts `work`, background requests that a user action waits on before it goes on, as a
   * waited-on request sent now, for the still-working state (4.4); settles as `work` does.
   */
  waitOn<Result>(work: Promise<Result>): Promise<Result>
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

/** What a core restart opens again: the document shown when the core stopped, in its session (4.4). */
type Reopening = {
  readonly recipe: Document
  readonly name: string
  readonly file: FileReference | null
  readonly recoveryId: string
}

/**
 * How long the core may answer nothing while a waited-on request waits before it is still working
 * (4.4).
 */
export const STILL_WORKING_SECONDS = 10

/**
 * The commands of background requests: those the UI sends on its own to keep what it shows
 * current, which nobody waits on, so they never make the core still working (4.4).
 */
export const BACKGROUND_COMMANDS: ReadonlySet<Command['type']> = new Set([
  'analyze_print',
  'list_elements',
  'resolve_references',
  'measure',
  'evaluate',
])

/** Whether the user waits on a request: every one but a background request (4.4). */
function isWaitedOn(command: Command): boolean {
  return !BACKGROUND_COMMANDS.has(command.type)
}

/** Notes the requests sent to one core and how they end, for the still-working state (4.4). */
type ResponseWatch = {
  /**
   * Notes a request sent, or a user action's wait on background requests (`App.waitOn`), which
   * the user waits on unless `waitedOn` is false; the returned function notes its end, `answered`
   * false when no response came.
   */
  sent(waitedOn: boolean): (answered: boolean) => void
  /** Forgets the requests in flight, whose core is gone, and leaves the still-working state. */
  reset(): void
}

/**
 * Sets `core.stillWorking` once the core has answered nothing for `STILL_WORKING_SECONDS` while a
 * waited-on request waits, counted from the later of the oldest such request's sending and the
 * latest response to any request, which the core gives in request order. A background request
 * starts no count, but its response clears the state and counts again, as any response does.
 */
function watchResponses(store: AppStore): ResponseWatch {
  // Waited-on requests sent and not yet ended.
  let waitedOnRequests = 0
  // Requests sent before a reset end without touching the count.
  let generation = 0
  let timer: ReturnType<typeof setTimeout> | null = null

  function setStillWorking(stillWorking: boolean): void {
    if (store.getState().core.stillWorking !== stillWorking) {
      store.setState((state) => ({ core: { ...state.core, stillWorking } }))
    }
  }

  // Counts the time without a response from now, while a waited-on request waits.
  function countFromNow(): void {
    if (timer !== null) {
      clearTimeout(timer)
      timer = null
    }
    if (waitedOnRequests > 0) {
      timer = setTimeout(() => {
        timer = null
        setStillWorking(true)
      }, STILL_WORKING_SECONDS * 1_000)
    }
  }

  return {
    sent(waitedOn) {
      const sentIn = generation
      if (waitedOn) {
        waitedOnRequests += 1
        if (waitedOnRequests === 1) {
          countFromNow()
        }
      }
      return (answered) => {
        if (sentIn !== generation) {
          return
        }
        if (waitedOn) {
          waitedOnRequests -= 1
        }
        if (answered) {
          setStillWorking(false)
        }
        if (answered || waitedOnRequests === 0) {
          countFromNow()
        }
      }
    },
    reset() {
      generation += 1
      waitedOnRequests = 0
      countFromNow()
      setStillWorking(false)
    },
  }
}

/**
 * Follows the settled recipe of the shown document (1.3): its recipe as it was last without an
 * open transaction, which is what the user has confirmed. The recovery copy keeps it and a restart
 * reopens it, so a previewed feature is never made permanent (protocol.md 2.7, 3.19, 4.4).
 * Returns the reader of that recipe for a document handle.
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
  const responses = watchResponses(store)
  let viewport: ViewportHandle | null = null
  /** Set by the first restart attempt and kept until a restart has opened it again. */
  let reopening: Reopening | null = null
  /** Set when a stop dropped changes the core had not finished, until a restart has reopened the document. */
  let changesDropped = false

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
    responses.reset()
    markCoreFailed(store, error.message)
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
   * Sends a command to the running core and watches for its response: only a waited-on request
   * left unanswered makes the core still working (4.4). A command that rebuilds a document counts
   * in `pendingMutations` until it is answered, and its time from sending to the applied response
   * is the document's rebuild time (3.21) and `ps:rebuild` once the viewport drew it (2.18).
   */
  async function request(command: Command): Promise<Reply> {
    const ended = responses.sent(isWaitedOn(command))
    const rebuilds = rebuildsDocument(command)
    const started = performance.now()
    if (rebuilds) {
      changePending(1)
    }
    try {
      const reply = await client.request(command)
      ended(true)
      const documentHandle = rebuiltDocument(command, reply.result)
      if (documentHandle !== null) {
        recordRebuildTime(store, documentHandle, performance.now() - started)
      }
      if (rebuilds) {
        measureToNextFrame(TIMING_NAMES.rebuild, started)
      }
      return reply
    } catch (error) {
      // A refusal is a response; a core that stopped gave none.
      ended(!(error instanceof CoreFatalError))
      throw error
    } finally {
      if (rebuilds) {
        changePending(-1)
      }
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
      recoveryId: shown.recoveryId,
    }
  }

  /** Opens the document a restart keeps, or a new "Untitled" when there is none (4.4). */
  async function reopen(): Promise<void> {
    const kept = reopening
    if (kept === null) {
      await openUntitled(app)
      store.setState((state) => ({
        view: {
          ...state.view,
          sessionVisible: initialSessionVisibility(state.document?.recipe ?? null),
        },
      }))
      return
    }
    const opened = await mutations.run(
      () => core.openDocument(JSON.stringify(kept.recipe, null, 2)),
      'other',
    )
    adoptDocument(store, opened.documentHandle, {
      name: kept.name,
      file: kept.file,
      localUnsaved: true,
      recoveryId: kept.recoveryId,
    })
    reopening = null
    // The session goes on, so its recovery entry carries on (3.19).
    continueRecoverySession(app, kept.recoveryId)
  }

  /**
   * Whether stopping the core drops changes it had not finished: a stop while it is still working
   * with a change started or waiting in the mutation queue, or a feature command open (4.4).
   */
  function dropsChanges(): boolean {
    const { core: state, command } = store.getState()
    return state.stillWorking && (mutations.changing || command !== null)
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
      changesDropped ||= dropsChanges()
      // None of the waiting mutations reaches the new core.
      mutations.discard(new CoreFatalError('The geometry core was stopped.'))
      // Taken before the store forgets the document, and kept if this attempt fails.
      reopening = reopeningOf()
      responses.reset()
      store.setState((state) => ({
        core: { ...state.core, status: 'loading', fatal: null },
        context: state.context === 'sketch' ? 'model' : state.context,
        selection: [],
        hover: null,
        command: null,
        sketch: null,
      }))
      releaseDocuments(store)
      // Stopping the transport ends the worker and rejects the request in flight.
      client.close()
      client = connect()
      try {
        const ready = await options.transport.start()
        await loadCoreData(app)
        await reopen()
        markCoreReady(store, ready.engineVersion)
      } catch (error) {
        failed(error instanceof CoreFatalError ? error : new CoreFatalError(errorMessage(error)))
        return
      }
      if (changesDropped) {
        changesDropped = false
        notify(app, {
          severity: 'warning',
          message: MESSAGES.changes_not_applied(),
          code: 'changes_not_applied',
          actions: [],
        })
      }
    },
    async waitOn<Result>(work: Promise<Result>): Promise<Result> {
      const ended = responses.sent(true)
      try {
        return await work
      } finally {
        // The requests behind the work noted their own responses.
        ended(false)
      }
    },
  }
  connectApp(app, {
    transport: options.transport,
    now,
    eventListeners,
    settledRecipe,
  })
  eventListeners.add(installRecoveryCopy(app))
  installPrintAnalysis(app)
  installSelectionUpkeep(app)
  installFailureDismissal(app)
  return app
}

/** Makes the browser ask before leaving the page while the document has unsaved changes (3.19). */
export function installUnloadGuard(app: App, target: Window): () => void {
  function onBeforeUnload(event: BeforeUnloadEvent): void {
    if (selectUnsaved(app.store.getState())) {
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
