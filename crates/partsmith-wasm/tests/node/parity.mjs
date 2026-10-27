// engine.md test 154: passes the request frames that crates/partsmith-wasm/tests/web_parity.rs wrote through the
// built web core module and writes its response frames back, following the calling sequence of ui/src/core/bridge.ts
// (protocol.md section 2.10). The Rust test compares them with the native session's.
//
// Usage: node parity.mjs <partsmith_core.mjs> <partsmith_core.wasm> <request file> <response file>
//
// Both files hold frames, each after its length as a little-endian u32. The script appends each response frame to the
// response file as soon as it has it, so a trap leaves the frames before it; it exits with a nonzero status when the
// module traps or a check of core.mjs fails.

import assert from 'node:assert/strict'
import { appendFileSync, readFileSync } from 'node:fs'

import { callCore, loadCore } from './core.mjs'

const [modulePath, wasmPath, requestPath, responsePath] = process.argv.slice(2)
assert.ok(
  modulePath && wasmPath && requestPath && responsePath,
  'usage: node parity.mjs <partsmith_core.mjs> <partsmith_core.wasm> <request file> <response file>',
)

/** The frames of a file of length-prefixed frames, in order. */
function readFrames(bytes) {
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength)
  const frames = []
  let offset = 0
  while (offset < bytes.length) {
    assert.ok(
      offset + 4 <= bytes.length,
      `the request file ends inside the length at byte ${offset}`,
    )
    const start = offset + 4
    const end = start + view.getUint32(offset, true)
    assert.ok(end <= bytes.length, `the request file ends inside the frame at byte ${start}`)
    frames.push(bytes.subarray(start, end))
    offset = end
  }
  return frames
}

/** A frame after its length as a little-endian u32, in one block, so that one write appends both. */
function lengthPrefixed(frame) {
  const record = new Uint8Array(4 + frame.length)
  new DataView(record.buffer).setUint32(0, frame.length, true)
  record.set(frame, 4)
  return record
}

const requests = readFrames(readFileSync(requestPath))
const core = await loadCore(modulePath, wasmPath)
for (const request of requests) {
  appendFileSync(responsePath, lengthPrefixed(callCore(core, request)))
}
