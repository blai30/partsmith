import { clsx } from 'clsx/lite'
import type { Cause, FixAction } from '../generated/protocol'
import { COLOR_CLASSES } from '../theme/roles'
import type { FixPlace } from '../state/selectors'
import { FixButtons } from './FixButtons'

// The cause line of a message (ui.md 4.1, 4.3): the edit that started the problem, in `textMuted`,
// followed by its own actions (reverts, Undo, Show).

export function CauseLine({
  cause,
  place,
  onRan,
}: {
  readonly cause: Cause
  readonly place: FixPlace
  readonly onRan?: (action: FixAction) => void
}) {
  return (
    <>
      <p data-testid="cause-line" className={clsx('text-left', COLOR_CLASSES.textMuted.foreground)}>
        {cause.text}
      </p>
      {cause.actions.length === 0 ? null : (
        <div className="flex flex-wrap gap-2">
          <FixButtons
            actions={cause.actions}
            cause={cause}
            place={place}
            {...(onRan === undefined ? {} : { onRan })}
          />
        </div>
      )}
    </>
  )
}
