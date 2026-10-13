/// <reference lib="dom" />
/// <reference lib="es2024.arraybuffer" />
// The module worker of e2e/pages/transfer.html (ui.md 6.7 P-12). For each request it encodes a new
// response frame, posts it as the core worker posts frames (protocol.md 2.11) with the frame in the
// transfer list, and then reports when it posted and what became of its frame.
import { encodeFrame } from '../../src/core/frame'
import type { FromWorker } from '../../src/core/worker-host'
import { PROTOCOL_VERSION } from '../../src/generated/protocol'

/** Asks for one frame: an `undo` response with this id and one `u8` buffer of `bytes` bytes. */
export type TransferRequest = { readonly id: number; readonly bytes: number }

/** Sent after the frame: when the worker posted it and the state of its `frame` afterwards. */
export type SentReport = {
  readonly type: 'sent'
  readonly id: number
  /** `performance.timeOrigin + performance.now()` read just before `postMessage`. */
  readonly sentAt: number
  readonly detached: boolean
  readonly byteLength: number
}

// The global of this dedicated worker; the end-to-end program is type-checked with the DOM library.
declare const self: {
  postMessage(message: FromWorker | SentReport, transfer?: Transferable[]): void
  addEventListener(type: 'message', listener: (event: MessageEvent<TransferRequest>) => void): void
}

self.addEventListener('message', (event) => {
  const { id, bytes } = event.data
  const frame = encodeFrame(
    {
      protocol_version: PROTOCOL_VERSION,
      messages: [
        { kind: 'response', id, result: { kind: 'ok', type: 'undo', revision: 1, label: 'Edit' } },
      ],
    },
    [new Uint8Array(bytes)],
  )
  const sentAt = performance.timeOrigin + performance.now()
  self.postMessage({ type: 'frame', frame }, [frame])
  self.postMessage(
    { type: 'sent', id, sentAt, detached: frame.detached, byteLength: frame.byteLength },
    [],
  )
})
