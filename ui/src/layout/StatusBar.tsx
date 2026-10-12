import { clsx } from 'clsx/lite'
import { LoaderCircle } from 'lucide-react'
import { useEffect, useRef, useState, type Ref } from 'react'
import { setUnits } from '../app/actions/document'
import { openPrintHealth, setPrinter } from '../app/actions/print'
import { useApp, useAppState } from '../app/AppProvider'
import { Menu, type MenuEntry } from '../components/Menu'
import { Tooltip } from '../components/Tooltip'
import type { Units } from '../generated/protocol'
import { HealthBadge } from '../print/HealthBadge'
import { PrinterMenu } from '../print/PrinterMenu'
import { printerName } from '../print/printers'
import { selectHealthBadge, selectSketchDof } from '../state/selectors'
import type { AppState } from '../state/store'
import { COLOR_CLASSES, FONT_CLASSES } from '../theme/roles'

// The status bar (ui.md 3.21): core status, sketch DOF, rebuild time, units, printer and the print
// health badge.

/** How long a mutation is pending before the status bar says "Rebuilding". */
const REBUILDING_DELAY_MILLISECONDS = 300

const UNITS: readonly Units[] = ['mm', 'cm', 'm', 'in']

function report(action: string): (error: unknown) => void {
  return (error) => {
    console.error(`${action} failed.`, error)
  }
}

function Spinner() {
  return (
    <LoaderCircle
      aria-hidden="true"
      strokeWidth={1.75}
      className="size-3.5 shrink-0 animate-spin motion-reduce:animate-none"
    />
  )
}

/** Whether a mutation has been pending for more than 300 ms. */
function useRebuilding(): boolean {
  const pending = useAppState((state) => state.core.pendingMutations > 0)
  const [late, setLate] = useState(false)
  useEffect(() => {
    if (!pending) {
      return undefined
    }
    const timer = setTimeout(() => {
      setLate(true)
    }, REBUILDING_DELAY_MILLISECONDS)
    return () => {
      clearTimeout(timer)
      setLate(false)
    }
  }, [pending])
  return pending && late
}

function CoreStatus() {
  const status = useAppState((state) => state.core.status)
  const rebuilding = useRebuilding()
  if (status === 'failed') {
    return (
      <span data-testid="status-core" className={COLOR_CLASSES.errorText.foreground}>
        Core stopped
      </span>
    )
  }
  if (status === 'loading' || rebuilding) {
    return (
      <span data-testid="status-core" className="flex items-center gap-1.5">
        <Spinner />
        {status === 'loading' ? 'Loading core' : 'Rebuilding'}
      </span>
    )
  }
  return null
}

function SketchDof() {
  const dof = useAppState(selectSketchDof)
  if (dof === null || dof.state === 'picking') {
    return null
  }
  return (
    <span
      data-testid="status-dof"
      data-dof={dof.dof}
      data-state={dof.state}
      className="tabular-nums"
    >
      <span className={FONT_CLASSES.values}>{dof.dof}</span> DOF
    </span>
  )
}

/** "12" ms below a second, "1.2" s from a second on (3.21). */
function rebuildTime(milliseconds: number): { readonly value: string; readonly unit: string } {
  return milliseconds < 1000
    ? { value: String(Math.round(milliseconds)), unit: 'ms' }
    : { value: (milliseconds / 1000).toFixed(1), unit: 's' }
}

function RebuildTime() {
  const milliseconds = useAppState((state) => state.document?.lastRebuildMilliseconds ?? null)
  if (milliseconds === null) {
    return null
  }
  const time = rebuildTime(milliseconds)
  return (
    <Tooltip label="Time the last change took to rebuild">
      <span data-testid="status-recompute" className="whitespace-nowrap">
        Rebuilt in <span className={FONT_CLASSES.values}>{time.value}</span> {time.unit}
      </span>
    </Tooltip>
  )
}

function StatusButton({
  testId,
  label,
  expanded,
  buttonRef,
  onClick,
}: {
  readonly testId: string
  readonly label: string
  readonly expanded: boolean
  readonly buttonRef: Ref<HTMLButtonElement>
  readonly onClick: () => void
}) {
  return (
    <button
      ref={buttonRef}
      type="button"
      data-testid={testId}
      aria-haspopup="menu"
      aria-expanded={expanded}
      onClick={onClick}
      className={clsx(
        'inline-flex h-6 shrink-0 items-center rounded-sm px-2 text-xs whitespace-nowrap transition-colors duration-150 motion-reduce:transition-none',
        COLOR_CLASSES.text.foreground,
        COLOR_CLASSES.hover.background,
        COLOR_CLASSES.focusRing.outline,
        COLOR_CLASSES.accent.outline,
      )}
    >
      {label}
    </button>
  )
}

function UnitsButton() {
  const app = useApp()
  const units = useAppState((state) => state.document?.recipe.units ?? null)
  const anchor = useRef<HTMLButtonElement>(null)
  const [open, setOpen] = useState(false)
  if (units === null) {
    return null
  }
  const entries: MenuEntry[] = UNITS.map((unit) => ({
    kind: 'radio',
    label: unit,
    checked: unit === units,
    onSelect: () => {
      if (unit !== units) {
        setUnits(app, unit).catch(report('Changing the units'))
      }
    },
  }))
  return (
    <>
      <StatusButton
        testId="status-units"
        label={units}
        expanded={open}
        buttonRef={anchor}
        onClick={() => {
          setOpen((current) => !current)
        }}
      />
      <Menu label="Units" open={open} anchor={anchor} entries={entries} onOpenChange={setOpen} />
    </>
  )
}

function selectPrinterLabel(state: AppState): string {
  const printer = state.document?.recipe.printer ?? null
  return printer === null ? 'No printer' : printerName(state.printers, printer)
}

function PrinterButton() {
  const app = useApp()
  const label = useAppState(selectPrinterLabel)
  const printers = useAppState((state) => state.printers)
  const current = useAppState((state) => state.document?.recipe.printer ?? null)
  const anchor = useRef<HTMLButtonElement>(null)
  const [open, setOpen] = useState(false)
  return (
    <>
      <StatusButton
        testId="status-printer"
        label={label}
        expanded={open}
        buttonRef={anchor}
        onClick={() => {
          setOpen((value) => !value)
        }}
      />
      <PrinterMenu
        open={open}
        anchor={anchor}
        onOpenChange={setOpen}
        printers={printers}
        current={current}
        onChoose={(profileId) => {
          setPrinter(app, profileId).catch(report('Changing the printer'))
        }}
      />
    </>
  )
}

function Health() {
  const app = useApp()
  const badge = useAppState(selectHealthBadge)
  return (
    <HealthBadge
      badge={badge}
      onClick={() => {
        openPrintHealth(app).catch(report('Checking the print'))
      }}
    />
  )
}

export function StatusBar() {
  return (
    <footer
      data-testid="status-bar"
      className={clsx(
        'flex h-7 shrink-0 items-center gap-3 overflow-x-auto border-t px-2 text-xs',
        COLOR_CLASSES.surface.background,
        COLOR_CLASSES.border.border,
        COLOR_CLASSES.textMuted.foreground,
      )}
    >
      <CoreStatus />
      <SketchDof />
      <RebuildTime />
      <div className="ml-auto flex shrink-0 items-center gap-1">
        <UnitsButton />
        <PrinterButton />
        <Health />
      </div>
    </footer>
  )
}
