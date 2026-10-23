// Loads the built web core module under Node and round-trips new_document, then list_uses of the new
// document (engine.md section 3.24), through ps_alloc, ps_call and ps_free (protocol.md section 2.10).
// The protocol version and the engine version stay the same when a command is added, so this is what
// tells a module built before list_uses existed, which answers it with malformed_request, from a current one.
//
// Usage: node list_uses_round_trip.mjs <partsmith_core.mjs> <partsmith_core.wasm>
//
// Prints the list_uses response and exits with status 0 when the round trip matches the spec;
// otherwise an assertion fails and Node exits with a nonzero status.

import assert from 'node:assert/strict'

import { callCore, decodeFrameWithoutBuffers, loadCore, requestFrame } from './core.mjs'

const [modulePath, wasmPath] = process.argv.slice(2)
assert.ok(modulePath && wasmPath, 'usage: node list_uses_round_trip.mjs <partsmith_core.mjs> <partsmith_core.wasm>')

const core = await loadCore(modulePath, wasmPath)

/** Sends request id with command through the module and returns its response, the last message of the response frame. */
function send(id, command) {
  const header = decodeFrameWithoutBuffers(callCore(core, requestFrame(id, command)))
  assert.equal(header.protocol_version, 1)
  const response = header.messages.at(-1)
  assert.equal(response.kind, 'response', 'a response frame ends with its response')
  assert.equal(response.id, id)
  return response
}

const created = send(1, { type: 'new_document' })
assert.equal(created.result.kind, 'ok', JSON.stringify(created))
assert.equal(created.result.type, 'new_document')

const listed = send(2, { type: 'list_uses', doc: created.result.doc })
assert.deepEqual(listed.result, { kind: 'ok', type: 'list_uses', uses: [] }, JSON.stringify(listed))

console.log(JSON.stringify(listed))
