import { describe, expect, it } from 'vitest'
import type { Platform } from './app/createApp'
import { capitalized, counted, MESSAGES } from './messages'

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
    'core_still_working',
    () => MESSAGES.core_still_working(10),
    'The geometry core has been working for more than 10 seconds, and Partsmith cannot tell whether it will finish. Stopping it reopens your document as it was after the last change the core finished, and closes any open feature command without keeping it.',
  ],
  [
    'core_still_working_desktop',
    () => MESSAGES.core_still_working_desktop(10),
    'The geometry core has been working for more than 10 seconds, and Partsmith cannot tell whether it will finish. The desktop app cannot stop it yet: wait, or quit Partsmith, which keeps your changes up to the last one the core finished, except those of an open feature command.',
  ],
  [
    'changes_not_applied',
    () => MESSAGES.changes_not_applied(),
    'Your latest changes were not applied, because the geometry core was stopped before it finished them. Make them again if you still want them.',
  ],
  [
    'save_stopped',
    () => MESSAGES.save_stopped('bracket.ptsm'),
    'bracket.ptsm was not saved, because the geometry core stopped before saving it. Save it again.',
  ],
  [
    'export_stopped',
    () => MESSAGES.export_stopped('bracket.stl'),
    'bracket.stl was not exported, because the geometry core stopped before exporting it. Export it again.',
  ],
  [
    'open_stopped',
    () => MESSAGES.open_stopped('bracket.ptsm'),
    'bracket.ptsm was not opened, because the geometry core stopped before opening it. Open it again.',
  ],
  [
    'recovery_open_stopped',
    () => MESSAGES.recovery_open_stopped('bracket'),
    'The unsaved changes to bracket were not opened, because the geometry core stopped before opening them. They are still kept, and Partsmith offers them again the next time it starts.',
  ],
  [
    'new_stopped',
    () => MESSAGES.new_stopped(),
    'The new document was not opened, because the geometry core stopped before opening it. Choose New again.',
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
    'printer_offer_add',
    () => MESSAGES.printer_offer_add('bracket.ptsm', 'My MK4'),
    "bracket.ptsm uses the printer 'My MK4', which is not among your printers.",
  ],
  [
    'printer_offer_differs',
    () => MESSAGES.printer_offer_differs('bracket.ptsm', 'My MK4'),
    "bracket.ptsm keeps its own copy of the printer 'My MK4', which differs from yours.",
  ],
  ['printer_added', () => MESSAGES.printer_added('My MK4'), "Added 'My MK4' to your printers."],
  [
    'printer_not_kept',
    () => MESSAGES.printer_not_kept('My MK4'),
    "'My MK4' could not be kept among your printers. This document stores it, and new documents can use it until Partsmith reloads.",
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
    'save_conflict',
    () => MESSAGES.save_conflict('cube.ptsm'),
    'cube.ptsm was changed elsewhere after you opened or saved it, so Partsmith did not save over it.',
  ],
  [
    'save_assets',
    () => MESSAGES.save_assets('block.ptsm'),
    'block.ptsm was not saved, because Partsmith could not write its imported files to the assets folder next to it.',
  ],
  [
    'save_folder_needed',
    () => MESSAGES.save_folder_needed('block.ptsm'),
    'block.ptsm is not saved yet, because its imported files are saved in the folder it goes in and the browser has not given Partsmith that folder. Choose the folder you are saving block.ptsm in.',
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
    'assets_folder_needed',
    () => MESSAGES.assets_folder_needed('block.ptsm'),
    'The imported geometry of block.ptsm is missing, because the browser has not given Partsmith the folder that holds it and its assets folder. Choose the folder block.ptsm is in.',
  ],
  [
    'assets_wrong_folder',
    () => MESSAGES.assets_wrong_folder('parts', 'block.ptsm'),
    'parts is not the folder that holds block.ptsm. Choose the folder block.ptsm is in.',
  ],
  [
    'assets_without_folder',
    () => MESSAGES.assets_without_folder('block.ptsm'),
    'The imported geometry of block.ptsm is missing, because Partsmith has no file of it to find its assets folder by. Open the file with Open... instead.',
  ],
  [
    'assets_unsupported',
    () => MESSAGES.assets_unsupported('block.ptsm'),
    'The imported geometry of block.ptsm is missing, because this browser does not let Partsmith reach files next to the one you chose. Open it in Chrome or Edge, or in the Partsmith desktop app.',
  ],
  [
    'assets_not_read',
    () => MESSAGES.assets_not_read('block.ptsm'),
    'Partsmith could not read some imported files from the assets folder next to block.ptsm, so their geometry is missing.',
  ],
  [
    'assets_mismatch',
    () => MESSAGES.assets_mismatch('block.ptsm'),
    'Some files in the assets folder next to block.ptsm are not the files it imported, so Partsmith opened it without them and its imported geometry is missing.',
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
    'recovered_more',
    () => MESSAGES.recovered_more('bracket', '14:32', 2),
    'Partsmith kept unsaved changes to bracket from 14:32, and to 2 other documents.',
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
  [
    'feature_lost',
    () => MESSAGES.feature_lost('Fillet 1', "the front top edge of 'Box 1'"),
    "'Fillet 1' lost the front top edge of 'Box 1'.",
  ],
  [
    'features_lost',
    () => MESSAGES.features_lost('Extrude 1', "the region of 'Sketch 1' around (40, 25)", 2),
    "'Extrude 1' lost the region of 'Sketch 1' around (40, 25), so 2 features after it failed too.",
  ],
  [
    'feature_check',
    () => MESSAGES.feature_check('Fillet 1'),
    "'Fillet 1' needs a check after this change.",
  ],
  ['show_lost', () => MESSAGES.show_lost('Fillet 1'), "Show what 'Fillet 1' lost"],
  ['check_feature', () => MESSAGES.check_feature('Fillet 1'), "Check 'Fillet 1'"],
  [
    'repair_missing',
    () => MESSAGES.repair_missing("The front top edge of 'Box 1'"),
    "The front top edge of 'Box 1' (missing)",
  ],
  [
    'repair_check',
    () => MESSAGES.repair_check("The back top edge of 'Extrude 1'"),
    "The back top edge of 'Extrude 1' (check it)",
  ],
  ['cause_ghost', () => MESSAGES.cause_ghost(), 'Before this change'],
  [
    'needs_closed_shape',
    () => MESSAGES.needs_closed_shape('Sketch 1', 2),
    "'Sketch 1' has no closed shape: 2 ends are not connected.",
  ],
  [
    'drop_breaks_use',
    () => MESSAGES.drop_breaks_use('Fillet 1', 'an edge', 'Extrude 2', 'user'),
    "'Fillet 1' uses an edge made by 'Extrude 2'. Drop below 'Extrude 2' instead?",
  ],
  [
    'used_by',
    () => MESSAGES.used_by(['Fillet 1', 'Datum plane 1']),
    "Used by 'Fillet 1', 'Datum plane 1'",
  ],
  [
    'entity_used',
    () => MESSAGES.entity_used('Fillet 1', 'edge', 'line'),
    "'Fillet 1' uses the edge made from this line. Draw its replacement in the same place and 'Fillet 1' will follow it.",
  ],
  [
    'keep_sizes',
    () => MESSAGES.keep_sizes('mm', 4),
    'Keep sizes: add mm to the 4 numbers without a unit',
  ],
  ['field_limit', () => MESSAGES.field_limit('2.3 mm'), 'Largest that works: 2.3 mm'],
  [
    'reattach_prompt',
    () => MESSAGES.reattach_prompt('Sketch 2'),
    "Pick a plane or a flat face for 'Sketch 2'",
  ],
  ['attach_label', () => MESSAGES.attach_label('Sketch 2'), "Attach 'Sketch 2'"],
  [
    'attach_after',
    () => MESSAGES.attach_after('Sketch 2', 'Extrude 2'),
    "'Sketch 2' cannot be placed on 'Extrude 2', which comes after it. Pick an origin plane, or a flat face or datum plane made before 'Sketch 2'.",
  ],
  [
    'attach_datum_unbuilt',
    () => MESSAGES.attach_datum_unbuilt('Sketch 2', 'Datum plane 1', 'suppressed'),
    "'Sketch 2' cannot be placed on 'Datum plane 1', which is suppressed. Unsuppress 'Datum plane 1', or pick another plane or face.",
  ],
  [
    'attach_moved',
    () => MESSAGES.attach_moved('Sketch 2', 'the Top plane (XY)'),
    "'Sketch 2' cannot be placed on the Top plane (XY): the two do not move together, and what later features place on the faces made from 'Sketch 2' could shift with no message. Pick a plane or face that moves the same way as 'Sketch 2'.",
  ],
  [
    'datum_base_moved',
    () => MESSAGES.datum_base_moved('Datum plane 2', 'the Top plane (XY)'),
    "'Datum plane 2' cannot be placed on the Top plane (XY): the two do not move together, and what later features place on the faces made from the sketches that build on 'Datum plane 2' could shift with no message. Pick a plane or face that moves the same way as 'Datum plane 2'.",
  ],
  [
    'datum_base_self',
    () => MESSAGES.datum_base_self('Datum plane 2'),
    "'Datum plane 2' cannot be placed on itself. Pick an origin plane, or a flat face or datum plane made before 'Datum plane 2'.",
  ],
  ['preview_pending', () => MESSAGES.preview_pending(), 'Wait for the preview to update.'],
  ['details_copied', () => MESSAGES.details_copied(), 'Details copied.'],
  [
    'details_not_copied',
    () => MESSAGES.details_not_copied(),
    'The details could not be copied, because the browser did not allow access to the clipboard.',
  ],
  [
    'core_still_working_stoppable',
    () => MESSAGES.core_still_working_stoppable(10),
    'The geometry core has been working for more than 10 seconds, and Partsmith cannot tell whether it will finish. Stop ends this step and keeps your changes and your undo history.',
  ],
  [
    'core_stop_waiting_desktop',
    () => MESSAGES.core_stop_waiting_desktop(),
    'Partsmith asked the geometry core to stop, but it is in a step that cannot be stopped. Wait, or quit Partsmith, which keeps your changes up to the last one the core finished, except those of an open feature command.',
  ],
  [
    'rebuild_stopped',
    () => MESSAGES.rebuild_stopped('Extrude 1', 3),
    "The rebuild stopped before 'Extrude 1' was built. 3 features are not built.",
  ],
  [
    'rebuild_stopped_one',
    () => MESSAGES.rebuild_stopped_one('Extrude 1'),
    "The rebuild stopped before 'Extrude 1' was built.",
  ],
  [
    'timeline_not_built',
    () => MESSAGES.timeline_not_built(),
    'Not built: the rebuild was stopped before it.',
  ],
  [
    'notices_installed',
    () => MESSAGES.notices_installed('/usr/lib/Partsmith/THIRD-PARTY-NOTICES.txt'),
    'The licenses of Open CASCADE Technology and the other software in Partsmith are in /usr/lib/Partsmith/THIRD-PARTY-NOTICES.txt.',
  ],
]

