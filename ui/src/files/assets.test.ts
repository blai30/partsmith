import { describe, expect, it } from 'vitest'
import * as assets from './assets'
import { assetKindOf } from './assets'

// The asset kinds of file names (ui.md 2.15, since M2) and the check that no module reads `.ptsm`
// text (U-276, m2-plan.md gate 13, ADR 0064). U-217 retired with `referencedAssetNames`.

/** The text of every `.ts` and `.tsx` file under `ui/src` but test files and `src/test/`, by path. */
const SOURCES = import.meta.glob<string>(
  ['/src/**/*.{ts,tsx}', '!/src/**/*.test.{ts,tsx}', '!/src/test/**'],
  { query: '?raw', import: 'default', eager: true },
)

describe('no module parses .ptsm text (since M2)', () => {
  it('U-276 only the frame codec and the settings store call JSON.parse', () => {
    const parsers = Object.entries(SOURCES)
      .filter(([, text]) => text.includes('JSON.parse'))
      .map(([path]) => path)

    expect(parsers.toSorted()).toEqual(['/src/core/frame.ts', '/src/settings/settings.ts'])
  })

  it('U-276 assets.ts exports neither ASSET_NAME nor referencedAssetNames', () => {
    expect(Object.keys(assets)).not.toContain('ASSET_NAME')
    expect(Object.keys(assets)).not.toContain('referencedAssetNames')
  })

  it('the files it reads are the modules, without tests or test support', () => {
    const paths = Object.keys(SOURCES)

    expect(paths).toContain('/src/core/frame.ts')
    expect(paths).toContain('/src/files/assets.ts')
    expect(paths.some((path) => path.endsWith('.test.ts') || path.startsWith('/src/test/'))).toBe(
      false,
    )
  })
})

describe('assetKindOf (since M2, 2.15)', () => {
  it.each([
    ['a.step', 'step'],
    ['a.STEP', 'step'],
    ['a.stp', 'step'],
    [`${'c'.repeat(64)}.ttf`, 'ttf'],
    ['font.TTF', 'ttf'],
    ['font.otf', 'otf'],
  ])('U-277 %s is a %s file', (name, kind) => {
    expect(assetKindOf(name)).toBe(kind)
  })

  it.each(['a.woff2', 'a.ptsm', 'step', 'a.step.txt'])('U-277 %s has no asset kind', (name) => {
    expect(assetKindOf(name)).toBeNull()
  })
})
