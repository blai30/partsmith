// Test support for the ui-model slice, on top of the shared fakes in src/test/fakes: an App built
// by hand around a FakeCoreApi with the built-in and catalog commands registered, the React provider
// around it, a fake viewport handle, and the browser features jsdom lacks (matchMedia, animation
// frames under the test's control, element sizes). Nothing here is part of the app.

import { render, type RenderResult } from '@testing-library/react'
import type { ReactElement } from 'react'
import { vi, type Mock } from 'vitest'
import { App as AppShell } from '../App'
import { AppProvider } from '../app/AppProvider'
import type { App, Platform } from '../app/createApp'
import { builtinCommands } from '../commands/builtin'
import { featureCommands } from '../commands/features'
import { createCommandRegistry } from '../commands/registry'
import { createAppStore, type AppState } from '../state/store'
import { createTestApp, LINUX_WEB } from '../test/fakes/app'
import { createFakeCoreApi, type FakeCoreApi } from '../test/fakes/core-api'
import { DEFAULT_CAMERA } from '../viewport/camera'
import type { ViewportHandle } from '../viewport/controller'

/** Registers the built-in commands and the commands of the store's catalog that are not registered yet. */
export function registerMissingCommands(app: App): void {
  const catalog = app.store.getState().catalog
  const definitions = [
    ...builtinCommands(app.platform),
    ...(catalog === null ? [] : featureCommands(catalog)),
  ]
  for (const definition of definitions) {
    if (app.commands.get(definition.id) === undefined) {
      app.commands.register(definition)
    }
  }
}

export type ModelApp = {
  readonly app: App
  readonly core: FakeCoreApi
}

/** An App by hand around a store with the given state, a FakeCoreApi and every command. */
export function createModelApp(
  options: { readonly state?: Partial<AppState>; readonly platform?: Platform } = {},
): ModelApp {
  const core = createFakeCoreApi()
  const app = createTestApp({
    store: createAppStore(options.state),
    core,
    commands: createCommandRegistry(),
    platform: options.platform ?? LINUX_WEB,
  })
  registerMissingCommands(app)
  return { app, core }
}

/** Renders an element (the whole shell by default) inside the app's provider. */
export function renderWithApp(app: App, element: ReactElement = <AppShell />): RenderResult {
  return render(<AppProvider app={app}>{element}</AppProvider>)
}

/** A ViewportHandle whose every method is a mock. */
export type FakeViewport = { readonly [Method in keyof ViewportHandle]: Mock<ViewportHandle[Method]> }

export function createFakeViewport(): FakeViewport {
  return {
    getCamera: vi.fn<ViewportHandle['getCamera']>(() => DEFAULT_CAMERA),
    setCamera: vi.fn<ViewportHandle['setCamera']>(),
    fit: vi.fn<ViewportHandle['fit']>(),
    setStandardView: vi.fn<ViewportHandle['setStandardView']>(),
    toggleProjection: vi.fn<ViewportHandle['toggleProjection']>(),
    lookAlong: vi.fn<ViewportHandle['lookAlong']>(),
    zoomToBounds: vi.fn<ViewportHandle['zoomToBounds']>(),
    screenRectangle: vi.fn<ViewportHandle['screenRectangle']>(() => null),
    requestRender: vi.fn<ViewportHandle['requestRender']>(),
  }
}

// Browser features jsdom lacks. `vi.unstubAllGlobals()` removes the stubs.

export type MediaOptions = { readonly dark?: boolean; readonly reducedMotion?: boolean }

/** Installs `window.matchMedia` answering the color scheme and reduced motion queries. */
export function stubMatchMedia(options: MediaOptions = {}): void {
  vi.stubGlobal('matchMedia', (query: string): MediaQueryList => {
    const matches =
      (query.includes('prefers-color-scheme: dark') && options.dark === true) ||
      (query.includes('prefers-reduced-motion: reduce') && options.reducedMotion === true)
    const list = {
      matches,
      media: query,
      onchange: null,
      addEventListener: () => undefined,
      removeEventListener: () => undefined,
      addListener: () => undefined,
      removeListener: () => undefined,
      dispatchEvent: () => true,
    }
    return list as unknown as MediaQueryList
  })
}

/** Animation frames that run only when the test flushes them, each 16 ms after the last on `performance.now()`. */
export type FrameQueue = {
  /** Runs the queued callbacks, then those they queue, up to `rounds` rounds. */
  flush(rounds?: number): void
  readonly pending: number
}

/** Stubs the frame functions and `performance.now()`; `vi.unstubAllGlobals()` and `vi.restoreAllMocks()` undo it. */
export function stubAnimationFrames(): FrameQueue {
  let nextId = 1
  let time = 1000
  vi.spyOn(performance, 'now').mockImplementation(() => time)
  const callbacks = new Map<number, FrameRequestCallback>()
  vi.stubGlobal('requestAnimationFrame', (callback: FrameRequestCallback) => {
    const id = nextId++
    callbacks.set(id, callback)
    return id
  })
  vi.stubGlobal('cancelAnimationFrame', (id: number) => {
    callbacks.delete(id)
  })
  return {
    flush(rounds = 20) {
      for (let round = 0; round < rounds && callbacks.size > 0; round++) {
        time += 16
        const due = [...callbacks.values()]
        callbacks.clear()
        for (const callback of due) {
          callback(time)
        }
      }
    },
    get pending() {
      return callbacks.size
    },
  }
}

/** Gives every element of a prototype a layout size, which jsdom never computes; returns the restorer. */
export function stubElementSize(
  prototype: HTMLElement,
  size: { readonly width: number; readonly height: number },
): () => void {
  const rectangle = vi
    .spyOn(prototype, 'getBoundingClientRect')
    .mockReturnValue(DOMRect.fromRect({ x: 0, y: 0, width: size.width, height: size.height }))
  const width = vi.spyOn(prototype, 'clientWidth', 'get').mockReturnValue(size.width)
  const height = vi.spyOn(prototype, 'clientHeight', 'get').mockReturnValue(size.height)
  return () => {
    rectangle.mockRestore()
    width.mockRestore()
    height.mockRestore()
  }
}

function ignore(): void {
  // Replaced by the promise executor before anyone can call it.
}

/** A promise with its resolve and reject functions, for answering a core call later. */
export class Deferred<Value> {
  readonly promise: Promise<Value>
  resolve: (value: Value) => void = ignore
  reject: (error: unknown) => void = ignore

  constructor() {
    this.promise = new Promise<Value>((resolve, reject) => {
      this.resolve = resolve
      this.reject = reject
    })
  }
}

export function deferred<Value>(): Deferred<Value> {
  return new Deferred<Value>()
}
