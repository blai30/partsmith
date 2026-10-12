import { clsx } from 'clsx/lite'
import { ArrowDownToLine } from 'lucide-react'
import { useId, useState, type ReactNode, type RefObject } from 'react'
import { exportModel, type ExportRequest } from '../app/actions/export'
import { openPrintHealth, setPrintOrientation } from '../app/actions/print'
import { useApp, useAppState } from '../app/AppProvider'
import { Button } from '../components/Button'
import { Popover } from '../components/Popover'
import { SegmentedControl, type SegmentedOption } from '../components/SegmentedControl'
import { HealthBadgeIcon } from '../print/HealthBadge'
import { selectOrientationBody } from '../print/orientation'
import { SeverityIcon } from '../print/SeverityIcon'
import { selectHealthBadge } from '../state/selectors'
import type { AppState } from '../state/store'
import { COLOR_CLASSES } from '../theme/roles'
import { exportFailedText, exportStatus, type ExportStatusLine } from './status'

// The export popover (ui.md 3.18): a format, which bodies, what the export is about to write, and
// "Export". It is open only while shown, so a closed popover leaves nothing in the page.

const FORMATS: readonly SegmentedOption<ExportRequest['format']>[] = [
  { value: '3mf', label: '3MF' },
  { value: 'stl', label: 'STL' },
  { value: 'obj', label: 'OBJ' },
  { value: 'step', label: 'STEP' },
]

const SCOPES: readonly { readonly value: ExportRequest['scope']; readonly label: string }[] = [
  { value: 'visible', label: 'All visible bodies' },
  { value: 'selected', label: 'Selected bodies' },
]

function selectHasSelectedBody(state: AppState): boolean {
  return state.selection.some((item) => item.kind === 'body')
}

/** One status line (`export-status`): an icon, the text and the action that line offers. */
function StatusLine({
  icon,
  text,
  children,
}: {
  readonly icon: ReactNode
  readonly text: string
  readonly children?: ReactNode
}) {
  return (
    <div data-testid="export-status" className="flex items-center gap-2 text-xs">
      {icon}
      <span className="min-w-0 flex-1">{text}</span>
      {children}
    </div>
  )
}

/** The status lines of 3.18 for the chosen format, with their actions. */
function StatusLines({
  format,
  onClose,
}: {
  readonly format: ExportRequest['format']
  readonly onClose: () => void
}) {
  const app = useApp()
  // The lines derive from these; each is a stable value, so the popover re-renders only for them.
  useAppState(exportFailedText)
  useAppState(selectHealthBadge)
  useAppState(selectOrientationBody)
  const lines = exportStatus(app.store.getState(), format)
  if (lines.length === 0) {
    return null
  }
  const lineFor = (line: ExportStatusLine): ReactNode => {
    switch (line.kind) {
      case 'failed':
        return (
          <StatusLine key={line.kind} icon={<SeverityIcon severity="warning" />} text={line.text}>
            <Button
              onClick={() => {
                onClose()
                app.store.setState((state) => ({ panels: { ...state.panels, side: 'problems' } }))
              }}
            >
              Show problems
            </Button>
          </StatusLine>
        )
      case 'print_check':
        return (
          <StatusLine
            key={line.kind}
            icon={<HealthBadgeIcon state={line.badge.state} />}
            text={line.text}
          >
            {line.checkOffered ? (
              <Button
                onClick={() => {
                  onClose()
                  openPrintHealth(app).catch((error: unknown) => {
                    console.error('The print check failed.', error)
                  })
                }}
              >
                Check print
              </Button>
            ) : null}
          </StatusLine>
        )
      case 'orientation':
        return (
          <StatusLine
            key={line.kind}
            icon={
              <ArrowDownToLine
                aria-hidden="true"
                strokeWidth={1.75}
                className={clsx('size-4 shrink-0', COLOR_CLASSES.textMuted.foreground)}
              />
            }
            text={line.text}
          >
            <Button
              onClick={() => {
                setPrintOrientation(app, null)
              }}
            >
              Print as modeled
            </Button>
          </StatusLine>
        )
    }
  }
  return <div className="flex flex-col gap-2">{lines.map(lineFor)}</div>
}

export type ExportPopoverProps = {
  /** The Export button the popover opens under and returns focus to. */
  readonly anchor: RefObject<HTMLElement | null>
  readonly onClose: () => void
}

export function ExportPopover({ anchor, onClose }: ExportPopoverProps) {
  const app = useApp()
  const scopeName = useId()
  const hasSelectedBody = useAppState(selectHasSelectedBody)
  const [format, setFormat] = useState<ExportRequest['format']>('3mf')
  const [chosenScope, setScope] = useState<ExportRequest['scope']>('visible')
  // Selected bodies need a selected body; without one the export covers the visible bodies.
  const scope = hasSelectedBody ? chosenScope : 'visible'

  return (
    <Popover
      label="Export"
      open
      anchor={anchor}
      onOpenChange={(open) => {
        if (!open) {
          onClose()
        }
      }}
    >
      <div data-testid="export-popover" className="flex w-72 flex-col gap-4">
        <div className="flex flex-col gap-2">
          <span className={clsx('text-xs font-medium', COLOR_CLASSES.textMuted.foreground)}>
            Format
          </span>
          <SegmentedControl label="Format" options={FORMATS} value={format} onChange={setFormat} />
        </div>
        <div className="flex flex-col gap-2">
          <span className={clsx('text-xs font-medium', COLOR_CLASSES.textMuted.foreground)}>
            Bodies
          </span>
          <div role="radiogroup" aria-label="Bodies" className="flex flex-col gap-1">
            {SCOPES.map((option) => {
              const disabled = option.value === 'selected' && !hasSelectedBody
              return (
                <label
                  key={option.value}
                  className={clsx(
                    'flex h-8 items-center gap-2 text-sm',
                    disabled
                      ? COLOR_CLASSES.textDisabled.foreground
                      : COLOR_CLASSES.text.foreground,
                  )}
                >
                  <input
                    type="radio"
                    name={scopeName}
                    value={option.value}
                    checked={scope === option.value}
                    disabled={disabled}
                    onChange={() => {
                      setScope(option.value)
                    }}
                    className={clsx(
                      'm-0 size-4 shrink-0 appearance-none rounded-full',
                      COLOR_CLASSES.field.background,
                      COLOR_CLASSES.focusRing.outline,
                      COLOR_CLASSES.accent.outline,
                      scope === option.value
                        ? clsx('border-4', COLOR_CLASSES.accent.border)
                        : clsx('border', COLOR_CLASSES.borderControl.border),
                      disabled && 'cursor-not-allowed opacity-50',
                    )}
                  />
                  {option.label}
                </label>
              )
            })}
          </div>
        </div>
        <StatusLines format={format} onClose={onClose} />
        <div className="flex justify-end">
          <Button
            variant="primary"
            onClick={() => {
              onClose()
              exportModel(app, { format, scope }).catch((error: unknown) => {
                console.error('The export failed.', error)
              })
            }}
          >
            Export
          </Button>
        </div>
      </div>
    </Popover>
  )
}
