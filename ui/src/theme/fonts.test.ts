import { describe, expect, it } from 'vitest'

import {
  listProjectFolder,
  projectFileSha256,
  readProjectBytes,
  readProjectFile,
} from '../test/project-file'

// The self-hosted fonts and their loading (ui.md 2.16.1, 2.16.7, ADR 0031).

type FontFile = { readonly name: string; readonly bytes: number; readonly sha256: string }

// The table of 2.16.7.
const FONT_FILES: readonly FontFile[] = [
  {
    name: 'Geist-OFL.txt',
    bytes: 4368,
    sha256: '930853ee1daa68554d9e35c8a9175affb74f699fad9a5da6ee5ebe76379d9137',
  },
  {
    name: 'Geist-Variable.woff2',
    bytes: 69652,
    sha256: 'a369fcf5628ea2aa4e1b9e2ec6a5b3624e365bda588e1f0f2f12b564f728fbb8',
  },
  {
    name: 'JetBrainsMono-OFL.txt',
    bytes: 4399,
    sha256: '30f0c136e3c88e422d0791acd97238870f9054a9729bc34cf2ff0d4ed8cac4ad',
  },
  {
    name: 'JetBrainsMono-Variable.woff2',
    bytes: 56316,
    sha256: '4b7bb5bad76ad8b93fb40d378f76184e84f3f08b504b7034bb14f6eb92ac3f46',
  },
]

const LICENSE_STARTS: readonly (readonly [string, string])[] = [
  ['Geist-OFL.txt', 'Copyright (c) 2023 Vercel, in collaboration with basement.studio'],
  ['JetBrainsMono-OFL.txt', 'Copyright 2020 The JetBrains Mono Project Authors'],
]

const FONT_HOSTS = ['fonts.googleapis.com', 'fonts.gstatic.com']

/** The `url(...)` targets of a stylesheet, without quotes. */
function stylesheetUrls(stylesheet: string): string[] {
  return [...stylesheet.matchAll(/url\(\s*['"]?([^'")]*)['"]?\s*\)/g)].map(([, url]) => url ?? '')
}

/** The `src` URL of the `@font-face` rule whose `font-family` is `family` (quoted as in index.css). */
function fontFaceUrl(stylesheet: string, family: string): string | undefined {
  const rule = [...stylesheet.matchAll(/@font-face\s*\{([^}]*)\}/g)]
    .map(([, body]) => body ?? '')
    .find((body) => body.includes(`font-family: ${family};`))
  return rule === undefined ? undefined : stylesheetUrls(rule)[0]
}

// The page of ui/index.html as the browser parses it before the bundle runs (as U-163 reads it).
async function indexPage(): Promise<Document> {
  return new DOMParser().parseFromString(await readProjectFile('/index.html'), 'text/html')
}

function linksWithRel(page: Document, rel: string): HTMLLinkElement[] {
  return Array.from(page.querySelectorAll('link')).filter(
    (link) => link.getAttribute('rel') === rel,
  )
}

async function onlyPreload(): Promise<{ readonly page: Document; readonly link: HTMLLinkElement }> {
  const page = await indexPage()
  const [link] = linksWithRel(page, 'preload')
  if (link === undefined) {
    throw new Error('index.html has no preload link.')
  }
  return { page, link }
}

const SOURCES = import.meta.glob<string>(
  ['/src/**/*.{ts,tsx}', '!/src/generated/**', '!/src/**/*.test.{ts,tsx}'],
  { query: '?raw', import: 'default', eager: true },
)

// The JSX elements Tailwind's preflight would set in the code face (2.16.7).
const CODE_FACE_ELEMENT = /<(?:kbd|samp|pre)[\s>]/

