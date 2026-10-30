// Acceptance test 65 of docs/spec/delivery.md section 6: the web app's response headers of ui/public/_headers on every
// response of Vite's preview and dev servers, 304 Not Modified included, the dev server's without the content security
// policy (section 2.7).
import assert from 'node:assert/strict'
import { mkdir, mkdtemp, readFile, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { test } from 'node:test'
import { fileURLToPath } from 'node:url'

import { createServer, preview } from 'vite'

const uiDirectory = fileURLToPath(new URL('..', import.meta.url))

// The file whose headers production serves (section 2.7).
const headersPath = join(uiDirectory, 'public/_headers')

// The one header the dev server leaves out: its page needs inline script and style, and it also serves the
// desktop's dev window (section 2.7).
const POLICY = 'content-security-policy'

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

/**
 * The headers of `_headers` text, by lower-case name: after the line `/*`, every line is two spaces, a name, `: ` and a
 * value (section 2.7).
 */
function parseHeaders(text) {
  const [path, ...lines] = text.split('\n')
  assert.equal(path, '/*', '_headers starts with the line /*')
  assert.equal(lines.pop(), '', '_headers ends with a line feed')
  return Object.fromEntries(
    lines.map((line) => {
      const match = /^ {2}([A-Za-z-]+): (.+)$/.exec(line)
      assert.ok(
        match,
        `_headers line ${JSON.stringify(line)} is two spaces, a name, ": " and a value`,
      )
      return [match[1].toLowerCase(), match[2]]
    }),
  )
}

/** The headers that production serves, read from ui/public/_headers. */
async function productionHeaders() {
  return parseHeaders(await readFile(headersPath, 'utf8'))
}

/**
 * Checks that `response` has `status`, each of `expected` with its value, none of `absent`, and no `Vary` that names
 * `Origin`.
 */
function assertHeaders(response, status, expected, absent, request) {
  assert.equal(response.status, status, `${request}: status`)
  for (const [name, value] of Object.entries(expected)) {
    assert.equal(response.headers.get(name), value, `${request}: ${name}`)
  }
  for (const name of absent) {
    assert.equal(response.headers.get(name), null, `${request}: ${name} is not sent`)
  }
  assert.doesNotMatch(response.headers.get('vary') ?? '', /\borigin\b/i, `${request}: vary`)
}

/**
 * Requests `path` from `server`, then again with `If-None-Match` set to the first response's `ETag`, and checks both
 * responses' headers.
 */
async function assertHeadersWhenRevalidated(server, path, expected, absent = []) {
  const first = await fetch(serverUrl(server, path))
  await first.arrayBuffer()
  assertHeaders(first, 200, expected, absent, `GET ${path}`)
  const etag = first.headers.get('etag')
  assert.ok(etag, `GET ${path} has no ETag`)

  const second = await fetch(serverUrl(server, path), { headers: { 'If-None-Match': etag } })
  await second.arrayBuffer()
  assertHeaders(second, 304, expected, absent, `GET ${path} with If-None-Match`)
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

test('65: _headers holds the five headers of section 2.7', async () => {
  assert.deepEqual(Object.keys(await productionHeaders()), [
    'cross-origin-opener-policy',
    'cross-origin-embedder-policy',
    POLICY,
    'x-content-type-options',
    'referrer-policy',
  ])
})

test('65: the preview server sends the headers of _headers on every response, 304s included', async (context) => {
  const expected = await productionHeaders()
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
    ['/index.html', '/assets/a.js'].map((path) =>
      assertHeadersWhenRevalidated(server, path, expected),
    ),
  )
})

test('65: the dev server sends the headers of _headers but the policy on every response, 304s included', async (context) => {
  const { [POLICY]: policy, ...expected } = await productionHeaders()
  assert.ok(policy, '_headers sets a content security policy')
  // A cache folder of its own leaves ui/node_modules/.vite as it was. Without discovery, Vite optimizes only the
  // dependencies its plugins name, and listen() returns once they are written, so nothing writes to the folder later.
  const server = await serveWithTemporaryFolder(context, 'partsmith-vite-cache-', (cacheDir) =>
    createServer(configuration({ cacheDir, optimizeDeps: { noDiscovery: true }, server: LISTEN })),
  )
  await server.listen()

  await Promise.all(
    ['/index.html', '/_headers', '/src/main.tsx'].map((path) =>
      assertHeadersWhenRevalidated(server, path, expected, [POLICY]),
    ),
  )
})
