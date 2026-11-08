import { readdirSync, readFileSync } from 'node:fs'
import { join, relative } from 'node:path'
import { fileURLToPath } from 'node:url'
import { describe, expect, it } from 'vitest'
import * as assets from './assets'
import { assetKindOf } from './assets'

// The asset kinds of file names (ui.md 2.15, since M2) and the check that no module reads `.ptsm`
// text (U-276, m2-plan.md gate 13, ADR 0064). U-217 retired with `referencedAssetNames`.

/** The folder `ui/src`. */
const SOURCE = fileURLToPath(new URL('..', import.meta.url))

/** Every `.ts` and `.tsx` file under `folder`, as paths relative to `ui/src` with `/` separators. */
function sourceFiles(folder: string): readonly string[] {
  return readdirSync(folder, { withFileTypes: true }).flatMap((entry) => {
    const path = join(folder, entry.name)
    if (entry.isDirectory()) {
      return sourceFiles(path)
    }
    return /\.tsx?$/.test(entry.name) ? [relative(SOURCE, path).replaceAll('\\', '/')] : []
  })
}

/** Whether a source file is a test file or part of the test support, which U-276 leaves out. */
function isTestCode(path: string): boolean {
  return /\.test\.tsx?$/.test(path) || path.startsWith('test/')
}

describe('no module parses .ptsm text (since M2)', () => {
  it('U-276 only the frame codec and the settings store call JSON.parse', () => {
    const parsers = sourceFiles(SOURCE)
      .filter((path) => !isTestCode(path))
      .filter((path) => readFileSync(join(SOURCE, path), 'utf8').includes('JSON.parse'))

    expect(parsers.toSorted()).toEqual(['core/frame.ts', 'settings/settings.ts'])
  })

  it('U-276 assets.ts exports neither ASSET_NAME nor referencedAssetNames', () => {
    expect(Object.keys(assets)).not.toContain('ASSET_NAME')
    expect(Object.keys(assets)).not.toContain('referencedAssetNames')
  })

  it('the walk finds the modules it checks', () => {
    const files = sourceFiles(SOURCE)

    expect(files).toContain('core/frame.ts')
    expect(files).toContain('files/assets.ts')
    expect(files).toContain('files/assets.test.ts')
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
