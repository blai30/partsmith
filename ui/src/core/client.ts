import {
  PROTOCOL_VERSION,
  type Command,
  type CommandResult,
  type ErrorCode,
  type ErrorInfo,
  type Event,
  type FixAction,
  type Message,
} from '../generated/protocol'
import { decodeEvent, type CoreEvent } from './events'
import { errorMessage } from './errors'
import { decodeFrame, encodeFrame, type BufferView } from './frame'
import { CoreFatalError, type Transport } from './transport'

// The core client (ui.md 2.5): numbers requests, encodes and decodes frames, and dispatches the
// events of each response frame before settling its request.

/** A successful response with the frame's buffers (views alias the frame). */
export type Reply = {
  readonly result: CommandResult
  readonly buffers: readonly BufferView[]
}

/** A protocol error response. `message` is the core's text, shown verbatim. */
export class CoreError extends Error {
  override readonly name = 'CoreError'
  readonly code: ErrorCode
  readonly editIndex: number | null
  /** UTF-8 byte offsets of an `expression_error`'s span when the core reports one (2.13, open issue 9), else null. */
  readonly span: { readonly start: number; readonly end: number } | null
  /** Technical text for reports (a panic message, a parser's text), never shown as the message (4.1); null without one. */
  readonly detail: string | null
  /** The core's one-click ways out, in the order to offer them (4.2); empty without any. */
  readonly actions: readonly FixAction[]

  constructor(info: ErrorInfo) {
    super(info.message)
    this.code = info.code
    this.editIndex = info.edit_index ?? null
    this.span =
      info.span === null || info.span === undefined
        ? null
        : { start: info.span.start, end: info.span.end }
    this.detail = info.detail ?? null
    this.actions = info.actions ?? []
  }
}

export type CoreClient = {
  /** Sends one command. Events in the response frame are dispatched before the promise settles. */
  request(command: Command): Promise<Reply>
  onEvent(listener: (event: CoreEvent) => void): () => void
  onFatal(listener: (error: CoreFatalError) => void): () => void
  close(): void
}

type DecodedResponse = {
  readonly events: readonly Event[]
  readonly response: Extract<Message, { kind: 'response' }>
  readonly buffers: readonly BufferView[]
}

/** Thrown inside the client for a frame it cannot trust; becomes a CoreFatalError. */
class UnreadableResponse extends Error {}

function readResponse(frame: ArrayBuffer, id: number): DecodedResponse {
  const { header, buffers } = decodeFrame(frame)
  const events: Event[] = []
  const messages = header.messages
  messages.forEach((message, index) => {
    if (index < messages.length - 1) {
      if (message.kind !== 'event') {
        throw new UnreadableResponse(
          'a response frame holds a message other than events before its response',
        )
      }
      events.push(message)
    }
  })
  const response = messages.at(-1)
  if (response?.kind !== 'response') {
    throw new UnreadableResponse('the frame does not end with a response')
  }
  if (response.id !== id) {
    throw new UnreadableResponse(
      `it answers request ${String(response.id)} instead of request ${String(id)}`,
    )
  }
  return { events, response, buffers }
}

/** The command result of an ok outcome, without the outcome's `kind` tag. */
function commandResultOf(outcome: CommandResult & { readonly kind: 'ok' }): CommandResult {
  const { kind: _outcome, ...result } = outcome
  return result
}

export function createCoreClient(transport: Transport): CoreClient {
  const eventListeners = new Set<(event: CoreEvent) => void>()
  const fatalListeners = new Set<(error: CoreFatalError) => void>()
  let nextId = 1
  let broken: CoreFatalError | null = null

  function reportFatal(error: CoreFatalError): void {
    if (broken !== null) {
      return
    }
    broken = error
    for (const listener of Array.from(fatalListeners)) {
      listener(error)
    }
  }

  const stopListening = transport.onFatal(reportFatal)

  function takeId(): number {
    const id = nextId
    nextId = id >= Number.MAX_SAFE_INTEGER ? 1 : id + 1
    return id
  }

  function dispatch(event: CoreEvent): void {
    for (const listener of Array.from(eventListeners)) {
      try {
        listener(event)
      } catch (error) {
        console.error('A core event listener failed.', error)
      }
    }
  }

  return {
    async request(command) {
      if (broken !== null) {
        throw broken
      }
      const id = takeId()
      const frame = encodeFrame(
        { protocol_version: PROTOCOL_VERSION, messages: [{ kind: 'request', id, command }] },
        [],
      )
      const responseFrame = await transport.send(frame)
      let decoded: { readonly response: DecodedResponse; readonly events: (CoreEvent | null)[] }
      try {
        const response = readResponse(responseFrame, id)
        decoded = {
          response,
          events: response.events.map((event) => decodeEvent(event, response.buffers)),
        }
      } catch (error) {
        const fatal = new CoreFatalError(
          `The geometry core sent a response the app cannot read: ${errorMessage(error)}`,
        )
        reportFatal(fatal)
        throw fatal
      }
      for (const event of decoded.events) {
        if (event !== null) {
          dispatch(event)
        }
      }
      const { result } = decoded.response.response
      if (result.kind === 'error') {
        throw new CoreError(result)
      }
      return { result: commandResultOf(result), buffers: decoded.response.buffers }
    },
    onEvent(listener) {
      eventListeners.add(listener)
      return () => {
        eventListeners.delete(listener)
      }
    },
    onFatal(listener) {
      fatalListeners.add(listener)
      return () => {
        fatalListeners.delete(listener)
      }
    },
    close() {
      stopListening()
      if (broken === null) {
        broken = new CoreFatalError('The geometry core was stopped.')
      }
      transport.stop()
    },
  }
}
