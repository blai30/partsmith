// UI-owned messages (ui.md 4.3), each a function of its placeholders. Core messages are shown verbatim
// and only wrapped by these templates; file names appear as given, without quotation marks.

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
  nothing_to_export: () => 'There is no visible body with geometry to export.',
  exported: (file: string) => `Exported ${file}.`,
  deleted: (label: string) => `Deleted ${label}.`,
  feature_failed: (label: string, message: string) => `${label} failed. ${message}`,
  features_failed: (count: number) => `${String(count)} features failed after this change.`,
  over_constrained: (title: string, list: string) =>
    `${title} would over-constrain the sketch: it conflicts with ${list}.`,
  redundant_constraint: (title: string) =>
    `${title} is already implied by other constraints, so it was not added.`,
  dimension_over_constrained: () =>
    'This dimension would over-constrain the sketch. Add it as a reference dimension that only measures?',
  dimension_unsolvable: () =>
    'The sketch cannot be solved with this value. Check for dimensions that cannot all be true at once.',
  cannot_dimension: (first: string, second: string) =>
    `Partsmith cannot dimension ${first} and ${second}.`,
  arc_collinear: () =>
    'These three points are in a line. Move the last point off the line to make an arc.',
  profile_problems: (label: string, count: number) =>
    `${label} has ${String(count)} profile problems, so some regions may not extrude.`,
  units_changed: (unit: string) =>
    `Units are now ${unit}. Numbers typed without a unit now mean ${unit}.`,
  pick_plane: () => 'Pick a flat face, a datum plane or an origin plane.',
  pick_axis: () => 'Pick a straight edge, a sketch line, a round face or an origin axis.',
  pick_kind: (pluralNoun: string) => `Pick ${pluralNoun}.`,
  pick_same_body: (kind: string, body: string) => `Pick ${kind}s of '${body}', or clear Body first.`,
  parameter_name: () => 'Names start with a letter or _ and use only letters, digits and _.',
  field_required: (label: string) => `${label} needs a value.`,
  field_not_number: (label: string) => `${label} must be a number.`,
  look_at_disabled: () => 'Select a flat face or a plane first.',
  nothing_to_undo: () => 'Nothing to undo.',
  nothing_to_redo: () => 'Nothing to redo.',
  finish_command_first: (title: string) => `Finish or cancel ${title} first.`,
  import_not_editable: () => 'Imported geometry has no settings to edit.',
  fill_fields: () => 'Fill in the highlighted fields first.',
  core_not_running: () => 'The geometry core is not running.',
  region_crashed: () => 'This part of the window hit a problem. Reload to restore it.',
} as const

/** The id of a UI-owned message, such as 'not_ptsm'. */
export type MessageId = keyof typeof MESSAGES
