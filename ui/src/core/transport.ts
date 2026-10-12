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

/** Creates the web transport. `createWorker` is injectable for tests. */
export function createWorkerTransport(options: WorkerTransportOptions = {}): Transport {
  const createWorker = options.createWorker ?? createCoreWorker
  const startTimeout = options.startTimeoutMilliseconds ?? DEFAULT_START_TIMEOUT_MILLISECONDS
  const pending: Settlers<ArrayBuffer>[] = []
  const fatalListeners = new Set<(error: CoreFatalError) => void>()
  let worker: Worker | null = null
  let starting: Deferred<CoreReady> | null = null
  let started: Promise<CoreReady> | null = null
  let ready = false
  let ended: CoreFatalError | null = null
  let startTimer: ReturnType<typeof setTimeout> | null = null

  /** Ends the transport: the worker is gone and every pending and later send rejects with `error`. */
  function end(error: CoreFatalError, reportFatal: boolean): void {
    if (ended !== null) {
      return
    }
    ended = error
    if (startTimer !== null) {
      clearTimeout(startTimer)
      startTimer = null
    }
    worker?.terminate()
    worker = null
    starting?.reject(error)
    starting = null
    for (const settlers of pending.splice(0)) {
      settlers.reject(error)
    }
    if (reportFatal && ready) {
      for (const listener of [...fatalListeners]) {
        listener(error)
      }
    }
  }

  function receive(data: FromWorker): void {
    switch (data.type) {
      case 'ready':
        if (starting !== null) {
          if (startTimer !== null) {
            clearTimeout(startTimer)
            startTimer = null
          }
          ready = true
          starting.resolve({
            protocolVersion: data.protocolVersion,
            engineVersion: data.engineVersion,
          })
          starting = null
        }
        return
      case 'frame': {
        // The worker answers in request order (protocol.md 2.11 rule 5).
        const oldest = pending.shift()
        if (oldest === undefined) {
          end(new CoreFatalError('The geometry core sent a response nobody asked for.'), true)
          return
        }
        oldest.resolve(data.frame)
        return
      }
      case 'fatal':
        end(new CoreFatalError(data.message), true)
    }
  }

  function listen(target: Worker): void {
    target.addEventListener('message', (event: MessageEvent<unknown>) => {
      if (isFromWorker(event.data)) {
        receive(event.data)
      } else {
        end(new CoreFatalError('The geometry core sent a message the app cannot read.'), true)
      }
    })
    target.addEventListener('error', (event: Event) => {
      const message =
        event instanceof ErrorEvent && event.message !== ''
          ? event.message
          : 'The geometry core stopped.'
      end(new CoreFatalError(message), true)
    })
    target.addEventListener('messageerror', () => {
      end(new CoreFatalError('The geometry core sent a message the app cannot read.'), true)
    })
  }

  return {
    kind: 'worker',
    start() {
      if (started !== null) {
        return started
      }
      if (ended !== null) {
        return Promise.reject(ended)
      }
      const startup = deferred<CoreReady>()
      const promise = startup.promise
      starting = startup
      started = promise
      try {
        worker = createWorker()
        listen(worker)
      } catch (error) {
        end(new CoreFatalError(error instanceof Error ? error.message : String(error)), false)
        return promise
      }
      startTimer = setTimeout(() => {
        startTimer = null
        end(
          new CoreFatalError(
            `The geometry core did not start within ${String(startTimeout / 1000)} seconds.`,
          ),
          false,
        )
      }, startTimeout)
      return promise
    },
    send(request) {
      if (ended !== null) {
        return Promise.reject(ended)
      }
      if (worker === null) {
        return Promise.reject(new CoreFatalError('The geometry core is not running.'))
      }
      const target = worker
      return new Promise<ArrayBuffer>((resolve, reject) => {
        pending.push({ resolve, reject })
        const message: ToWorker = { type: 'call', frame: request }
        target.postMessage(message, [request])
      })
    },
    stop() {
      end(new CoreFatalError('The geometry core was stopped.'), false)
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
