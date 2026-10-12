import {
  Cuboid,
  FileOutput,
  Grid3x3,
  Magnet,
  RectangleHorizontal,
  type LucideIcon,
} from 'lucide-react'
import type { AppContext } from '../state/store'

// The toolbar of each context (ui.md 3.3). Entries name registry commands; an entry whose command
// is not registered (a catalog without that feature type) is not rendered.

export type ToolbarEntry =
  | { readonly kind: 'command'; readonly command: string }
  | {
      readonly kind: 'group'
      readonly id: string
      readonly title: string
      readonly icon: LucideIcon
      readonly commands: readonly string[]
    }
  | { readonly kind: 'separator' }

function command(id: string): ToolbarEntry {
  return { kind: 'command', command: id }
}

/** The 13 constraint commands in the order of 3.16.6. */
const CONSTRAINT_COMMANDS: readonly string[] = [
  'coincident',
  'point_on_object',
  'horizontal',
  'vertical',
  'parallel',
  'perpendicular',
  'tangent',
  'collinear',
  'concentric',
  'equal',
  'symmetric',
  'midpoint',
  'fix',
].map((type) => `sketch.constrain_${type}`)

export const TOOLBARS: Readonly<Record<AppContext, readonly ToolbarEntry[]>> = {
  model: [
    command('feature.sketch'),
    command('feature.extrude'),
    command('feature.revolve'),
    command('feature.hole'),
    command('feature.fillet'),
    command('feature.chamfer'),
    command('feature.shell'),
    {
      kind: 'group',
      id: 'model.primitive',
      title: 'Shapes',
      icon: Cuboid,
      commands: [
        'feature.box',
        'feature.cylinder',
        'feature.sphere',
        'feature.cone',
        'feature.torus',
      ],
    },
    {
      kind: 'group',
      id: 'model.pattern',
      title: 'Pattern',
      icon: Grid3x3,
      commands: ['feature.linear_pattern', 'feature.circular_pattern'],
    },
    command('feature.mirror'),
    command('feature.boolean'),
  ],
  sketch: [
    command('sketch.line'),
    {
      kind: 'group',
      id: 'sketch.rectangle',
      title: 'Rectangle',
      icon: RectangleHorizontal,
      commands: ['sketch.rectangle_2point', 'sketch.rectangle_center'],
    },
    command('sketch.circle'),
    command('sketch.arc_3point'),
    command('sketch.point'),
    command('sketch.construction'),
    command('sketch.dimension'),
    {
      kind: 'group',
      id: 'sketch.constrain',
      title: 'Constrain',
      icon: Magnet,
      commands: CONSTRAINT_COMMANDS,
    },
    { kind: 'separator' },
    command('sketch.finish'),
  ],
  print: [
    command('print.printer'),
    command('print.overhangs'),
    command('print.checks'),
    command('view.health'),
    command('print.lay_on_face'),
    { kind: 'separator' },
    command('print.export_3mf'),
    {
      kind: 'group',
      id: 'print.export',
      title: 'Export',
      icon: FileOutput,
      commands: ['print.export_stl', 'print.export_obj', 'print.export_step'],
    },
  ],
}

/** Commands drawn in the primary (accent) style (3.3). */
export const PRIMARY_COMMANDS: ReadonlySet<string> = new Set(['sketch.finish', 'print.export_3mf'])
