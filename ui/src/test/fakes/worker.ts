import type { FromWorker } from '../../core/worker-host'

/** One `postMessage` call on the fake worker. */
export type PostedMessage = {
  readonly message: unknown
  readonly transfer: readonly Transferable[]
}

/** A Worker stand-in: records what the transport posts and lets the test speak for the worker. */
export class FakeWorker extends EventTarget {
  onmessage: ((event: MessageEvent) => void) | null = null
  onerror: ((event: Event) => void) | null = null
  onmessageerror: ((event: MessageEvent) => void) | null = null
  readonly posted: PostedMessage[] = []
  terminated = false

  postMessage(message: unknown, transfer?: Transferable[] | StructuredSerializeOptions): void {
    const list = Array.isArray(transfer) ? transfer : (transfer?.transfer ?? [])
    this.posted.push({ message, transfer: list })
  }

  terminate(): void {
    this.terminated = true
  }

  /** The worker posts a message to the main thread. */
  emit(data: FromWorker): void {
    const event = new MessageEvent('message', { data })
    this.dispatchEvent(event)
    this.onmessage?.(event)
  }

  /** The worker script throws. */
  fail(message: string): void {
    const event = new ErrorEvent('error', { message })
    this.dispatchEvent(event)
    this.onerror?.(event)
  }

  /** A message from the worker cannot be deserialized. */
  failMessage(): void {
    const event = new MessageEvent('messageerror')
    this.dispatchEvent(event)
    this.onmessageerror?.(event)
  }
}
