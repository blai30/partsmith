import { createLatestOnly, type LatestOnly } from '../../core/queue'
import type { Health } from '../../generated/protocol'
import { ALWAYS_SHOWN_CHECK } from '../../print/checks'
import { findingAt, locationCenter } from '../../print/health'
import type { AppState, AppStore, FindingKey } from '../../state/store'
import type { App } from '../createApp'
import { commitEdits, showCoreError } from './edits'
import { updateSettings } from './settings'

// The Print context (ui.md 3.17): printer choice, check visibility, the overhang map, analysis
// runs and findings.

/** Delay after the last revision change before the analysis runs in Print context. */
const ANALYSIS_DELAY_MILLISECONDS = 300
/** The smallest side of the cube framed around a finding, in mm. */
const MINIMUM_FOCUS_SIDE = 20
/** Share of the body's bounding-box diagonal used as the side of the framed cube. */
const FOCUS_DIAGONAL_SHARE = 0.25
const FOCUS_MARGIN = 0.3

/** One analysis at a time per app, newest waiting input only (2.5). */
const analyses = new WeakMap<AppStore, LatestOnly<number>>()

function analysisOf(app: App): LatestOnly<number> {
  let latest = analyses.get(app.store)
  if (latest === undefined) {
    const running = (value: boolean): void => {
      app.store.setState((state) =>
        state.print.running === value ? {} : { print: { ...state.print, running: value } },
      )
    }
    latest = createLatestOnly<number, Health | null>(
      (documentHandle) => {
        running(true)
        return app.core.analyzePrint(documentHandle, { orientation: { kind: 'as_modeled' } })
      },
      () => {
        // The report itself arrives as a print_health event; a superseded (null) answer changes nothing.
        running(false)
      },
      (error) => {
        running(false)
        showCoreError(app, error)
      },
    )
    analyses.set(app.store, latest)
  }
  return latest
}

/** Whether the open document has no report of its current revision. */
function reportStale(state: AppState): boolean {
  return (
    state.document !== null &&
    (state.print.report === null || state.print.reportRevision !== state.document.revision)
  )
}

/** Chooses the document's printer profile, or none, labeled "Change printer". */
export async function setPrinter(app: App, profileId: string | null): Promise<void> {
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

/** Turns the overhang color map and its legend on or off for this session (the Overhangs toggle). */
export function setOverhangMap(app: App, shown: boolean): void {
  app.store.setState((state) =>
    state.print.overhangMap === shown ? {} : { print: { ...state.print, overhangMap: shown } },
  )
}

/** Analyzes the open document as modeled, outside the mutation queue; resolves when no analysis is left. */
export async function runPrintAnalysis(app: App): Promise<void> {
  const document = app.store.getState().document
  if (document === null) {
    return
  }
  const latest = analysisOf(app)
  latest.submit(document.documentHandle)
  await latest.flush()
}

/**
 * The health badge's click (3.17): opens the health panel, and runs the analysis when the report
 * is missing or describes an older revision.
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

/** Whether the focused finding's marker must go: the health panel closed or the document changed (3.17). */
function focusOutdated(state: AppState, previous: AppState): boolean {
  return (
    state.print.focusedFinding !== null &&
    ((previous.panels.side === 'health' && state.panels.side !== 'health') ||
      state.document?.documentHandle !== previous.document?.documentHandle ||
      state.document?.revision !== previous.document?.revision)
  )
}

/**
 * Keeps the Print context's derived state (3.17). The analysis runs on its own in Print context:
 * 300 ms after the document revision last changed with no transaction open (or after the
 * transaction closes), and at once on entering Print when the report is stale or missing. The
 * focused finding is dropped when the health panel closes or the document changes. Returns the
 * function that stops both.
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
    const transactionClosed = previous.document?.transactionOpen === true && !document.transactionOpen
    if (!revisionChanged && !transactionClosed) {
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
  })
  return () => {
    cancelTimer()
    unsubscribe()
  }
}