describe('font files (U-202)', () => {
  it('U-202 public/fonts holds exactly the four files of 2.16.7', async () => {
    expect(await listProjectFolder('/public/fonts')).toEqual(FONT_FILES.map((file) => file.name))
  })

  it.each(FONT_FILES)('U-202 $name has $bytes bytes and its SHA-256', async (file) => {
    const path = `/public/fonts/${file.name}`
    expect((await readProjectBytes(path)).length).toBe(file.bytes)
    expect(await projectFileSha256(path)).toBe(file.sha256)
  })

  it.each(FONT_FILES.filter((file) => file.name.endsWith('.woff2')))(
    'U-202 $name starts with wOF2',
    async (file) => {
      const bytes = await readProjectBytes(`/public/fonts/${file.name}`)
      expect(String.fromCharCode(...bytes.subarray(0, 4))).toBe('wOF2')
    },
  )

  it.each(LICENSE_STARTS)('U-202 %s starts with its copyright line', async (name, start) => {
    expect((await readProjectFile(`/public/fonts/${name}`)).startsWith(start)).toBe(true)
  })

  it.each(LICENSE_STARTS)('U-202 %s is the SIL Open Font License 1.1', async (name) => {
    expect(await readProjectFile(`/public/fonts/${name}`)).toContain(
      'SIL OPEN FONT LICENSE Version 1.1',
    )
  })

  it('U-202 src/index.css names each woff2 file once, under /fonts/', async () => {
    const urls = stylesheetUrls(await readProjectFile('/src/index.css'))
    expect(urls.toSorted()).toEqual([
      '/fonts/Geist-Variable.woff2',
      '/fonts/JetBrainsMono-Variable.woff2',
    ])
  })
})

describe('the preload (U-203)', () => {
  it('U-203 index.html has exactly one preload link, a child of head', async () => {
    const page = await indexPage()
    const links = linksWithRel(page, 'preload')
    expect(links).toHaveLength(1)
    expect(links[0]?.parentElement).toBe(page.head)
  })

  it('U-203 the preload is the URL of the Geist @font-face', async () => {
    const { link } = await onlyPreload()
    const stylesheet = await readProjectFile('/src/index.css')
    expect(link.getAttribute('href')).toBe('/fonts/Geist-Variable.woff2')
    expect(link.getAttribute('href')).toBe(fontFaceUrl(stylesheet, "'Geist'"))
  })

  it('U-203 the preload is a woff2 font fetched in CORS mode', async () => {
    const { link } = await onlyPreload()
    expect(link.getAttribute('as')).toBe('font')
    expect(link.getAttribute('type')).toBe('font/woff2')
    expect(['', 'anonymous']).toContain(link.getAttribute('crossorigin'))
  })

  it('U-203 index.html links no stylesheet', async () => {
    expect(linksWithRel(await indexPage(), 'stylesheet')).toEqual([])
  })

  it.each(['/index.html', '/src/index.css'])(
    'U-203 %s names no Google Fonts host',
    async (path) => {
      const text = await readProjectFile(path)
      for (const host of FONT_HOSTS) {
        expect(text).not.toContain(host)
      }
    },
  )
})

describe('type rules in source (U-204)', () => {
  it('reads the TypeScript sources it checks, without the tests', () => {
    expect(Object.keys(SOURCES)).toContain('/src/main.tsx')
    expect(Object.keys(SOURCES)).not.toContain('/src/theme/fonts.test.ts')
  })

  // The families and their features are named only in src/index.css (2.16.1), which is not TypeScript.
  it.each(['FONT_CLASSES', 'font-sans', 'font-[', 'font-feature', 'tnum', 'slashed-zero'])(
    'U-204 no source file contains %s',
    (needle) => {
      const offenders = Object.entries(SOURCES)
        .filter(([, text]) => text.includes(needle))
        .map(([path]) => path)
      expect(offenders).toEqual([])
    },
  )

  it('U-204 no source file renders a kbd, samp or pre element', () => {
    const offenders = Object.entries(SOURCES)
      .filter(([, text]) => CODE_FACE_ELEMENT.test(text))
      .map(([path]) => path)
    expect(offenders).toEqual([])
  })
})
