import { clsx } from 'clsx/lite'
import { LoaderCircle } from 'lucide-react'
import { useEffect, useRef, useState, type Ref } from 'react'
import { setUnits } from '../app/actions/document'
import { openPrintHealth, setPrinter } from '../app/actions/print'
import { useApp, useAppState } from '../app/AppProvider'
import { reportFailure } from '../app/report'
import { explainDisabled, type DisabledAvailability } from '../commands/registry'
import { Menu, type MenuEntry } from '../components/Menu'
import { Tooltip } from '../components/Tooltip'
import { DOCUMENT_UNITS, formatQuantity } from '../expression/format'
import { HealthBadge } from '../print/HealthBadge'
import { PrinterMenu } from '../print/PrinterMenu'
import { printerLabel } from '../print/printer'
import { selectHealthBadge, selectSketchDof } from '../state/selectors'
import type { AppState, CoreState } from '../state/store'
import { COLOR_CLASSES } from '../theme/roles'
import { useCommandViews } from './use-commands'

// The status bar (ui.md 3.21): core status, sketch DOF, grid spacing, rebuild time, units, printer
// and the print health badge.

/** How long a mutation is pending before the status bar says "Rebuilding". */
const REBUILDING_DELAY_MILLISECONDS = 300

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

/** The core's state while it is not idle: loading, rebuilding, still working (4.4) or stopped. */
function busyText(
  status: CoreState['status'],
  stillWorking: boolean,
  rebuilding: boolean,
): string | null {
  if (status === 'loading') {
    return 'Loading core'
  }
  if (stillWorking) {
    return 'Still working'
  }
  return rebuilding ? 'Rebuilding' : null
}

function CoreStatus() {
  const status = useAppState((state) => state.core.status)
  const stillWorking = useAppState((state) => state.core.stillWorking)
  const rebuilding = useRebuilding()
  if (status === 'failed') {
    return (
      <span data-testid="status-core" className={COLOR_CLASSES.errorText.foreground}>
        Core stopped
      </span>
    )
  }
  const text = busyText(status, stillWorking, rebuilding)
  if (text === null) {
    return null
  }
  return (
    <span data-testid="status-core" className="flex items-center gap-1.5">
      <Spinner />
      {text}
    </span>
  )
}

function SketchDof() {
  const dof = useAppState(selectSketchDof)
  if (dof === null || dof.state === 'picking') {
    return null
  }
  // "DOF" is explained where it is shown, in plain words (3.16.8).
  return (
    <Tooltip label="Degrees of freedom: how many ways the sketch can still move.">
      <span
        data-testid="status-dof"
        data-dof={dof.dof}
        data-state={dof.state}
        className="tabular-nums"
      >
        {dof.dof} DOF
      </span>
    </Tooltip>
  )
}

/** "Grid 10 mm": the drawn grid's minor spacing in the document unit, while a grid is drawn (3.21). */
function GridSpacing() {
  const text = useAppState((state) => {
    const spacing = state.view.gridSpacing
    return spacing === null
      ? null
      : formatQuantity(spacing, 'length', state.document?.recipe.units ?? 'mm')
  })
  if (text === null) {
    return null
  }
  return (
    <span data-testid="status-grid" className="whitespace-nowrap tabular-nums">
      Grid {text}
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
      <span data-testid="status-recompute" className="whitespace-nowrap tabular-nums">
        Rebuilt in {time.value} {time.unit}
      </span>
    </Tooltip>
  )
}

function StatusButton({
  testId,
  label,
  expanded,
  buttonRef,
  disabled = false,
  onClick,
  ...rest
}: {
  readonly testId: string
  readonly label: string
  readonly expanded: boolean
  readonly buttonRef: Ref<HTMLButtonElement>
  /** Shown with `aria-disabled`, so it stays focusable and a click can say why (3.3). */
  readonly disabled?: boolean
  readonly onClick: () => void
  /** Set by a wrapping `Tooltip`. */
  readonly 'aria-describedby'?: string | undefined
}) {
  return (
    <button
      ref={buttonRef}
      type="button"
      data-testid={testId}
      aria-haspopup="menu"
      aria-expanded={expanded}
      aria-disabled={disabled ? true : undefined}
      aria-describedby={rest['aria-describedby']}
      onClick={onClick}
      className={clsx(
        'inline-flex h-6 shrink-0 items-center rounded-md px-2 text-xs whitespace-nowrap transition-colors duration-150 motion-reduce:transition-none',
        COLOR_CLASSES.focusRing.inset,
        COLOR_CLASSES.accent.outline,
        disabled
          ? clsx(COLOR_CLASSES.textDisabled.foreground, 'cursor-not-allowed')
          : clsx(COLOR_CLASSES.text.foreground, COLOR_CLASSES.hover.background),
      )}
    >
      {label}
    </button>
  )
}

