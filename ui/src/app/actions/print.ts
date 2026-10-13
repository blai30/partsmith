import { CoreError } from '../../core/client'
import { createLatestOnly } from '../../core/queue'
import type { DocHandle, Health, Orientation } from '../../generated/protocol'
import { MESSAGES } from '../../messages'
import { ALWAYS_SHOWN_CHECK } from '../../print/checks'
import { findingAt, locationCenter } from '../../print/health'
import { orientationToSend, sameOrientation } from '../../print/orientation'
import type { AppState, AppStore, FindingKey, SelectionItem } from '../../state/store'
import type { App } from '../createApp'
import { commitEdits, refusedDuringCommand, showCoreError } from './edits'
import { notify, showPickHint } from './notifications'
import { updateSettings } from './settings'

// The Print context (ui.md 3.17): printer choice, check visibility, Lay on face, analysis runs and
// findings.

/** Delay after the last revision change before the analysis runs in Print context. */
const ANALYSIS_DELAY_MILLISECONDS = 300
/** The smallest side of the cube framed around a finding, in mm. */
const MINIMUM_FOCUS_SIDE = 20
/** Share of the body's bounding-box diagonal used as the side of the framed cube. */
const FOCUS_DIAGONAL_SHARE = 0.25
const FOCUS_MARGIN = 0.3

/** What one analysis checks: a document at a revision, placed on the bed one way. */
type AnalysisInput = {
  readonly documentHandle: DocHandle
  readonly revision: number
  /** The session orientation sent, null as modeled. */
  readonly orientation: Orientation | null
}

/** Core errors that refuse the session orientation: the face is gone or cannot rest on the bed (3.17). */
const ORIENTATION_ERRORS: ReadonlySet<string> = new Set([
  'invalid_orientation',
  'reference_unresolved',
])

/** The print analyses of one app: one at a time, newest waiting input only (2.5). */
type Analyses = {
  /** Sends an analysis unless the one in flight or waiting already checks the same revision. */
  submit(input: AnalysisInput): void
  /** Resolves when no analysis is left. */
  flush(): Promise<void>
}

const analysesByStore = new WeakMap<AppStore, Analyses>()

function analysesOf(app: App): Analyses {
  const existing = analysesByStore.get(app.store)
  if (existing !== undefined) {
    return existing
  }
  const running = (value: boolean): void => {
    app.store.setState((state) =>
      state.print.running === value ? {} : { print: { ...state.print, running: value } },
    )
  }
  const latest = createLatestOnly<AnalysisInput, Health | null>(
    ({ documentHandle, orientation }) => {
      running(true)
      return app.core.analyzePrint(documentHandle, { orientation: orientationToSend(orientation) })
    },
    (health, { orientation }) => {
      // The report itself arrived as a print_health event before this answer; a superseded (null)
      // answer changes nothing.
      if (health === null) {
        running(false)
        return
      }
      app.store.setState((state) => ({
        print: { ...state.print, running: false, reportOrientation: orientation },
      }))
    },
    (error, { orientation }) => {
      running(false)
      if (!showOrientationError(app, error, orientation)) {
        showCoreError(app, error)
      }
    },
  )
  // The input submitted last. While `latest` is busy it is the analysis whose report comes next:
  // the waiting one, else the one in flight.
  let submitted: AnalysisInput | null = null
  const analyses: Analyses = {
    submit(input) {
      if (
        latest.busy &&
        submitted !== null &&
        submitted.documentHandle === input.documentHandle &&
        submitted.revision === input.revision &&
        sameOrientation(submitted.orientation, input.orientation)
      ) {
        return
      }
      submitted = input
      latest.submit(input)
    },
    flush: () => latest.flush(),
  }
  analysesByStore.set(app.store, analyses)
  return analyses
}

/** Whether the open document has no report of its current revision and print orientation. */
function reportStale(state: AppState): boolean {
  return (
    state.document !== null &&
    (state.print.report === null ||
      state.print.reportRevision !== state.document.revision ||
      !sameOrientation(state.print.reportOrientation, state.print.orientation))
  )
}

