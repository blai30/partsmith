import { createLatestOnly, type LatestOnly } from '../../core/queue'
import type { BoundingBox, Health } from '../../generated/protocol'
import type { AppStore, FindingKey } from '../../state/store'
import type { Vector3 } from '../../viewport/camera'
import type { App } from '../createApp'
import { commitEdits, showCoreError } from './edits'
import { updateSettings } from './settings'

// The Print context (ui.md 3.17): printer choice, check visibility, analysis runs and findings.

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

/** Chooses the document's printer profile, or none, labeled "Change printer". */
export async function setPrinter(app: App, profileId: string | null): Promise<void> {
  await commitEdits(app, 'Change printer', [
    profileId === null ? { type: 'set_printer' } : { type: 'set_printer', printer: profileId },
  ])
}

/** Hides or shows a check's findings; the choice persists in the settings. */
export function setCheckHidden(app: App, check: string, hidden: boolean): void {
  const others = app.store.getState().settings.hiddenChecks.filter((name) => name !== check)
  updateSettings(app, { hiddenChecks: hidden ? [...others, check] : others })
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

function centerOf(bounds: BoundingBox): Vector3 {
  return [
    (bounds.min[0] + bounds.max[0]) / 2,
    (bounds.min[1] + bounds.max[1]) / 2,
    (bounds.min[2] + bounds.max[2]) / 2,
  ]
}

/** Shows a finding: marks it and frames a cube around its location (3.17). */
export function focusFinding(app: App, finding: FindingKey): void {
  app.store.setState((state) => ({ print: { ...state.print, focusedFinding: finding } }))
  const state = app.store.getState()
  const located = state.print.report?.bodies
    .find((body) => body.body === finding.body)
    ?.report.checks.find((check) => check.check === finding.check)?.findings[finding.index]
  const viewport = app.viewport()
  if (located === undefined || viewport === null) {
    return
  }
  const location = located.location
  const center: Vector3 =
    location.kind === 'point' ? location.at : centerOf({ min: location.min, max: location.max })
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

/**
 * Runs the analysis on its own in Print context: 300 ms after the document revision last changed
 * with no transaction open, and on entering Print when the report is stale or missing (3.17).
 * Returns the function that stops it.
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

  const unsubscribe = app.store.subscribe((state, previous) => {
    const document = state.document
    if (state.context !== 'print' || document === null) {
      cancelTimer()
      return
    }
    if (previous.context !== 'print') {
      cancelTimer()
      const stale = state.print.report === null || state.print.reportRevision !== document.revision
      if (stale && !document.transactionOpen) {
        run()
      }
      return
    }
    if (document.revision === previous.document?.revision) {
      return
    }
    cancelTimer()
    if (!document.transactionOpen) {
      timer = setTimeout(() => {
        timer = null
        run()
      }, ANALYSIS_DELAY_MILLISECONDS)
    }
  })
  return () => {
    cancelTimer()
    unsubscribe()
  }
}