/**
 * Why a status bar control is disabled, from the command it stands for, or null while it is
 * enabled: units and printer changes are edits, which wait for the active command (3.7).
 */
function useDisabled(commandId: string): DisabledAvailability | null {
  const app = useApp()
  const [command] = useCommandViews(app, [commandId])
  return command?.availability.state === 'disabled' ? command.availability : null
}

function UnitsButton() {
  const app = useApp()
  const units = useAppState((state) => state.document?.recipe.units ?? null)
  // Every unit command shares one availability rule.
  const disabled = useDisabled('document.units.mm')
  const reason = disabled?.reason ?? null
  const anchor = useRef<HTMLButtonElement>(null)
  const [open, setOpen] = useState(false)
  if (units === null) {
    return null
  }
  const entries: MenuEntry[] = DOCUMENT_UNITS.map((unit) => ({
    kind: 'radio',
    label: unit,
    checked: unit === units,
    onSelect: () => {
      if (unit !== units) {
        setUnits(app, unit).catch(reportFailure('Changing the units'))
      }
    },
  }))
  return (
    <>
      <Tooltip label={reason === null ? 'Units' : `Units: ${reason}`}>
        <StatusButton
          testId="status-units"
          label={units}
          expanded={open}
          buttonRef={anchor}
          disabled={reason !== null}
          onClick={() => {
            if (disabled === null) {
              setOpen((current) => !current)
            } else {
              explainDisabled(app, disabled)
            }
          }}
        />
      </Tooltip>
      <Menu label="Units" open={open} anchor={anchor} entries={entries} onOpenChange={setOpen} />
    </>
  )
}

function selectPrinterLabel(state: AppState): string {
  return printerLabel(state.printers, state.document?.recipe.printer)
}

/** The printer button (3.21): the profile in use, opening the printer menu (3.17). */
function PrinterButton() {
  const app = useApp()
  const label = useAppState(selectPrinterLabel)
  const printers = useAppState((state) => state.printers)
  const current = useAppState((state) => state.document?.recipe.printer ?? null)
  const disabled = useDisabled('print.printer')
  const reason = disabled?.reason ?? null
  const anchor = useRef<HTMLButtonElement>(null)
  const [open, setOpen] = useState(false)
  return (
    <>
      <Tooltip label={reason === null ? 'Choose printer' : `Choose printer: ${reason}`}>
        <StatusButton
          testId="status-printer"
          label={label}
          expanded={open}
          buttonRef={anchor}
          disabled={reason !== null}
          onClick={() => {
            if (disabled === null) {
              setOpen((value) => !value)
            } else {
              explainDisabled(app, disabled)
            }
          }}
        />
      </Tooltip>
      <PrinterMenu
        open={open}
        anchor={anchor}
        onOpenChange={setOpen}
        printers={printers}
        current={current}
        onChoose={(profileId) => {
          setPrinter(app, profileId).catch(reportFailure('Changing the printer'))
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
        openPrintHealth(app).catch(reportFailure('Checking the print'))
      }}
    />
  )
}

export function StatusBar() {
  return (
    <footer
      data-testid="status-bar"
      // A one-row island: the menus and tooltips of its controls keep clear of it (3.2).
      data-bar=""
      className={clsx(
        'relative z-10 col-span-3 row-start-4 mt-2 flex h-7 min-w-0 items-center gap-3 overflow-x-auto rounded-lg border px-2 text-xs shadow-md',
        COLOR_CLASSES.surface.background,
        COLOR_CLASSES.forcedEdge.border,
        COLOR_CLASSES.elevation.shadow,
        COLOR_CLASSES.textMuted.foreground,
      )}
    >
      <CoreStatus />
      <SketchDof />
      <GridSpacing />
      <RebuildTime />
      <div className="ml-auto flex shrink-0 items-center gap-1">
        <UnitsButton />
        <PrinterButton />
        <Health />
      </div>
    </footer>
  )
}
