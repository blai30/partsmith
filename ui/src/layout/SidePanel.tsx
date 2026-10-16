import { clsx } from 'clsx/lite'
import { useEffect, useId, useRef } from 'react'
import { setContext } from '../app/actions/context'
import { closeSidePanel } from '../app/actions/panels'
import {
  focusFinding,
  runPrintAnalysis,
  setPrintOrientation,
  startLayOnFace,
} from '../app/actions/print'
import { useApp, useAppState } from '../app/AppProvider'
import { reportFailure } from '../app/report'
import { KeysPanel } from '../keys/KeysPanel'
import { ParametersPanel } from '../parameters/ParametersPanel'
import { hasPrintableBody, healthRows, missingReport } from '../print/health'
import { HealthList } from '../print/HealthList'
import { selectOrientationBody } from '../print/orientation'
import { healthProfileName } from '../print/printer'
import { ProblemsPanel } from '../problems/ProblemsPanel'
import type { PanelsState } from '../state/store'
import { COLOR_CLASSES } from '../theme/roles'
import { focusLost, focusView } from './focus'
import { SIDE_PANEL_ID } from './panel-ids'
import { useOverlayDismiss } from './use-overlay-dismiss'

// The right side panel (ui.md 3.2): Parameters, Print check, Problems or Keyboard and mouse, one at
// a time. In a window narrower than 65rem it floats over the free area as an overlay island.

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
          runPrintAnalysis(app).catch(reportFailure('The print check'))
        }}
        onFocusFinding={(finding) => {
          focusFinding(app, finding)
        }}
        onClose={() => {
          closeSidePanel(app)
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
            .catch(reportFailure('Starting Lay on face'))
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
    closeSidePanel(app)
  })
  // Closing the panel from inside it gives focus back to the control that opened it, or to the
  // 3D view when that control is gone (3.24).
  useEffect(() => {
    const active = document.activeElement
    const opener = active instanceof HTMLElement && active !== document.body ? active : null
    return () => {
      if (!focusLost()) {
        return
      }
      if (opener?.isConnected === true) {
        opener.focus()
      } else {
        focusView()
      }
    }
  }, [])
  return (
    <aside
      ref={panel}
      id={SIDE_PANEL_ID}
      data-testid="side-panel"
      aria-label={SIDE_PANEL_NAMES[side]}
      // A frame island (3.2), docked in the last column from 65rem and floating over the free
      // area's cell below it, a step raised over the cards there; each panel scrolls its own body.
      className={clsx(
        'relative z-10 col-start-3 row-start-2 ml-2 flex min-h-0 w-90 max-w-full flex-col overflow-hidden rounded-xl border shadow-md @max-5xl/app:absolute @max-5xl/app:inset-y-0 @max-5xl/app:right-0 @max-5xl/app:z-20 @max-5xl/app:col-start-2 @max-5xl/app:col-end-3 @max-5xl/app:row-end-3 @max-5xl/app:shadow-lg',
        COLOR_CLASSES.surface.background,
        COLOR_CLASSES.surfaceRaised.overlay,
        COLOR_CLASSES.forcedEdge.border,
        COLOR_CLASSES.elevation.shadow,
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
