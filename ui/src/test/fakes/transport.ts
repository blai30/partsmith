import { CoreFatalError, type CoreReady, type Transport } from '../../core/transport'
import { PROTOCOL_VERSION, type Command, type Request } from '../../generated/protocol'
import { createFakeCore, type FakeCore, type FakeCoreOptions, type FakeReply } from './core'
import { readRequest, responseFrame } from './protocol'

// A worker-kind Transport whose core is the in-memory fake of ./core. Every start() boots a fresh
// core (new documents, handles from 1), the way restarting the geometry core does. Requests are
// answered one at a time in arrival order, with promises only (no timers), so tests may use fake
// timers freely.

type CommandOfType<Type extends Command['type']> = Extract<Command, { type: Type }>

/** Answers one command type instead of the fake core's default; `core.reply(command)` gives the default. */
export type FakeHandler<Type extends Command['type']> = (
  command: CommandOfType<Type>,
  core: FakeCore,
) => FakeReply | Promise<FakeReply>

export type FakeTransportOptions = FakeCoreOptions & {
  /** Receives "command:<type>" for every request, so tests can order requests against other effects. */
  readonly journal?: string[]
}

export type FakeTransport = Transport & {
  /** Every request received, oldest first, across restarts. */
  readonly requests: readonly Request[]
  /** The commands of `requests`. */
  readonly commands: readonly Command[]
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
  const handlers = new Map<
    Command['type'],
    (command: Command, core: FakeCore) => FakeReply | Promise<FakeReply>
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

  async function answer(request: Request): Promise<ArrayBuffer> {
    if (gate !== null) {
      await gate
    }
    if (fatal !== null) {
      throw fatal
    }
    const answeringCore = currentCore()
    const handler = handlers.get(request.command.type)
    const reply = await (handler === undefined
      ? answeringCore.reply(request.command)
      : handler(request.command, answeringCore))
    if (fatal !== null) {
      throw fatal
    }
    return 'result' in reply
      ? responseFrame({
          id: request.id,
          result: reply.result,
          events: reply.events ?? [],
          buffers: reply.buffers ?? [],
        })
      : responseFrame({ id: request.id, error: reply.error, events: reply.events ?? [] })
  }

  const transport: FakeTransport = {
    kind: 'worker',
    get requests() {
      return requests
    },
    get commands() {
      return requests.map((request) => request.command)
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
      const request = readRequest(frame)
      requests.push(request)
      options.journal?.push(`command:${request.command.type}`)
      const response = chain.then(() => answer(request))
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
      handlers.set(type, (command, answeringCore) => {
        if (!isCommandOfType(command, type)) {
          throw new Error(`The handler for ${type} received ${command.type}.`)
        }
        return handler(command, answeringCore)
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
    },
  }
  return transport
}
