import { clsx } from 'clsx/lite'
import { COLOR_CLASSES } from '../theme/roles'

// How ghosts are drawn on the viewport overlay (ui.md 3.7, 4.2): the repair ghosts and centroid
// rings of repair mode and the cause ghost share one dashed `errorMark` stroke.

/** The dash pattern of a ghost's stroke, in overlay pixels. */
export const GHOST_DASHES = '4 3'

/** The classes of a ghost's stroke: unfilled, two pixels wide, in `errorMark`. */
export const GHOST_STROKE_CLASSES = clsx('fill-none stroke-2', COLOR_CLASSES.errorMark.stroke)
