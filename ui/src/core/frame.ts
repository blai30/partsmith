import {
  PROTOCOL_VERSION,
  type BufferInfo,
  type ElementType,
  type ErrorCode,
  type Message,
  type PartsmithProtocol,
} from '../generated/protocol'

// The PSM1 frame codec of protocol.md 2.2 and 3.2: a JSON header followed by 8-byte aligned raw
// buffers. Decoded buffers are views into the frame (no copies). Every JavaScript engine the app runs
// on is little-endian, which is the wire byte order, so buffers are copied and viewed as they are.

/** A raw buffer of a frame. */
export type BufferView = Uint8Array | Uint32Array | Float32Array

/** The JSON header exactly as it appears on the wire. */
export type FrameHeader = PartsmithProtocol

/** A decoded frame: its header and the buffers it references by index. */
export type DecodedFrame = { readonly header: FrameHeader; readonly buffers: BufferView[] }

/** Why a frame could not be decoded (protocol.md `FrameError`). */
export class FrameDecodeError extends Error {
  override readonly name = 'FrameDecodeError'
}

const MAGIC = [0x50, 0x53, 0x4d, 0x31] as const
const PREAMBLE_LENGTH = 8
/** Largest accepted JSON header, in bytes (64 MiB). */
export const MAX_HEADER_LENGTH = 67_108_864

const ELEMENT_SIZES: Readonly<Record<ElementType, number>> = { u8: 1, u32: 4, f32: 4 }

function paddingAfter(length: number): number {
  return (8 - (length % 8)) % 8
}

function elementTypeOf(view: BufferView): ElementType {
  // The tag survives realm boundaries, unlike instanceof.
  switch (view[Symbol.toStringTag]) {
    case 'Float32Array':
      return 'f32'
    case 'Uint32Array':
      return 'u32'
    default:
      return 'u8'
  }
}

function bytesOf(view: BufferView): Uint8Array {
  return new Uint8Array(view.buffer, view.byteOffset, view.byteLength)
}

/** Encodes a header (without its buffers table) and buffers into a frame, byte-identical to the Rust encoder. */
export function encodeFrame(
  header: Omit<FrameHeader, 'buffers'>,
  buffers: readonly BufferView[],
): ArrayBuffer {
  const infos: BufferInfo[] = []
  let end = 0
  for (const view of buffers) {
    const offset = end + paddingAfter(end)
    infos.push({ offset, length: view.byteLength, element_type: elementTypeOf(view) })
    end = offset + view.byteLength
  }
  const headerText = JSON.stringify({
    protocol_version: header.protocol_version,
    messages: header.messages,
    buffers: infos,
  })
  const headerBytes = new TextEncoder().encode(headerText)
  if (headerBytes.length > MAX_HEADER_LENGTH) {
    throw new RangeError(
      `The frame header is ${String(headerBytes.length)} bytes, more than the ${String(MAX_HEADER_LENGTH)}-byte limit.`,
    )
  }
  const bufferStart = PREAMBLE_LENGTH + headerBytes.length + paddingAfter(headerBytes.length)
  const frame = new ArrayBuffer(bufferStart + end)
  const bytes = new Uint8Array(frame)
  bytes.set(MAGIC)
  new DataView(frame).setUint32(4, headerBytes.length, true)
  bytes.set(headerBytes, PREAMBLE_LENGTH)
  buffers.forEach((view, index) => {
    bytes.set(bytesOf(view), bufferStart + (infos[index]?.offset ?? 0))
  })
  return frame
}

/** Encodes a frame holding a single error response and no buffers (protocol.md `error_response_frame`). */
export function errorResponseFrame(id: number, code: ErrorCode, message: string): ArrayBuffer {
  return encodeFrame(
    {
      protocol_version: PROTOCOL_VERSION,
      messages: [{ kind: 'response', id, result: { kind: 'error', code, message } }],
    },
    [],
  )
}

function isRecord(value: unknown): value is Readonly<Record<string, unknown>> {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
}

function isWireInteger(value: unknown): value is number {
  return typeof value === 'number' && Number.isSafeInteger(value) && value >= 0
}

function isElementType(value: unknown): value is ElementType {
  return value === 'u8' || value === 'u32' || value === 'f32'
}

function readBufferInfos(value: unknown): BufferInfo[] {
  if (!Array.isArray(value)) {
    throw new FrameDecodeError('The frame header is not a valid message: "buffers" is not a list.')
  }
  return value.map((entry: unknown, index) => {
    if (
      !isRecord(entry) ||
      !isWireInteger(entry['offset']) ||
      !isWireInteger(entry['length']) ||
      !isElementType(entry['element_type'])
    ) {
      throw new FrameDecodeError(
        `The frame header is not a valid message: buffer ${String(index)} is not a buffer description.`,
      )
    }
    return { offset: entry['offset'], length: entry['length'], element_type: entry['element_type'] }
  })
}

