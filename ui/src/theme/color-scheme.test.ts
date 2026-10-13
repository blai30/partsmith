import { describe, expect, it } from 'vitest'

type FileSystem = { readFileSync(path: URL, encoding: 'utf8'): string }

// Reads a file relative to this test through Node, which runs Vitest, as U-91 reads src/index.css.
async function readFile(relativePath: string): Promise<string> {
  const specifier: string = 'node:fs'
  const fileSystem = (await import(/* @vite-ignore */ specifier)) as FileSystem
  return fileSystem.readFileSync(new URL(relativePath, import.meta.url), 'utf8')
}

// The page of ui/index.html as the browser parses it before the bundle runs.
async function indexPage(): Promise<Document> {
  return new DOMParser().parseFromString(await readFile('../../index.html'), 'text/html')
}

function colorSchemeMetas(page: Document): HTMLMetaElement[] {
  return Array.from(page.querySelectorAll('meta')).filter((meta) => meta.name === 'color-scheme')
}

describe('theme before the bundle runs (U-163)', () => {
  it('U-163 index.html has exactly one color-scheme meta', async () => {
    expect(colorSchemeMetas(await indexPage())).toHaveLength(1)
  })

  it('U-163 the color-scheme meta is a child of head', async () => {
    const page = await indexPage()
    expect(colorSchemeMetas(page)[0]?.parentElement).toBe(page.head)
  })

  it('U-163 the color-scheme meta lets the page follow a light or a dark OS', async () => {
    expect(colorSchemeMetas(await indexPage())[0]?.content).toBe('light dark')
  })
})
