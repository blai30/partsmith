import { PROTOCOL_VERSION, type Request } from '../generated/protocol'
import { callCore, type CoreModule } from './bridge'
import { decodeFrame, encodeFrame } from './frame'
import { isSuperseded, supersededFrame } from './superseding'

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

/** Request id 0 is reserved for the host's own engine_info call (protocol.md 3.5). */
const ENGINE_INFO_ID = 0

function reasonOf(error: unknown): string {
  return error instanceof Error ? error.message : String(error)
}

function requestOf(frame: ArrayBuffer): Request | null {
  try {
    const [message] = decodeFrame(frame).header.messages
    return message?.kind === 'request' ? { id: message.id, command: message.command } : null
  } catch {
    // The core answers an unreadable frame itself (malformed_frame).
    return null
  }
}

/** Asks the core for its versions; throws when it is not a core this app can talk to. */
function engineVersion(core: CoreModule): string {
  const request = encodeFrame(
    {
      protocol_version: PROTOCOL_VERSION,
      messages: [{ kind: 'request', id: ENGINE_INFO_ID, command: { type: 'engine_info' } }],
    },
    [],
  )
  const response = decodeFrame(callCore(core, request)).header.messages.at(-1)
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

/**
 * Whether the call at the head of the queue is superseded. Superseding looks only as far as the
 * first unreadable frame, so a request the host cannot read never lets a later one overtake it.
 */
function headIsSuperseded(queue: readonly QueuedCall[]): boolean {
  const readable: Request[] = []
  for (const call of queue) {
    if (call.request === null) {
      break
    }
    readable.push(call.request)
  }
  return readable.length > 0 && isSuperseded(readable, 0)
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
        fail(reasonOf(error))
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
        fail(reasonOf(error))
        return
      }
      core = loaded
      options.post({ type: 'ready', protocolVersion: PROTOCOL_VERSION, engineVersion: version }, [])
      scheduleStep()
    },
    (error: unknown) => {
      fail(reasonOf(error))
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
