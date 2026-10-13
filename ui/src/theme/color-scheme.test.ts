import { describe, expect, it } from 'vitest'

import { readProjectFile } from '../test/project-file'

// The page of ui/index.html as the browser parses it before the bundle runs.
async function indexPage(): Promise<Document> {
  return new DOMParser().parseFromString(await readProjectFile('/index.html'), 'text/html')
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
