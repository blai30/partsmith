/// <reference lib="dom" />
/// <reference types="vite/client" />
// The module worker of e2e/pages/engine.html (engine.md section 5): it compiles the web core module once, makes new
// instances of it, and times each request around `callCore`, the path the app's core worker takes. The tests reach its
// four functions on `partsmithEngine` through Playwright's `Worker.evaluate`.
import {
  callCore,
  wrapEmscriptenModule,
  type CoreModule,
  type EmscriptenCoreModule,
} from '../../src/core/bridge'
import { decodeFrame, encodeFrame } from '../../src/core/frame'
import { PROTOCOL_VERSION, type Command, type Message } from '../../src/generated/protocol'

/** What `call` gives: the command's type, the request id it sent, the time `callCore` took and the messages. */
export type Answer = {
  readonly type: Command['type']
  readonly id: number
  readonly milliseconds: number
  /** The response frame's messages: the events, then the response. */
  readonly messages: readonly Message[]
}

/** The functions the worker puts on its global scope as `partsmithEngine`. */
export type PartsmithEngine = {
  /** Makes a new instance of the compiled module, whose `main` creates its session, and returns its number. */
  readonly start: () => Promise<number>
  /** Sends one command to an instance and returns its answer. */
  readonly call: (instance: number, command: Command) => Answer
  /** The byte length of an instance's wasm memory, which grows and never shrinks. */
  readonly memoryBytes: (instance: number) => number
  /** Drops an instance, so its memory can be collected. */
  readonly end: (instance: number) => void
}

/** The Emscripten hook that instantiates the module, handing the instance and module to `receive`. */
type InstantiateWasm = (
  imports: WebAssembly.Imports,
  receive: (instance: WebAssembly.Instance, module: WebAssembly.Module) => void,
) => WebAssembly.Exports

/** The default export of `partsmith_core.mjs`. */
type CoreFactory = (options: {
  readonly instantiateWasm: InstantiateWasm
}) => Promise<EmscriptenCoreModule>

/** A running instance and the id of its next request. */
type Instance = { readonly core: CoreModule; nextId: number }

// The global of this dedicated worker; the end-to-end program is type-checked with the DOM library.
declare const self: {
  partsmithEngine: PartsmithEngine
  postMessage(message: 'ready', transfer: Transferable[]): void
}

const coreDirectory = `${import.meta.env.BASE_URL}core/`

/** The compiled module and the factory, loaded by the first `start` and kept. */
let loaded: Promise<readonly [WebAssembly.Module, CoreFactory]> | null = null

const instances = new Map<number, Instance>()
let lastInstance = 0

function load(): Promise<readonly [WebAssembly.Module, CoreFactory]> {
  return Promise.all([
    WebAssembly.compileStreaming(fetch(`${coreDirectory}partsmith_core.wasm`)),
    // Vite rejects a literal import of a file in public/, so the URL is built as core.worker.ts builds it.
    import(/* @vite-ignore */ `${coreDirectory}partsmith_core.mjs`).then(
      (module: { readonly default: CoreFactory }) => module.default,
    ),
  ])
}

async function start(): Promise<number> {
  try {
    loaded ??= load()
    const [compiled, createCore] = await loaded
    const core = await createCore({
      // Synchronous, so a failure rejects the factory instead of leaving it waiting.
      instantiateWasm: (imports, receive) => {
        const instance = new WebAssembly.Instance(compiled, imports)
        receive(instance, compiled)
        return instance.exports
      },
    })
    lastInstance += 1
    instances.set(lastInstance, { core: wrapEmscriptenModule(core), nextId: 1 })
    return lastInstance
  } catch (error) {
    throw new Error(`The web core did not start: ${String(error)}`, { cause: error })
  }
}

function notRunning(instance: number): Error {
  return new Error(
    `No engine instance ${String(instance)} is running; start() makes one and end() drops it.`,
  )
}

function running(instance: number): Instance {
  const found = instances.get(instance)
  if (found === undefined) {
    throw notRunning(instance)
  }
  return found
}

function call(instance: number, command: Command): Answer {
  const target = running(instance)
  const id = target.nextId
  target.nextId += 1
  const request = encodeFrame(
    { protocol_version: PROTOCOL_VERSION, messages: [{ kind: 'request', id, command }] },
    [],
  )
  const startTime = performance.now()
  let response: ArrayBuffer
  try {
    response = callCore(target.core, request)
  } catch (error) {
    // A trap, an abort or an allocation the core refused leaves the instance unusable.
    instances.delete(instance)
    throw new Error(
      `The web core stopped during ${command.type} (request ${String(id)}): ${String(error)}`,
      { cause: error },
    )
  }
  const milliseconds = performance.now() - startTime
  return { type: command.type, id, milliseconds, messages: decodeFrame(response).header.messages }
}

self.partsmithEngine = {
  start,
  call,
  memoryBytes: (instance) => running(instance).core.memory().length,
  end: (instance) => {
    if (!instances.delete(instance)) {
      throw notRunning(instance)
    }
  },
}
self.postMessage('ready', [])
