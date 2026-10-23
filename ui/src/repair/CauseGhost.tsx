import { useEffect } from 'react'
import { useApp, useAppState } from '../app/AppProvider'
import { MESSAGES } from '../messages'
import { GHOST_DASHES, GHOST_STROKE_CLASSES } from './ghost-style'
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
        strokeDasharray={GHOST_DASHES}
        className={GHOST_STROKE_CLASSES}
      >
        <title>{MESSAGES.cause_ghost()}</title>
      </path>
    </svg>
  )
}
