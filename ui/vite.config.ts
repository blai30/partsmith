import tailwindcss from '@tailwindcss/vite'
import react from '@vitejs/plugin-react'
import type { Connect, Plugin } from 'vite'
import { defineConfig } from 'vitest/config'

// Cross-origin isolation lets the core use SharedArrayBuffer (technical design section 4).
const ISOLATION_HEADERS = {
  'Cross-Origin-Opener-Policy': 'same-origin',
  'Cross-Origin-Embedder-Policy': 'require-corp',
}

/** Sets the cross-origin isolation headers on a response, then hands the request on. */
const setIsolationHeaders: Connect.NextHandleFunction = (_request, response, next) => {
  for (const [name, value] of Object.entries(ISOLATION_HEADERS)) {
    response.setHeader(name, value)
  }
  next()
}

/**
 * Sets the cross-origin isolation headers on every response of `pnpm dev` and `pnpm preview`, 304 Not Modified
 * included. Its middleware runs before the ones with which Vite serves files, which answer a revalidation with a bare
 * 304 before they apply `server.headers` or `preview.headers` (delivery.md 2.7).
 */
function isolationHeaders(): Plugin {
  return {
    name: 'partsmith:isolation-headers',
    configureServer(server) {
      server.middlewares.use(setIsolationHeaders)
    },
    configurePreviewServer(server) {
      server.middlewares.use(setIsolationHeaders)
    },
  }
}

export default defineConfig({
  plugins: [react(), tailwindcss(), isolationHeaders()],
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
