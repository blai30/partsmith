// Acceptance test 65 of docs/spec/delivery.md section 6: the cross-origin isolation headers on every response of
// Vite's dev and preview servers, 304 Not Modified included (section 2.7).
import assert from 'node:assert/strict'
import { mkdir, mkdtemp, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { test } from 'node:test'
import { fileURLToPath } from 'node:url'

import { createServer, preview } from 'vite'

const uiDirectory = fileURLToPath(new URL('..', import.meta.url))

// The headers of section 2.7, which ui/public/_headers sets in production.
const ISOLATION_HEADERS = {
  'cross-origin-opener-policy': 'same-origin',
  'cross-origin-embedder-policy': 'require-corp',
}

// Where both servers listen: port 0 lets the system pick a free port.
const LISTEN = { host: '127.0.0.1', port: 0 }

/** The inline configuration of a server: ui/ with its vite.config.ts, logging nothing, plus `overrides`. */
function configuration(overrides) {
  return {
    root: uiDirectory,
    configFile: join(uiDirectory, 'vite.config.ts'),
    logLevel: 'silent',
    ...overrides,
  }
}

/** The URL of `path` on a listening server. */
function serverUrl(server, path) {
  return `http://${LISTEN.host}:${String(server.httpServer.address().port)}${path}`
}

/** Checks that `response` has `status`, both isolation headers and no `Vary` that names `Origin`. */
function assertIsolated(response, status, request) {
  assert.equal(response.status, status, `${request}: status`)
  for (const [name, value] of Object.entries(ISOLATION_HEADERS)) {
    assert.equal(response.headers.get(name), value, `${request}: ${name}`)
  }
  assert.doesNotMatch(response.headers.get('vary') ?? '', /\borigin\b/i, `${request}: vary`)
}

/** Requests `path` from `server`, then again with `If-None-Match` set to the first response's `ETag`. */
async function assertIsolatedWhenRevalidated(server, path) {
  const first = await fetch(serverUrl(server, path))
  await first.arrayBuffer()
  assertIsolated(first, 200, `GET ${path}`)
  const etag = first.headers.get('etag')
  assert.ok(etag, `GET ${path} has no ETag`)

  const second = await fetch(serverUrl(server, path), { headers: { 'If-None-Match': etag } })
  await second.arrayBuffer()
  assertIsolated(second, 304, `GET ${path} with If-None-Match`)
}

/**
 * Makes a folder under the system's temporary folder and starts a server with `start(folder)`. The test's teardown
 * closes the server before it removes the folder, so nothing writes to the folder once it is gone.
 */
async function serveWithTemporaryFolder(context, prefix, start) {
  const folder = await mkdtemp(join(tmpdir(), prefix))
  let server
  context.after(async () => {
    await server?.close()
    await rm(folder, { recursive: true, force: true })
  })
  server = await start(folder)
  return server
}

test('65: the preview server sends the isolation headers on every response, 304s included', async (context) => {
  const server = await serveWithTemporaryFolder(
    context,
    'partsmith-vite-servers-',
    async (outDir) => {
      await mkdir(join(outDir, 'assets'))
      await writeFile(join(outDir, 'index.html'), '<!doctype html>\n<title>Partsmith</title>\n')
      await writeFile(join(outDir, 'assets/a.js'), 'export const answer = 42\n')
      return preview(configuration({ build: { outDir }, preview: LISTEN }))
    },
  )

  await Promise.all(
    ['/index.html', '/assets/a.js'].map((path) => assertIsolatedWhenRevalidated(server, path)),
  )
})

test('65: the dev server sends the isolation headers on every response, 304s included', async (context) => {
  // A cache folder of its own leaves ui/node_modules/.vite as it was. Without discovery, Vite optimizes only the
  // dependencies its plugins name, and listen() returns once they are written, so nothing writes to the folder later.
  const server = await serveWithTemporaryFolder(context, 'partsmith-vite-cache-', (cacheDir) =>
    createServer(configuration({ cacheDir, optimizeDeps: { noDiscovery: true }, server: LISTEN })),
  )
  await server.listen()

  await Promise.all(
    ['/index.html', '/_headers', '/src/main.tsx'].map((path) =>
      assertIsolatedWhenRevalidated(server, path),
    ),
  )
})
