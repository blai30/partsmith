// Test support for the print and export tests: an `App` over a fake core, file service and
// viewport, plus recipe, mesh and print report fixtures (ui.md 2.8, 2.9, 3.17, 3.18; print.md 2.6).

import { vi, type Mock } from 'vitest'
import type { App, Platform } from '../app/createApp'
import { createCommandRegistry } from '../commands/registry'
import type { CoreApi, ExportedFile } from '../core/api'
import { CoreError, createCoreClient } from '../core/client'
import { decodeFrame, encodeFrame } from '../core/frame'
import { createMutationQueue } from '../core/queue'
import type { Transport } from '../core/transport'
import type { FileService } from '../files/file-service'
import { createMemoryRecentFiles } from '../files/recent'
import { createMemoryRecovery } from '../files/recovery'
import {
  PROTOCOL_VERSION,
  type Body,
  type BodyPrint,
  type BoundingBox,
  type CheckKind,
  type CheckResult,
  type Document,
  type ErrorCode,
  type Finding,
  type FindingCode,
  type Location,
  type PrinterProfile,
  type PrintReport,
  type Severity,
} from '../generated/protocol'
import {
  createAppStore,
  type AppState,
  type CoreState,
  type DocumentState,
  type MeshSummary,
  type PrintState,
} from '../state/store'
import { DEFAULT_CAMERA } from '../viewport/camera'
import type { ViewportHandle } from '../viewport/controller'

export const DOCUMENT_HANDLE = 7

const LINUX_WEB: Platform = { operatingSystem: 'linux', host: 'web' }

const READY_CORE: CoreState = {
  status: 'ready',
  engineVersion: '0.1.0',
  fatal: null,
  pendingMutations: 0,
}

// Printer profiles as list_printer_profiles returns them: built-ins, then session profiles (print.md 2.2).

export const GENERIC_FDM: PrinterProfile = {
  id: 'generic-fdm',
  name: 'Generic FDM 0.4 mm',
  technology: 'fdm',
  bed: { kind: 'rectangle', width: 256, depth: 256, height: 256 },
  nozzle_diameter: 0.4,
  layer_height: 0.2,
  max_overhang_angle: 0.7853981633974483,
  max_bridge_length: 10,
  fit: { press: 0.05, snug: 0.1, slip: 0.2, loose: 0.4 },
}

export const GENERIC_RESIN: PrinterProfile = {
  id: 'generic-resin',
  name: 'Generic resin MSLA',
  technology: 'resin',
  bed: { kind: 'rectangle', width: 130, depth: 80, height: 160 },
  layer_height: 0.05,
  max_overhang_angle: 0.5235987755982988,
  max_bridge_length: 5,
  fit: { press: 0.02, snug: 0.05, slip: 0.1, loose: 0.2 },
}

export const MY_MK4: PrinterProfile = {
  id: 'my-mk4',
  name: 'My MK4',
  technology: 'fdm',
  bed: { kind: 'rectangle', width: 250, depth: 210, height: 220 },
  nozzle_diameter: 0.4,
  layer_height: 0.15,
  max_overhang_angle: 0.7853981633974483,
  max_bridge_length: 10,
  fit: { press: 0.05, snug: 0.1, slip: 0.2, loose: 0.4 },
}

export const PRINTERS: readonly PrinterProfile[] = [GENERIC_FDM, GENERIC_RESIN, MY_MK4]

// Bodies of the test recipes.

export const CUBE: Body = { id: 'b-cube', name: 'Cube' }
export const PEG: Body = { id: 'b-peg', name: 'Peg' }
/** A body hidden in the viewport. */
export const GHOST: Body = { id: 'b-ghost', name: 'Ghost', visible: false }
/** A visible body without model-layer geometry. */
export const EMPTY: Body = { id: 'b-empty', name: 'Empty' }

export const CUBE_BOUNDS: BoundingBox = { min: [-10, -10, 0], max: [10, 10, 20] }
export const PEG_BOUNDS: BoundingBox = { min: [-4, -4, 10], max: [4, 4, 30] }

export function meshSummary(meshId: number, boundingBox: BoundingBox): MeshSummary {
  return { meshId, boundingBox, faceCount: 6, edgeCount: 12, vertexCount: 8 }
}

