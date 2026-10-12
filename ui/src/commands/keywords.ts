import type { Keyword } from './registry'

// The palette's search words for every command, feature and built-in alike (ui.md 2.10). A tagged
// word is another app's name for the command, so someone coming from Fusion or FreeCAD finds it by
// that name and sees the mapping in the option itself. A command without an entry has none.

function fusion(word: string): Keyword {
  return { word, app: 'Fusion' }
}

function freecad(word: string): Keyword {
  return { word, app: 'FreeCAD' }
}

/** Keywords by command id. */
export const COMMAND_KEYWORDS: Readonly<Record<string, readonly Keyword[]>> = {
  'feature.sketch': ['drawing', 'profile', 'new sketch', fusion('Create Sketch'), 'sketcher'],
  'feature.extrude': [
    freecad('Pad'),
    freecad('Pocket'),
    fusion('Press Pull'),
    'extrusion',
    'pull',
    'push',
  ],
  'feature.revolve': [freecad('Revolution'), freecad('Groove'), 'lathe', 'spin'],
  'feature.hole': ['drill', 'counterbore', 'countersink', 'screw hole'],
  'feature.fillet': ['round', 'radius', 'rounded edge'],
  'feature.chamfer': ['bevel'],
  'feature.shell': ['hollow', freecad('Thickness')],
  'feature.boolean': [
    fusion('Combine'),
    'union',
    'subtract',
    'intersect',
    'join',
    'cut',
    'merge',
    'group',
  ],
  'feature.mirror': [freecad('Mirrored'), 'symmetry'],
  'feature.linear_pattern': [fusion('Rectangular Pattern'), freecad('LinearPattern'), 'array'],
  'feature.circular_pattern': [freecad('PolarPattern'), 'polar pattern', 'array'],
  'feature.datum_plane': [fusion('Offset Plane'), 'construction plane', 'work plane'],
  'feature.move_body': [
    fusion('Move/Copy'),
    freecad('Placement'),
    'translate',
    'rotate',
    'transform',
  ],
  'feature.split_body': ['slice', 'cut body'],
  'feature.box': [freecad('Additive Box'), 'cube', 'block', 'primitive', 'shape'],
  'feature.cylinder': [freecad('Additive Cylinder'), 'rod', 'pin', 'primitive', 'shape'],
  'feature.sphere': [freecad('Additive Sphere'), 'ball', 'primitive', 'shape'],
  'feature.cone': [freecad('Additive Cone'), 'taper', 'primitive', 'shape'],
  'feature.torus': [freecad('Additive Torus'), 'ring', 'donut', 'primitive', 'shape'],
  'sketch.line': [freecad('Polyline'), 'segment'],
  'sketch.rectangle_2point': ['rectangle'],
  'sketch.rectangle_center': [freecad('Centered Rectangle'), 'rectangle'],
  'sketch.circle': [fusion('Center Diameter Circle')],
  'sketch.arc_3point': [fusion('3-Point Arc')],
  'sketch.point': ['hole center'],
  'sketch.construction': [freecad('Toggle Construction'), 'construction geometry', 'reference'],
  'sketch.dimension': [
    fusion('Sketch Dimension'),
    freecad('Constrain Distance'),
    'distance',
    'length',
    'radius',
    'diameter',
    'angle',
  ],
  'sketch.constrain_fix': [freecad('Lock'), freecad('Block'), 'fixed', 'ground'],
  'sketch.constrain_symmetric': [fusion('Symmetry'), 'symmetrical'],
  'sketch.constrain_point_on_object': ['on curve', 'on line'],
  'sketch.constrain_equal': ['same size'],
  'sketch.constrain_midpoint': ['middle'],
  'sketch.finish': [freecad('Close'), 'close sketch', 'leave sketch', 'exit sketch'],
  'view.fit': [freecad('Fit All'), 'zoom all', 'zoom extents'],
  'view.home': ['isometric', 'home'],
  'view.projection': ['perspective'],
  'view.section': [fusion('Section Analysis'), freecad('Clipping Plane'), 'cross section'],
  'view.parameters': [
    fusion('Change Parameters'),
    freecad('Spreadsheet'),
    'user parameters',
    'variables',
  ],
  'view.problems': ['warnings', 'errors'],
  'view.health': ['printability', 'health'],
  'view.visibility': ['hide', 'show'],
  'view.grid': ['grid', 'floor'],
  'app.keys': ['help', 'shortcuts', 'hotkeys', 'navigation', 'mouse'],
  'edit.suppress': ['disable', 'turn off', 'skip'],
  'edit.edit_feature': ['change', 'modify'],
  'edit.repeat': ['again', 'last command'],
  'file.export': ['3MF', 'STL', 'OBJ', 'STEP'],
}

/** The keywords of a command id; none without an entry. */
export function keywordsFor(id: string): readonly Keyword[] {
  return COMMAND_KEYWORDS[id] ?? []
}

/** The text a keyword is matched by: the word itself, or a tagged keyword's word. */
export function keywordWord(keyword: Keyword): string {
  return typeof keyword === 'string' ? keyword : keyword.word
}