/**
 * Shows a core refusal of the orientation that was sent (`invalid_orientation`,
 * `reference_unresolved`): the orientation goes back to as modeled and the core's message shows as
 * a warning (3.17). Returns whether the error was such a refusal.
 */
export function showOrientationError(
  app: App,
  error: unknown,
  orientation: Orientation | null,
): boolean {
  if (
    orientation === null ||
    !(error instanceof CoreError) ||
    !ORIENTATION_ERRORS.has(error.code)
  ) {
    return false
  }
  setPrintOrientation(app, null)
  notify(app, { severity: 'warning', message: error.message, actions: [], code: error.code })
  return true
}

/**
 * Chooses the document's printer profile, or none, labeled "Change printer". During a feature
 * command it is refused with `finish_command_first` (3.7); nothing happens while another
 * transaction is open, or when the choice is the document's printer already, which would only add
 * an undo entry.
 */
export async function setPrinter(app: App, profileId: string | null): Promise<void> {
  const document = app.store.getState().document
  if (
    document === null ||
    refusedDuringCommand(app) ||
    document.transactionOpen ||
    (document.recipe.printer ?? null) === profileId
  ) {
    return
  }
  await commitEdits(app, 'Change printer', [
    profileId === null ? { type: 'set_printer' } : { type: 'set_printer', printer: profileId },
  ])
}

/**
 * Hides or shows a check's findings; the choice persists in the settings. Mesh validity is always
 * shown, so hiding it does nothing.
 */
export function setCheckHidden(app: App, check: string, hidden: boolean): void {
  if (hidden && check === ALWAYS_SHOWN_CHECK) {
    return
  }
  const others = app.store.getState().settings.hiddenChecks.filter((name) => name !== check)
  updateSettings(app, { hiddenChecks: hidden ? [...others, check] : others })
}

/**
 * Analyzes the open document in the session print orientation (as modeled without one), outside
 * the mutation queue; resolves when no analysis is left. An analysis of the current revision and
 * orientation that is already in flight or waiting is not sent again.
 */
export async function runPrintAnalysis(app: App): Promise<void> {
  const state = app.store.getState()
  const document = state.document
  if (document === null) {
    return
  }
  const analyses = analysesOf(app)
  analyses.submit({
    documentHandle: document.documentHandle,
    revision: document.revision,
    orientation: state.print.orientation,
  })
  await analyses.flush()
}

/** Starts the Lay on face pick (3.17): the next flat face clicked rests on the bed. */
export function startLayOnFace(app: App): void {
  app.store.setState((state) => ({
    print: { ...state.print, pickingFace: true },
    pickHint: state.pickHint?.field === null ? null : state.pickHint,
  }))
}

/** Ends the Lay on face pick without a face (Esc ladder step 5, 3.5). */
export function cancelLayOnFace(app: App): void {
  app.store.setState((state) =>
    state.print.pickingFace ? { print: { ...state.print, pickingFace: false } } : {},
  )
}

/** Sets the session print orientation and ends the face pick; null prints as modeled. */
export function setPrintOrientation(app: App, orientation: Orientation | null): void {
  app.store.setState((state) => ({
    print: { ...state.print, orientation, pickingFace: false },
  }))
}

/**
 * The face picked for Lay on face (3.17): a flat face of a visible part body rests on the bed;
 * any other face shows `pick_flat_face` in the print hint for 3 s and the pick goes on. Returns
 * whether the face was taken.
 */
export function layOnFace(app: App, item: SelectionItem): boolean {
  const state = app.store.getState()
  const element = item.element
  const body = state.document?.recipe.bodies?.find((candidate) => candidate.id === item.body)
  const taken =
    item.kind === 'face' &&
    element?.surface === 'plane' &&
    body !== undefined &&
    body.visible !== false &&
    (body.role ?? 'part') === 'part'
  if (!taken) {
    showPickHint(app, MESSAGES.pick_flat_face(), null)
    return false
  }
  setPrintOrientation(app, { kind: 'face_down', body: body.id, face: element.reference })
  return true
}

/**
 * The print check badge's click (3.17): opens the print check panel, and runs the analysis when
 * the report is missing or describes an older revision.
 */