function messageProblem(message: unknown): string | null {
  if (!isRecord(message)) {
    return 'a message is not an object'
  }
  switch (message['kind']) {
    case 'request':
      return isWireInteger(message['id']) &&
        isRecord(message['command']) &&
        typeof message['command']['type'] === 'string'
        ? null
        : 'a request has no id or command'
    case 'response': {
      const result = message['result']
      return isWireInteger(message['id']) &&
        isRecord(result) &&
        (result['kind'] === 'ok' || result['kind'] === 'error')
        ? null
        : 'a response has no id or outcome'
    }
    case 'event':
      return typeof message['type'] === 'string' ? null : 'an event has no type'
    default:
      return 'a message has an unknown kind'
  }
}

function readMessages(value: unknown): Message[] {
  if (!Array.isArray(value)) {
    throw new FrameDecodeError('The frame header is not a valid message: "messages" is not a list.')
  }
  for (const message of value) {
    const problem = messageProblem(message)
    if (problem !== null) {
      throw new FrameDecodeError(`The frame header is not a valid message: ${problem}.`)
    }
  }
  // Shapes inside commands, results and events are the engine's contract; clients ignore unknown fields.
  return value as Message[]
}

function parseHeader(bytes: Uint8Array): Readonly<Record<string, unknown>> {
  let parsed: unknown
  try {
    parsed = JSON.parse(new TextDecoder('utf-8', { fatal: true }).decode(bytes))
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error)
    throw new FrameDecodeError(`The frame header is not valid JSON: ${reason}`)
  }
  if (!isRecord(parsed)) {
    throw new FrameDecodeError('The frame header is not a valid message: it is not an object.')
  }
  return parsed
}

function checkZero(bytes: Uint8Array, start: number, end: number): void {
  for (let i = start; i < end; i += 1) {
    if (bytes[i] !== 0) {
      throw new FrameDecodeError('Frame padding must be zero bytes.')
    }
  }
}

function truncated(length: number, expected: number): FrameDecodeError {
  return new FrameDecodeError(
    `The frame is ${String(length)} bytes long, but its layout needs ${String(expected)} bytes.`,
  )
}

function viewOf(frame: ArrayBuffer, start: number, info: BufferInfo): BufferView {
  const size = ELEMENT_SIZES[info.element_type]
  switch (info.element_type) {
    case 'f32':
      return new Float32Array(frame, start, info.length / size)
    case 'u32':
      return new Uint32Array(frame, start, info.length / size)
    case 'u8':
      return new Uint8Array(frame, start, info.length)
  }
}

/** Decodes a frame; buffer views alias the input ArrayBuffer (no copy). Throws FrameDecodeError. */
export function decodeFrame(frame: ArrayBuffer): DecodedFrame {
  const bytes = new Uint8Array(frame)
  if (bytes.length < PREAMBLE_LENGTH) {
    throw truncated(bytes.length, PREAMBLE_LENGTH)
  }
  if (MAGIC.some((byte, index) => bytes[index] !== byte)) {
    throw new FrameDecodeError('The frame does not start with PSM1.')
  }
  const headerLength = new DataView(frame).getUint32(4, true)
  if (headerLength > MAX_HEADER_LENGTH) {
    throw new FrameDecodeError(
      `The frame header is ${String(headerLength)} bytes, more than the ${String(MAX_HEADER_LENGTH)}-byte limit.`,
    )
  }
  const headerEnd = PREAMBLE_LENGTH + headerLength
  const bufferStart = headerEnd + paddingAfter(headerLength)
  if (bufferStart > bytes.length) {
    throw truncated(bytes.length, bufferStart)
  }
  const header = parseHeader(bytes.subarray(PREAMBLE_LENGTH, headerEnd))
  const version = header['protocol_version']
  if (!isWireInteger(version)) {
    throw new FrameDecodeError(
      'The frame header is not a valid message: "protocol_version" is not an integer.',
    )
  }
  if (version !== PROTOCOL_VERSION) {
    throw new FrameDecodeError(
      `The frame uses protocol version ${String(version)}, but this app speaks version ${String(PROTOCOL_VERSION)}. Reload the app.`,
    )
  }
  const infos = readBufferInfos(header['buffers'])
  checkZero(bytes, headerEnd, bufferStart)
  let end = 0
  infos.forEach((info, index) => {
    const expectedOffset = end + paddingAfter(end)
    if (info.offset !== expectedOffset || info.length % ELEMENT_SIZES[info.element_type] !== 0) {
      throw new FrameDecodeError(
        `Buffer ${String(index)} is laid out incorrectly: it should start at offset ${String(expectedOffset)} and hold whole elements.`,
      )
    }
    const bufferEnd = bufferStart + info.offset + info.length
    if (bufferEnd > bytes.length) {
      throw truncated(bytes.length, bufferEnd)
    }
    checkZero(bytes, bufferStart + end, bufferStart + info.offset)
    end = info.offset + info.length
  })
  const frameEnd = bufferStart + end
  if (bytes.length > frameEnd) {
    throw new FrameDecodeError(
      `The frame has ${String(bytes.length - frameEnd)} unexpected bytes after its last buffer.`,
    )
  }
  const messages = readMessages(header['messages'])
  return {
    header: { protocol_version: version, messages, buffers: infos },
    buffers: infos.map((info) => viewOf(frame, bufferStart + info.offset, info)),
  }
}
