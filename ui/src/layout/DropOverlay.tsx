import { clsx } from 'clsx/lite'
import { COLOR_CLASSES } from '../theme/roles'

/** The overlay while a file is dragged over the window: "Drop a .ptsm file to open it" (3.19). */
export function DropOverlay() {
  return (
    <div className="pointer-events-none fixed inset-0 z-50 flex items-center justify-center p-8">
      <div
        data-testid="drop-overlay"
        className={clsx(
          'rounded-xl border-2 border-dashed px-8 py-6 text-base font-semibold shadow-lg',
          COLOR_CLASSES.surfaceRaised.background,
          COLOR_CLASSES.accent.border,
          COLOR_CLASSES.elevation.shadow,
          COLOR_CLASSES.text.foreground,
        )}
      >
        Drop a .ptsm file to open it
      </div>
    </div>
  )
}
