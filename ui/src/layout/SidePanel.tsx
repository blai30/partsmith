import { clsx } from 'clsx/lite'
import { useId, useRef } from 'react'
import { setContext } from '../app/actions/context'
import {
  focusFinding,
  runPrintAnalysis,
  setPrintOrientation,
  startLayOnFace,
} from '../app/actions/print'
import { useApp, useAppState } from '../app/AppProvider'
import { KeysPanel } from '../keys/KeysPanel'
import { ParametersPanel } from '../parameters/ParametersPanel'
import { hasPrintableBody, healthRows, missingReport } from '../print/health'
import { HealthList } from '../print/HealthList'
import { selectOrientationBody } from '../print/orientation'
import { healthProfileName } from '../print/printer'
import { ProblemsPanel } from '../problems/ProblemsPanel'
import type { PanelsState } from '../state/store'
import { COLOR_CLASSES } from '../theme/roles'
import { SIDE_PANEL_ID } from './panel-ids'
import { useOverlayDismiss } from './use-overlay-dismiss'

// The right side panel (ui.md 3.2): Parameters, Print check, Problems or Keyboard and mouse, one at
// a time. Below the lg breakpoint it opens as an overlay above the viewport.

/** The print check panel (3.17), a region named by its visible heading "Print check: {profile name}". */
function PrintCheckPanel() {
  const app = useApp()
  const headingId = useId()
  const rows = useAppState(healthRows)
  const profileName = useAppState(healthProfileName)
  const hasGeometry = useAppState(hasPrintableBody)
  const missing = useAppState(missingReport)
  const focused = useAppState((state) => state.print.focusedFinding)
  const orientationBody = useAppState(selectOrientationBody)
  return (
    <section aria-labelledby={headingId} className="flex min-h-0 flex-1 flex-col">
      <HealthList
        headingId={headingId}
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
        orientationBody={orientationBody}
        onPrintAsModeled={() => {
          setPrintOrientation(app, null)
        }}
        onLayOnFace={() => {
          // Lay on face is a Print tool: the panel also shows in Model, which switches first.
          setContext(app, 'print')
            .then(() => {
              startLayOnFace(app)
            })
            .catch((error: unknown) => {
              console.error('Starting Lay on face failed.', error)
            })
        }}
      />
    </section>
  )
}

/** The accessible name of the side panel's landmark for each panel. */
const SIDE_PANEL_NAMES: Readonly<Record<NonNullable<PanelsState['side']>, string>> = {
  parameters: 'Parameters',
  problems: 'Problems',
  health: 'Print check',
  keys: 'Keyboard and mouse',
}

function OpenSidePanel({ side }: { readonly side: NonNullable<PanelsState['side']> }) {
  const app = useApp()
  const panel = useRef<HTMLElement>(null)
  useOverlayDismiss(panel, () => {
    app.store.setState((state) => ({ panels: { ...state.panels, side: null } }))
  })
  return (
    <aside
      ref={panel}
      id={SIDE_PANEL_ID}
      data-testid="side-panel"
      aria-label={SIDE_PANEL_NAMES[side]}
      className={clsx(
        'col-start-3 row-start-2 flex min-h-0 w-90 max-w-full flex-col border-l max-lg:absolute max-lg:inset-y-0 max-lg:right-0 max-lg:z-30 max-lg:shadow-lg',
        COLOR_CLASSES.surface.background,
        COLOR_CLASSES.border.border,
      )}
    >
      {side === 'parameters' ? <ParametersPanel /> : null}
      {side === 'problems' ? <ProblemsPanel /> : null}
      {side === 'health' ? <PrintCheckPanel /> : null}
      {side === 'keys' ? <KeysPanel /> : null}
    </aside>
  )
}

export function SidePanel() {
  const side = useAppState((state) => state.panels.side)
  return side === null ? null : <OpenSidePanel side={side} />
}
