import { describe, expect, it } from 'vitest'
import { categoryOrder, fuzzyScore, rankPaletteItems, type PaletteItem } from './fuzzy'
import type { CommandCategory, Keyword } from './registry'

type ItemOptions = {
  readonly kind?: 'command' | 'setting'
  readonly keywords?: readonly Keyword[]
  readonly category?: CommandCategory
  readonly detail?: string | null
  readonly disabledReason?: string | null
}

/** A command palette item; the kind defaults to command and the category to Edit. */
function item(id: string, title: string, options: ItemOptions = {}): PaletteItem {
  return {
    id,
    kind: options.kind ?? 'command',
    category: options.category ?? 'Edit',
    title,
    keywords: options.keywords ?? [],
    detail: options.detail ?? null,
    description: null,
    shortcut: null,
    disabledReason: options.disabledReason ?? null,
    run: () => undefined,
  }
}

/** A palette item for a document item or a recent file, which has no category. */
function documentItem(
  id: string,
  title: string,
  kind: 'feature' | 'body' | 'parameter' | 'recent',
): PaletteItem {
  return {
    id,
    kind,
    category: null,
    title,
    keywords: [],
    detail: null,
    description: null,
    shortcut: null,
    disabledReason: null,
    run: () => undefined,
  }
}

function titles(items: readonly PaletteItem[]): string[] {
  return items.map((entry) => entry.title)
}

describe('fuzzyScore', () => {
  it('U-26 fuzzyScore(ext, Extrude) is 9 + 5 + 5 - 0.07 = 18.93', () => {
    expect(fuzzyScore('ext', 'Extrude')).toBeCloseTo(18.93, 9)
  })

  it('U-26 fuzzyScore(ext, Export 3MF) is 9 + 5 + 1 - 0.10 = 14.9', () => {
    expect(fuzzyScore('ext', 'Export 3MF')).toBeCloseTo(14.9, 9)
  })

  it('U-26 fuzzyScore(ext, Text) is 1 + 5 + 5 - 0.04 = 10.96', () => {
    expect(fuzzyScore('ext', 'Text')).toBeCloseTo(10.96, 9)
  })

  it('U-26 fuzzyScore(xyz, Extrude) is null', () => {
    expect(fuzzyScore('xyz', 'Extrude')).toBeNull()
  })

  it('is null when the query is longer than the text', () => {
    expect(fuzzyScore('extrudes', 'Extrude')).toBeNull()
  })

  it('compares case-insensitively', () => {
    expect(fuzzyScore('EXT', 'extrude')).toBeCloseTo(18.93, 9)
  })

  it('removes leading and trailing whitespace of the query', () => {
    expect(fuzzyScore('  ext ', 'Extrude')).toBeCloseTo(18.93, 9)
  })

  it('keeps inner runs of whitespace as single spaces that match spaces in the text', () => {
    // e 9, x p o r t 5 each, the space 5, 3 1 + 6 (word start) + 4 (consecutive), minus 0.10.
    expect(fuzzyScore('export   3', 'Export 3MF')).toBeCloseTo(49.9, 9)
  })

  it('gives 8 extra points at position 0 only', () => {
    expect(fuzzyScore('a', 'a')).toBeCloseTo(8.99, 9)
    expect(fuzzyScore('b', 'ab')).toBeCloseTo(0.98, 9)
  })

  it.each([' ', '-', '_', '/', '.', ':', '('])(
    'gives 6 extra points to a word start after %j',
    (separator) => {
      expect(fuzzyScore('b', `a${separator}b`)).toBeCloseTo(6.97, 9)
    },
  )

  it('treats a lowercase letter followed by an uppercase letter in the original text as a word start', () => {
    expect(fuzzyScore('p', 'linearPattern')).toBeCloseTo(6.87, 9)
  })

  it('does not treat an uppercase letter after an uppercase letter as a word start', () => {
    expect(fuzzyScore('b', 'AB')).toBeCloseTo(0.98, 9)
  })

  it('takes the best of all matches, not the leftmost one', () => {
    // a at 4 (word start) 7, b at 5 (consecutive) 5, minus 0.06; the leftmost match gives only 6.
    expect(fuzzyScore('ab', 'xab ab')).toBeCloseTo(11.94, 9)
  })
})

