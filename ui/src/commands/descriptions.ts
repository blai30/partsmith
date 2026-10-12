// What each feature command does, in the words of the interface (ui.md 2.10). The toolbar tooltip
// and the palette footer show it. Sketch tools, Finish sketch and constraints carry theirs in
// their definitions (builtin.ts). A type without an entry has no description.

/** Descriptions of feature commands by command id. */
export const FEATURE_DESCRIPTIONS: Readonly<Record<string, string>> = {
  'feature.sketch':
    'Draws a 2D shape on a plane or a flat face, to extrude, revolve or place holes from.',
  'feature.extrude': 'Pulls a closed sketch shape straight out into a solid, or cuts it into one.',
  'feature.revolve':
    'Spins a closed sketch shape around an axis into a round solid, or cuts with it.',
  'feature.hole': 'Drills round holes where you click on a flat face or at sketch points.',
  'feature.box': 'Adds a box of the given length, width and height.',
  'feature.cylinder': 'Adds a cylinder of the given radius and height.',
  'feature.sphere': 'Adds a ball of the given radius.',
  'feature.cone': 'Adds a cone, or a tapered cylinder, from two radii and a height.',
  'feature.torus': 'Adds a ring from the radius of its center line and the radius of its tube.',
  'feature.fillet': 'Rounds the selected edges.',
  'feature.chamfer': 'Bevels the selected edges with a flat cut.',
  'feature.shell':
    'Hollows out a body, leaving walls of the given thickness and the picked faces open.',
  'feature.boolean': 'Joins, cuts or intersects one body with other bodies.',
  'feature.move_body': 'Moves and rotates a whole body.',
  'feature.split_body':
    'Cuts a body in two along a plane, for example to fit a large part on the bed.',
  'feature.mirror': 'Mirrors a body across a plane, joined to it or as a new body.',
  'feature.linear_pattern': 'Repeats a body in a row, or in a grid with a second direction.',
  'feature.circular_pattern': 'Repeats a body around an axis.',
  'feature.datum_plane': 'Adds a plane offset from another plane, to sketch on or mirror across.',
}
