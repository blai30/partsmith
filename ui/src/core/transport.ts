import { createTauriTransport, type TauriGlobal } from './tauri-transport'
import type { FromWorker, ToWorker } from './worker-host'

/** What the core reported when it became ready. */
export type CoreReady = {
  readonly protocolVersion: number
  readonly engineVersion: string
}

/** A byte pipe to the core: one request frame in, its response frame out. */
export type Transport = {
  readonly kind: 'worker' | 'tauri'
  /** Starts the core. Resolves once it accepts calls; rejects with CoreFatalError. */
  start(): Promise<CoreReady>
  /** Sends one request frame and resolves with the response frame for it. */
  send(request: ArrayBuffer): Promise<ArrayBuffer>
  /** Stops the core; pending and later sends reject with CoreFatalError. */
  stop(): void
  /** Called at most once, when the core dies outside the caller's control. */
  onFatal(listener: (error: CoreFatalError) => void): () => void
}

/** The core is gone; the app must restart it. */
export class CoreFatalError extends Error {
  override readonly name = 'CoreFatalError'
}

export type WorkerTransportOptions = {
  readonly createWorker?: () => Worker
  readonly startTimeoutMilliseconds?: number
}

const DEFAULT_START_TIMEOUT_MILLISECONDS = 30_000

type Settlers<Value> = {
  readonly resolve: (value: Value) => void
  readonly reject: (error: CoreFatalError) => void
}

type Deferred<Value> = Settlers<Value> & { readonly promise: Promise<Value> }

function deferred<Value>(): Deferred<Value> {
  let resolve: (value: Value) => void = () => undefined
  let reject: (error: CoreFatalError) => void = () => undefined
  const promise = new Promise<Value>((settleResolve, settleReject) => {
    resolve = settleResolve
    reject = settleReject
  })
  return { promise, resolve, reject }
}

function createCoreWorker(): Worker {
  return new Worker(new URL('./core.worker.ts', import.meta.url), {
    type: 'module',
    name: 'partsmith-core',
  })
}

function isFromWorker(data: unknown): data is FromWorker {
  if (typeof data !== 'object' || data === null) {
    return false
  }
  const type: unknown = Reflect.get(data, 'type')
  return type === 'ready' || type === 'frame' || type === 'fatal'
}

/** One run of the core worker, from start() until it ends. */
type WorkerSession = {
  readonly started: Promise<CoreReady>
  readonly startup: Deferred<CoreReady>
  readonly pending: Settlers<ArrayBuffer>[]
  worker: Worker | null
  ready: boolean
  ended: CoreFatalError | null
  startTimer: ReturnType<typeof setTimeout> | null
}

/** Creates the web transport. `createWorker` is injectable for tests. */
export function createWorkerTransport(options: WorkerTransportOptions = {}): Transport {
  const createWorker = options.createWorker ?? createCoreWorker
  const startTimeout = options.startTimeoutMilliseconds ?? DEFAULT_START_TIMEOUT_MILLISECONDS
  const fatalListeners = new Set<(error: CoreFatalError) => void>()
  let session: WorkerSession | null = null

  /** Ends a session: its worker is gone and every pending and later send rejects with `error`. */
  function end(ending: WorkerSession, error: CoreFatalError, reportFatal: boolean): void {
    if (ending.ended !== null) {
      return
    }
    ending.ended = error
    if (ending.startTimer !== null) {
      clearTimeout(ending.startTimer)
      ending.startTimer = null
    }
    ending.worker?.terminate()
    ending.worker = null
    if (!ending.ready) {
      ending.startup.reject(error)
    }
    for (const settlers of ending.pending.splice(0)) {
      settlers.reject(error)
    }
    // A core that never became ready is reported through start(), not as a fatal error.
    if (reportFatal && ending.ready) {
      for (const listener of [...fatalListeners]) {
        listener(error)
      }
    }
  }

  function receive(receiving: WorkerSession, data: FromWorker): void {
    switch (data.type) {
      case 'ready':
        if (!receiving.ready) {
          if (receiving.startTimer !== null) {
            clearTimeout(receiving.startTimer)
            receiving.startTimer = null
          }
          receiving.ready = true
          receiving.startup.resolve({
            protocolVersion: data.protocolVersion,
            engineVersion: data.engineVersion,
          })
        }
        return
      case 'frame': {
        // The worker answers in request order (protocol.md 2.11 rule 5).
        const oldest = receiving.pending.shift()
        if (oldest === undefined) {
          end(
            receiving,
            new CoreFatalError('The geometry core sent a response nobody asked for.'),
            true,
          )
          return
        }
        oldest.resolve(data.frame)
        return
      }
      case 'fatal':
        end(receiving, new CoreFatalError(data.message), true)
    }
  }

  function listen(listening: WorkerSession, target: Worker): void {
    target.addEventListener('message', (event: MessageEvent<unknown>) => {
      if (isFromWorker(event.data)) {
        receive(listening, event.data)
      } else {
        end(
          listening,
          new CoreFatalError('The geometry core sent a message the app cannot read.'),
          true,
        )
      }
    })
    target.addEventListener('error', (event: Event) => {
      const message =
        event instanceof ErrorEvent && event.message !== ''
          ? event.message
          : 'The geometry core stopped.'
      end(listening, new CoreFatalError(message), true)
    })
    target.addEventListener('messageerror', () => {
      end(
        listening,
        new CoreFatalError('The geometry core sent a message the app cannot read.'),
        true,
      )
    })
  }

  return {
    kind: 'worker',
    start() {
      if (session !== null && session.ended === null) {
        return session.started
      }
      // A transport whose core ended starts a fresh worker (the restart of ui.md 4.4).
      const startup = deferred<CoreReady>()
      const starting: WorkerSession = {
        started: startup.promise,
        startup,
        pending: [],
        worker: null,
        ready: false,
        ended: null,
        startTimer: null,
      }
      session = starting
      try {
        starting.worker = createWorker()
        listen(starting, starting.worker)
      } catch (error) {
        end(starting, new CoreFatalError(error instanceof Error ? error.message : String(error)), false)
        return starting.started
      }
      starting.startTimer = setTimeout(() => {
        starting.startTimer = null
        end(
          starting,
          new CoreFatalError(
            `The geometry core did not start within ${String(startTimeout / 1000)} seconds.`,
          ),
          false,
        )
      }, startTimeout)
      return starting.started
    },
    send(request) {
      if (session === null) {
        return Promise.reject(new CoreFatalError('The geometry core is not running.'))
      }
      const sending = session
      if (sending.ended !== null) {
        return Promise.reject(sending.ended)
      }
      return new Promise<ArrayBuffer>((resolve, reject) => {
        sending.pending.push({ resolve, reject })
        const message: ToWorker = { type: 'call', frame: request }
        sending.worker?.postMessage(message, [request])
      })
    },
    stop() {
      if (session !== null) {
        end(session, new CoreFatalError('The geometry core was stopped.'), false)
      }
    },
    onFatal(listener) {
      fatalListeners.add(listener)
      return () => {
        fatalListeners.delete(listener)
      }
    },
  }
}

/** Picks the Tauri transport when the Tauri global is present, else the worker transport. */
export function chooseTransport(environment: { readonly __TAURI__?: TauriGlobal }): Transport {
  const tauri = environment.__TAURI__
  if (tauri !== undefined && typeof tauri.core?.invoke === 'function') {
    return createTauriTransport(tauri)
  }
  return createWorkerTransport()
}
