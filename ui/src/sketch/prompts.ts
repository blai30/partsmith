import type { Platform } from '../app/createApp'
import { formatShortcut } from '../commands/shortcuts'

// The sketch prompts of ui.md 4.3 (`sketch_step_*`, `sketch_free_placement`, `sketch_leave`,
// `plane_pick_hint`, `sketch_dof_help`), keyed by their message ids. Shortcut placeholders are
// rendered with `formatShortcut` for the platform.

export const SKETCH_PROMPTS = {
  sketch_step_start: (platform: Platform) =>
    `Choose Line (${formatShortcut('L', platform)}), Rectangle (${formatShortcut('R', platform)}) or Circle (${formatShortcut('C', platform)}) to start drawing.`,
  sketch_step_line_start: () => 'Click to start a line.',
  sketch_step_line_next: (platform: Platform) =>
    `Click to end the segment, or type a length. Click the first point to close the shape, or press ${formatShortcut('Enter', platform)} to stop.`,
  sketch_step_corner: () => 'Click the first corner.',
  sketch_step_opposite_corner: () => 'Click the opposite corner, or type the width.',
  sketch_step_center: () => 'Click the center.',
  sketch_step_rectangle_corner: () => 'Click a corner, or type the width.',
  sketch_step_circle_size: () => 'Click to set the size, or type the diameter.',
  sketch_step_arc_start: () => 'Click where the arc starts.',
  sketch_step_arc_end: () => 'Click where it ends.',
  sketch_step_arc_shape: () => 'Move to shape the arc, then click, or type the radius.',
  sketch_step_point: () => 'Click to place a point.',
  sketch_step_dimension: () => 'Click a line, circle, arc or point.',
  sketch_step_dimension_place: () =>
    'Click to place the dimension, or click a second point or line to measure between them.',
  sketch_step_dimension_second: () => 'Click a second point or a line.',
  sketch_step_dimension_label: () => 'Click to place the dimension.',
  sketch_free_placement: (platform: Platform) =>
    `Hold ${platform.operatingSystem === 'mac' ? '⌘' : 'Ctrl'} to place freely.`,
  sketch_leave: (platform: Platform) =>
    `To leave the sketch, click Finish sketch or press ${formatShortcut('Mod+Enter', platform)}.`,
  plane_pick_hint: () => 'Top plane (XY) is the print bed.',
  sketch_dof_help: (platform: Platform) =>
    `Blue geometry can still move or change size. Add dimensions (${formatShortcut('D', platform)}) or constraints where the size matters; you can extrude at any time.`,
} as const
