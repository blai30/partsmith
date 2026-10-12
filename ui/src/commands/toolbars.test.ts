import { Cuboid, FileOutput, Grid3x3, Magnet, RectangleHorizontal } from 'lucide-react'
import { describe, expect, it } from 'vitest'
import { TOOLBARS, type ToolbarEntry } from './toolbars'

function command(id: string): ToolbarEntry {
  return { kind: 'command', command: id }
}

// The 13 constraint commands in the order of ui.md 3.16.6.
const CONSTRAINT_COMMANDS = [
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

describe('TOOLBARS (3.3)', () => {
  it('lists the Model tools in order', () => {
    expect(TOOLBARS.model).toEqual([
      command('feature.sketch'),
      command('feature.extrude'),
      command('feature.revolve'),
      command('feature.hole'),
      command('feature.fillet'),
      command('feature.chamfer'),
      command('feature.shell'),
      {
        kind: 'group',
        id: expect.any(String),
        title: 'Primitive',
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
        id: expect.any(String),
        title: 'Pattern',
        icon: Grid3x3,
        commands: ['feature.linear_pattern', 'feature.circular_pattern'],
      },
      command('feature.mirror'),
      command('feature.boolean'),
    ])
  })

  it('lists the Sketch tools in order, ending with Finish sketch after a separator', () => {
    expect(TOOLBARS.sketch).toEqual([
      command('sketch.line'),
      {
        kind: 'group',
        id: expect.any(String),
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
        id: expect.any(String),
        title: 'Constrain',
        icon: Magnet,
        commands: CONSTRAINT_COMMANDS,
      },
      { kind: 'separator' },
      command('sketch.finish'),
    ])
  })

  it('lists the Print tools in order, with Export 3MF and the Export group after a separator', () => {
    expect(TOOLBARS.print).toEqual([
      command('print.printer'),
      command('print.overhangs'),
      command('print.checks'),
      command('view.health'),
      { kind: 'separator' },
      command('print.export_3mf'),
      {
        kind: 'group',
        id: expect.any(String),
        title: 'Export',
        icon: FileOutput,
        commands: ['print.export_stl', 'print.export_obj', 'print.export_step'],
      },
    ])
  })

  it('gives every group a distinct id', () => {
    const ids = Object.values(TOOLBARS)
      .flat()
      .flatMap((entry) => (entry.kind === 'group' ? [entry.id] : []))
    expect(new Set(ids).size).toBe(ids.length)
  })

  it('keeps Datum plane, Move body and Split body off the toolbars (palette only)', () => {
    const ids = Object.values(TOOLBARS)
      .flat()
      .flatMap((entry) =>
        entry.kind === 'command' ? [entry.command] : entry.kind === 'group' ? entry.commands : [],
      )
    expect(ids).not.toContain('feature.datum_plane')
    expect(ids).not.toContain('feature.move_body')
    expect(ids).not.toContain('feature.split_body')
  })
})
