import { fileURLToPath } from 'node:url'
import { test } from '@playwright/test'
import { createServer, type ViteDevServer } from 'vite'

// The test pages in ../pages/ (ui.md 6.5 for the WebGL2 backend, 6.7 P-12): the production preview
// serves only the app, so a spec serves its page from a Vite dev server rooted at ui/, which
// compiles the page's modules from src/.

const uiDirectory = fileURLToPath(new URL('../..', import.meta.url))

/** A test page served for the tests of a file or describe block. */
export type TestPage = {
  /** The page's URL, known once the server has started. */
  readonly url: string
}

/**
 * Serves `e2e/pages/{name}` for the tests of the file or describe block that calls it: a Vite dev
 * server on a free port starts before those tests and closes after them.
 */
export function serveTestPage(name: string): TestPage {
  const path = `e2e/pages/${name}`
  let server: ViteDevServer | null = null
  let url: string | null = null
  test.beforeAll(async () => {
    server = await createServer({
      root: uiDirectory,
      logLevel: 'error',
      clearScreen: false,
      server: { port: 0, hmr: false },
      optimizeDeps: { entries: [path] },
    })
    await server.listen()
    const base = server.resolvedUrls?.local[0]
    if (base === undefined) {
      throw new Error('The Vite dev server reported no local URL.')
    }
    url = new URL(path, base).href
  })
  test.afterAll(async () => {
    await server?.close()
  })
  return {
    get url() {
      if (url === null) {
        throw new Error(`The server of ${path} has not started.`)
      }
      return url
    },
  }
}
