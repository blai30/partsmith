import { describe, expect, it } from 'vitest'

import { readProjectFile } from '../test/project-file'

// The approved block of ADR 0013 and ui.md 2.16.1, byte for byte.
const APPROVED_STYLESHEET = `@import url('https://fonts.googleapis.com/css2?family=Inter:ital,opsz,wght@0,14..32,100..900;1,14..32,100..900&family=JetBrains+Mono:ital,wght@0,100..800;1,100..800&display=swap');
@import 'tailwindcss';

@custom-variant dark (&:where(.dark, .dark *));
`

// Lazy globs only list paths: Vitest replaces the content of CSS imports with an empty string.
const STYLESHEET_PATHS = Object.keys(import.meta.glob('/src/**/*.css'))

const SOURCES = import.meta.glob<string>(['/src/**/*.{ts,tsx}', '!/src/generated/**'], {
  query: '?raw',
  import: 'default',
  eager: true,
})

// Needles are assembled so this file does not contain them itself.
const STYLE_PROPERTY = ['.style', '.'].join('')
const STYLE_INDEX = ['.style', '['].join('')
const PIXEL_VALUE = ['px', ']'].join('')
const HEX_VALUE = ['[', '#'].join('')

function filesContaining(needle: string, files: Readonly<Record<string, string>>): string[] {
  return Object.entries(files)
    .filter(([, text]) => text.includes(needle))
    .map(([path]) => path)
}

describe('styles in source (U-91)', () => {
  it('U-91 src/index.css equals the block of 2.16.1 byte for byte', async () => {
    expect(await readProjectFile('/src/index.css')).toBe(APPROVED_STYLESHEET)
  })

  it('U-91 src/index.css is the only .css file under src/', () => {
    expect(STYLESHEET_PATHS).toEqual(['/src/index.css'])
  })

  it('reads the TypeScript sources it checks', () => {
    expect(Object.keys(SOURCES)).toContain('/src/main.tsx')
    expect(SOURCES['/src/main.tsx']).toContain('createRoot')
  })

  it('U-91 no .tsx file under src/ has a style JSX attribute', () => {
    const offenders = Object.entries(SOURCES)
      .filter(([path, text]) => path.endsWith('.tsx') && /\sstyle=[{"']/.test(text))
      .map(([path]) => path)
    expect(offenders).toEqual([])
  })

  it('U-91 outside src/viewport/overlay.ts no file under src/ touches element styles', () => {
    const offenders = [
      ...filesContaining(STYLE_PROPERTY, SOURCES),
      ...filesContaining(STYLE_INDEX, SOURCES),
    ].filter((path) => path !== '/src/viewport/overlay.ts')
    expect(offenders).toEqual([])
  })

  it('U-91 no file under src/ contains a pixel arbitrary value or an arbitrary hex color', async () => {
    const stylesheets = await Promise.all(
      STYLESHEET_PATHS.map(async (path) => [path, await readProjectFile(path)] as const),
    )
    const files = { ...SOURCES, ...Object.fromEntries(stylesheets) }
    expect(filesContaining(PIXEL_VALUE, files)).toEqual([])
    expect(filesContaining(HEX_VALUE, files)).toEqual([])
  })
})
