// Loads the built web core module under Node and round-trips an engine_info request through
// ps_alloc, ps_call and ps_free, following the calling sequence of ui/src/core/bridge.ts
// (protocol.md section 2.10).
//
// Usage: node engine_info_round_trip.mjs <partsmith_core.mjs> <partsmith_core.wasm> <engine version> <format version>
//
// Prints the response frame header and exits with status 0 when the round trip matches the spec;
// otherwise an assertion fails and Node exits with a nonzero status.

import assert from 'node:assert/strict'

import { callCore, decodeHeaderOnlyFrame, frameWithoutBuffers, loadCore } from './core.mjs'

const [modulePath, wasmPath, expectedEngineVersion, expectedFormatVersion] = process.argv.slice(2)
assert.ok(
  modulePath && wasmPath && expectedEngineVersion && expectedFormatVersion,
  'usage: node engine_info_round_trip.mjs <partsmith_core.mjs> <partsmith_core.wasm> <engine version> <format version>',
)

// Example A of protocol.md section 2.2: request id 1, engine_info, no buffers, 120 bytes.
const requestHeader =
  '{"protocol_version":1,"messages":[{"kind":"request","id":1,"command":{"type":"engine_info"}}],"buffers":[]}'

const core = await loadCore(modulePath, wasmPath)

const request = frameWithoutBuffers(requestHeader)
assert.equal(request.length, 120, 'example A is 120 bytes')
const header = decodeHeaderOnlyFrame(callCore(core, request))

assert.equal(header.protocol_version, 1)
assert.deepEqual(header.buffers, [])
assert.equal(header.messages.length, 1, 'engine_info emits no events')
const [response] = header.messages
assert.equal(response.kind, 'response')
assert.equal(response.id, 1)
assert.equal(response.result.kind, 'ok')
assert.equal(response.result.type, 'engine_info')
assert.equal(response.result.protocol_version, 1)
assert.equal(response.result.engine_version, expectedEngineVersion)
assert.equal(response.result.format_version, Number(expectedFormatVersion))
assert.match(response.result.kernel, /^OCCT \d+\.\d+\.\d+$/)

console.log(JSON.stringify(header))
