import { describe, expect, it } from 'vitest'
import type { Platform } from './app/createApp'
import { MESSAGES } from './messages'

type Case = readonly [id: string, render: () => string, text: string]

const LINUX: Platform = { operatingSystem: 'linux', host: 'web' }
const MAC: Platform = { operatingSystem: 'mac', host: 'web' }

// Every message of ui.md 4.3, rendered with sample placeholders in the order they appear in the text.
const CASES: readonly Case[] = [
  [
    'unsupported_webgl2',
    () => MESSAGES.unsupported_webgl2(),
    'Partsmith needs WebGL2 to draw the 3D view, and this browser does not provide it. Turn on hardware acceleration in your browser settings, or use a current version of Chrome, Edge, Firefox or Safari.',
  ],
  [
    'unsupported_wasm',
    () => MESSAGES.unsupported_wasm(),
    'Partsmith needs WebAssembly, and this browser does not provide it. Use a current version of Chrome, Edge, Firefox or Safari.',
  ],
  [
    'core_load_failed',
    () => MESSAGES.core_load_failed('HTTP 404'),
    'The geometry core did not load: HTTP 404. Check your connection and reload the page.',
  ],
  [
    'core_stopped',
    () => MESSAGES.core_stopped('out of memory'),
    'The geometry core stopped: out of memory. Restart it to keep working; your latest changes are kept.',
  ],
  [
    'not_ptsm',
    () => MESSAGES.not_ptsm('model.stl'),
    'model.stl is not a Partsmith model. Partsmith opens .ptsm files.',
  ],
  ['not_text', () => MESSAGES.not_text('photo.ptsm'), 'photo.ptsm could not be read as text.'],
  [
    'open_failed',
    () => MESSAGES.open_failed('bracket.ptsm', 'The file is not valid JSON: line 3, column 7.'),
    'Could not open bracket.ptsm. The file is not valid JSON: line 3, column 7.',
  ],
  [
    'migrated',
    () => MESSAGES.migrated('bracket.ptsm', 1),
    'bracket.ptsm was updated from format version 1. Saving writes the current format.',
  ],
  [
    'save_permission',
    () => MESSAGES.save_permission('bracket.ptsm'),
    'Could not save bracket.ptsm because Partsmith no longer has permission to write it.',
  ],
  [
    'save_not_found',
    () => MESSAGES.save_not_found('bracket.ptsm'),
    'bracket.ptsm was moved or deleted. Save it again to choose a location.',
  ],
  [
    'save_io',
    () => MESSAGES.save_io('bracket.ptsm', 'The disk is full.'),
    'Could not save bracket.ptsm: The disk is full.',
  ],
  [
    'download_hint',
    () => MESSAGES.download_hint('bracket.ptsm'),
    'This browser saves by downloading. bracket.ptsm is in your downloads folder.',
  ],
  [
    'recent_permission',
    () => MESSAGES.recent_permission('bracket.ptsm'),
    'Partsmith needs permission to open bracket.ptsm. Choose it again to grant access.',
  ],
  [
    'recent_missing',
    () => MESSAGES.recent_missing('bracket.ptsm'),
    'bracket.ptsm has moved or been deleted, so it was removed from recent files.',
  ],
  [
    'stashed',
    () => MESSAGES.stashed('Untitled'),
    'Untitled had unsaved changes. They are kept for you.',
  ],
  [
    'recovered',
    () => MESSAGES.recovered('bracket', '14:32'),
    'Partsmith kept unsaved changes to bracket from 14:32.',
  ],
  [
    'nothing_to_export',
    () => MESSAGES.nothing_to_export(),
    'There is no visible body with geometry to export.',
  ],
  ['exported', () => MESSAGES.exported('cube.3mf'), 'Exported cube.3mf.'],
  [
    'exported_with_failures',
    () => MESSAGES.exported_with_failures('cube.3mf', MESSAGES.export_failed_feature('Fillet 1')),
    'Exported cube.3mf. Fillet 1 failed, so it is not in the export.',
  ],
  [
    'export_failed_feature',
    () => MESSAGES.export_failed_feature('Fillet 1'),
    'Fillet 1 failed, so it is not in the export.',
  ],
  [
    'export_failed_features',
    () => MESSAGES.export_failed_features(2),
    '2 features failed, so they are not in the export.',
  ],
  ['deleted', () => MESSAGES.deleted('Fillet 1'), 'Deleted Fillet 1.'],
  [
    'feature_failed',
    () => MESSAGES.feature_failed('Fillet 1', 'The radius 15 mm is too large for these edges.'),
    'Fillet 1 failed. The radius 15 mm is too large for these edges.',
  ],
  ['features_failed', () => MESSAGES.features_failed(3), '3 features failed after this change.'],
  [
    'over_constrained',
    () => MESSAGES.over_constrained('Horizontal', 'Vertical and Distance 40'),
    'Horizontal conflicts with Vertical and Distance 40, so it was not added.',
  ],
  [
    'redundant_constraint',
    () => MESSAGES.redundant_constraint('Parallel'),
    'Parallel is already implied by other constraints, so it was not added.',
  ],
  [
    'dimension_over_constrained',
    () => MESSAGES.dimension_over_constrained(),
    'Other dimensions and constraints already set this size. Add it as a reference dimension that only shows the measurement?',
  ],
  [
    'dimension_unsolvable',
    () => MESSAGES.dimension_unsolvable(),
    'The sketch cannot be solved with this value. Check for dimensions that cannot all be true at once.',
  ],
  [
    'cannot_dimension',
    () => MESSAGES.cannot_dimension('a point', 'a circle'),
    'Partsmith cannot dimension a point and a circle. Pick two points, a point and a line, two lines, or one circle or arc.',
  ],
  [
    'arc_collinear',
    () => MESSAGES.arc_collinear(),
    'These three points are in a line. Move the last point off the line to make an arc.',
  ],
  [
    'profile_problems',
    () => MESSAGES.profile_problems('Sketch 1', '2 open ends'),
    'Sketch 1 has 2 open ends, so some shapes cannot be extruded.',
  ],
  [
    'units_changed',
    () => MESSAGES.units_changed('in'),
    'Units are now in. Numbers typed without a unit now mean in.',
  ],
  [
    'pick_plane',
    () => MESSAGES.pick_plane(),
    'Pick a flat face, a datum plane or an origin plane.',
  ],
  [
    'pick_axis',
    () => MESSAGES.pick_axis(),
    'Pick a straight edge, a sketch line, a round face or an origin axis.',
  ],
  ['pick_kind', () => MESSAGES.pick_kind('edges'), 'Pick edges.'],
  ['pick_points', () => MESSAGES.pick_points(), 'Click a flat face or pick sketch points.'],
  ['pick_flat_face', () => MESSAGES.pick_flat_face(), 'Pick a flat face.'],
  [
    'hole_one_face',
    () => MESSAGES.hole_one_face(),
    'One Hole puts its holes on one face. Press Enter to finish this one, then start another.',
  ],
  [
    'pick_same_body',
    () => MESSAGES.pick_same_body('edges', 'Cube'),
    "All edges must be on 'Cube'. Remove them to pick edges of another body.",
  ],
  [
    'parameter_name',
    () => MESSAGES.parameter_name(),
    'Names start with a letter or _ and use only letters, digits and _.',
  ],
  ['field_required', () => MESSAGES.field_required('Distance'), 'Distance needs a value.'],
  ['field_not_number', () => MESSAGES.field_not_number('Count'), 'Count must be a number.'],
  ['look_at_disabled', () => MESSAGES.look_at_disabled(), 'Select a flat face or a plane first.'],
  ['nothing_to_undo', () => MESSAGES.nothing_to_undo(), 'Nothing to undo.'],
  ['nothing_to_redo', () => MESSAGES.nothing_to_redo(), 'Nothing to redo.'],
  ['nothing_to_repeat', () => MESSAGES.nothing_to_repeat(), 'No command to repeat yet.'],
  [
    'finish_command_first',
    () => MESSAGES.finish_command_first('Extrude'),
    'Finish or cancel Extrude first.',
  ],
  [
    'import_not_editable',
    () => MESSAGES.import_not_editable(),
    'Imported geometry has no settings to edit.',
  ],
  ['fill_fields', () => MESSAGES.fill_fields(), 'Fill in the highlighted fields first.'],
  ['core_not_running', () => MESSAGES.core_not_running(), 'The geometry core is not running.'],
  ['command_canceled', () => MESSAGES.command_canceled('Fillet'), 'Fillet was canceled.'],
  [
    'needs_sketch_region',
    () => MESSAGES.needs_sketch_region(),
    'Draw a closed shape in a sketch first.',
  ],
  [
    'needs_solid',
    () => MESSAGES.needs_solid(),
    'Make a solid first, for example with Extrude or Box.',
  ],
  [
    'needs_two_bodies',
    () => MESSAGES.needs_two_bodies(),
    'Boolean needs two bodies with geometry. Make a second body first, for example with Box.',
  ],
  [
    'sketch_dof_help',
    () => MESSAGES.sketch_dof_help(LINUX),
    'Blue geometry can still move or change size. Add dimensions (D) or constraints where the size matters; you can extrude at any time.',
  ],
  ['plane_pick_hint', () => MESSAGES.plane_pick_hint(), 'Top plane (XY) is the print bed.'],
  [
    'sketch_step_start',
    () => MESSAGES.sketch_step_start(LINUX),
    'Choose Line (L), Rectangle (R) or Circle (C) to start drawing.',
  ],
  ['sketch_step_line_start', () => MESSAGES.sketch_step_line_start(), 'Click to start a line.'],
  [
    'sketch_step_line_next',
    () => MESSAGES.sketch_step_line_next(LINUX),
    'Click to end the segment, or type a length. Click the first point to close the shape, or press Enter to stop.',
  ],
  ['sketch_step_corner', () => MESSAGES.sketch_step_corner(), 'Click the first corner.'],
  [
    'sketch_step_opposite_corner',
    () => MESSAGES.sketch_step_opposite_corner(),
    'Click the opposite corner, or type the width.',
  ],
  ['sketch_step_center', () => MESSAGES.sketch_step_center(), 'Click the center.'],
  [
    'sketch_step_rectangle_corner',
    () => MESSAGES.sketch_step_rectangle_corner(),
    'Click a corner, or type the width.',
  ],
  [
    'sketch_step_circle_size',
    () => MESSAGES.sketch_step_circle_size(),
    'Click to set the size, or type the diameter.',
  ],
  ['sketch_step_arc_start', () => MESSAGES.sketch_step_arc_start(), 'Click where the arc starts.'],
  ['sketch_step_arc_end', () => MESSAGES.sketch_step_arc_end(), 'Click where it ends.'],
  [
    'sketch_step_arc_shape',
    () => MESSAGES.sketch_step_arc_shape(),
    'Move to shape the arc, then click, or type the radius.',
  ],
  ['sketch_step_point', () => MESSAGES.sketch_step_point(), 'Click to place a point.'],
  [
    'sketch_step_dimension',
    () => MESSAGES.sketch_step_dimension(),
    'Click a line, circle, arc or point.',
  ],
  [
    'sketch_step_dimension_place',
    () => MESSAGES.sketch_step_dimension_place(),
    'Click to place the dimension, or click a second point or line to measure between them.',
  ],
  [
    'sketch_step_dimension_second',
    () => MESSAGES.sketch_step_dimension_second(),
    'Click a second point or a line.',
  ],
  [
    'sketch_step_dimension_label',
    () => MESSAGES.sketch_step_dimension_label(),
    'Click to place the dimension.',
  ],
  [
    'sketch_free_placement',
    () => MESSAGES.sketch_free_placement(LINUX),
    'Hold Ctrl to place freely.',
  ],
  [
    'sketch_leave',
    () => MESSAGES.sketch_leave(LINUX),
    'To leave the sketch, click Finish sketch or press Ctrl+Enter.',
  ],
  ['print_pick_face', () => MESSAGES.print_pick_face(), 'Pick the face that should touch the bed.'],
  [
    'orientation_set',
    () => MESSAGES.orientation_set('Cube'),
    'Printing with the picked face of Cube on the bed.',
  ],
  [
    'region_crashed',
    () => MESSAGES.region_crashed(),
    'This part of the window hit a problem. Reload to restore it.',
  ],
]

describe('MESSAGES', () => {
  it.each(CASES)('U-73 %s renders exactly its text', (_id, render, text) => {
    expect(render()).toBe(text)
  })

  it('U-73 has an entry for every message id of 4.3', () => {
    expect(Object.keys(MESSAGES)).toEqual(expect.arrayContaining(CASES.map(([id]) => id)))
    expect(CASES).toHaveLength(77)
  })

  it('renders shortcut placeholders and the free placement key for macOS', () => {
    expect(MESSAGES.sketch_step_start(MAC)).toBe(
      'Choose Line (L), Rectangle (R) or Circle (C) to start drawing.',
    )
    expect(MESSAGES.sketch_step_line_next(MAC)).toMatch(/press ↩ to stop\.$/)
    expect(MESSAGES.sketch_leave(MAC)).toBe('To leave the sketch, click Finish sketch or press ⌘↩.')
    expect(MESSAGES.sketch_free_placement(MAC)).toBe('Hold ⌘ to place freely.')
  })
})