describe('rankPaletteItems', () => {
  const extrude = item('feature.extrude', 'Extrude', {
    category: 'Create',
    keywords: [{ word: 'Pad', app: 'FreeCAD' }, { word: 'Press Pull', app: 'Fusion' }, 'pull'],
  })
  const exportThreeMf = item('print.export_3mf', 'Export 3MF', { category: 'Print' })
  const text = item('text', 'Text')

  it('U-26 orders Extrude, Export 3MF, Text for ext', () => {
    expect(titles(rankPaletteItems('ext', [text, exportThreeMf, extrude], [], 'model'))).toEqual([
      'Extrude',
      'Export 3MF',
      'Text',
    ])
  })

  it('excludes items with no score', () => {
    const fillet = item('feature.fillet', 'Fillet')
    expect(titles(rankPaletteItems('ext', [fillet, extrude], [], 'model'))).toEqual(['Extrude'])
  })

  it('U-26 pad matches Extrude through its tagged keyword {word: Pad, app: FreeCAD} with score 16.97', () => {
    expect((fuzzyScore('pad', 'Pad') ?? Number.NaN) - 2).toBeCloseTo(16.97, 9)
    const [first] = rankPaletteItems('pad', [extrude], [], 'model')
    expect(first?.title).toBe('Extrude')
  })

  it('U-26 a keyword score counts 2 less than the same title score', () => {
    // Pads scores 18.96 and ranks above Extrude (16.97 through Pad); Map pad scores 16.93 and ranks below.
    const pads = item('pads', 'Pads')
    const mapPad = item('map-pad', 'Map pad')
    expect(titles(rankPaletteItems('pad', [mapPad, extrude, pads], [], 'model'))).toEqual([
      'Pads',
      'Extrude',
      'Map pad',
    ])
  })

  it('scores each text on its own, though the texts share the scoring rows', () => {
    // Extra text 20.9, Latex sheet 6.89, Context 10.93: rows left over from an earlier text would
    // lift Latex sheet above Context.
    const items = [item('a', 'Extra text'), item('b', 'Latex sheet'), item('c', 'Context')]
    expect(titles(rankPaletteItems('ext', items, [], 'model'))).toEqual([
      'Extra text',
      'Context',
      'Latex sheet',
    ])
  })

  it('ranks by score before recency', () => {
    expect(
      titles(rankPaletteItems('ext', [text, exportThreeMf, extrude], ['text'], 'model')),
    ).toEqual(['Extrude', 'Export 3MF', 'Text'])
  })

  describe('other apps’ names (2.10)', () => {
    it('shows "{app}: {word}" as the detail when a tagged keyword matched', () => {
      expect(rankPaletteItems('pad', [extrude], [], 'model')[0]?.detail).toBe('FreeCAD: Pad')
      expect(rankPaletteItems('press pull', [extrude], [], 'model')[0]?.detail).toBe(
        'Fusion: Press Pull',
      )
    })

    it('keeps the item’s own detail when the title or an untagged keyword matched', () => {
      const withDetail = { ...extrude, detail: 'Own detail' }
      expect(rankPaletteItems('extr', [withDetail], [], 'model')[0]?.detail).toBe('Own detail')
      expect(rankPaletteItems('pul', [withDetail], [], 'model')[0]?.detail).toBe('Own detail')
    })

    it('leaves the detail to a disabled reason', () => {
      const disabled = { ...extrude, disabledReason: 'Draw a closed shape in a sketch first.' }
      expect(rankPaletteItems('pad', [disabled], [], 'model')[0]?.detail).toBeNull()
    })
  })

  describe('context ranking (2.10)', () => {
    const symmetric = item('sketch.constrain_symmetric', 'Symmetric', {
      category: 'Constrain',
      keywords: [{ word: 'Symmetry', app: 'Fusion' }],
    })
    const mirror = item('feature.mirror', 'Mirror', {
      category: 'Pattern',
      keywords: [{ word: 'Mirrored', app: 'FreeCAD' }, 'symmetry'],
    })

    it('adds 3 to Sketch and Constrain items in Sketch context', () => {
      expect(titles(rankPaletteItems('symmetry', [mirror, symmetric], [], 'sketch'))).toEqual([
        'Symmetric',
        'Mirror',
      ])
    })

    it('adds nothing in Model context, where equal scores order by title', () => {
      expect(titles(rankPaletteItems('symmetry', [symmetric, mirror], [], 'model'))).toEqual([
        'Mirror',
        'Symmetric',
      ])
    })

    it('adds 3 to Print items in Print context', () => {
      const exportStl = item('print.export_stl', 'Export STL', { category: 'Print' })
      const exportFile = item('file.export', 'Export', { category: 'File', keywords: ['STL'] })
      expect(titles(rankPaletteItems('stl', [exportStl, exportFile], [], 'model'))).toEqual([
        'Export',
        'Export STL',
      ])
      expect(titles(rankPaletteItems('stl', [exportStl, exportFile], [], 'print'))).toEqual([
        'Export STL',
        'Export',
      ])
    })
  })

  describe('equal scores', () => {
    const bodies = [
      documentItem('body:b-2', 'Body 2', 'body'),
      documentItem('body:b-3', 'Body 3', 'body'),
      documentItem('body:b-1', 'Body 1', 'body'),
    ]

    it('U-27 order by title when none is recent', () => {
      expect(titles(rankPaletteItems('body', bodies, [], 'model'))).toEqual([
        'Body 1',
        'Body 2',
        'Body 3',
      ])
    })

    it('U-27 order by position in recentIds, then by title', () => {
      expect(titles(rankPaletteItems('body', bodies, ['body:b-3'], 'model'))).toEqual([
        'Body 3',
        'Body 1',
        'Body 2',
      ])
    })

    it('U-27 put the most recent first', () => {
      expect(titles(rankPaletteItems('body', bodies, ['body:b-2', 'body:b-3'], 'model'))).toEqual([
        'Body 2',
        'Body 3',
        'Body 1',
      ])
    })
  })

  describe('empty query', () => {
    const items = [
      documentItem('recent:bracket', 'Open recent: bracket', 'recent'),
      documentItem('parameter:p-size', 'size = 20 mm', 'parameter'),
      documentItem('body:b-peg', 'Peg', 'body'),
      item('settings.theme.dark', 'Theme: Dark', { kind: 'setting', category: 'Settings' }),
      documentItem('feature:f-ext1', 'Extrude 1', 'feature'),
      item('view.standard.top', 'View: Top', { category: 'View' }),
      documentItem('body:b-cube', 'Cube', 'body'),
      item('edit.undo', 'Undo', { category: 'Edit' }),
      item('file.open', 'Open', { category: 'File' }),
      item('feature.fillet', 'Fillet', { category: 'Modify' }),
      item('feature.box', 'Box', { category: 'Create' }),
      item('sketch.line', 'Line', { category: 'Sketch' }),
      item('sketch.constrain_fix', 'Fix', { category: 'Constrain' }),
      item('print.check_now', 'Check print', { category: 'Print' }),
    ]

    it('U-27 lists every command and setting by the Model category order, and no document items or recent files', () => {
      expect(titles(rankPaletteItems('', items, [], 'model'))).toEqual([
        'Box',
        'Fillet',
        'Undo',
        'View: Top',
        'Open',
        'Check print',
        'Theme: Dark',
        'Line',
        'Fix',
      ])
    })

    it('U-27 starts with the Sketch category in Sketch context', () => {
      const ranked = rankPaletteItems('', items, [], 'sketch')
      expect(ranked[0]?.category).toBe('Sketch')
      expect(titles(ranked)).toEqual([
        'Line',
        'Fix',
        'Undo',
        'View: Top',
        'Box',
        'Fillet',
        'Open',
        'Check print',
        'Theme: Dark',
      ])
    })

    it('U-27 starts with the Print category in Print context', () => {
      expect(titles(rankPaletteItems('', items, [], 'print'))).toEqual([
        'Check print',
        'View: Top',
        'Undo',
        'Open',
        'Box',
        'Fillet',
        'Theme: Dark',
        'Line',
        'Fix',
      ])
    })

    it('U-27 lists recent items first, in recentIds order, whatever their kind', () => {
      expect(
        titles(rankPaletteItems('', items, ['body:b-peg', 'edit.undo', 'feature:f-ext1'], 'model')),
      ).toEqual([
        'Peg',
        'Undo',
        'Extrude 1',
        'Box',
        'Fillet',
        'View: Top',
        'Open',
        'Check print',
        'Theme: Dark',
        'Line',
        'Fix',
      ])
    })

    it('U-27 lists at most 5 recent items first', () => {
      const recentIds = [
        'parameter:p-size',
        'body:b-peg',
        'feature:f-ext1',
        'settings.theme.dark',
        'edit.undo',
        'file.open',
        'body:b-cube',
      ]
      const ranked = titles(rankPaletteItems('', items, recentIds, 'model'))
      expect(ranked.slice(0, 6)).toEqual([
        'size = 20 mm',
        'Peg',
        'Extrude 1',
        'Theme: Dark',
        'Undo',
        'Box',
      ])
      expect(ranked).toContain('Open')
      expect(ranked).not.toContain('Cube')
    })

    it('U-27 lists every command with no limit', () => {
      const many = Array.from({ length: 60 }, (_, index) =>
        item(`command.${String(index)}`, `Command ${String(index).padStart(2, '0')}`),
      )
      expect(rankPaletteItems('', many, [], 'model')).toHaveLength(60)
    })
  })

  describe('limit', () => {
    const many = Array.from({ length: 60 }, (_, index) =>
      documentItem(
        `parameter:p${String(index)}`,
        `p${String(index)} = ${String(index)} mm`,
        'parameter',
      ),
    )

    it('U-27 never returns more than 50 items for a query', () => {
      expect(rankPaletteItems('p', many, [], 'model')).toHaveLength(50)
    })
  })
})

describe('categoryOrder (2.10)', () => {
  it('follows the context order, then the categories it does not name in CommandCategory order', () => {
    expect(categoryOrder('model')).toEqual([
      'Create',
      'Modify',
      'Pattern',
      'Construct',
      'Edit',
      'View',
      'File',
      'Print',
      'Settings',
      'Sketch',
      'Constrain',
    ])
    expect(categoryOrder('sketch')).toEqual([
      'Sketch',
      'Constrain',
      'Edit',
      'View',
      'Create',
      'Modify',
      'Pattern',
      'Construct',
      'File',
      'Print',
      'Settings',
    ])
    expect(categoryOrder('print')).toEqual([
      'Print',
      'View',
      'Edit',
      'File',
      'Create',
      'Modify',
      'Pattern',
      'Construct',
      'Settings',
      'Sketch',
      'Constrain',
    ])
  })
})
