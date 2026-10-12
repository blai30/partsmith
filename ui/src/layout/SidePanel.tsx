import { clsx } from 'clsx/lite'
import { useId } from 'react'
import { focusFinding, runPrintAnalysis } from '../app/actions/print'
import { useApp, useAppState } from '../app/AppProvider'
import { ParametersPanel } from '../parameters/ParametersPanel'
import { hasPrintableBody, healthRows, missingReport } from '../print/health'
import { HealthList } from '../print/HealthList'
import { healthProfileName } from '../print/printers'
import { ProblemsPanel } from '../problems/ProblemsPanel'
import { COLOR_CLASSES } from '../theme/roles'

// The right side panel (ui.md 3.2): Parameters, Print health or Problems, one at a time. Below the
// lg breakpoint it opens as an overlay above the viewport.

function HealthPanel() {
  const app = useApp()
  const titleId = useId()
  const rows = useAppState(healthRows)
  const profileName = useAppState(healthProfileName)
  const hasGeometry = useAppState(hasPrintableBody)
  const missing = useAppState(missingReport)
  const focused = useAppState((state) => state.print.focusedFinding)
  return (
    <section aria-labelledby={titleId} className="flex min-h-0 flex-1 flex-col">
      <h2 id={titleId} className="sr-only">
        Print health
      </h2>
      <HealthList
        profileName={profileName}
        rows={rows}
        hasGeometry={hasGeometry}
        missingReport={missing}
        focusedFinding={focused}
        onCheckAgain={() => {
          runPrintAnalysis(app).catch((error: unknown) => {
            console.error('The print check failed.', error)
          })
        }}
        onFocusFinding={(finding) => {
          focusFinding(app, finding)
        }}
        onClose={() => {
          app.store.setState((state) => ({ panels: { ...state.panels, side: null } }))
        }}
      />
    </section>
  )
}

export function SidePanel() {
  const side = useAppState((state) => state.panels.side)
  if (side === null) {
    return null
  }
  return (
    <aside
      data-testid="side-panel"
      aria-label={side === 'parameters' ? 'Parameters' : side === 'problems' ? 'Problems' : 'Print health'}
      className={clsx(
        'flex w-90 max-w-full shrink-0 flex-col border-l max-lg:absolute max-lg:inset-y-0 max-lg:right-0 max-lg:z-30 max-lg:shadow-lg',
        COLOR_CLASSES.surface.background,
        COLOR_CLASSES.border.border,
      )}
    >
      {side === 'parameters' ? <ParametersPanel /> : null}
      {side === 'problems' ? <ProblemsPanel /> : null}
      {side === 'health' ? <HealthPanel /> : null}
    </aside>
  )
}
