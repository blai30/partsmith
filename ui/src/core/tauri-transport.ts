import { PROTOCOL_VERSION, type Request } from '../generated/protocol'
import { decodeFrame, encodeFrame, errorResponseFrame, ProtocolVersionError } from './frame'
import { isSuperseded, supersededFrame } from './superseding'
import { CoreFatalError, type Transport } from './transport'

// The desktop transport (ui.md 2.4, protocol.md 2.12): every frame goes through the host's `call`
// command, one at a time and in order, with the worker host's superseding rules.

/** The part of `window.__TAURI__` the UI uses (requires `app.withGlobalTauri` in the desktop config). */
export type TauriGlobal = {
  readonly core: { invoke(command: string, payload: ArrayBuffer): Promise<ArrayBuffer> }
}

type QueuedSend = {
  readonly frame: ArrayBuffer
  readonly request: Request | null
  readonly resolve: (frame: ArrayBuffer) => void
  readonly reject: (error: CoreFatalError) => void
}

const VERSION_MISMATCH =
  'This app and its core speak different protocol versions. Reinstall Partsmith.'

function reasonOf(error: unknown): string {
  return error instanceof Error ? error.message : String(error)
}

function requestOf(frame: ArrayBuffer): Request | null {
  try {
    const [message] = decodeFrame(frame).header.messages
    return message?.kind === 'request' ? { id: message.id, command: message.command } : null
  } catch {
    // The core answers an unreadable frame itself.
    return null
  }
}

/** The protocol's internal_error text (protocol.md section 4) for a failed host call. */
function internalErrorMessage(detail: string): string {
  return `Something went wrong inside Partsmith (${detail}). Save your work and reload. Please report this.`
}

/** Whether the head of the queue is superseded, looking no further than the first unreadable frame. */
function headIsSuperseded(queue: readonly QueuedSend[]): boolean {
  const readable: Request[] = []
  for (const send of queue) {
    if (send.request === null) {
      break
    }
    readable.push(send.request)
  }
  return readable.length > 0 && isSuperseded(readable, 0)
}

export function createTauriTransport(tauri: TauriGlobal): Transport {
  const queue: QueuedSend[] = []
  let running = false
  let stopped: CoreFatalError | null = null

  function invoke(frame: ArrayBuffer): Promise<ArrayBuffer> {
    return tauri.core.invoke('call', frame)
  }

  /** Answers the head of the queue when nothing is in flight: superseded at once, else through the host. */
  function runNext(): void {
    while (!running) {
      const current = queue[0]
      if (current === undefined) {
        return
      }
      if (!headIsSuperseded(queue)) {
        running = true
        call(current)
        return
      }
      queue.shift()
      current.resolve(supersededFrame(current.request?.id ?? 0))
    }
  }

  function call(current: QueuedSend): void {
    let response: Promise<ArrayBuffer>
    try {
      response = invoke(current.frame)
    } catch (error) {
      response = Promise.reject(error)
    }
    response
      .catch((error: unknown) =>
        // The desktop core lives in the app process, so a failed call is an error for this request only.
        errorResponseFrame(
          current.request?.id ?? 0,
          'internal_error',
          internalErrorMessage(reasonOf(error)),
        ),
      )
      .then((frame) => {
        running = false
        // A stop() while the call was in flight already rejected it.
        if (queue[0] === current) {
          queue.shift()
          current.resolve(frame)
        }
        runNext()
      })
      .catch((error: unknown) => {
        console.error('The desktop transport failed to answer a request.', error)
      })
  }

  return {
    kind: 'tauri',
    async start() {
      // Starting again after stop() (a core restart) accepts sends again.
      stopped = null
      const request = encodeFrame(
        {
          protocol_version: PROTOCOL_VERSION,
          messages: [{ kind: 'request', id: 0, command: { type: 'engine_info' } }],
        },
        [],
      )
      let result
      try {
        const response = decodeFrame(await invoke(request)).header.messages.at(-1)
        result = response?.kind === 'response' ? response.result : null
      } catch (error) {
        // A core of another version may answer in its own version's frame.
        throw new CoreFatalError(
          error instanceof ProtocolVersionError ? VERSION_MISMATCH : reasonOf(error),
        )
      }
      if (result?.kind === 'error' && result.code === 'protocol_version_mismatch') {
        throw new CoreFatalError(VERSION_MISMATCH)
      }
      if (result === null || result.kind !== 'ok' || result.type !== 'engine_info') {
        throw new CoreFatalError('The desktop core did not report its version.')
      }
      if (result.protocol_version !== PROTOCOL_VERSION) {
        throw new CoreFatalError(VERSION_MISMATCH)
      }
      return { protocolVersion: result.protocol_version, engineVersion: result.engine_version }
    },
    send(frame) {
      if (stopped !== null) {
        return Promise.reject(stopped)
      }
      return new Promise<ArrayBuffer>((resolve, reject) => {
        queue.push({ frame, request: requestOf(frame), resolve, reject })
        runNext()
      })
    },
    stop() {
      if (stopped !== null) {
        return
      }
      const error = new CoreFatalError('The geometry core was stopped.')
      stopped = error
      for (const send of queue.splice(0)) {
        send.reject(error)
      }
    },
    onFatal() {
      // The desktop core shares the app's process: it never dies on its own.
      return () => undefined
    },
  }
}
