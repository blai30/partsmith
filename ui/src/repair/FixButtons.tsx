import { copyDetails, runFixAction } from '../app/actions/fixes'
import { refusedDuringCommand } from '../app/actions/edits'
import { useApp, useAppState } from '../app/AppProvider'
import { reportFailure } from '../app/report'
import { Button } from '../components/Button'
import { Tooltip } from '../components/Tooltip'
import type { Cause, Diagnostic, FixAction } from '../generated/protocol'
import { selectFixBlock, type FixPlace } from '../state/selectors'

// The buttons of a message's and a cause line's `FixAction`s (ui.md 4.1, 4.2), and Copy details.

export type FixButtonProps = {
  readonly action: FixAction
  /** The cause line the action belongs to, whose ghost a `show` draws; absent for a message action. */
  readonly cause?: Cause
  readonly place: FixPlace
  /** Called once the action has run, for the focus rule of the banner (3.7). */
  readonly onRan?: (action: FixAction) => void
}

/** One action button; while it waits it is `aria-disabled` with the reason as tooltip, and a click on a panel's says why (3.7). */
export function FixButton({ action, cause, place, onRan }: FixButtonProps) {
  const app = useApp()
  const blocked = useAppState((state) => selectFixBlock(state, action, place))
  return (
    <Tooltip label={blocked}>
      <Button
        variant={place === 'banner' ? 'banner' : 'default'}
        aria-disabled={blocked === null ? undefined : true}
        onClick={() => {
          if (blocked !== null) {
            if (place !== 'banner') {
              refusedDuringCommand(app)
            }
            return
          }
          runFixAction(app, action, place, cause)
            .then(() => onRan?.(action))
            .catch(reportFailure(`Running "${action.label}"`))
        }}
      >
        {action.label}
      </Button>
    </Tooltip>
  )
}

/** The buttons of actions, in order; `finish_transaction` is never shown (4.2). */
export function FixButtons({
  actions,
  cause,
  place,
  onRan,
}: Omit<FixButtonProps, 'action'> & { readonly actions: readonly FixAction[] }) {
  const shown = actions.filter((action) => action.run.kind !== 'finish_transaction')
  return shown.map((action) => (
    <FixButton
      key={action.label}
      action={action}
      place={place}
      {...(cause === undefined ? {} : { cause })}
      {...(onRan === undefined ? {} : { onRan })}
    />
  ))
}

/** "Copy details": the code, the message, the cause and the detail on the clipboard (4.1). */
export function CopyDetailsButton({
  message,
  place,
}: {
  readonly message: Diagnostic
  readonly place: FixPlace
}) {
  const app = useApp()
  return (
    <Button
      variant={place === 'banner' ? 'banner' : 'default'}
      onClick={() => {
        copyDetails(app, message).catch(reportFailure('Copying the details'))
      }}
    >
      Copy details
    </Button>
  )
}
