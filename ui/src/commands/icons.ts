import {
  ArrowUpFromLine,
  Box,
  CircleDot,
  Columns3,
  Combine,
  Cone,
  createLucideIcon,
  Cylinder,
  FileInput,
  FlipHorizontal2,
  Globe,
  Layers2,
  Move3d,
  Octagon,
  Orbit,
  PackageOpen,
  PencilRuler,
  Rotate3d,
  SquareRoundCorner,
  SquareSplitVertical,
  Torus,
  type IconNode,
  type LucideIcon,
} from 'lucide-react'

// Icons of feature types and sketch constraints (ui.md 2.10).

/** Lucide icons by manifest icon name (features.md 3.13). */
export const FEATURE_ICONS: Readonly<Record<string, LucideIcon>> = {
  sketch: PencilRuler,
  extrude: ArrowUpFromLine,
  revolve: Rotate3d,
  hole: CircleDot,
  box: Box,
  cylinder: Cylinder,
  sphere: Globe,
  cone: Cone,
  torus: Torus,
  fillet: SquareRoundCorner,
  chamfer: Octagon,
  shell: PackageOpen,
  boolean: Combine,
  'move-body': Move3d,
  'split-body': SquareSplitVertical,
  mirror: FlipHorizontal2,
  'linear-pattern': Columns3,
  'circular-pattern': Orbit,
  'datum-plane': Layers2,
  import: FileInput,
}

function path(d: string, key: string): IconNode[number] {
  return ['path', { d, key }]
}

function circle(cx: number, cy: number, r: number, key: string): IconNode[number] {
  return ['circle', { cx: String(cx), cy: String(cy), r: String(r), key }]
}

/**
 * In-house constraint glyphs in the lucide style (24 by 24, stroke width 2, round caps and joins,
 * currentColor), because lucide has no parallel, perpendicular or coincident symbols.
 */
export const CONSTRAINT_ICONS = {
  coincident: createLucideIcon('constraint-coincident', [
    path('M4 20 9.5 14.5', 'line-in'),
    path('M20 4 14.5 9.5', 'line-out'),
    circle(12, 12, 2.5, 'point'),
  ]),
  point_on_object: createLucideIcon('constraint-point-on-object', [
    path('M3 18 21 6', 'curve'),
    circle(12, 12, 2.5, 'point'),
  ]),
  horizontal: createLucideIcon('constraint-horizontal', [
    path('M4 12h16', 'line'),
    path('M4 8v8', 'start'),
    path('M20 8v8', 'end'),
  ]),
  vertical: createLucideIcon('constraint-vertical', [
    path('M12 4v16', 'line'),
    path('M8 4h8', 'start'),
    path('M8 20h8', 'end'),
  ]),
  parallel: createLucideIcon('constraint-parallel', [
    path('M6 19 12 5', 'first'),
    path('M12 19 18 5', 'second'),
  ]),
  perpendicular: createLucideIcon('constraint-perpendicular', [
    path('M4 19h16', 'base'),
    path('M12 19V5', 'upright'),
    path('M12 15h4v4', 'right-angle'),
  ]),
  tangent: createLucideIcon('constraint-tangent', [
    circle(12, 14, 6, 'circle'),
    path('M3 8h18', 'line'),
  ]),
  collinear: createLucideIcon('constraint-collinear', [
    path('M3 18 9 14', 'first'),
    path('M15 10 21 6', 'second'),
    path('M11 12.7h.01', 'gap'),
  ]),
  concentric: createLucideIcon('constraint-concentric', [
    circle(12, 12, 3, 'inner'),
    circle(12, 12, 8, 'outer'),
  ]),
  equal: createLucideIcon('constraint-equal', [
    path('M5 9h14', 'upper'),
    path('M5 15h14', 'lower'),
  ]),
  symmetric: createLucideIcon('constraint-symmetric', [
    path('M12 3v4', 'axis-top'),
    path('M12 10v4', 'axis-middle'),
    path('M12 17v4', 'axis-bottom'),
    path('M8 8 4 12l4 4', 'left'),
    path('m16 8 4 4-4 4', 'right'),
  ]),
  midpoint: createLucideIcon('constraint-midpoint', [
    path('M3 12h6', 'left'),
    path('M15 12h6', 'right'),
    path('M12 9l3 3-3 3-3-3z', 'middle'),
  ]),
  fix: createLucideIcon('constraint-fix', [
    circle(12, 7, 3, 'point'),
    path('M12 10v6', 'stem'),
    path('M6 16h12', 'ground'),
    path('m8 16-2 4', 'hatch-left'),
    path('m12 16-2 4', 'hatch-middle'),
    path('m16 16-2 4', 'hatch-right'),
  ]),
} as const satisfies Readonly<Record<string, LucideIcon>>
