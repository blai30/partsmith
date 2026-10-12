import { wrapEmscriptenModule, type EmscriptenCoreModule } from './bridge'
import { createWorkerHost, type ToWorker } from './worker-host'

// The core worker entry (ui.md 2.3): loads the Emscripten module from public/core and hands every
// message to the worker host.

declare const self: DedicatedWorkerGlobalScope

/** The factory `createPartsmithCore` that the Emscripten module exports by default. */
type CoreFactory = () => Promise<EmscriptenCoreModule>

function factoryOf(module: unknown): CoreFactory {
  const factory: unknown =
    typeof module === 'object' && module !== null ? Reflect.get(module, 'default') : undefined
  if (typeof factory !== 'function') {
    throw new TypeError('partsmith_core.mjs does not export the core factory.')
  }
  return factory as CoreFactory
}

const host = createWorkerHost({
  load: () =>
    import(/* @vite-ignore */ `${import.meta.env.BASE_URL}core/partsmith_core.mjs`)
      .then((module: unknown) => factoryOf(module)())
      .then(wrapEmscriptenModule),
  post: (message, transfer) => {
    self.postMessage(message, transfer)
  },
  schedule: (task) => {
    setTimeout(task, 0)
  },
})

self.addEventListener('message', (event: MessageEvent<ToWorker>) => {
  host.receive(event.data)
})
