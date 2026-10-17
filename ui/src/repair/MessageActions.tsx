import type { ReactNode } from 'react'
import type { Diagnostic, FixAction } from '../generated/protocol'
import { CauseLine } from './CauseLine'
import type { FixPlace } from '../state/selectors'
import { CopyDetailsButton, FixButtons } from './FixButtons'

// What follows a message's text wherever it is shown (ui.md 4.1): its actions, its cause line with
// the cause's actions, and Copy details last when it has a `detail`.

type MessageActionsProps = {
  readonly message: Diagnostic
  readonly place: FixPlace
  /** Actions shown elsewhere, such as the candidates of repair mode (3.7), left out here. */
  readonly omitted?: ReadonlySet<FixAction>
  /** The UI's own fixes, after the message's actions (3.7: "Reverse and use {text}"). */
  readonly own?: ReactNode
  readonly onRan?: (() => void) | undefined
}

export function MessageActions({
  message,
  place,
  omitted,
  own = null,
  onRan,
}: MessageActionsProps) {
  const actions = (message.actions ?? []).filter((action) => omitted?.has(action) !== true)
  const cause = message.cause ?? null
  const detail = message.detail ?? null
  return (
    <>
      {actions.length === 0 && own === null ? null : (
        <div className="flex flex-wrap gap-2">
          <FixButtons actions={actions} place={place} onRan={onRan} />
          {own}
        </div>
      )}
      {cause === null ? null : <CauseLine cause={cause} place={place} onRan={onRan} />}
      {detail === null ? null : (
        <div className="flex flex-wrap gap-2">
          <CopyDetailsButton message={message} place={place} />
        </div>
      )}
    </>
  )
}
