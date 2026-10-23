import { decodeFrame, encodeFrame, type BufferView } from '../../core/frame'
import {
  PROTOCOL_VERSION,
  type BoundingBox,
  type Command,
  type CommandResult,
  type Edit,
  type ErrorCode,
  type Event as ProtocolEvent,
  type FixAction,
  type MeshBuffers,
  type Message,
  type Request,
} from '../../generated/protocol'

// Builders and readers for protocol frames, so tests speak the wire format of protocol.md 2.2 and 2.3
// through the app's own frame codec.

/** The error part of a failed response (protocol.md 2.8). */
export type ErrorReply = {
  readonly code: ErrorCode
  readonly message: string
  readonly edit_index?: number
  readonly span?: { readonly start: number; readonly end: number }
  readonly detail?: string
  readonly actions?: FixAction[]
}

/** A response message as `decodeFrame` returns it. */
export type ResponseMessage = Extract<Message, { kind: 'response' }>

/** A response without its id: the events caused by the request, the buffers, then the outcome. */
export type ResponseBody = {
  readonly events?: readonly ProtocolEvent[]
  readonly buffers?: readonly BufferView[]
} & ({ readonly result: CommandResult } | { readonly error: ErrorReply })

/** The parts of a response frame. */
export type ResponseParts = { readonly id: number } & ResponseBody

/** A decoded response frame. */
export type DecodedResponse = {
  readonly events: readonly ProtocolEvent[]
  readonly response: ResponseMessage
  readonly buffers: readonly BufferView[]
}

/** A request frame with one request, as the core client encodes it. */
export function requestFrame(
  id: number,
  command: Command,
  buffers: readonly BufferView[] = [],
): ArrayBuffer {
  return encodeFrame(
    { protocol_version: PROTOCOL_VERSION, messages: [{ kind: 'request', id, command }] },
    [...buffers],
  )
}

function eventMessage(event: ProtocolEvent): Message {
  return { kind: 'event', ...event }
}

function responseMessage(parts: ResponseParts): Message {
  if ('result' in parts) {
    return { kind: 'response', id: parts.id, result: { kind: 'ok', ...parts.result } }
  }
  return { kind: 'response', id: parts.id, result: { kind: 'error', ...parts.error } }
}

/** A response frame: the events in order, then the response. */
export function responseFrame(parts: ResponseParts): ArrayBuffer {
  return encodeFrame(
    {
      protocol_version: PROTOCOL_VERSION,
      messages: [...(parts.events ?? []).map(eventMessage), responseMessage(parts)],
    },
    [...(parts.buffers ?? [])],
  )
}

/** The single request of a request frame and the frame's buffers, such as an `open_document`'s assets. */
export type RequestParts = { readonly request: Request; readonly buffers: readonly BufferView[] }

/** The single request of a request frame with the frame's buffers; throws for anything else. */
export function readRequestFrame(frame: ArrayBuffer): RequestParts {
  const { header, buffers } = decodeFrame(frame)
  const { messages } = header
  const message = messages[0]
  if (messages.length !== 1 || message === undefined || message.kind !== 'request') {
    throw new Error('The frame does not hold exactly one request.')
  }
  return { request: { id: message.id, command: message.command }, buffers }
}

/** The single request of a request frame; throws for anything else. */
export function readRequest(frame: ArrayBuffer): Request {
  return readRequestFrame(frame).request
}

/** The events, the response and the buffers of a response frame; throws for anything else. */
export function readResponse(frame: ArrayBuffer): DecodedResponse {
  const { header, buffers } = decodeFrame(frame)
  const events: ProtocolEvent[] = []
  let response: ResponseMessage | null = null
  for (const message of header.messages) {
    if (response !== null) {
      throw new Error('The response is not the last message of the frame.')
    }
    if (message.kind === 'event') {
      events.push(message)
    } else if (message.kind === 'response') {
      response = message
    } else {
      throw new Error('A response frame holds a request.')
    }
  }
  if (response === null) {
    throw new Error('The frame holds no response.')
  }
  return { events, response, buffers }
}

/** Mesh buffers of one body in `MeshBuffers` field order, and the `MeshBuffers` indices into a frame whose buffer list already holds `firstIndex` buffers. */
export type MeshParts = { readonly buffers: readonly BufferView[]; readonly mesh: MeshBuffers }

/**
 * A minimal mesh that keeps every invariant of protocol.md 3.4: one triangle (face 0) spanning the
 * box, one edge segment (edge 0) along its bottom, and one vertex marker (vertex 0) at its minimum.
 */
export function meshParts(bounds: BoundingBox, firstIndex: number): MeshParts {
  const [minimumX, minimumY, minimumZ] = bounds.min
  const [maximumX, maximumY, maximumZ] = bounds.max
  const buffers: BufferView[] = [
    Float32Array.of(
      minimumX,
      minimumY,
      minimumZ,
      maximumX,
      minimumY,
      minimumZ,
      minimumX,
      maximumY,
      maximumZ,
    ),
    Float32Array.of(0, -1, 0, 0, -1, 0, 0, -1, 0),
    Uint32Array.of(0, 1, 2),
    Uint32Array.of(0, 0, 0),
    Float32Array.of(minimumX, minimumY, minimumZ, maximumX, minimumY, minimumZ),
    Uint32Array.of(0),
    Float32Array.of(minimumX, minimumY, minimumZ),
    Uint32Array.of(0),
  ]
  return {
    buffers,
    mesh: {
      positions: firstIndex,
      normals: firstIndex + 1,
      indices: firstIndex + 2,
      face_ids: firstIndex + 3,
      edge_positions: firstIndex + 4,
      edge_ids: firstIndex + 5,
      vertex_positions: firstIndex + 6,
      vertex_ids: firstIndex + 7,
      face_count: 1,
      edge_count: 1,
      vertex_count: 1,
      bounding_box: bounds,
    },
  }
}

/** Copies an `ArrayBuffer`'s bytes into a plain array, for comparing frames. */
export function bytesOf(buffer: ArrayBuffer): number[] {
  return Array.from(new Uint8Array(buffer))
}

/**
 * The value with every null-valued object key removed, recursively. The protocol treats a null
 * optional field and an absent one alike (protocol.md 2.3), so tests compare commands this way.
 */
export function withoutNulls(value: unknown): unknown {
  if (Array.isArray(value)) {
    return value.map(withoutNulls)
  }
  if (typeof value === 'object' && value !== null) {
    return Object.fromEntries(
      Object.entries(value)
        .filter(([, entry]) => entry !== null)
        .map(([key, entry]) => [key, withoutNulls(entry)]),
    )
  }
  return value
}

/** The edits of an apply_edits command; the protocol may omit the empty list. */
export function editsOf(
  command: Extract<Command, { type: 'apply_edits' }> | undefined,
): readonly Edit[] {
  return command?.edits ?? []
}
