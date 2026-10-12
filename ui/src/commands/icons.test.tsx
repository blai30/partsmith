import { cleanup, render } from '@testing-library/react'
import {
  ArrowUpFromLine,
  Box,
  CircleDot,
  Columns3,
  Combine,
  Cone,
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
} from 'lucide-react'
import { afterEach, describe, expect, it } from 'vitest'
import { CONSTRAINT_ICONS, FEATURE_ICONS } from './icons'

afterEach(() => {
  cleanup()
})

// Manifest icon names (features.md 3.13) and their lucide icons (ui.md 2.10).
const FEATURE_ICON_TABLE = [
  ['sketch', PencilRuler],
  ['extrude', ArrowUpFromLine],
  ['revolve', Rotate3d],
  ['hole', CircleDot],
  ['box', Box],
  ['cylinder', Cylinder],
  ['sphere', Globe],
  ['cone', Cone],
  ['torus', Torus],
  ['fillet', SquareRoundCorner],
  ['chamfer', Octagon],
  ['shell', PackageOpen],
  ['boolean', Combine],
  ['move-body', Move3d],
  ['split-body', SquareSplitVertical],
  ['mirror', FlipHorizontal2],
  ['linear-pattern', Columns3],
  ['circular-pattern', Orbit],
  ['datum-plane', Layers2],
  ['import', FileInput],
] as const

const CONSTRAINT_TYPES = [
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
] as const

describe('FEATURE_ICONS', () => {
  it.each(FEATURE_ICON_TABLE)('maps %s to its lucide icon', (name, icon) => {
    expect(FEATURE_ICONS[name]).toBe(icon)
  })

  it('maps exactly the manifest icon names of the built-in catalog', () => {
    expect(Object.keys(FEATURE_ICONS).toSorted()).toEqual(
      FEATURE_ICON_TABLE.map(([name]) => name).toSorted(),
    )
  })
})

describe('CONSTRAINT_ICONS', () => {
  it('has one glyph per constraint type', () => {
    expect(Object.keys(CONSTRAINT_ICONS).toSorted()).toEqual([...CONSTRAINT_TYPES].toSorted())
  })

  it.each(CONSTRAINT_TYPES)(
    'draws %s in the lucide style: 24 by 24, stroke width 2, round caps and joins, currentColor',
    (type) => {
      const Icon = CONSTRAINT_ICONS[type]
      const { container } = render(<Icon aria-hidden="true" />)
      const svg = container.querySelector('svg')
      expect(svg).not.toBeNull()
      expect(svg?.getAttribute('viewBox')).toBe('0 0 24 24')
      expect(svg?.getAttribute('stroke')).toBe('currentColor')
      expect(svg?.getAttribute('stroke-width')).toBe('2')
      expect(svg?.getAttribute('stroke-linecap')).toBe('round')
      expect(svg?.getAttribute('stroke-linejoin')).toBe('round')
      expect(svg?.childElementCount).toBeGreaterThan(0)
    },
  )

  it('draws a different glyph for every constraint type', () => {
    const drawings = CONSTRAINT_TYPES.map((type) => {
      const Icon = CONSTRAINT_ICONS[type]
      const { container } = render(<Icon aria-hidden="true" />)
      const markup = container.querySelector('svg')?.innerHTML ?? ''
      cleanup()
      return markup
    })
    expect(new Set(drawings).size).toBe(CONSTRAINT_TYPES.length)
  })
})
