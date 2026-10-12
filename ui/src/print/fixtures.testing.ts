// Fixtures for the print and export tests (ui.md 3.17, 3.18; print.md 2.6): recipes with bodies and
// model meshes, print reports, a recording viewport, and an App over the shared fakes of
// src/test/fakes with a chosen starting state.

import { vi, type Mock } from 'vitest'
import type { App } from '../app/createApp'
import type { ExportedFile } from '../core/api'
import type {
  Body,
  BodyPrint,
  BoundingBox,
  CheckKind,
  CheckResult,
  Document,
  Finding,
  FindingCode,
  Location,
  PrinterProfile,
  PrintReport,
  Severity,
} from '../generated/protocol'
import {
  createAppStore,
  type AppState,
  type CoreState,
  type DocumentState,
  type MeshSummary,
  type PrintState,
} from '../state/store'
import { createTestApp } from '../test/fakes/app'
import { createFakeCoreApi, type FakeCoreApi } from '../test/fakes/core-api'
import { createFakeFileService, type FakeFileService } from '../test/fakes/files'
import { TEST_PRINTERS } from '../test/fakes/printers'
import { DEFAULT_CAMERA } from '../viewport/camera'
import type { ViewportHandle } from '../viewport/controller'

export const DOCUMENT_HANDLE = 7

const READY_CORE: CoreState = {
  status: 'ready',
  engineVersion: '0.1.0',
  fatal: null,
  pendingMutations: 0,
}

/** A session profile, listed after the built-in ones (protocol.md list_printer_profiles). */
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

/** Generic FDM, generic resin, then My MK4. */
export const PRINTERS: readonly PrinterProfile[] = [...TEST_PRINTERS, MY_MK4]

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
    reportOrientation: null,
    running: false,
    overhangMap: true,
    focusedFinding: null,
    orientation: null,
    pickingFace: false,
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

// Export results.

/** Bytes of a 3MF file: a ZIP local file header. */
export const THREE_MF_BYTES = new Uint8Array([0x50, 0x4b, 0x03, 0x04, 0x14, 0x00])

export function exportedFile(name: string, bytes: Uint8Array): ExportedFile {
  return { name, mediaType: 'model/3mf', bytes }
}

// The App.

/** A `ViewportHandle` whose every method is a mock. */
export type FakeViewport = {
  readonly [Method in keyof ViewportHandle]: Mock<ViewportHandle[Method]>
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

/** The state tests start from: the core ready, the test printers and the cube document, then `overrides`. */
export function baseState(overrides: Partial<AppState> = {}): Partial<AppState> {
  return { core: READY_CORE, printers: PRINTERS, document: documentState(), ...overrides }
}

/** An `AppState` for the pure functions of the state. */
export function stateWith(overrides: Partial<AppState> = {}): AppState {
  return createAppStore(baseState(overrides)).getState()
}

export type PrintApp = {
  readonly app: App
  readonly core: FakeCoreApi
  readonly files: FakeFileService
  readonly viewport: FakeViewport
}

export type PrintAppOptions = {
  /** Whether a viewport is mounted (default true). */
  readonly viewportMounted?: boolean
}

/**
 * An App assembled by hand (createTestApp) with the state of `baseState(overrides)`, a recording
 * core, a file service without the File System Access API (every save target is a download) and
 * a recording viewport.
 */
export function createPrintApp(
  overrides: Partial<AppState> = {},
  options: PrintAppOptions = {},
): PrintApp {
  const core = createFakeCoreApi({
    analyzePrint: () => Promise.resolve('ok'),
    exportBodies: () =>
      Promise.resolve({ files: [exportedFile('model.3mf', THREE_MF_BYTES)], warnings: [] }),
  })
  const files = createFakeFileService({ fileSystemAccess: false })
  const viewport = createFakeViewport()
  const app = createTestApp({
    store: createAppStore(baseState(overrides)),
    core,
    files,
    viewport: options.viewportMounted === false ? null : viewport,
  })
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
}

/** A promise the test resolves by hand. */
export function deferred<Value>(): Deferred<Value> {
  let resolvePromise: ((value: Value) => void) | undefined
  const promise = new Promise<Value>((resolve) => {
    resolvePromise = resolve
  })
  return { promise, resolve: (value) => resolvePromise?.(value) }
}
