import { clsx } from 'clsx/lite'
import { Fragment } from 'react'
import type { WorldAxis } from '../core/labels'
import { AXIS_ROLES, COLOR_CLASSES, FONT_CLASSES } from '../theme/roles'

// Axis letters and coordinate readouts (ui.md 3.7, 3.8): X, Y and Z in the axis roles' colors
// (2.16.3), always as letters, so color is never the only signal (3.24).

const AXES: readonly WorldAxis[] = ['X', 'Y', 'Z']

export type AxisLetterProps = {
  readonly axis: WorldAxis
  /** False when the letter names an axis of a plane's own frame rather than a world axis: then it is muted (3.7). */
  readonly world?: boolean
  /** Hides the letter from assistive technology where a label already names the axis (fields, 3.7). */
  readonly decorative?: boolean
  /** Layout classes of the place it sits in. */
  readonly className?: string
}

/** An axis letter, semibold in the interface font, in its axis role's foreground. */
export function AxisLetter({ axis, world = true, decorative = false, className }: AxisLetterProps) {
  return (
    <span
      data-testid="axis-letter"
      data-axis={axis}
      data-world={String(world)}
      aria-hidden={decorative ? true : undefined}
      className={clsx(
        'font-semibold',
        world ? COLOR_CLASSES[AXIS_ROLES[axis]].foreground : COLOR_CLASSES.textMuted.foreground,
        className,
      )}
    >
      {axis}
    </span>
  )
}

export type AxisValuesProps = {
  readonly testId: string
  /** The X, Y and Z values, formatted. */
  readonly values: readonly [string, string, string]
  readonly unit: string
  /** The plain text of the readout (`formatSize` or `formatPoint`), kept in `data-value`. */
  readonly text: string
}

/**
 * A coordinate readout (3.8): three equal cells of a letter and its value, then the unit. The
 * cells grow from a zero basis, so readouts of the same width line up letter over letter, and wrap
 * rather than shrink below their content. Space text nodes separate the parts, so the readout
 * reads "X 40 Y 20 Z 10 mm".
 */
export function AxisValues({ testId, values, unit, text }: AxisValuesProps) {
  return (
    <span
      data-testid={testId}
      data-value={text}
      className="flex w-full flex-wrap items-baseline gap-x-2 gap-y-1"
    >
      {AXES.map((axis, index) => (
        <Fragment key={axis}>
          <span className="flex flex-1 basis-0 items-baseline gap-1">
            <AxisLetter axis={axis} />{' '}
            <span className={clsx('tabular-nums', FONT_CLASSES.values)}>{values[index]}</span>
          </span>{' '}
        </Fragment>
      ))}
      <span className="shrink-0">{unit}</span>
    </span>
  )
}
