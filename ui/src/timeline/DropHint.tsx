import { clsx } from 'clsx/lite'
import type { RefObject } from 'react'
import { useAppState } from '../app/AppProvider'
import { MESSAGES } from '../messages'
import { selectFeatureLabel } from '../state/selectors'
import { COLOR_CLASSES } from '../theme/roles'
import type { DropWarning } from './uses'

type DropHintProps = {
  readonly ref: RefObject<HTMLDivElement | null>
  readonly warning: DropWarning | null
}

/**
 * The drop warning's chip above the insertion bar while a feature is dragged (ui.md 3.9): a live
 * region from the drag's start, so a warning is announced when it appears, and transparent while
 * empty.
 */
export function DropHint({ ref, warning }: DropHintProps) {
  const text = useAppState((state) => {
    if (warning === null) {
      return ''
    }
    const user = selectFeatureLabel(state, warning.user)
    const feature = selectFeatureLabel(state, warning.feature)
    return MESSAGES.drop_breaks_use(
      user,
      warning.what,
      feature,
      warning.moved === warning.user ? 'user' : 'feature',
    )
  })
  return (
    <div
      ref={ref}
      data-testid="drop-hint"
      // ui.md 3.9 names the status role of this chip.
      // oxlint-disable-next-line jsx-a11y/prefer-tag-over-role
      role="status"
      className={clsx(
        'pointer-events-none absolute bottom-full left-0 mb-2 rounded-lg border px-2 py-1 text-xs whitespace-nowrap shadow-lg',
        COLOR_CLASSES.surface.background,
        COLOR_CLASSES.forcedEdge.border,
        COLOR_CLASSES.elevation.shadow,
        COLOR_CLASSES.text.foreground,
        text === '' && 'opacity-0',
      )}
    >
      {text}
    </div>
  )
}
