import type { BufferView } from '../../core/frame'
import { PROTOCOL_VERSION, type Command, type Request } from '../../generated/protocol'
import { createFakeCore, replyFrame, type FakeCore, type FakeCoreOptions } from './core'
import { readRequestFrame } from './protocol'
import { FakeWorker } from './worker'

// Test support: workers for `createWorkerTransport` that run the in-memory core of ./core the way
// core.worker.ts runs the real one (protocol.md 2.11). Each worker boots a fresh core, reports ready,
// and answers its calls one at a time in arrival order. A call the test marks as stalled is never
// answered, nor is any call after it, as when the core is stuck in a long kernel operation; only
// terminating the worker ends that. Everything runs on promises, so tests may use fake timers.

/** A fake worker running a fake core. */
export type FakeCoreWorker = FakeWorker & {
  /** The commands of the calls this worker received, oldest first, answered or not. */
  readonly commands: readonly Command[]
}

export type FakeCoreWorkers = {
  /** The `createWorker` option of `createWorkerTransport`. */
  readonly createWorker: () => Worker
  /** Every worker created so far, oldest first. */
  readonly workers: readonly FakeCoreWorker[]
  /** From now on, calls whose command matches are never answered; null answers every call again. */
  stallWhen(predicate: ((command: Command) => boolean) | null): void
}

function isCall(
  message: unknown,
): message is { readonly type: 'call'; readonly frame: ArrayBuffer } {
  return (
    typeof message === 'object' &&
    message !== null &&
    Reflect.get(message, 'type') === 'call' &&
    Reflect.get(message, 'frame') instanceof ArrayBuffer
  )
}

class CoreWorker extends FakeWorker implements FakeCoreWorker {
  readonly commands: Command[] = []
  readonly #core: FakeCore
  readonly #stalls: (command: Command) => boolean
  #answering: Promise<void> = Promise.resolve()

  constructor(core: FakeCore, stalls: (command: Command) => boolean) {
    super()
    this.#core = core
    this.#stalls = stalls
    // The worker host reports ready once the core module has loaded.
    queueMicrotask(() => {
      if (!this.terminated) {
        this.emit({ type: 'ready', protocolVersion: PROTOCOL_VERSION, engineVersion: '0.1.0' })
      }
    })
  }

  override postMessage(
    message: unknown,
    transfer?: Transferable[] | StructuredSerializeOptions,
  ): void {
    super.postMessage(message, transfer)
    if (!isCall(message)) {
      return
    }
    const { request, buffers } = readRequestFrame(message.frame)
    this.commands.push(request.command)
    this.#answering = this.#answering.then(() => this.#answer(request, buffers))
  }

  async #answer(request: Request, buffers: readonly BufferView[]): Promise<void> {
    if (this.#stalls(request.command)) {
      // The core never returns from this call; the worker is busy until it is terminated.
      await new Promise<never>(() => undefined)
    }
    const frame = replyFrame(request.id, this.#core.reply(request.command, buffers))
    if (!this.terminated) {
      this.emit({ type: 'frame', frame })
    }
  }
}

/** Workers that each boot a fake core with these options. */
export function createFakeCoreWorkers(options: FakeCoreOptions = {}): FakeCoreWorkers {
  const workers: CoreWorker[] = []
  let stalled: ((command: Command) => boolean) | null = null
  return {
    createWorker: () => {
      const worker = new CoreWorker(
        createFakeCore(options),
        (command) => stalled?.(command) ?? false,
      )
      workers.push(worker)
      return worker as unknown as Worker
    },
    workers,
    stallWhen(predicate) {
      stalled = predicate
    },
  }
}
