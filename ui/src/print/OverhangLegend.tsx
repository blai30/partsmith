import { clsx } from 'clsx/lite'
import { formatQuantity } from '../expression/format'
import { COLOR_CLASSES, FONT_CLASSES } from '../theme/roles'

export type OverhangLegendProps = {
  /** The overhang limit in radians from vertical, where the ramp starts (`overhangLegendLimit`). */
  readonly limit: number
}

/** An angle in degrees with `formatNumber`; angles read the same in every document unit. */
function degrees(radians: number): string {
  return formatQuantity(radians, 'angle', 'mm')
}

/**
 * The legend of the overhang color map (`overhang-legend`, ui.md 3.17), a card at the bottom of
 * the free area's left column (3.14): the `analysis` ramp from the limit to 90 degrees and what its
 * colors mean. It is not interactive, so presses on it reach the viewport (orbit, pick) wherever it
 * is mounted.
 */
export function OverhangLegend({ limit }: OverhangLegendProps) {
  const limitText = degrees(limit)
  return (
    <div
      data-testid="overhang-legend"
      className={clsx(
        'pointer-events-none mt-auto flex w-56 flex-col gap-2 rounded-xl border p-3 text-xs shadow-md',
        COLOR_CLASSES.surfaceRaised.background,
        COLOR_CLASSES.border.border,
        COLOR_CLASSES.elevation.shadow,
        COLOR_CLASSES.text.foreground,
      )}
    >
      <p className="text-sm font-medium">Overhangs</p>
      <div className="flex flex-col gap-1">
        <div
          aria-hidden="true"
          className={clsx(
            'h-2 rounded-xs border',
            COLOR_CLASSES.analysis.background,
            COLOR_CLASSES.borderControl.border,
          )}
        />
        <div
          className={clsx(
            'flex justify-between tabular-nums',
            FONT_CLASSES.values,
            COLOR_CLASSES.textMuted.foreground,
          )}
        >
          <span>{limitText}</span>
          <span>{degrees(Math.PI / 2)}</span>
        </div>
      </div>
      <p className={COLOR_CLASSES.textMuted.foreground}>
        Faces steeper than {limitText} from vertical need support.
      </p>
    </div>
  )
}
