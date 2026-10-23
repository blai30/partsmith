import { clsx } from 'clsx/lite'
import { useEffect } from 'react'
import { useApp, useAppState } from '../app/AppProvider'
import { MESSAGES } from '../messages'
import { COLOR_CLASSES } from '../theme/roles'
import { causeGhost } from './repair'

// The ghost of the cause line whose Show ran (ui.md 4.2): the changed sketch entity as it was
// before the edit, dashed in `errorMark` on the plane of its sketch, until the selection changes.
// `paint.ts` writes its path in each frame.

/** The shown cause's ghost on the viewport overlay, or nothing while no cause with a ghost is shown. */
export function CauseGhost() {
  const app = useApp()
  const shown = useAppState((state) => causeGhost(state) !== undefined)
  // A new ghost is drawn by the next frame.
  useEffect(() => {
    if (shown) {
      app.viewport()?.requestRender()
    }
  }, [app, shown])
  if (!shown) {
    return null
  }
  return (
    <svg aria-hidden="true" className="pointer-events-none absolute inset-0 size-full">
      <path
        data-testid="cause-ghost"
        strokeDasharray="4 3"
        className={clsx('fill-none stroke-2', COLOR_CLASSES.errorMark.stroke)}
      >
        <title>{MESSAGES.cause_ghost()}</title>
      </path>
    </svg>
  )
}
