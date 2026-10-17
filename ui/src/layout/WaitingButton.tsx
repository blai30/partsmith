import type { ReactNode } from 'react'
import { refusedDuringCommand } from '../app/actions/edits'
import { useApp } from '../app/AppProvider'
import { Button } from '../components/Button'
import { Tooltip } from '../components/Tooltip'

/**
 * A button whose action can wait during a feature command (3.7): while `blocked` names why, it is
 * `aria-disabled` with the reason as tooltip, and a click runs nothing, saying why when `refuses`.
 */
export function WaitingButton({
  blocked,
  refuses,
  variant = 'default',
  onRun,
  children,
}: {
  /** Why the action waits now, or null when it can run. */
  readonly blocked: string | null
  /** Whether a click while it waits shows `finish_command_first`. */
  readonly refuses: boolean
  readonly variant?: 'default' | 'banner'
  readonly onRun: () => void
  readonly children: ReactNode
}) {
  const app = useApp()
  return (
    <Tooltip label={blocked}>
      <Button
        variant={variant}
        aria-disabled={blocked === null ? undefined : true}
        onClick={() => {
          if (blocked === null) {
            onRun()
          } else if (refuses) {
            refusedDuringCommand(app)
          }
        }}
      >
        {children}
      </Button>
    </Tooltip>
  )
}
