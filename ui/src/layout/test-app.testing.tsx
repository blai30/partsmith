// Test support for the ui-model slice: an App built from the public pieces (store, mutation
// queue, command registry with the built-in and catalog commands) around a recording fake
// CoreApi, the React provider around it, and the browser features jsdom lacks (matchMedia,
// animation frames under the test's control, element sizes). Nothing here is part of the app.

import { render, type RenderResult } from '@testing-library/react'
import type { ReactElement } from 'react'
import { vi, type Mock } from 'vitest'
import { App as AppShell } from '../App'
import { AppProvider } from '../app/AppProvider'
import type { App, Platform } from '../app/createApp'
import { builtinCommands } from '../commands/builtin'
import { featureCommands } from '../commands/features'
import { createCommandRegistry } from '../commands/registry'
import type { CoreApi } from '../core/api'
import { CoreError } from '../core/client'
import { createMutationQueue } from '../core/queue'
import type { Dimension, ErrorCode } from '../generated/protocol'
import type { FileService } from '../files/file-service'
import { createMemoryRecentFiles } from '../files/recent'
import { createMemoryRecovery } from '../files/recovery'
import { createAppStore, type AppState } from '../state/store'
import { DEFAULT_CAMERA } from '../viewport/camera'
import type { ViewportHandle } from '../viewport/controller'
import { GENERIC_FDM, TEST_CATALOG_ENTRIES } from './fixtures.testing'

export const LINUX_WEB: Platform = { operatingSystem: 'linux', host: 'web' }
export const MAC_WEB: Platform = { operatingSystem: 'mac', host: 'web' }
export const WINDOWS_WEB: Platform = { operatingSystem: 'windows', host: 'web' }

/** A CoreApi whose every method is a mock with a working default. */
export type FakeCore = { readonly [Method in keyof CoreApi]: Mock<CoreApi[Method]> }

/** A protocol error as the core client raises it, without depending on its constructor. */
export function coreError(
  code: ErrorCode,
  message: string,
  extras: {
    readonly editIndex?: number | null
    readonly span?: { readonly start: number; readonly end: number } | null
  } = {},
): CoreError {
  const error = Object.create(CoreError.prototype) as CoreError
  Object.defineProperties(error, {
    name: { value: 'CoreError' },
    message: { value: message },
    code: { value: code },
    editIndex: { value: extras.editIndex ?? null },
    span: { value: extras.span ?? null },
  })
  return error
}

// A small evaluator for the fake core's `evaluate`: numbers with the units mm, cm, m, in, deg
// and rad, + - * / and parentheses. Values are in internal units (mm, rad).

type Quantity = { readonly value: number; readonly dimension: Dimension }

const UNITS: Readonly<Record<string, Quantity>> = {
  mm: { value: 1, dimension: { length: 1, angle: 0 } },
  cm: { value: 10, dimension: { length: 1, angle: 0 } },
  m: { value: 1000, dimension: { length: 1, angle: 0 } },
  in: { value: 25.4, dimension: { length: 1, angle: 0 } },
  deg: { value: Math.PI / 180, dimension: { length: 0, angle: 1 } },
  rad: { value: 1, dimension: { length: 0, angle: 1 } },
}

const TOKEN = /\s*(\d+(?:\.\d*)?|\.\d+|[A-Za-z_][A-Za-z0-9_]*|[-+*/()])/y

function tokenize(text: string): string[] {
  const tokens: string[] = []
  TOKEN.lastIndex = 0
  while (TOKEN.lastIndex < text.length) {
    const start = TOKEN.lastIndex
    const match = TOKEN.exec(text)
    if (match === null) {
      if (text.slice(start).trim() === '') {
        break
      }
      throw coreError('expression_error', `Unexpected '${text.slice(start).trim()}'.`)
    }
    tokens.push(match[1] ?? '')
  }
  return tokens
}

function sameDimension(first: Dimension, second: Dimension): boolean {
  return first.length === second.length && first.angle === second.angle
}