export function recipe(bodies: readonly Body[], printer: string | null = null): Document {
  return {
    format: 'partsmith',
    format_version: 1,
    units: 'mm',
    ...(printer === null ? {} : { printer }),
    parameters: [],
    bodies: [...bodies],
    features: [],
  }
}

export type DocumentOptions = {
  readonly name?: string
  readonly revision?: number
  readonly bodies?: readonly Body[]
  readonly printer?: string | null
  readonly meshes?: Readonly<Record<string, MeshSummary>>
  readonly transactionOpen?: boolean
}

/** An open document; by default "cube" at revision 3 with the Cube body and its model mesh. */
export function documentState(options: DocumentOptions = {}): DocumentState {
  return {
    documentHandle: DOCUMENT_HANDLE,
    name: options.name ?? 'cube',
    file: null,
    revision: options.revision ?? 3,
    recipe: recipe(options.bodies ?? [CUBE], options.printer ?? null),
    dirty: false,
    localUnsaved: false,
    undoLabel: null,
    redoLabel: null,
    transactionOpen: options.transactionOpen ?? false,
    focus: null,
    featureStatus: {},
    parameterValues: {},
    diagnostics: [],
    sketches: {},
    datums: {},
    meshes: options.meshes ?? { [CUBE.id]: meshSummary(1, CUBE_BOUNDS) },
    lastRebuildMilliseconds: null,
  }
}

export function printState(overrides: Partial<PrintState> = {}): PrintState {
  return {
    report: null,
    reportRevision: null,
    running: false,
    overhangMap: true,
    focusedFinding: null,
    ...overrides,
  }
}

// Print reports (protocol.md print_health, print.md 2.6).

/** CheckKind::ALL, the order of `BodyReport.checks`. */
const REPORT_CHECK_ORDER: readonly CheckKind[] = [
  'mesh_validity',
  'bed_fit',
  'first_layer_contact',
  'islands',
  'overhangs',
  'thin_walls',
  'small_features',
  'trapped_volumes',
]

const SEVERITY_RANK: Readonly<Record<Severity, number>> = { info: 1, warning: 2, error: 3 }

export type FindingOptions = {
  readonly check: CheckKind
  readonly code: FindingCode
  readonly severity: Severity
  readonly message: string
  /** Default: the point (0, 0, 0). */
  readonly location?: Location
  readonly limit?: number
}

export function finding(options: FindingOptions): Finding {
  return {
    check: options.check,
    code: options.code,
    severity: options.severity,
    message: options.message,
    location: options.location ?? { kind: 'point', at: [0, 0, 0] },
    ...(options.limit === undefined ? {} : { value: options.limit + 0.2, limit: options.limit }),
    triangles: [],
    faces: [],
  }
}

function worstSeverity(findings: readonly Finding[]): Severity | null {
  let worst: Severity | null = null
  for (const item of findings) {
    if (worst === null || SEVERITY_RANK[item.severity] > SEVERITY_RANK[worst]) {
      worst = item.severity
    }
  }
  return worst
}

function checkResult(check: CheckKind, findings: readonly Finding[], omitted: number): CheckResult {
  return { check, status: worstSeverity(findings) ?? 'ok', findings: [...findings], omitted }
}

/** Listed findings per check; checks left out are ok. */
export type BodyFindings = Partial<Record<CheckKind, readonly Finding[]>>
/** Findings beyond the cap per check (print.md MAX_FINDINGS_PER_CHECK). */
export type OmittedCounts = Partial<Record<CheckKind, number>>

export type BodyPrintOptions = {
  readonly body: string
  /** The analyzed model-layer mesh. */
  readonly meshId: number
  readonly findings?: BodyFindings
  readonly omitted?: OmittedCounts
}

/** One analyzed body with a result for every check, in canonical order. */
export function bodyPrint(options: BodyPrintOptions): BodyPrint {
  const checks = REPORT_CHECK_ORDER.map((check) =>
    checkResult(check, options.findings?.[check] ?? [], options.omitted?.[check] ?? 0),
  )
  const worst = worstSeverity(checks.flatMap((result) => result.findings))
  return {
    body: options.body,
    transform: [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 128, 128, 0, 1],
    mesh_id: options.meshId,
    overhang: 0,
    report: {
      profile: 'generic-fdm',
      rotation: {
        rows: [
          [1, 0, 0],
          [0, 1, 0],
          [0, 0, 1],
        ],
      },
      health: worst === 'error' ? 'error' : worst === 'warning' ? 'warning' : 'ok',
      checks,
    },
  }
}

