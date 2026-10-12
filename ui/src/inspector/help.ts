// The hover help of inspector fields whose label alone does not say what they do, in the words of
// the interface (ui.md 2.12). A field without an entry has no help; schema descriptions are
// developer documentation and are never shown.

const OPERATION =
  'New body makes a separate part. Join adds to the body, Cut removes from it, and Intersect keeps only the overlap.'
const CHANGED_BODY = 'The body that Join, Cut and Intersect change.'

/** Field help by feature type and field path. */
export const FIELD_HELP: Readonly<Record<string, Readonly<Record<string, string>>>> = {
  extrude: {
    profile: 'The closed sketch shapes to extrude. Click inside a shape to add it.',
    'extent.kind':
      'Distance goes one way, Symmetric the same distance both ways, Two sides sets each side, Through all goes through the whole body.',
    op: OPERATION,
    reverse: 'Extrudes to the other side of the sketch.',
    body: CHANGED_BODY,
  },
  revolve: {
    axis: 'The line to spin around: an origin axis, a sketch line or a straight edge.',
    'extent.kind': 'Full makes a complete turn. Angle stops part way.',
    op: OPERATION,
    reverse: 'Turns the other way around the axis.',
    body: CHANGED_BODY,
  },
  hole: {
    points: 'Where the holes go: click a flat face, or pick points of a sketch.',
    'hole_type.kind':
      'Counterbore adds a wider flat recess for a screw head. Countersink adds a cone for a flat-head screw.',
    'hole_type.diameter': 'Diameter of the wider recess at the top of the hole.',
    'hole_type.depth': 'Depth of the recess.',
    'hole_type.angle': 'Angle of the countersink cone. 90° fits most flat-head screws.',
    'extent.kind': 'Depth stops at a set depth. Through all goes through the whole body.',
    tip_angle:
      'Gives the bottom a drill-point cone of this angle. Leave it empty for a flat bottom.',
    reverse: 'Drills out of the face instead of into it.',
  },
  fillet: {
    edges: 'The edges to round. Click a face to add all of its edges.',
  },
  chamfer: {
    edges: 'The edges to bevel. Click a face to add all of its edges.',
    'distance.kind':
      'Equal distance cuts the same amount from both faces. Two distances sets each side.',
  },
  shell: {
    remove_faces: 'Faces to leave open. With none, the body becomes a closed hollow.',
    thickness: 'Wall thickness. For FDM printing, a multiple of the nozzle width prints best.',
  },
  boolean: {
    target: 'The body that changes.',
    tools:
      'The bodies that join, cut or intersect the target. They are used up unless Keep tools is on.',
    op: 'Join merges the bodies, Cut removes the tools from the target, and Intersect keeps only the overlap.',
    keep_tools: 'Keeps the tool bodies after the operation instead of using them up.',
  },
  mirror: {
    op: 'Join adds the mirrored copy to the body. New body puts it in a separate body.',
  },
  split_body: {
    target: 'The body that gets the other part. Leave it empty to discard that part.',
    flip: 'Keeps the back part in Body instead of the front part.',
  },
  linear_pattern: {
    'direction.count': 'How many instances along this direction, counting the original.',
    'direction.spacing': 'Distance between neighboring instances.',
  },
  circular_pattern: {
    count: 'How many instances around the axis, counting the original.',
    angle: 'The angle the instances spread over.',
  },
  datum_plane: {
    offset: 'Distance from the base plane along its normal.',
  },
  move_body: {
    'rotation.axis': 'The axis to rotate around. Leave it empty to only move.',
  },
}

/** The help of a feature type's field, or null when it has none. */
export function fieldHelp(type: string, path: string): string | null {
  return FIELD_HELP[type]?.[path] ?? null
}
