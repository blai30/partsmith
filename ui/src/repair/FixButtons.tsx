import { copyDetails, offeredActions, runOfferedFix } from '../app/actions/fixes'
import { useApp, useAppState } from '../app/AppProvider'
import { reportFailure } from '../app/report'
import { Button } from '../components/Button'
import type { Cause, Diagnostic, FixAction } from '../generated/protocol'
import { WaitingButton } from '../layout/WaitingButton'
import { selectFixBlock, type FixPlace } from '../state/selectors'

// The buttons of a message's and a cause line's `FixAction`s (ui.md 4.1, 4.2), and Copy details.

type FixButtonProps = {
  readonly action: FixAction
  /** The cause line the action belongs to, whose ghost a `show` draws; absent for a message action. */
  readonly cause?: Cause | undefined
  readonly place: FixPlace
  /** Called once the action has run, for the focus rule of the banner (3.7). */
  readonly onRan?: (() => void) | undefined
}

/** One action button; while it waits it is `aria-disabled` with the reason as tooltip, and a click on a panel's says why (3.7). */
function FixButton({ action, cause, place, onRan }: FixButtonProps) {
  const app = useApp()
  const blocked = useAppState((state) => selectFixBlock(state, action, place))
  return (
    <WaitingButton
      blocked={blocked}
      refuses={place !== 'banner'}
      variant={place === 'banner' ? 'banner' : 'default'}
      onRun={() => {
        runOfferedFix(app, action, place, { cause, onRan })
      }}
    >
      {action.label}
    </WaitingButton>
  )
}

/** The buttons of actions, in order; `finish_transaction` is never shown (4.2). */
export function FixButtons({
  actions,
  cause,
  place,
  onRan,
}: Omit<FixButtonProps, 'action'> & { readonly actions: readonly FixAction[] }) {
  return offeredActions(actions).map((action) => (
    <FixButton key={action.label} action={action} cause={cause} place={place} onRan={onRan} />
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
