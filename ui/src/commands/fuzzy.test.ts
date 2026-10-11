import { describe, expect, it } from 'vitest'
import { fuzzyScore, rankPaletteItems, type PaletteItem } from './fuzzy'

type ItemOptions = {
  readonly kind?: PaletteItem['kind']
  readonly keywords?: readonly string[]
}

function item(id: string, title: string, options: ItemOptions = {}): PaletteItem {
  return {
    id,
    kind: options.kind ?? 'command',
    title,
    keywords: options.keywords ?? [],
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

  it('ranks by score before recency', () => {
    expect(titles(rankPaletteItems('ext', [text, exportThreeMf, extrude], ['text']))).toEqual([
      'Extrude',
      'Export 3MF',
      'Text',
    ])
  })

  describe('equal scores', () => {
    const bodies = [
      item('body:b-2', 'Body 2', { kind: 'body' }),
      item('body:b-3', 'Body 3', { kind: 'body' }),
      item('body:b-1', 'Body 1', { kind: 'body' }),
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
      item('recent:bracket', 'Open recent: bracket', { kind: 'recent' }),
      item('parameter:p-size', 'size = 20 mm', { kind: 'parameter' }),
      item('body:b-peg', 'Peg', { kind: 'body' }),
      item('settings.theme.dark', 'Theme: Dark', { kind: 'setting' }),
      item('feature:f-ext1', 'Extrude 1', { kind: 'feature' }),
      item('view.standard.top', 'View: Top'),
      item('body:b-cube', 'Cube', { kind: 'body' }),
      item('edit.undo', 'Undo'),
      item('file.open', 'Open'),
    ]

    it('U-27 lists commands, settings, features, bodies, parameters and recent files, each in title order', () => {
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
      item(`parameter:p${String(index)}`, `p${String(index)} = ${String(index)} mm`, {
        kind: 'parameter',
      }),
    )

    it('U-27 never returns more than 50 items for a query', () => {
      expect(rankPaletteItems('p', many, [])).toHaveLength(50)
    })

    it('U-27 never returns more than 50 items for an empty query', () => {
      expect(rankPaletteItems('', many, [])).toHaveLength(50)
    })
  })
})
