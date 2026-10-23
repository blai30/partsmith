import type { BufferView } from '../../core/frame'
import { CoreFatalError, type CoreReady, type Transport } from '../../core/transport'
import { PROTOCOL_VERSION, type Command, type Request } from '../../generated/protocol'
import {
  createFakeCore,
  replyFrame,
  type FakeCore,
  type FakeCoreOptions,
  type FakeReply,
} from './core'
import { readRequestFrame } from './protocol'

// A Transport whose core is the in-memory fake of ./core, of the worker kind unless a test asks
// for the Tauri kind. Every start() boots a fresh
// core (new documents, handles from 1), the way restarting the geometry core does. Requests are
// answered one at a time in arrival order, with promises only (no timers), so tests may use fake
// timers freely.

type CommandOfType<Type extends Command['type']> = Extract<Command, { type: Type }>

/**
 * Answers one command type instead of the fake core's default; `core.reply(command, buffers)` gives
 * the default. `buffers` are the request frame's buffers.
 */
export type FakeHandler<Type extends Command['type']> = (
  command: CommandOfType<Type>,
  core: FakeCore,
  buffers: readonly BufferView[],
) => FakeReply | Promise<FakeReply>

export type FakeTransportOptions = FakeCoreOptions & {
  /** Receives "command:<type>" for every request, so tests can order requests against other effects. */
  readonly journal?: string[]
  /** The transport's kind; default 'worker'. */
  readonly kind?: Transport['kind']
}

export type FakeTransport = Transport & {
  /** Every request received, oldest first, across restarts. */
  readonly requests: readonly Request[]
  /** The commands of `requests`. */
  readonly commands: readonly Command[]
  /** The buffers of each request frame, in the order of `requests`. */
  readonly requestBuffers: readonly (readonly BufferView[])[]
  /** The buffers of the request frame that carried `command`, one of `commands`; throws for another. */
  buffersOf(command: Command): readonly BufferView[]
  /** The received commands of one type, oldest first. */
  commandsOfType<Type extends Command['type']>(type: Type): readonly CommandOfType<Type>[]
  /** The types of the received commands, oldest first. */
  commandTypes(): readonly Command['type'][]
  /** How many times start() was called. */
  readonly starts: number
  /** The core booted by the latest successful start(); throws before the first. */
  readonly core: FakeCore
  /** Replaces the answer to one command type; null restores the fake core's default. */
  handle<Type extends Command['type']>(type: Type, handler: FakeHandler<Type> | null): void
  /** Makes the next start() reject with this error. */
  failNextStart(error: CoreFatalError): void
  /** The core dies outside the caller's control: requests not yet answered and later ones reject, and the fatal listeners hear it once. */
  die(message: string): void
  /** Holds every answer until resume(). */
  pause(): void
  /** Answers the held requests in order. */
  resume(): void
  /** Forgets the recorded requests. */
  clearRequests(): void
}

function isCommandOfType<Type extends Command['type']>(
  command: Command,
  type: Type,
): command is CommandOfType<Type> {
  return command.type === type
}

export function createFakeTransport(options: FakeTransportOptions = {}): FakeTransport {
  const requests: Request[] = []
  const requestBuffers: (readonly BufferView[])[] = []
  const handlers = new Map<
    Command['type'],
    (
      command: Command,
      core: FakeCore,
      buffers: readonly BufferView[],
    ) => FakeReply | Promise<FakeReply>
  >()
  const fatalListeners = new Set<(error: CoreFatalError) => void>()
  let core: FakeCore | null = null
  let starts = 0
  let startError: CoreFatalError | null = null
  let fatal: CoreFatalError | null = null
  let fatalReported = false
  let chain: Promise<unknown> = Promise.resolve()
  let gate: Promise<void> | null = null
  let openGate: (() => void) | null = null

  function currentCore(): FakeCore {
    if (core === null) {
      throw new Error('The fake transport has not been started.')
    }
    return core
  }

  function reportFatal(error: CoreFatalError): void {
    fatal = error
    if (fatalReported) {
      return
    }
    fatalReported = true
    for (const listener of fatalListeners) {
      listener(error)
    }
  }

  async function answer(request: Request, buffers: readonly BufferView[]): Promise<ArrayBuffer> {
    if (gate !== null) {
      await gate
    }
    if (fatal !== null) {
      throw fatal
    }
    const answeringCore = currentCore()
    const handler = handlers.get(request.command.type)
    const reply = await (handler === undefined
      ? answeringCore.reply(request.command, buffers)
      : handler(request.command, answeringCore, buffers))
    if (fatal !== null) {
      throw fatal
    }
    return replyFrame(request.id, reply)
  }

  const transport: FakeTransport = {
    kind: options.kind ?? 'worker',
    get requests() {
      return requests
    },
    get commands() {
      return requests.map((request) => request.command)
    },
    get requestBuffers() {
      return requestBuffers
    },
    buffersOf(command) {
      const index = requests.findIndex((request) => request.command === command)
      const buffers = requestBuffers[index]
      if (buffers === undefined) {
        throw new Error(`The fake transport received no such ${command.type} request.`)
      }
      return buffers
    },
    commandsOfType(type) {
      return requests.flatMap((request) =>
        isCommandOfType(request.command, type) ? [request.command] : [],
      )
    },
    commandTypes() {
      return requests.map((request) => request.command.type)
    },
    get starts() {
      return starts
    },
    get core() {
      return currentCore()
    },
    async start(): Promise<CoreReady> {
      starts += 1
      if (startError !== null) {
        const error = startError
        startError = null
        throw error
      }
      core = createFakeCore(options)
      fatal = null
      fatalReported = false
      return {
        protocolVersion: options.protocolVersion ?? PROTOCOL_VERSION,
        engineVersion: options.engineVersion ?? '0.1.0',
      }
    },
    send(frame) {
      if (fatal !== null) {
        return Promise.reject(fatal)
      }
      const { request, buffers } = readRequestFrame(frame)
      requests.push(request)
      requestBuffers.push(buffers)
      options.journal?.push(`command:${request.command.type}`)
      const response = chain.then(() => answer(request, buffers))
      chain = response.catch(() => undefined)
      return response
    },
    stop() {
      fatal = new CoreFatalError('The geometry core was stopped.')
      fatalReported = true
    },
    onFatal(listener) {
      fatalListeners.add(listener)
      return () => {
        fatalListeners.delete(listener)
      }
    },
    handle(type, handler) {
      if (handler === null) {
        handlers.delete(type)
        return
      }
      handlers.set(type, (command, answeringCore, buffers) => {
        if (!isCommandOfType(command, type)) {
          throw new Error(`The handler for ${type} received ${command.type}.`)
        }
        return handler(command, answeringCore, buffers)
      })
    },
    failNextStart(error) {
      startError = error
    },
    die(message) {
      reportFatal(new CoreFatalError(message))
    },
    pause() {
      if (gate === null) {
        gate = new Promise((resolve) => {
          openGate = resolve
        })
      }
    },
    resume() {
      openGate?.()
      gate = null
      openGate = null
    },
    clearRequests() {
      requests.length = 0
      requestBuffers.length = 0
    },
  }
  return transport
}
