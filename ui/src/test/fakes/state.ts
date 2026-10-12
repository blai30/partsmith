import type { Document, PrintReport } from '../../generated/protocol'
import type { AppState, DocumentState } from '../../state/store'
import { INITIAL_STATE } from '../../state/store'
import { TEST_CATALOG } from './catalog'
import { TEST_PRINTERS } from './printers'

// Store states for tests that read or start from a state without going through the core.

/** A DocumentState for a recipe at the given revision, with nothing computed yet. */
export function documentStateFor(
  recipe: Document,
  overrides: Partial<DocumentState> = {},
): DocumentState {
  return {
    documentHandle: 1,
    name: 'Untitled',
    file: null,
    revision: 1,
    recipe,
    dirty: false,
    localUnsaved: false,
    undoLabel: null,
    redoLabel: null,
    transactionOpen: false,
    focus: null,
    featureStatus: {},
    parameterValues: {},
    diagnostics: [],
    sketches: {},
    datums: {},
    meshes: {},
    lastRebuildMilliseconds: null,
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
    core: { status: 'ready', engineVersion: '0.1.0', fatal: null, pendingMutations: 0 },
    catalog: TEST_CATALOG,
    printers: TEST_PRINTERS,
    document: documentStateFor(recipe, documentOverrides),
    ...overrides,
  }
}

type BodyPrint = PrintReport['bodies'][number]
type CheckResult = BodyPrint['report']['checks'][number]
type Finding = CheckResult['findings'][number]

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
