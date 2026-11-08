// Protocol.md tests 101 and 102 (section 2.10, ADR 0063): the built web core module reads the page's cancel word
// through its import partsmith_cancel_word, stops the request whose id the word holds, and keeps its three exports and
// an unshared memory.
//
// Usage: node cancel_word.mjs <partsmith_core.mjs> <partsmith_core.wasm> <reference part .ptsm>
//
// Exits with status 0 when both tests hold; otherwise an assertion fails and Node exits with a nonzero status.

import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'

import { callCore, loadCore, requestFrame, useCancelWord } from './core.mjs'

const [modulePath, wasmPath, referencePartPath] = process.argv.slice(2)
assert.ok(
  modulePath && wasmPath && referencePartPath,
  'usage: node cancel_word.mjs <partsmith_core.mjs> <partsmith_core.wasm> <reference part .ptsm>',
)

// Test 102: the glue names the import as one value of its wasmImports map, as it names OCCT's EM_JS functions.
const glue = await readFile(modulePath, 'utf8')
const imports = glue.match(/wasmImports\s*=\s*\{([^}]*)\}/)
assert.ok(imports, 'the glue defines its wasmImports map')
const importValues = imports[1].split(',').map((entry) => entry.split(':').at(-1).trim())
assert.ok(importValues.includes('partsmith_cancel_word'), `wasmImports names partsmith_cancel_word: ${imports[1]}`)

const core = await loadCore(modulePath, wasmPath)

// Test 102: three exports and main, no fourth export, no export of the import, and a memory that is not shared.
for (const name of ['_main', '_ps_alloc', '_ps_free', '_ps_call']) {
  assert.equal(typeof core[name], 'function', `the module has the function ${name}`)
}
const bridgeExports = Object.getOwnPropertyNames(core).filter((name) => name.startsWith('_ps_'))
assert.deepEqual(bridgeExports.toSorted(), ['_ps_alloc', '_ps_call', '_ps_free'])
assert.equal('_partsmith_cancel_word' in core, false, 'the module does not export the import')
assert.ok(core.HEAPU8.buffer instanceof ArrayBuffer, 'the memory is an ArrayBuffer')
assert.equal(core.HEAPU8.buffer instanceof SharedArrayBuffer, false, 'the memory is not shared')

// Test 101: the word holds 7n before request 7 starts, so request 7 stops at its first check and request 8 does not.
const word = new BigInt64Array(new SharedArrayBuffer(8))
Atomics.store(word, 0, 7n)
useCancelWord(core, word)
const text = await readFile(referencePartPath, 'utf8')

/** The JSON header of a response frame, which may carry buffers (the module's session displays meshes). */
function decodeHeader(frame) {
  assert.equal(new TextDecoder().decode(frame.subarray(0, 4)), 'PSM1', 'the response magic')
  const headerLength = new DataView(frame.buffer, frame.byteOffset).getUint32(4, true)
  return JSON.parse(new TextDecoder('utf-8', { fatal: true }).decode(frame.subarray(8, 8 + headerLength)))
}

/** Sends request id with command through the module and returns the messages of its response frame. */
function send(id, command) {
  const header = decodeHeader(callCore(core, requestFrame(id, command)))
  const response = header.messages.at(-1)
  assert.equal(response.kind, 'response', 'a response frame ends with its response')
  assert.equal(response.id, id)
  assert.equal(response.result.kind, 'ok', JSON.stringify(response.result))
  assert.equal(response.result.type, 'open_document')
  return header.messages
}

/** The feature statuses of the recomputed event among messages. */
function statuses(messages) {
  const recomputed = messages.find((message) => message.kind === 'event' && message.type === 'recomputed')
  assert.ok(recomputed, 'the open emits recomputed')
  return recomputed.features.map((feature) => feature.status)
}

const stopped = statuses(send(7, { type: 'open_document', text, assets: [] }))
assert.ok(stopped.length > 0, 'the reference part has features')
assert.ok(
  stopped.every((status) => status === 'not_run'),
  `request 7 stops at its first check: ${stopped}`,
)

const finished = statuses(send(8, { type: 'open_document', text, assets: [] }))
assert.equal(finished.includes('not_run'), false, `request 8 is not the request the word holds: ${finished}`)
assert.equal(Atomics.load(word, 0), 7n, 'the module never writes the word')

console.log(JSON.stringify({ stopped: stopped.length, finished: finished.length }))