/** Evaluates a test expression; throws an `expression_error` CoreError when it cannot. */
export function evaluateExpression(text: string): { value: number; dimension: Dimension } {
  const tokens = tokenize(text)
  let position = 0
  const peek = () => tokens[position]
  const next = () => tokens[position++]

  function primary(): Quantity {
    const token = next()
    if (token === undefined) {
      throw coreError('expression_error', 'The expression ends too early.')
    }
    if (token === '(') {
      const inner = sum()
      if (next() !== ')') {
        throw coreError('expression_error', "A ')' is missing.")
      }
      return inner
    }
    if (token === '-') {
      const operand = primary()
      return { value: -operand.value, dimension: operand.dimension }
    }
    const number = Number(token)
    if (Number.isNaN(number)) {
      throw coreError('expression_error', `Unknown name '${token}'.`)
    }
    const unit = UNITS[peek() ?? '']
    if (unit !== undefined) {
      position++
      return { value: number * unit.value, dimension: unit.dimension }
    }
    return { value: number, dimension: { length: 0, angle: 0 } }
  }

  function product(): Quantity {
    let result = primary()
    while (peek() === '*' || peek() === '/') {
      const operator = next()
      const operand = primary()
      const sign = operator === '*' ? 1 : -1
      result = {
        value: operator === '*' ? result.value * operand.value : result.value / operand.value,
        dimension: {
          length: result.dimension.length + sign * operand.dimension.length,
          angle: result.dimension.angle + sign * operand.dimension.angle,
        },
      }
    }
    return result
  }

  function sum(): Quantity {
    let result = product()
    while (peek() === '+' || peek() === '-') {
      const operator = next()
      const operand = product()
      if (!sameDimension(result.dimension, operand.dimension)) {
        throw coreError('expression_error', 'Both sides of + and - need the same kind of value.')
      }
      result = {
        value: operator === '+' ? result.value + operand.value : result.value - operand.value,
        dimension: result.dimension,
      }
    }
    return result
  }

  if (tokens.length === 0) {
    throw coreError('expression_error', 'The expression is empty.')
  }
  const result = sum()
  if (position !== tokens.length) {
    throw coreError('expression_error', `Unexpected '${tokens[position] ?? ''}'.`)
  }
  return { value: result.value, dimension: result.dimension }
}

/** A recording CoreApi; each method resolves like a healthy core until a test overrides it. */
export function createFakeCore(): FakeCore {
  let revision = 1
  const nextRevision = () => ++revision
  return {
    engineInfo: vi.fn<CoreApi['engineInfo']>(() =>
      Promise.resolve({
        protocolVersion: 1,
        engineVersion: '0.1.0',
        formatVersion: 1,
        kernel: 'OCCT 7.9.3',
      }),
    ),
    featureCatalog: vi.fn<CoreApi['featureCatalog']>(() => Promise.resolve(TEST_CATALOG_ENTRIES)),
    listPrinterProfiles: vi.fn<CoreApi['listPrinterProfiles']>(() =>
      Promise.resolve([GENERIC_FDM]),
    ),
    newDocument: vi.fn<CoreApi['newDocument']>(() => Promise.resolve(2)),
    openDocument: vi.fn<CoreApi['openDocument']>(() =>
      Promise.resolve({ documentHandle: 2, migratedFrom: null }),
    ),
    saveDocument: vi.fn<CoreApi['saveDocument']>(() =>
      Promise.resolve({ text: '{}\n', revision }),
    ),
    markSaved: vi.fn<CoreApi['markSaved']>(() => Promise.resolve()),
    closeDocument: vi.fn<CoreApi['closeDocument']>(() => Promise.resolve()),
    applyEdits: vi.fn<CoreApi['applyEdits']>(() => Promise.resolve(nextRevision())),
    renameParameter: vi.fn<CoreApi['renameParameter']>(() => Promise.resolve(nextRevision())),
    beginTransaction: vi.fn<CoreApi['beginTransaction']>(() => Promise.resolve()),
    setFocus: vi.fn<CoreApi['setFocus']>(() => Promise.resolve()),
    commitTransaction: vi.fn<CoreApi['commitTransaction']>(() =>
      Promise.resolve({ revision: nextRevision(), recorded: true }),
    ),
    cancelTransaction: vi.fn<CoreApi['cancelTransaction']>(() => Promise.resolve(nextRevision())),
    undo: vi.fn<CoreApi['undo']>(() => Promise.resolve({ revision: nextRevision(), label: 'Edit' })),
    redo: vi.fn<CoreApi['redo']>(() => Promise.resolve({ revision: nextRevision(), label: 'Edit' })),
    solveSketch: vi.fn<CoreApi['solveSketch']>(() => Promise.resolve(null)),
    dragSketch: vi.fn<CoreApi['dragSketch']>(() => Promise.resolve(null)),
    endSketchDrag: vi.fn<CoreApi['endSketchDrag']>(() => Promise.resolve(nextRevision())),
    evaluate: vi.fn<CoreApi['evaluate']>((_documentHandle, expression) => {
      try {
        return Promise.resolve(evaluateExpression(expression))
      } catch (error: unknown) {
        return Promise.reject(error instanceof Error ? error : new Error(String(error)))
      }
    }),
    listElements: vi.fn<CoreApi['listElements']>(() => Promise.resolve([])),
    pickReference: vi.fn<CoreApi['pickReference']>(() =>
      Promise.reject(coreError('stale_pick', 'The model changed since this pick.')),
    ),
    resolveReferences: vi.fn<CoreApi['resolveReferences']>((_documentHandle, references) =>
      Promise.resolve(references.map(() => ({ status: 'resolved', matches: [] }))),
    ),
    analyzePrint: vi.fn<CoreApi['analyzePrint']>(() => Promise.resolve(null)),
    exportBodies: vi.fn<CoreApi['exportBodies']>(() =>
      Promise.resolve({ files: [], warnings: [] }),
    ),
  }
}

export type FakeFiles = { readonly [Method in Exclude<keyof FileService, 'fileSystemAccess'>]: Mock<FileService[Method]> } & {
  readonly fileSystemAccess: boolean
}

