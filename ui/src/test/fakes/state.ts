import type {
  BodyPrint,
  BoundingBox,
  CheckResult,
  Document,
  FeatureStatus,
  Finding,
  PrintReport,
} from '../../generated/protocol'
import type { AppState, CoreState, DocumentState, MeshSummary } from '../../state/store'
import { INITIAL_STATE } from '../../state/store'
import { TEST_CATALOG } from './catalog'
import { featuresOf } from './documents'
import { TEST_PRINTERS } from './printers'

// Store states for tests that read or start from a state without going through the core.

/** The core started and answering. */
export const READY_CORE: CoreState = {
  status: 'ready',
  engineVersion: '0.1.0',
  fatal: null,
  pendingMutations: 0,
  stillWorking: false,
}

/** The bounds of the 20 mm cube of cube.ptsm. */
export const CUBE_BOUNDS: BoundingBox = { min: [-10, -10, 0], max: [10, 10, 20] }
/** The bounds of the peg of two-bodies.ptsm. */
export const PEG_BOUNDS: BoundingBox = { min: [-4, -4, 10], max: [4, 4, 30] }

/** A model-layer mesh with a box's element counts. */
export function meshSummary(meshId: number, boundingBox: BoundingBox): MeshSummary {
  return { meshId, boundingBox, faceCount: 6, edgeCount: 12, vertexCount: 8 }
}

function okStatus(id: string): FeatureStatus {
  return { id, status: 'ok', messages: [], repairs: [] }
}

export type DocumentStateOptions = {
  /** Gives every feature of the recipe an ok status, as after a rebuild (default: no statuses yet). */
  readonly okStatuses?: boolean
}

/** A DocumentState (document handle 1) for a recipe at revision 1, with nothing computed yet. */
export function documentStateFor(
  recipe: Document,
  overrides: Partial<DocumentState> = {},
  options: DocumentStateOptions = {},
): DocumentState {
  return {
    documentHandle: 1,
    name: 'Untitled',
    file: null,
    revision: 1,
    recipe,
    dirty: false,
    localUnsaved: false,
    recoveryId: 'session-1',
    undoLabel: null,
    redoLabel: null,
    transactionOpen: false,
    focus: null,
    featureStatus:
      options.okStatuses === true
        ? Object.fromEntries(
            featuresOf(recipe).map((feature) => [feature.id, okStatus(feature.id)]),
          )
        : {},
    parameterValues: {},
    diagnostics: [],
    sketches: {},
    datums: {},
    meshes: {},
    lastRebuildMilliseconds: null,
    bareNumbers: 0,
    ghosts: {},
    ...overrides,
  }
}

/** The initial state with the core ready, the test catalog and printers, and the recipe open. */
export function stateWithDocument(
  recipe: Document,
  overrides: Partial<AppState> = {},
  documentOverrides: Partial<DocumentState> = {},
): AppState {
  return {
    ...INITIAL_STATE,
    core: READY_CORE,
    catalog: TEST_CATALOG,
    printers: TEST_PRINTERS,
    document: documentStateFor(recipe, documentOverrides),
    ...overrides,
  }
}

/** A print finding of one check at a point. */
export function finding(
  check: Finding['check'],
  severity: Finding['severity'],
  message = `${check} ${severity}`,
): Finding {
  return {
    check,
    code: check === 'overhangs' ? 'overhang' : 'footprint_too_large',
    severity,
    message,
    location: { kind: 'point', at: [0, 0, 0] },
    triangles: [],
    faces: [],
  }
}

/** A check result holding the findings, with the given number of omitted findings. */
export function checkResult(
  check: CheckResult['check'],
  findings: readonly Finding[],
  omitted = 0,
): CheckResult {
  const severities = new Set(findings.map((item) => item.severity))
  const status = severities.has('error')
    ? 'error'
    : severities.has('warning')
      ? 'warning'
      : severities.has('info')
        ? 'info'
        : 'ok'
  return { check, status, findings: [...findings], omitted }
}

function bodyPrint(body: string, checks: readonly CheckResult[]): BodyPrint {
  const statuses = new Set(checks.map((check) => check.status))
  return {
    body,
    transform: [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 128, 128, 0, 1],
    report: {
      profile: 'generic-fdm',
      rotation: {
        rows: [
          [1, 0, 0],
          [0, 1, 0],
          [0, 0, 1],
        ],
      },
      health: statuses.has('error') ? 'error' : statuses.has('warning') ? 'warning' : 'ok',
      checks: [...checks],
    },
  }
}

/** A print report for one body per entry of `checksByBody`, keyed by body id. */
export function printReport(
  checksByBody: Readonly<Record<string, readonly CheckResult[]>>,
): PrintReport {
  return {
    profile: 'generic-fdm',
    orientation: { kind: 'as_modeled' },
    summary: { health: 'ok', errors: 0, warnings: 0, infos: 0, label: 'Ready to print' },
    bodies: Object.entries(checksByBody).map(([body, checks]) => bodyPrint(body, checks)),
  }
}
