import { describe, expect, it } from 'vitest'
import { decodeFrame, encodeFrame, FrameDecodeError } from './frame'

// Example frames A and B of protocol.md 2.2, byte for byte as the spec's hex dumps list them.
const EXAMPLE_A_HEX = `
  50 53 4d 31 6b 00 00 00 7b 22 70 72 6f 74 6f 63
  6f 6c 5f 76 65 72 73 69 6f 6e 22 3a 31 2c 22 6d
  65 73 73 61 67 65 73 22 3a 5b 7b 22 6b 69 6e 64
  22 3a 22 72 65 71 75 65 73 74 22 2c 22 69 64 22
  3a 31 2c 22 63 6f 6d 6d 61 6e 64 22 3a 7b 22 74
  79 70 65 22 3a 22 65 6e 67 69 6e 65 5f 69 6e 66
  6f 22 7d 7d 5d 2c 22 62 75 66 66 65 72 73 22 3a
  5b 5d 7d 00 00 00 00 00`

const EXAMPLE_B_HEX = `
  50 53 4d 31 b9 00 00 00 7b 22 70 72 6f 74 6f 63
  6f 6c 5f 76 65 72 73 69 6f 6e 22 3a 31 2c 22 6d
  65 73 73 61 67 65 73 22 3a 5b 7b 22 6b 69 6e 64
  22 3a 22 72 65 71 75 65 73 74 22 2c 22 69 64 22
  3a 32 2c 22 63 6f 6d 6d 61 6e 64 22 3a 7b 22 74
  79 70 65 22 3a 22 69 6d 70 6f 72 74 5f 73 74 65
  70 22 2c 22 64 6f 63 22 3a 31 2c 22 64 61 74 61
  22 3a 30 2c 22 6e 61 6d 65 22 3a 22 42 72 61 63
  6b 65 74 22 7d 7d 5d 2c 22 62 75 66 66 65 72 73
  22 3a 5b 7b 22 6f 66 66 73 65 74 22 3a 30 2c 22
  6c 65 6e 67 74 68 22 3a 31 34 2c 22 65 6c 65 6d
  65 6e 74 5f 74 79 70 65 22 3a 22 75 38 22 7d 5d
  7d 00 00 00 00 00 00 00 49 53 4f 2d 31 30 33 30
  33 2d 32 31 3b 0a`

function bytesFromHex(hex: string): Uint8Array {
  return Uint8Array.from(
    hex
      .trim()
      .split(/\s+/)
      .map((pair) => Number.parseInt(pair, 16)),
  )
}

/** A fresh ArrayBuffer holding exactly these bytes. */
function frameOf(bytes: Uint8Array): ArrayBuffer {
  const buffer = new ArrayBuffer(bytes.length)
  new Uint8Array(buffer).set(bytes)
  return buffer
}

function exampleA(): Uint8Array {
  return bytesFromHex(EXAMPLE_A_HEX)
}

function exampleB(): Uint8Array {
  return bytesFromHex(EXAMPLE_B_HEX)
}

/** A frame laid out by the rules of protocol.md 2.2 from a header text and raw buffer bytes. */
function layOut(header: string, buffers: readonly Uint8Array[]): Uint8Array {
  const headerBytes = new TextEncoder().encode(header)
  const padding = (8 - (headerBytes.length % 8)) % 8
  const start = 8 + headerBytes.length + padding
  let length = start
  const offsets = buffers.map((buffer) => {
    const offset = Math.ceil((length - start) / 8) * 8
    length = start + offset + buffer.length
    return offset
  })
  const bytes = new Uint8Array(length)
  bytes.set([0x50, 0x53, 0x4d, 0x31])
  new DataView(bytes.buffer).setUint32(4, headerBytes.length, true)
  bytes.set(headerBytes, 8)
  buffers.forEach((buffer, index) => {
    bytes.set(buffer, start + (offsets[index] ?? 0))
  })
  return bytes
}

function decodeError(bytes: Uint8Array): unknown {
  try {
    decodeFrame(frameOf(bytes))
  } catch (error) {
    return error
  }
  return null
}