/** A report of `analyze_print` with `orientation: as_modeled`. */
export function printReport(bodies: readonly BodyPrint[], profile = 'generic-fdm'): PrintReport {
  const counts = { info: 0, warning: 0, error: 0 }
  for (const body of bodies) {
    for (const check of body.report.checks) {
      for (const item of check.findings) {
        counts[item.severity] += 1
      }
      const status = worstSeverity(check.findings)
      if (status !== null) {
        counts[status] += check.omitted
      }
    }
  }
  const health = counts.error > 0 ? 'error' : counts.warning > 0 ? 'warning' : 'ok'
  return {
    profile,
    orientation: { kind: 'as_modeled' },
    summary: {
      health,
      errors: counts.error,
      warnings: counts.warning,
      infos: counts.info,
      label: health === 'ok' ? 'Ready to print' : `${String(counts.error)} errors`,
    },
    bodies: [...bodies],
  }
}

// The fake services.

/** A `CoreApi` whose every method is a mock; methods a test does not expect throw. */
export type FakeCore = { readonly [Method in keyof CoreApi]: Mock<CoreApi[Method]> }

export type FakeFiles = { readonly fileSystemAccess: boolean } & {
  readonly [Method in Exclude<keyof FileService, 'fileSystemAccess'>]: Mock<FileService[Method]>
}

export type FakeViewport = { readonly [Method in keyof ViewportHandle]: Mock<ViewportHandle[Method]> }

function unexpected(service: string, method: string): () => never {
  return () => {
    throw new Error(`The test ${service} does not expect ${method} to be called.`)
  }
}

export function exportedFile(name: string, bytes: Uint8Array): ExportedFile {
  return { name, mediaType: 'model/3mf', bytes }
}

/** Bytes of a 3MF file: a ZIP local file header. */
export const THREE_MF_BYTES = new Uint8Array([0x50, 0x4b, 0x03, 0x04, 0x14, 0x00])

export function createFakeCore(): FakeCore {
  const core = (method: string) => unexpected('core', method)
  return {
    engineInfo: vi.fn<CoreApi['engineInfo']>(core('engineInfo')),
    featureCatalog: vi.fn<CoreApi['featureCatalog']>(core('featureCatalog')),
    listPrinterProfiles: vi.fn<CoreApi['listPrinterProfiles']>(() => Promise.resolve(PRINTERS)),
    newDocument: vi.fn<CoreApi['newDocument']>(core('newDocument')),
    openDocument: vi.fn<CoreApi['openDocument']>(core('openDocument')),
    saveDocument: vi.fn<CoreApi['saveDocument']>(core('saveDocument')),
    markSaved: vi.fn<CoreApi['markSaved']>(core('markSaved')),
    closeDocument: vi.fn<CoreApi['closeDocument']>(core('closeDocument')),
    applyEdits: vi.fn<CoreApi['applyEdits']>(() => Promise.resolve(4)),
    renameParameter: vi.fn<CoreApi['renameParameter']>(core('renameParameter')),
    beginTransaction: vi.fn<CoreApi['beginTransaction']>(core('beginTransaction')),
    setFocus: vi.fn<CoreApi['setFocus']>(core('setFocus')),
    commitTransaction: vi.fn<CoreApi['commitTransaction']>(core('commitTransaction')),
    cancelTransaction: vi.fn<CoreApi['cancelTransaction']>(core('cancelTransaction')),
    undo: vi.fn<CoreApi['undo']>(core('undo')),
    redo: vi.fn<CoreApi['redo']>(core('redo')),
    solveSketch: vi.fn<CoreApi['solveSketch']>(core('solveSketch')),
    dragSketch: vi.fn<CoreApi['dragSketch']>(core('dragSketch')),
    endSketchDrag: vi.fn<CoreApi['endSketchDrag']>(core('endSketchDrag')),
    evaluate: vi.fn<CoreApi['evaluate']>(core('evaluate')),
    listElements: vi.fn<CoreApi['listElements']>(core('listElements')),
    pickReference: vi.fn<CoreApi['pickReference']>(core('pickReference')),
    resolveReferences: vi.fn<CoreApi['resolveReferences']>(core('resolveReferences')),
    analyzePrint: vi.fn<CoreApi['analyzePrint']>(() => Promise.resolve('ok')),
    exportBodies: vi.fn<CoreApi['exportBodies']>(() =>
      Promise.resolve({ files: [exportedFile('model.3mf', THREE_MF_BYTES)], warnings: [] }),
    ),
  }
}

