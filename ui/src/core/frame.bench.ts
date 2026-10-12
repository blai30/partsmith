import { describe, expect, test } from 'vitest'
import { PROTOCOL_VERSION, type Document, type Message } from '../generated/protocol'
import { decodeFrame, encodeFrame, type BufferView } from './frame'

// The TypeScript codec budgets of protocol.md section 5, on the frame the Rust budgets bench uses: a
// header of about 200 KB (a document of 2,700 parameters) and 50 MB of buffers. Run with
// `pnpm vitest bench --run src/core/frame.bench.ts`; each benchmark fails when its median is over
// budget. The budgets hold on the reference laptop, which the CI performance runner stands in for.

/** Parameters in the benchmark document, which make the header about 200 KB. */
const PARAMETER_COUNT = 2_700
/** Elements of each 4-byte buffer: 12.5 MB, so three of them and an equally long byte buffer make 50 MB. */
const ELEMENT_COUNT = 3_125_000

const DECODE_BUDGET_MILLISECONDS = 10
const ENCODE_BUDGET_MILLISECONDS = 120

function benchmarkDocument(): Document {
  return {
    format: 'partsmith',
    format_version: 1,
    units: 'mm',
    parameters: Array.from({ length: PARAMETER_COUNT }, (_, i) => ({
      id: `p-${i.toString(16).padStart(12, '0')}`,
      name: `length_${String(i)}`,
      expr: `${String(i)} mm + 2 * width`,
    })),
    bodies: [],
    features: [],
  }
}

const messages: Message[] = [
  {
    kind: 'event',
    type: 'document_changed',
    doc: 1,
    revision: 1,
    document: benchmarkDocument(),
    dirty: false,
    transaction_open: false,
  },
  { kind: 'response', id: 1, result: { kind: 'ok', type: 'undo', revision: 1, label: 'Edit' } },
]

const positions = Float32Array.from({ length: ELEMENT_COUNT }, (_, i) => i * 0.25)
const buffers: BufferView[] = [
  positions,
  positions.slice(),
  Uint32Array.from({ length: ELEMENT_COUNT }, (_, i) => i),
  new Uint8Array(4 * ELEMENT_COUNT).fill(0x5a),
]

const frame = encodeFrame({ protocol_version: PROTOCOL_VERSION, messages }, buffers)

describe('frame codec budgets (protocol.md 5)', () => {
  test('decodeFrame of a 200 KB header and 50 MB of buffers takes at most 10 ms', async ({
    bench,
  }) => {
    const result = await bench('decodeFrame', () => {
      decodeFrame(frame)
    }).run()
    expect(result.latency.p50).toBeLessThanOrEqual(DECODE_BUDGET_MILLISECONDS)
  })

  test('encodeFrame of a 200 KB header and 50 MB of buffers takes at most 120 ms', async ({
    bench,
  }) => {
    const result = await bench('encodeFrame', () => {
      encodeFrame({ protocol_version: PROTOCOL_VERSION, messages }, buffers)
    }).run()
    expect(result.latency.p50).toBeLessThanOrEqual(ENCODE_BUDGET_MILLISECONDS)
  })
})