describe('encodeFrame', () => {
  it('protocol test 53: encodes request id 1 engine_info with no buffers as exactly example A', () => {
    const frame = encodeFrame(
      {
        protocol_version: 1,
        messages: [{ kind: 'request', id: 1, command: { type: 'engine_info' } }],
      },
      [],
    )
    expect(frame).toBeInstanceOf(ArrayBuffer)
    expect(new Uint8Array(frame)).toEqual(exampleA())
  })

  it('encodes example B with its u8 buffer byte for byte', () => {
    const frame = encodeFrame(
      {
        protocol_version: 1,
        messages: [
          {
            kind: 'request',
            id: 2,
            command: { type: 'import_step', doc: 1, data: 0, name: 'Bracket' },
          },
        ],
      },
      [new TextEncoder().encode('ISO-10303-21;\n')],
    )
    expect(new Uint8Array(frame)).toEqual(exampleB())
  })

  it('places a u32 buffer after a 14-byte u8 buffer at offset 16 with zero gap bytes', () => {
    const frame = new Uint8Array(
      encodeFrame(
        { protocol_version: 1, messages: [{ kind: 'request', id: 1, command: { type: 'engine_info' } }] },
        [new Uint8Array(14).fill(7), Uint32Array.of(1, 2, 3)],
      ),
    )
    const { header } = decodeFrame(frameOf(frame))
    expect(header.buffers).toEqual([
      { offset: 0, length: 14, element_type: 'u8' },
      { offset: 16, length: 12, element_type: 'u32' },
    ])
    const headerLength = new DataView(frame.buffer).getUint32(4, true)
    const start = 8 + headerLength + ((8 - (headerLength % 8)) % 8)
    expect(frame.length).toBe(start + 28)
    expect(Array.from(frame.slice(start + 14, start + 16))).toEqual([0, 0])
    expect(Array.from(frame.slice(start + 16, start + 28))).toEqual([1, 0, 0, 0, 2, 0, 0, 0, 3, 0, 0, 0])
  })

  it('writes f32 values little-endian', () => {
    const frame = new Uint8Array(
      encodeFrame(
        { protocol_version: 1, messages: [{ kind: 'request', id: 1, command: { type: 'engine_info' } }] },
        [Float32Array.of(1)],
      ),
    )
    expect(Array.from(frame.slice(-4))).toEqual([0x00, 0x00, 0x80, 0x3f])
  })

  it('round-trips through decodeFrame', () => {
    const frame = encodeFrame(
      {
        protocol_version: 1,
        messages: [
          { kind: 'request', id: 9, command: { type: 'undo', doc: 3 } },
        ],
      },
      [Uint8Array.of(1, 2, 3), Float32Array.of(0.5, -2), Uint32Array.of(7)],
    )
    const decoded = decodeFrame(frame)
    expect(decoded.header.messages).toEqual([
      { kind: 'request', id: 9, command: { type: 'undo', doc: 3 } },
    ])
    expect(decoded.buffers.map((buffer) => Array.from(buffer))).toEqual([[1, 2, 3], [0.5, -2], [7]])
  })
})

describe('decodeFrame', () => {
  it('decodes example A to request id 1 engine_info with no buffers', () => {
    const decoded = decodeFrame(frameOf(exampleA()))
    expect(decoded.header).toEqual({
      protocol_version: 1,
      messages: [{ kind: 'request', id: 1, command: { type: 'engine_info' } }],
      buffers: [],
    })
    expect(decoded.buffers).toEqual([])
  })

  it('protocol test 54: example B gives one Uint8Array at byteOffset 200, length 14, aliasing the input', () => {
    const input = frameOf(exampleB())
    const { buffers } = decodeFrame(input)
    expect(buffers).toHaveLength(1)
    const [buffer] = buffers
    expect(buffer).toBeInstanceOf(Uint8Array)
    expect(buffer?.byteOffset).toBe(200)
    expect(buffer?.length).toBe(14)
    expect(buffer?.buffer).toBe(input)
    expect(new TextDecoder().decode(buffer)).toBe('ISO-10303-21;\n')
  })

  it('protocol test 55: an f32 buffer becomes a Float32Array aliasing the input', () => {
    const header =
      '{"protocol_version":1,"messages":[{"kind":"request","id":1,"command":{"type":"engine_info"}}],"buffers":[{"offset":0,"length":8,"element_type":"f32"}]}'
    const values = new Uint8Array(Float32Array.of(1.5, -3).buffer)
    const input = frameOf(layOut(header, [values]))
    const [buffer] = decodeFrame(input).buffers
    expect(buffer).toBeInstanceOf(Float32Array)
    expect(buffer?.buffer).toBe(input)
    expect(Array.from(buffer ?? [])).toEqual([1.5, -3])
  })

  it('a u32 buffer becomes a Uint32Array aliasing the input', () => {
    const header =
      '{"protocol_version":1,"messages":[{"kind":"request","id":1,"command":{"type":"engine_info"}}],"buffers":[{"offset":0,"length":8,"element_type":"u32"}]}'
    const input = frameOf(layOut(header, [new Uint8Array(Uint32Array.of(4, 5).buffer)]))
    const [buffer] = decodeFrame(input).buffers
    expect(buffer).toBeInstanceOf(Uint32Array)
    expect(buffer?.buffer).toBe(input)
    expect(Array.from(buffer ?? [])).toEqual([4, 5])
  })
})

describe('protocol test 56: decodeFrame throws FrameDecodeError for malformed frames', () => {
  it('test 9: three bytes "PSM"', () => {
    expect(decodeError(Uint8Array.of(0x50, 0x53, 0x4d))).toBeInstanceOf(FrameDecodeError)
  })

  it('test 10: example A with the first byte changed to 0x51', () => {
    const bytes = exampleA()
    bytes[0] = 0x51
    expect(decodeError(bytes)).toBeInstanceOf(FrameDecodeError)
  })

  it('test 17: example A with a padding byte set to 0x01', () => {
    const bytes = exampleA()
    bytes[116] = 0x01
    expect(decodeError(bytes)).toBeInstanceOf(FrameDecodeError)
  })

  it('test 18: example B with the buffer offset changed to 8', () => {
    const header = new TextDecoder()
      .decode(exampleB().slice(8, 8 + 185))
      .replace('"offset":0', '"offset":8')
    const bytes = exampleB()
    bytes.set(new TextEncoder().encode(header), 8)
    expect(decodeError(bytes)).toBeInstanceOf(FrameDecodeError)
  })

  it('test 22: example B with one extra byte, and example A with 8 zero bytes appended', () => {
    const longerB = new Uint8Array(215)
    longerB.set(exampleB())
    expect(decodeError(longerB)).toBeInstanceOf(FrameDecodeError)
    const longerA = new Uint8Array(128)
    longerA.set(exampleA())
    expect(decodeError(longerA)).toBeInstanceOf(FrameDecodeError)
  })

})
