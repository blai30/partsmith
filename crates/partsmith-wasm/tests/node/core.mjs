// Helpers of the Node scripts that run the built web core module, following the calling sequence of
// ui/src/core/bridge.ts (protocol.md section 2.10).

import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import { pathToFileURL } from 'node:url'

/** Instantiates the module factory at modulePath with the bytes of wasmPath and checks the exports of the bridge. */
export async function loadCore(modulePath, wasmPath) {
  const { default: createPartsmithCore } = await import(pathToFileURL(modulePath).href)
  // The worker build fetches the wasm next to the module; Node's fetch cannot read file URLs,
  // so the bytes are handed to the factory directly.
  const core = await createPartsmithCore({ wasmBinary: await readFile(wasmPath) })
  for (const name of ['_ps_alloc', '_ps_free', '_ps_call']) {
    assert.equal(typeof core[name], 'function', `the module exports ${name}`)
  }
  assert.ok(core.HEAPU8 instanceof Uint8Array, 'the module exports HEAPU8')
  return core
}

/** Encodes a frame without buffers: magic, little-endian header length, header, zero padding to 8. */
export function frameWithoutBuffers(header) {
  const headerBytes = new TextEncoder().encode(header)
  const padding = (8 - (headerBytes.length % 8)) % 8
  const frame = new Uint8Array(8 + headerBytes.length + padding)
  frame.set(new TextEncoder().encode('PSM1'), 0)
  new DataView(frame.buffer).setUint32(4, headerBytes.length, true)
  frame.set(headerBytes, 8)
  return frame
}

/** Encodes a protocol version 1 frame holding one request with id and command, without buffers. */
export function requestFrame(id, command) {
  return frameWithoutBuffers(
    JSON.stringify({ protocol_version: 1, messages: [{ kind: 'request', id, command }], buffers: [] }),
  )
}

/** Copies a request frame into wasm memory, calls ps_call, copies the response out and frees all three blocks. */
export function callCore(core, request) {
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
export function decodeHeaderOnlyFrame(frame) {
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