/** A file service without the File System Access API: every save target is a download. */
export function createFakeFiles(): FakeFiles {
  return {
    fileSystemAccess: false,
    pickToOpen: vi.fn<FileService['pickToOpen']>(unexpected('file service', 'pickToOpen')),
    readHandle: vi.fn<FileService['readHandle']>(unexpected('file service', 'readHandle')),
    chooseSaveTarget: vi.fn<FileService['chooseSaveTarget']>((suggestedName) =>
      Promise.resolve({ kind: 'download', name: suggestedName }),
    ),
    write: vi.fn<FileService['write']>(() => Promise.resolve()),
  }
}

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

/** The state a test starts from: the core ready, the test printers and the cube document, then `overrides`. */
export function baseState(overrides: Partial<AppState> = {}): Partial<AppState> {
  return { core: READY_CORE, printers: PRINTERS, document: documentState(), ...overrides }
}

/** An `AppState` for pure functions of the state. */
export function stateWith(overrides: Partial<AppState> = {}): AppState {
  return createAppStore(baseState(overrides)).getState()
}

export type TestApp = {
  readonly app: App
  readonly core: FakeCore
  readonly files: FakeFiles
  readonly viewport: FakeViewport
}

export type TestAppOptions = {
  /** Whether a viewport is mounted (default true). */
  readonly viewportMounted?: boolean
}

export function createTestApp(
  overrides: Partial<AppState> = {},
  options: TestAppOptions = {},
): TestApp {
  const core = createFakeCore()
  const files = createFakeFiles()
  const viewport = createFakeViewport()
  let attached: ViewportHandle | null = options.viewportMounted === false ? null : viewport
  const app: App = {
    store: createAppStore(baseState(overrides)),
    core,
    mutations: createMutationQueue(),
    commands: createCommandRegistry(),
    files,
    recent: createMemoryRecentFiles(),
    recovery: createMemoryRecovery(),
    platform: LINUX_WEB,
    viewport: () => attached,
    attachViewport: (handle) => {
      attached = handle
    },
    restartCore: () => Promise.resolve(),
  }
  return { app, core, files, viewport }
}

/** Sets the open document's revision, as a document_changed event would. */
export function setRevision(app: App, revision: number): void {
  app.store.setState((state) =>
    state.document === null ? {} : { document: { ...state.document, revision } },
  )
}

export type Deferred<Value> = {
  readonly promise: Promise<Value>
  resolve(value: Value): void
  reject(reason: unknown): void
}

/** A promise the test settles by hand. */
export function deferred<Value>(): Deferred<Value> {
  let resolve: (value: Value) => void = () => undefined
  let reject: (reason: unknown) => void = () => undefined
  const promise = new Promise<Value>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise
    reject = rejectPromise
  })
  return { promise, resolve, reject }
}

/** Waits for one macrotask, so every promise callback queued before it has run. */
export function nextTask(): Promise<void> {
  return new Promise((resolve) => {
    setTimeout(resolve, 0)
  })
}

/**
 * A `CoreError` exactly as the core client raises it: the error response frame of a fake
 * transport, decoded by `createCoreClient` (ui.md 2.5).
 */
export async function coreError(code: ErrorCode, message: string): Promise<CoreError> {
  const transport: Transport = {
    kind: 'worker',
    start: () => Promise.resolve({ protocolVersion: PROTOCOL_VERSION, engineVersion: '0.1.0' }),
    send: (request) => {
      const sent = decodeFrame(request).header.messages[0]
      if (sent?.kind !== 'request') {
        return Promise.reject(new Error('The client sent a frame without a request.'))
      }
      return Promise.resolve(
        encodeFrame(
          {
            protocol_version: PROTOCOL_VERSION,
            messages: [{ kind: 'response', id: sent.id, result: { kind: 'error', code, message } }],
          },
          [],
        ),
      )
    },
    stop: () => undefined,
    onFatal: () => () => undefined,
  }
  const client = createCoreClient(transport)
  try {
    await client.request({ type: 'engine_info' })
  } catch (error) {
    if (error instanceof CoreError) {
      return error
    }
    throw error
  } finally {
    client.close()
  }
  throw new Error('The fake core answered with success instead of an error.')
}
