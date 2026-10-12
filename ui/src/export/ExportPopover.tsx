import { clsx } from 'clsx/lite'
import { useId, useState, type RefObject } from 'react'
import { exportModel, type ExportRequest } from '../app/actions/export'
import { useApp, useAppState } from '../app/AppProvider'
import { Button } from '../components/Button'
import { Popover } from '../components/Popover'
import { SegmentedControl, type SegmentedOption } from '../components/SegmentedControl'
import type { AppState } from '../state/store'
import { COLOR_CLASSES } from '../theme/roles'

// The export popover (ui.md 3.18): a format, which bodies, and "Export". It is open only while
// shown, so a closed popover leaves nothing in the page.

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
                    disabled ? COLOR_CLASSES.textDisabled.foreground : COLOR_CLASSES.text.foreground,
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
