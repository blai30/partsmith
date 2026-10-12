import { describe, expect, it } from 'vitest'
import { fuzzyScore, rankPaletteItems, type PaletteItem } from './fuzzy'
import type { CommandCategory } from './registry'

type ItemOptions = {
  readonly kind?: 'command' | 'setting'
  readonly keywords?: readonly string[]
  readonly category?: CommandCategory
}

/** A command palette item; the kind defaults to command and the category to Edit. */
function item(id: string, title: string, options: ItemOptions = {}): PaletteItem {
  return {
    id,
    kind: options.kind ?? 'command',
    category: options.category ?? 'Edit',
    title,
    keywords: options.keywords ?? [],
    detail: null,
    shortcut: null,
    disabledReason: null,
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
  const extrude = item('feature.extrude', 'Extrude', { keywords: ['pad', 'pocket', 'extrusion'] })
  const exportThreeMf = item('print.export_3mf', 'Export 3MF')
  const text = item('text', 'Text')

  it('U-26 orders Extrude, Export 3MF, Text for ext', () => {
    expect(titles(rankPaletteItems('ext', [text, exportThreeMf, extrude], []))).toEqual([
      'Extrude',
      'Export 3MF',
      'Text',
    ])
  })

  it('excludes items with no score', () => {
    const fillet = item('feature.fillet', 'Fillet')
    expect(titles(rankPaletteItems('ext', [fillet, extrude], []))).toEqual(['Extrude'])
  })

  it('U-26 pad matches Extrude through its keyword with score 18.97 - 2 = 16.97', () => {
    expect((fuzzyScore('pad', 'pad') ?? Number.NaN) - 2).toBeCloseTo(16.97, 9)
    expect(titles(rankPaletteItems('pad', [extrude], []))).toEqual(['Extrude'])
  })

  it('U-26 a keyword score counts 2 less than the same title score', () => {
    // Pads scores 18.96 and ranks above Extrude (16.97 through pad); Map pad scores 16.93 and ranks below.
    const pads = item('pads', 'Pads')
    const mapPad = item('map-pad', 'Map pad')
    expect(titles(rankPaletteItems('pad', [mapPad, extrude, pads], []))).toEqual([
      'Pads',
      'Extrude',
      'Map pad',
    ])
  })

  it('scores each text on its own, though the texts share the scoring rows', () => {
    // Extra text 20.9, Latex sheet 6.89, Context 10.93: rows left over from an earlier text would
    // lift Latex sheet above Context.
    const items = [item('a', 'Extra text'), item('b', 'Latex sheet'), item('c', 'Context')]
    expect(titles(rankPaletteItems('ext', items, []))).toEqual([
      'Extra text',
      'Context',
      'Latex sheet',
    ])
  })

  it('ranks by score before recency', () => {
    expect(titles(rankPaletteItems('ext', [text, exportThreeMf, extrude], ['text']))).toEqual([
      'Extrude',
      'Export 3MF',
      'Text',
    ])
  })

  describe('equal scores', () => {
    const bodies = [
      documentItem('body:b-2', 'Body 2', 'body'),
      documentItem('body:b-3', 'Body 3', 'body'),
      documentItem('body:b-1', 'Body 1', 'body'),
    ]

    it('U-27 order by title when none is recent', () => {
      expect(titles(rankPaletteItems('body', bodies, []))).toEqual(['Body 1', 'Body 2', 'Body 3'])
    })

    it('U-27 order by position in recentIds, then by title', () => {
      expect(titles(rankPaletteItems('body', bodies, ['body:b-3']))).toEqual([
        'Body 3',
        'Body 1',
        'Body 2',
      ])
    })

    it('U-27 put the most recent first', () => {
      expect(titles(rankPaletteItems('body', bodies, ['body:b-2', 'body:b-3']))).toEqual([
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
    ]

    it('U-27 lists commands by category, then features, bodies, parameters and recent files, each in title order', () => {
      expect(titles(rankPaletteItems('', items, []))).toEqual([
        'Open',
        'Undo',
        'View: Top',
        'Theme: Dark',
        'Extrude 1',
        'Cube',
        'Peg',
        'size = 20 mm',
        'Open recent: bracket',
      ])
    })

    it('U-27 orders commands by category, not by title', () => {
      const commands = [
        item('view.fit', 'Fit to view', { category: 'View' }),
        item('file.open', 'Open', { category: 'File' }),
        item('edit.undo', 'Undo', { category: 'Edit' }),
      ]
      expect(titles(rankPaletteItems('', commands, []))).toEqual(['Open', 'Undo', 'Fit to view'])
    })

    it('U-27 follows the category order of 2.10 from File to Settings', () => {
      const categories: readonly CommandCategory[] = [
        'Settings',
        'Print',
        'Constrain',
        'Sketch',
        'Construct',
        'Pattern',
        'Modify',
        'Create',
        'View',
        'Edit',
        'File',
      ]
      // Titles run against the category order, so only the categories can put File first.
      const commands = categories.map((category, index) =>
        item(`command.${String(index)}`, `${String.fromCodePoint(65 + index)} ${category}`, {
          category,
        }),
      )
      expect(titles(rankPaletteItems('', commands, []))).toEqual([
        'K File',
        'J Edit',
        'I View',
        'H Create',
        'G Modify',
        'F Pattern',
        'E Construct',
        'D Sketch',
        'C Constrain',
        'B Print',
        'A Settings',
      ])
    })

    it('U-27 lists recent items first, in recentIds order', () => {
      expect(titles(rankPaletteItems('', items, ['body:b-peg', 'feature:f-ext1']))).toEqual([
        'Peg',
        'Extrude 1',
        'Open',
        'Undo',
        'View: Top',
        'Theme: Dark',
        'Cube',
        'size = 20 mm',
        'Open recent: bracket',
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
      expect(titles(rankPaletteItems('', items, recentIds))).toEqual([
        'size = 20 mm',
        'Peg',
        'Extrude 1',
        'Theme: Dark',
        'Undo',
        'Open',
        'View: Top',
        'Cube',
        'Open recent: bracket',
      ])
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
      expect(rankPaletteItems('p', many, [])).toHaveLength(50)
    })

    it('U-27 never returns more than 50 items for an empty query', () => {
      expect(rankPaletteItems('', many, [])).toHaveLength(50)
    })
  })
})
