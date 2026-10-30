import { readFileSync } from 'node:fs'
import tailwindcss from '@tailwindcss/vite'
import react from '@vitejs/plugin-react'
import type { Connect, Plugin } from 'vite'
import { defineConfig } from 'vitest/config'

// The file whose headers Cloudflare Pages sends with every response, the one place that holds them
// (delivery.md 2.7).
const HEADERS_FILE = new URL('public/_headers', import.meta.url)

// A header line of `_headers`: two spaces, a name, `: ` and a value.
const HEADER_LINE = /^ {2}([A-Za-z-]+): (.+)$/

// The header the dev server leaves out: its page runs an inline script and inserts `<style>`
// elements, and it also serves the desktop's dev window, where a policy would refuse Tauri's IPC.
const POLICY_HEADER = 'Content-Security-Policy'

/** The headers of `_headers`, in order: after the line `/*`, each line is one header. */
function readHeaders(file: URL): [string, string][] {
  const [route, ...lines] = readFileSync(file, 'utf8').trimEnd().split('\n')
  if (route !== '/*') {
    throw new Error(`${file.pathname} must start with the line /* (delivery.md 2.7).`)
  }
  return lines.map((line) => {
    const [, name, value] = HEADER_LINE.exec(line) ?? []
    if (name === undefined || value === undefined) {
      throw new Error(
        `${file.pathname}: "${line}" is not two spaces, a name, ": " and a value (delivery.md 2.7).`,
      )
    }
    return [name, value]
  })
}

/** Sets `headers` on a response, then hands the request on. */
function headersSetter(headers: readonly [string, string][]): Connect.NextHandleFunction {
  return (_request, response, next) => {
    for (const [name, value] of headers) {
      response.setHeader(name, value)
    }
    next()
  }
}

/**
 * Sends the headers of `public/_headers` on every response of `pnpm preview`, and all but the
 * content security policy on every response of `pnpm dev`, 304 Not Modified included. Its middleware
 * runs before the ones with which Vite serves files, which answer a revalidation with a bare 304
 * before they apply `server.headers` or `preview.headers` (delivery.md 2.7).
 */
function webHeaders(): Plugin {
  const headers = readHeaders(HEADERS_FILE)
  return {
    name: 'partsmith:web-headers',
    configureServer(server) {
      server.middlewares.use(headersSetter(headers.filter(([name]) => name !== POLICY_HEADER)))
    },
    configurePreviewServer(server) {
      server.middlewares.use(headersSetter(headers))
    },
  }
}

export default defineConfig({
  plugins: [react(), tailwindcss(), webHeaders()],
  // The app requests nothing from another origin, and Vite's CORS middleware adds Vary: Origin to every response
  // (delivery.md 2.7). The preview server takes this value too.
  server: { cors: false },
  // Keep Vite's default modulePreload and output names: check:dist finds the initial JavaScript
  // through them (delivery.md 2.6).
  worker: { format: 'es' },
  build: { target: 'es2023', sourcemap: true },
  test: {
    environment: 'jsdom',
    include: ['src/**/*.test.{ts,tsx}'],
    setupFiles: ['src/test/setup.ts'],
  },
})