// The message lint of docs/guidelines/errors.md: what no message text may hold.
const MESSAGE_LINT: readonly RegExp[] = [
  /repair panel/,
  /\bop (new|join|cut|intersect)\b/,
  /\b[fbec]-[0-9a-z]+\b/,
  /(Edge|Face|Vertex|Curve) [0-9]/,
  /[0-9]+% match/,
  /`/,
  /os error/,
]

describe('MESSAGES', () => {
  it.each(CASES)('U-73 %s renders exactly its text', (_id, render, text) => {
    expect(render()).toBe(text)
  })

  it('U-73 has an entry for every message id of 4.3', () => {
    expect(Object.keys(MESSAGES)).toEqual(expect.arrayContaining(CASES.map(([id]) => id)))
    expect(CASES).toHaveLength(128)
  })

  it.each([
    'core_still_working_stoppable',
    'core_stop_waiting_desktop',
    'rebuild_stopped',
    'rebuild_stopped_one',
    'timeline_not_built',
  ])('U-275 %s passes the message lint of the error guidelines', (id) => {
    const text = CASES.find(([caseId]) => caseId === id)?.[2]
    expect(text).toBeDefined()
    for (const pattern of MESSAGE_LINT) {
      expect(text).not.toMatch(pattern)
    }
  })

  it('U-73 has no dimension_unsolvable: an edited dimension shows the probe message (3.16.7)', () => {
    expect(Object.keys(MESSAGES)).not.toContain('dimension_unsolvable')
  })

  it('U-140 needs_closed_shape counts one open end in the singular', () => {
    expect(MESSAGES.needs_closed_shape('Sketch 2', 1)).toBe(
      "'Sketch 2' has no closed shape: 1 end is not connected.",
    )
  })

  it('U-138 keep_sizes counts one number in the singular, with the current unit', () => {
    expect(MESSAGES.keep_sizes('in', 1)).toBe('Keep sizes: add in to the 1 number without a unit')
  })

  it('U-73 recovered_more counts one other document in the singular', () => {
    expect(MESSAGES.recovered_more('bracket', '14:32', 1)).toBe(
      'Partsmith kept unsaved changes to bracket from 14:32, and to 1 other document.',
    )
  })

  it('U-135 drop_breaks_use asks to drop above the user when the used feature is dragged', () => {
    expect(MESSAGES.drop_breaks_use('Fillet 1', 'an edge', 'Extrude 2', 'feature')).toBe(
      "'Fillet 1' uses an edge made by 'Extrude 2'. Drop above 'Fillet 1' instead?",
    )
  })

  it('U-136 used_by quotes up to three labels, then adds " and {n} more"', () => {
    expect(MESSAGES.used_by(['Fillet 1'])).toBe("Used by 'Fillet 1'")
    expect(MESSAGES.used_by(['A', 'B', 'C'])).toBe("Used by 'A', 'B', 'C'")
    expect(MESSAGES.used_by(['A', 'B', 'C', 'D'])).toBe("Used by 'A', 'B', 'C' and 1 more")
    expect(MESSAGES.used_by(['A', 'B', 'C', 'D', 'E'])).toBe("Used by 'A', 'B', 'C' and 2 more")
  })

  it('U-137 entity_used names a circle and an arc like a line', () => {
    expect(MESSAGES.entity_used('Extrude 2', 'face', 'circle')).toBe(
      "'Extrude 2' uses the face made from this circle. Draw its replacement in the same place and 'Extrude 2' will follow it.",
    )
    expect(MESSAGES.entity_used('Fillet 1', 'edge', 'arc')).toBe(
      "'Fillet 1' uses the edge made from this arc. Draw its replacement in the same place and 'Fillet 1' will follow it.",
    )
  })

  it('U-137 entity_used of a point says only what uses it', () => {
    expect(MESSAGES.entity_used('Chamfer 1', 'edge', 'point')).toBe(
      "'Chamfer 1' uses the edge made from this point.",
    )
  })

  it('U-250 attach_datum_unbuilt names an error and asks to fix the datum first', () => {
    expect(MESSAGES.attach_datum_unbuilt('Sketch 2', 'Datum plane 1', 'error')).toBe(
      "'Sketch 2' cannot be placed on 'Datum plane 1', which has an error. Fix 'Datum plane 1' first, or pick another plane or face.",
    )
  })

  it('U-250 attach_moved names a datum plane in quotes and a face by its feature', () => {
    expect(MESSAGES.attach_moved('Sketch 2', "'Datum plane 1'")).toBe(
      "'Sketch 2' cannot be placed on 'Datum plane 1': the two do not move together, and what later features place on the faces made from 'Sketch 2' could shift with no message. Pick a plane or face that moves the same way as 'Sketch 2'.",
    )
    expect(MESSAGES.attach_moved('Sketch 2', "this face of 'Extrude 3'")).toContain(
      "cannot be placed on this face of 'Extrude 3': the two",
    )
  })

  it('U-251 datum_base_moved names a datum plane in quotes and a face by its feature', () => {
    expect(MESSAGES.datum_base_moved('Datum plane 2', "'Datum plane 1'")).toBe(
      "'Datum plane 2' cannot be placed on 'Datum plane 1': the two do not move together, and what later features place on the faces made from the sketches that build on 'Datum plane 2' could shift with no message. Pick a plane or face that moves the same way as 'Datum plane 2'.",
    )
    expect(MESSAGES.datum_base_moved('Datum plane 2', "this face of 'Box 1'")).toContain(
      "cannot be placed on this face of 'Box 1': the two",
    )
  })

  it('features_lost counts one feature in the singular', () => {
    expect(MESSAGES.features_lost('Extrude 1', "the region of 'Sketch 1' around (40, 25)", 1)).toBe(
      "'Extrude 1' lost the region of 'Sketch 1' around (40, 25), so 1 feature after it failed too.",
    )
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

describe('capitalized', () => {
  it('capitalizes the first letter and keeps the rest', () => {
    expect(capitalized("the top edge of 'Extrude 1'")).toBe("The top edge of 'Extrude 1'")
    expect(capitalized('')).toBe('')
  })
})

describe('counted', () => {
  it('uses the singular only for one', () => {
    expect(counted(1, 'warning')).toBe('1 warning')
    expect(counted(0, 'warning')).toBe('0 warnings')
    expect(counted(3, 'warning')).toBe('3 warnings')
  })

  it('takes an irregular plural', () => {
    expect(counted(1, 'body', 'bodies')).toBe('1 body')
    expect(counted(2, 'body', 'bodies')).toBe('2 bodies')
  })
})
