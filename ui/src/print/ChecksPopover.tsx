import type { RefObject } from 'react'
import { Popover } from '../components/Popover'
import { Switch } from '../components/Switch'
import { ALWAYS_SHOWN_CHECK, PRINT_CHECKS } from './checks'

export type ChecksPopoverProps = {
  readonly open: boolean
  /** The toolbar's "Checks" button the popover is anchored to. */
  readonly anchor: RefObject<HTMLElement | null>
  readonly onOpenChange: (open: boolean) => void
  /** Checks whose findings are hidden (`settings.hiddenChecks`). */
  readonly hiddenChecks: readonly string[]
  /** Hides or shows a check's findings (`setCheckHidden`). */
  readonly onHiddenChange: (check: string, hidden: boolean) => void
}

/**
 * The Checks popover (`checks-popover`, ui.md 3.17): one switch per check. The core runs every
 * check; a switched-off check only hides its findings from the badge, the list and, for
 * Overhangs, the color map. Mesh validity is always on.
 */
export function ChecksPopover({
  open,
  anchor,
  onOpenChange,
  hiddenChecks,
  onHiddenChange,
}: ChecksPopoverProps) {
  return (
    <Popover label="Checks" open={open} anchor={anchor} onOpenChange={onOpenChange}>
      <div data-testid="checks-popover" className="flex w-64 flex-col gap-1">
        {PRINT_CHECKS.map(({ check, title }) =>
          check === ALWAYS_SHOWN_CHECK ? (
            <Switch
              key={check}
              label={title}
              checked
              disabled
              description="Always checked before export"
              onChange={() => undefined}
            />
          ) : (
            <Switch
              key={check}
              label={title}
              checked={!hiddenChecks.includes(check)}
              onChange={(checked) => {
                onHiddenChange(check, !checked)
              }}
            />
          ),
        )}
      </div>
    </Popover>
  )
}
