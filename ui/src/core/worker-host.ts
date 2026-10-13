import { PROTOCOL_VERSION, type Request } from '../generated/protocol'
import { callCore, type CoreModule } from './bridge'
import { errorMessage } from './errors'
import { decodeFrame, engineInfoRequestFrame } from './frame'
import { headIsSuperseded, requestOf, supersededFrame } from './superseding'

// The logic of the core worker (protocol.md 2.11), kept apart from the worker global so tests drive
// it with a fake module and a manual scheduler.

/** Main thread to worker. The frame's ArrayBuffer is transferred. */
export type ToWorker = { readonly type: 'call'; readonly frame: ArrayBuffer }

/** Worker to main thread. Frames are transferred. */
export type FromWorker =
  | { readonly type: 'ready'; readonly protocolVersion: number; readonly engineVersion: string }
  | { readonly type: 'frame'; readonly frame: ArrayBuffer }
  | { readonly type: 'fatal'; readonly message: string }

export type WorkerHostOptions = {
  /** Loads and wraps the Emscripten module. */
  load(): Promise<CoreModule>
  /** Posts a message to the main thread with a transfer list. */
  post(message: FromWorker, transfer: Transferable[]): void
  /** Runs a task later (setTimeout in production, manual in tests). */
  schedule(task: () => void): void
}

export type WorkerHost = {
  /** Handles one message from the main thread. */
  receive(message: ToWorker): void
}

/** A queued call: its frame and, when the frame decodes, its request. */
type QueuedCall = { readonly frame: ArrayBuffer; readonly request: Request | null }

/** Asks the core for its versions; throws when it is not a core this app can talk to. */
function engineVersion(core: CoreModule): string {
  const response = decodeFrame(callCore(core, engineInfoRequestFrame())).header.messages.at(-1)
  if (response?.kind !== 'response' || response.result.kind !== 'ok') {
    throw new Error('the core did not report its version')
  }
  const result = response.result
  if (result.type !== 'engine_info') {
    throw new Error('the core answered engine_info with another result')
  }
  if (result.protocol_version !== PROTOCOL_VERSION) {
    throw new Error(
      `the core speaks protocol version ${String(result.protocol_version)}, but this app speaks version ${String(PROTOCOL_VERSION)}`,
    )
  }
  return result.engine_version
}

export function createWorkerHost(options: WorkerHostOptions): WorkerHost {
  const queue: QueuedCall[] = []
  let core: CoreModule | null = null
  let fatal: string | null = null
  let stepScheduled = false

  function postFatal(message: string): void {
    options.post({ type: 'fatal', message }, [])
  }

  function fail(message: string): void {
    fatal = message
    core = null
    queue.length = 0
    postFatal(message)
  }

  function postFrame(frame: ArrayBuffer): void {
    options.post({ type: 'frame', frame }, [frame])
  }

  function scheduleStep(): void {
    if (stepScheduled || core === null || queue.length === 0) {
      return
    }
    stepScheduled = true
    options.schedule(step)
  }

  function step(): void {
    stepScheduled = false
    const current = queue[0]
    if (core === null || current === undefined) {
      return
    }
    if (headIsSuperseded(queue)) {
      queue.shift()
      postFrame(supersededFrame(current.request?.id ?? 0))
    } else {
      let response: ArrayBuffer
      try {
        response = callCore(core, current.frame)
      } catch (error) {
        // A trap or an allocation failure leaves the module unusable (rule 6).
        fail(errorMessage(error))
        return
      }
      queue.shift()
      postFrame(response)
    }
    scheduleStep()
  }

  options.load().then(
    (loaded) => {
      let version: string
      try {
        version = engineVersion(loaded)
      } catch (error) {
        fail(errorMessage(error))
        return
      }
      core = loaded
      options.post({ type: 'ready', protocolVersion: PROTOCOL_VERSION, engineVersion: version }, [])
      scheduleStep()
    },
    (error: unknown) => {
      fail(errorMessage(error))
    },
  )

  return {
    receive(message) {
      if (fatal !== null) {
        postFatal(fatal)
        return
      }
      queue.push({ frame: message.frame, request: requestOf(message.frame) })
      scheduleStep()
    },
  }
}