export function createFakeFiles(): FakeFiles {
  return {
    fileSystemAccess: true,
    pickToOpen: vi.fn<FileService['pickToOpen']>(() => Promise.resolve(null)),
    readHandle: vi.fn<FileService['readHandle']>(() =>
      Promise.reject(new Error('The test file service has no files.')),
    ),
    chooseSaveTarget: vi.fn<FileService['chooseSaveTarget']>(() => Promise.resolve(null)),
    write: vi.fn<FileService['write']>(() => Promise.resolve()),
  }
}

export type TestApp = {
  readonly app: App
  readonly core: FakeCore
  readonly files: FakeFiles
}

/**
 * An App around a fresh store with the given state, the fake core, the built-in commands and the
 * commands of the state's catalog.
 */
export function createTestApp(
  options: { readonly state?: Partial<AppState>; readonly platform?: Platform } = {},
): TestApp {
  const store = createAppStore(options.state)
  const core = createFakeCore()
  const files = createFakeFiles()
  const platform = options.platform ?? LINUX_WEB
  const commands = createCommandRegistry()
  for (const command of builtinCommands(platform)) {
    commands.register(command)
  }
  const catalog = store.getState().catalog
  if (catalog !== null) {
    for (const command of featureCommands(catalog)) {
      commands.register(command)
    }
  }
  let viewport: ViewportHandle | null = null
  const app: App = {
    store,
    core,
    mutations: createMutationQueue(),
    commands,
    files,
    recent: createMemoryRecentFiles(),
    recovery: createMemoryRecovery(),
    platform,
    viewport: () => viewport,
    attachViewport: (handle) => {
      viewport = handle
    },
    restartCore: () => Promise.resolve(),
  }
  return { app, core, files }
}

/** Renders an element (the whole shell by default) inside the app's provider. */
export function renderWithApp(app: App, element: ReactElement = <AppShell />): RenderResult {
  return render(<AppProvider app={app}>{element}</AppProvider>)
}

// Browser features jsdom lacks.

export type MediaOptions = { readonly dark?: boolean; readonly reducedMotion?: boolean }

/** Installs `window.matchMedia` answering the color scheme and reduced motion queries; `vi.unstubAllGlobals()` removes it. */
export function stubMatchMedia(options: MediaOptions = {}): void {
  const listeners = new Set<(event: MediaQueryListEvent) => void>()
  const matchMedia = (query: string): MediaQueryList => {
    const matches =
      (query.includes('prefers-color-scheme: dark') && options.dark === true) ||
      (query.includes('prefers-reduced-motion: reduce') && options.reducedMotion === true)
    return {
      matches,
      media: query,
      onchange: null,
      addEventListener: (_type: string, listener: (event: MediaQueryListEvent) => void) => {
        listeners.add(listener)
      },
      removeEventListener: (_type: string, listener: (event: MediaQueryListEvent) => void) => {
        listeners.delete(listener)
      },
      addListener: (listener: (event: MediaQueryListEvent) => void) => {
        listeners.add(listener)
      },
      removeListener: (listener: (event: MediaQueryListEvent) => void) => {
        listeners.delete(listener)
      },
      dispatchEvent: () => true,
    } as unknown as MediaQueryList
  }
  vi.stubGlobal('matchMedia', matchMedia)
}

/** Animation frames that run only when the test flushes them; `vi.unstubAllGlobals()` removes them. */
export type FrameQueue = {
  /** Runs every queued callback, including ones queued while running, up to `limit` rounds. */
  flush(limit?: number): void
  readonly pending: number
}

export function stubAnimationFrames(): FrameQueue {
  let nextId = 1
  let time = 0
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
    flush(limit = 20) {
      for (let round = 0; round < limit && callbacks.size > 0; round++) {
        time += 16
        const due = [...callbacks.entries()]
        callbacks.clear()
        for (const [, callback] of due) {
          callback(time)
        }
      }
    },
    get pending() {
      return callbacks.size
    },
  }
}

/** Gives every element of a tag a layout size, which jsdom never computes. */
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

/** A promise with its resolve and reject functions, for answering a core call later. */
export type Deferred<Value> = {
  readonly promise: Promise<Value>
  resolve(value: Value): void
  reject(error: unknown): void
}

export function deferred<Value>(): Deferred<Value> {
  let resolve: (value: Value) => void = () => undefined
  let reject: (error: unknown) => void = () => undefined
  const promise = new Promise<Value>((onResolve, onReject) => {
    resolve = onResolve
    reject = onReject
  })
  return { promise, resolve, reject }
}

/** Lets pending promise callbacks run. */
export async function settle(rounds = 5): Promise<void> {
  for (let round = 0; round < rounds; round++) {
    await Promise.resolve()
  }
}

/** The labels of an `apply_edits` call, for asserting edit batches by their `type`. */
export function editTypes(edits: readonly { readonly type: string }[]): string[] {
  return edits.map((edit) => edit.type)
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
