import type { Platform } from './app/createApp'
import { formatShortcut } from './commands/shortcuts'

// UI-owned messages (ui.md 4.3), each a function of its placeholders. Core messages are shown verbatim
// and only wrapped by these templates; file names appear as given, without quotation marks. A
// shortcut placeholder is rendered with `formatShortcut` for the platform.

const BROWSERS = 'Chrome, Edge, Firefox or Safari'

export const MESSAGES = {
  unsupported_webgl2: () =>
    `Partsmith needs WebGL2 to draw the 3D view, and this browser does not provide it. Turn on hardware acceleration in your browser settings, or use a current version of ${BROWSERS}.`,
  unsupported_wasm: () =>
    `Partsmith needs WebAssembly, and this browser does not provide it. Use a current version of ${BROWSERS}.`,
  core_load_failed: (reason: string) =>
    `The geometry core did not load: ${reason}. Check your connection and reload the page.`,
  core_stopped: (reason: string) =>
    `The geometry core stopped: ${reason}. Restart it to keep working; your latest changes are kept.`,
  core_still_working: (seconds: number) =>
    `The geometry core has been working for more than ${String(seconds)} seconds, and Partsmith cannot tell whether it will finish. Stopping it reopens your document as it was after the last change the core finished, and closes any open feature command without keeping it.`,
  core_still_working_desktop: (seconds: number) =>
    `The geometry core has been working for more than ${String(seconds)} seconds, and Partsmith cannot tell whether it will finish. The desktop app cannot stop it yet: wait, or quit Partsmith, which keeps your changes up to the last one the core finished, except those of an open feature command.`,
  changes_not_applied: () =>
    'Your latest changes were not applied, because the geometry core was stopped before it finished them. Make them again if you still want them.',
  save_stopped: (file: string) =>
    `${file} was not saved, because the geometry core stopped before saving it. Save it again.`,
  export_stopped: (file: string) =>
    `${file} was not exported, because the geometry core stopped before exporting it. Export it again.`,
  open_stopped: (file: string) =>
    `${file} was not opened, because the geometry core stopped before opening it. Open it again.`,
  recovery_open_stopped: (name: string) =>
    `The unsaved changes to ${name} were not opened, because the geometry core stopped before opening them. They are still kept, and Partsmith offers them again the next time it starts.`,
  new_stopped: () =>
    'The new document was not opened, because the geometry core stopped before opening it. Choose New again.',
  not_ptsm: (file: string) => `${file} is not a Partsmith model. Partsmith opens .ptsm files.`,
  not_text: (file: string) => `${file} could not be read as text.`,
  open_failed: (file: string, coreMessage: string) => `Could not open ${file}. ${coreMessage}`,
  migrated: (file: string, formatVersion: number) =>
    `${file} was updated from format version ${String(formatVersion)}. Saving writes the current format.`,
  save_permission: (file: string) =>
    `Could not save ${file} because Partsmith no longer has permission to write it.`,
  save_not_found: (file: string) =>
    `${file} was moved or deleted. Save it again to choose a location.`,
  save_io: (file: string, reason: string) => `Could not save ${file}: ${reason}`,
  download_hint: (file: string) =>
    `This browser saves by downloading. ${file} is in your downloads folder.`,
  recent_permission: (file: string) =>
    `Partsmith needs permission to open ${file}. Choose it again to grant access.`,
  recent_missing: (file: string) =>
    `${file} has moved or been deleted, so it was removed from recent files.`,
  stashed: (name: string) => `${name} had unsaved changes. They are kept for you.`,
  recovered: (name: string, time: string) =>
    `Partsmith kept unsaved changes to ${name} from ${time}.`,
  /** `count` is the number of other entries still waiting to be offered (3.19). */
  recovered_more: (name: string, time: string, count: number) =>
    `Partsmith kept unsaved changes to ${name} from ${time}, and to ${String(count)} other ${count === 1 ? 'document' : 'documents'}.`,
  nothing_to_export: () => 'There is no visible body with geometry to export.',
  exported: (file: string) => `Exported ${file}.`,
  /** `failed` is the text of `export_failed_feature` or `export_failed_features`. */
  exported_with_failures: (file: string, failed: string) => `Exported ${file}. ${failed}`,
  export_failed_feature: (label: string) => `${label} failed, so it is not in the export.`,
  export_failed_features: (count: number) =>
    `${String(count)} features failed, so they are not in the export.`,
  deleted: (label: string) => `Deleted ${label}.`,
  feature_failed: (label: string, message: string) => `${label} failed. ${message}`,
  features_failed: (count: number) => `${String(count)} features failed after this change.`,
  over_constrained: (title: string, list: string) =>
    `${title} conflicts with ${list}, so it was not added.`,
  redundant_constraint: (title: string) =>
    `${title} is already implied by other constraints, so it was not added.`,
  dimension_over_constrained: () =>
    'Other dimensions and constraints already set this size. Add it as a reference dimension that only shows the measurement?',
  dimension_unsolvable: () =>
    'The sketch cannot be solved with this value. Check for dimensions that cannot all be true at once.',
  cannot_dimension: (first: string, second: string) =>
    `Partsmith cannot dimension ${first} and ${second}. Pick two points, a point and a line, two lines, or one circle or arc.`,
  arc_collinear: () =>
    'These three points are in a line. Move the last point off the line to make an arc.',
  /** `outline` is `outlineProblems` of the sketch's solution, such as "2 open ends" (2.14). */
  profile_problems: (label: string, outline: string) =>
    `${label} has ${outline}, so some shapes cannot be extruded.`,
  sketch_dof_help: (platform: Platform) =>
    `Blue geometry can still move or change size. Add dimensions (${formatShortcut('D', platform)}) or constraints where the size matters; you can extrude at any time.`,
  units_changed: (unit: string) =>
    `Units are now ${unit}. Numbers typed without a unit now mean ${unit}.`,
  pick_plane: () => 'Pick a flat face, a datum plane or an origin plane.',
  pick_axis: () => 'Pick a straight edge, a sketch line, a round face or an origin axis.',
  pick_kind: (pluralNoun: string) => `Pick ${pluralNoun}.`,
  pick_points: () => 'Click a flat face or pick sketch points.',
  /** `pluralNoun` is the field's kind in the plural, such as "edges". */
  pick_same_body: (pluralNoun: string, body: string) =>
    `All ${pluralNoun} must be on '${body}'. Remove them to pick ${pluralNoun} of another body.`,
  pick_flat_face: () => 'Pick a flat face.',
  hole_one_face: () =>
    'One Hole puts its holes on one face. Press Enter to finish this one, then start another.',
  parameter_name: () => 'Names start with a letter or _ and use only letters, digits and _.',
  field_required: (label: string) => `${label} needs a value.`,
  field_not_number: (label: string) => `${label} must be a number.`,
  look_at_disabled: () => 'Select a flat face or a plane first.',
  nothing_to_undo: () => 'Nothing to undo.',
  nothing_to_redo: () => 'Nothing to redo.',
  finish_command_first: (title: string) => `Finish or cancel ${title} first.`,
  needs_sketch_region: () => 'Draw a closed shape in a sketch first.',
  needs_solid: () => 'Make a solid first, for example with Extrude or Box.',
  needs_two_bodies: () =>
    'Boolean needs two bodies with geometry. Make a second body first, for example with Box.',
  nothing_to_repeat: () => 'No command to repeat yet.',
  import_not_editable: () => 'Imported geometry has no settings to edit.',
  fill_fields: () => 'Fill in the highlighted fields first.',
  core_not_running: () => 'The geometry core is not running.',
  command_canceled: (title: string) => `${title} was canceled.`,
  plane_pick_hint: () => 'Top plane (XY) is the print bed.',
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
  /** The key that suppresses inference: "Ctrl" on Windows and Linux, "⌘" on macOS (3.16.5). */
  sketch_free_placement: (platform: Platform) =>
    `Hold ${platform.operatingSystem === 'mac' ? '⌘' : 'Ctrl'} to place freely.`,
  sketch_leave: (platform: Platform) =>
    `To leave the sketch, click Finish sketch or press ${formatShortcut('Mod+Enter', platform)}.`,
  print_pick_face: () => 'Pick the face that should touch the bed.',
  orientation_set: (body: string) => `Printing with the picked face of ${body} on the bed.`,
  region_crashed: () => 'This part of the window hit a problem. Reload to restore it.',
  feature_lost: (label: string, description: string) => `'${label}' lost ${description}.`,
  features_lost: (label: string, description: string, failed: number) =>
    `'${label}' lost ${description}, so ${counted(failed, 'feature')} after it failed too.`,
  feature_check: (label: string) => `'${label}' needs a check after this change.`,
  show_lost: (label: string) => `Show what '${label}' lost`,
  check_feature: (label: string) => `Check '${label}'`,
  repair_missing: (description: string) => `${description} (missing)`,
  repair_check: (description: string) => `${description} (check it)`,
  cause_ghost: () => 'Before this change',
  reattach_prompt: (label: string) => `Pick a plane or a flat face for '${label}'`,
  attach_label: (label: string) => `Attach '${label}'`,
  preview_pending: () => 'Wait for the preview to update.',
  details_copied: () => 'Details copied.',
  details_not_copied: () =>
    'The details could not be copied, because the browser did not allow access to the clipboard.',
} as const

/** The id of a UI-owned message, such as 'not_ptsm'. */
export type MessageId = keyof typeof MESSAGES

/** "the top edge" as "The top edge", for the start of a line, a title or a label (4.3). */
export function capitalized(text: string): string {
  return text.charAt(0).toUpperCase() + text.slice(1)
}

/** "1 face", "3 faces", "2 bodies": a count with its noun, singular only for one. */
export function counted(count: number, singular: string, plural = `${singular}s`): string {
  return `${String(count)} ${count === 1 ? singular : plural}`
}
