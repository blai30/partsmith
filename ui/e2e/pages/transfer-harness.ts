/// <reference lib="dom" />
// What e2e/pages/transfer.html puts on window for perf.spec.ts P-12 (ui.md 6.7): a module worker
// that posts response frames with a transfer list, and the page side that times each transfer and
// decodes the frame that arrives.
import { decodeFrame } from '../../src/core/frame'
import type { FromWorker } from '../../src/core/worker-host'
import type { SentReport, TransferRequest } from './transfer-worker'

/** One transfer as the page saw it. */
export type Transfer = {
  /** From the worker's reading just before `postMessage` to the page's first reading in its handler. */
  readonly milliseconds: number
  /** Whether the worker's `frame` was detached after `postMessage`, and its byte length then. */
  readonly sentDetached: boolean
  readonly sentByteLength: number
  /** The messages of the frame that arrived. */
  readonly messages: unknown
  /** How many buffers that frame holds, and the type and length of its first one. */
  readonly bufferCount: number
  readonly bufferType: string | null
  readonly bufferLength: number | null
}

const worker = new Worker(new URL('./transfer-worker.ts', import.meta.url), { type: 'module' })

/** Asks the worker for the frame with this id and waits for it and the worker's report. */
function transferOnce(id: number, bytes: number): Promise<Transfer> {
  return new Promise((resolve, reject) => {
    let arrived: { readonly frame: ArrayBuffer; readonly receivedAt: number } | null = null
    const stop = () => {
      worker.removeEventListener('message', onMessage)
      worker.removeEventListener('error', onError)
    }
    const onError = (event: ErrorEvent) => {
      stop()
      reject(new Error(`The transfer worker failed: ${event.message}`))
    }
    const onMessage = (event: MessageEvent<FromWorker | SentReport>) => {
      // Read first, so the time covers the transfer and nothing the handler does.
      const receivedAt = performance.timeOrigin + performance.now()
      const message = event.data
      if (message.type === 'frame') {
        arrived = { frame: message.frame, receivedAt }
        return
      }
      stop()
      if (message.type !== 'sent' || message.id !== id || arrived === null) {
        reject(new Error(`Unexpected message from the transfer worker: ${JSON.stringify(message)}`))
        return
      }
      const decoded = decodeFrame(arrived.frame)
      const [buffer] = decoded.buffers
      resolve({
        milliseconds: arrived.receivedAt - message.sentAt,
        sentDetached: message.detached,
        sentByteLength: message.byteLength,
        messages: decoded.header.messages,
        bufferCount: decoded.buffers.length,
        bufferType: buffer === undefined ? null : buffer[Symbol.toStringTag],
        bufferLength: buffer === undefined ? null : buffer.length,
      })
    }
    worker.addEventListener('message', onMessage)
    worker.addEventListener('error', onError)
    const request: TransferRequest = { id, bytes }
    worker.postMessage(request, [])
  })
}

/**
 * Has the worker send `count` frames of `bytes` buffer bytes with ids 1 to `count`, one in flight
 * at a time, and returns each transfer in order.
 */
export async function measureTransfers(count: number, bytes: number): Promise<Transfer[]> {
  const transfers: Transfer[] = []
  for (let id = 1; id <= count; id++) {
    // One frame in flight at a time, so no transfer waits behind another.
    // oxlint-disable-next-line no-await-in-loop
    transfers.push(await transferOnce(id, bytes))
  }
  return transfers
}
