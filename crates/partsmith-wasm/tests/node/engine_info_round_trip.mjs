// Loads the built web core module under Node and round-trips an engine_info request through
// ps_alloc, ps_call and ps_free, following the calling sequence of ui/src/core/bridge.ts
// (protocol.md section 2.10).
//
// Usage: node engine_info_round_trip.mjs <partsmith_core.mjs> <partsmith_core.wasm> <engine version>
//
// Prints the response frame header and exits with status 0 when the round trip matches the spec;
// otherwise an assertion fails and Node exits with a nonzero status.

import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import { pathToFileURL } from 'node:url'

const [modulePath, wasmPath, expectedEngineVersion] = process.argv.slice(2)
assert.ok(
  modulePath && wasmPath && expectedEngineVersion,
  'usage: node engine_info_round_trip.mjs <partsmith_core.mjs> <partsmith_core.wasm> <engine version>',
)

// Example A of protocol.md section 2.2: request id 1, engine_info, no buffers, 120 bytes.
const requestHeader =
  '{"protocol_version":1,"messages":[{"kind":"request","id":1,"command":{"type":"engine_info"}}],"buffers":[]}'

/** Encodes a frame without buffers: magic, little-endian header length, header, zero padding to 8. */
function frameWithoutBuffers(header) {
  const headerBytes = new TextEncoder().encode(header)
  const padding = (8 - (headerBytes.length % 8)) % 8
  const frame = new Uint8Array(8 + headerBytes.length + padding)
  frame.set(new TextEncoder().encode('PSM1'), 0)
  new DataView(frame.buffer).setUint32(4, headerBytes.length, true)
  frame.set(headerBytes, 8)
  return frame
}

/** Copies a request frame into wasm memory, calls ps_call, copies the response out and frees all three blocks. */
function callCore(core, request) {
  // Pointers come back as signed 32-bit integers; addresses above 2 GiB need the unsigned reading.
  const requestPointer = core._ps_alloc(request.length) >>> 0
  assert.notEqual(requestPointer, 0, 'ps_alloc returned 0 for the request block')
  const lengthSlot = core._ps_alloc(4) >>> 0
  assert.notEqual(lengthSlot, 0, 'ps_alloc returned 0 for the length slot')
  core.HEAPU8.set(request, requestPointer)
  const responsePointer = core._ps_call(requestPointer, request.length, lengthSlot) >>> 0
  assert.notEqual(responsePointer, 0, 'ps_call returned 0')
  // Memory growth replaces the ArrayBuffer, so HEAPU8 is read again after the call.
  const memory = core.HEAPU8
  const responseLength = new DataView(memory.buffer).getUint32(lengthSlot, true)
  const response = memory.slice(responsePointer, responsePointer + responseLength)
  core._ps_free(requestPointer, request.length)
  core._ps_free(lengthSlot, 4)
  core._ps_free(responsePointer, responseLength)
  return response
}

/** Decodes the header of a frame without buffers and checks its layout. */
function decodeHeaderOnlyFrame(frame) {
  assert.ok(frame.length >= 8, `the response frame is ${frame.length} bytes, shorter than 8`)
  assert.equal(new TextDecoder().decode(frame.subarray(0, 4)), 'PSM1', 'the response magic')
  const headerLength = new DataView(frame.buffer, frame.byteOffset).getUint32(4, true)
  const padding = (8 - (headerLength % 8)) % 8
  assert.equal(frame.length, 8 + headerLength + padding, 'a frame without buffers ends after its padding')
  for (const byte of frame.subarray(8 + headerLength)) {
    assert.equal(byte, 0, 'padding bytes are zero')
  }
  return JSON.parse(new TextDecoder('utf-8', { fatal: true }).decode(frame.subarray(8, 8 + headerLength)))
}

const { default: createPartsmithCore } = await import(pathToFileURL(modulePath).href)
// The worker build fetches the wasm next to the module; Node's fetch cannot read file URLs,
// so the bytes are handed to the factory directly.
const core = await createPartsmithCore({ wasmBinary: await readFile(wasmPath) })
for (const name of ['_ps_alloc', '_ps_free', '_ps_call']) {
  assert.equal(typeof core[name], 'function', `the module exports ${name}`)
}
assert.ok(core.HEAPU8 instanceof Uint8Array, 'the module exports HEAPU8')

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
assert.equal(response.result.format_version, 1)
assert.match(response.result.kernel, /^OCCT \d+\.\d+\.\d+$/)

console.log(JSON.stringify(header))