export async function openPrintHealth(app: App): Promise<void> {
  app.store.setState((state) =>
    state.panels.side === 'health' ? {} : { panels: { ...state.panels, side: 'health' } },
  )
  if (reportStale(app.store.getState())) {
    await runPrintAnalysis(app)
  }
}

/** Shows a finding: marks it and frames a cube around its location (3.17). */
export function focusFinding(app: App, finding: FindingKey): void {
  app.store.setState((state) => ({ print: { ...state.print, focusedFinding: finding } }))
  const state = app.store.getState()
  const located = findingAt(state.print.report, finding)
  const viewport = app.viewport()
  if (located === undefined || viewport === null) {
    return
  }
  const center = locationCenter(located.location)
  const bodyBounds = state.document?.meshes[finding.body]?.boundingBox
  const diagonal =
    bodyBounds === undefined
      ? 0
      : Math.hypot(
          bodyBounds.max[0] - bodyBounds.min[0],
          bodyBounds.max[1] - bodyBounds.min[1],
          bodyBounds.max[2] - bodyBounds.min[2],
        )
  const half = Math.max(MINIMUM_FOCUS_SIDE, FOCUS_DIAGONAL_SHARE * diagonal) / 2
  viewport.zoomToBounds(
    {
      min: [center[0] - half, center[1] - half, center[2] - half],
      max: [center[0] + half, center[1] + half, center[2] + half],
    },
    FOCUS_MARGIN,
  )
}

/** Whether the focused finding's marker must go: the print check panel closed or the document changed (3.17). */
function focusOutdated(state: AppState, previous: AppState): boolean {
  return (
    state.print.focusedFinding !== null &&
    ((previous.panels.side === 'health' && state.panels.side !== 'health') ||
      state.document?.documentHandle !== previous.document?.documentHandle ||
      state.document?.revision !== previous.document?.revision)
  )
}

/** Whether the Lay on face pick must end: Print was left or a feature command started (3.17). */
function pickOutdated(state: AppState): boolean {
  return state.print.pickingFace && (state.context !== 'print' || state.command !== null)
}

/**
 * Keeps the Print context's derived state (3.17). The analysis runs on its own in Print context:
 * 300 ms after the document revision or the print orientation last changed with no transaction
 * open (or after the transaction closes), and at once on entering Print when the report is stale or
 * missing. The focused finding is dropped when the print check panel closes or the document
 * changes, and the Lay on face pick ends when Print is left. Returns the function that stops them.
 */
export function installPrintAnalysis(app: App): () => void {
  let timer: ReturnType<typeof setTimeout> | null = null

  function cancelTimer(): void {
    if (timer !== null) {
      clearTimeout(timer)
      timer = null
    }
  }

  function run(): void {
    void runPrintAnalysis(app)
  }

  function schedule(state: AppState, previous: AppState): void {
    const document = state.document
    if (state.context !== 'print' || document === null) {
      cancelTimer()
      return
    }
    if (previous.context !== 'print') {
      cancelTimer()
      if (reportStale(state) && !document.transactionOpen) {
        run()
      }
      return
    }
    const revisionChanged =
      document.documentHandle !== previous.document?.documentHandle ||
      document.revision !== previous.document?.revision
    const orientationChanged = !sameOrientation(state.print.orientation, previous.print.orientation)
    const transactionClosed =
      previous.document?.transactionOpen === true && !document.transactionOpen
    if (!revisionChanged && !orientationChanged && !transactionClosed) {
      return
    }
    cancelTimer()
    if (!document.transactionOpen && reportStale(state)) {
      timer = setTimeout(() => {
        timer = null
        run()
      }, ANALYSIS_DELAY_MILLISECONDS)
    }
  }

  const unsubscribe = app.store.subscribe((state, previous) => {
    schedule(state, previous)
    if (focusOutdated(state, previous)) {
      // Notifies every listener again; this one then sees no change that schedules anything.
      app.store.setState((current) => ({ print: { ...current.print, focusedFinding: null } }))
    }
    if (pickOutdated(app.store.getState())) {
      cancelLayOnFace(app)
    }
  })
  return () => {
    cancelTimer()
    unsubscribe()
  }
}
