import { describe, expect, it } from 'vitest'
import { createFakeCoreModule } from '../test/fakes/core-module'
import { readRequest, readResponse, requestFrame, responseFrame } from '../test/fakes/protocol'
import { callCore, CoreOutOfMemoryError } from './bridge'

function engineInfoModule() {
  return createFakeCoreModule((request) =>
    responseFrame({
      id: readRequest(request).id,
      result: {
        type: 'engine_info',
        protocol_version: 1,
        engine_version: '0.1.0',
        format_version: 1,
        kernel: 'OCCT 7.9.3',
      },
    }),
  )
}

describe('callCore', () => {
  it('protocol test 57: frees exactly the request block, the length slot and the response block, with their lengths', () => {
    const module = engineInfoModule()
    const request = requestFrame(1, { type: 'engine_info' })
    const requestLength = request.byteLength
    callCore(module, request)
    expect(module.allocations).toHaveLength(3)
    const lengths = module.allocations.map((block) => block.length)
    expect(lengths).toContain(requestLength)
    expect(lengths).toContain(4)
    expect(module.frees.toSorted((first, second) => first.pointer - second.pointer)).toEqual(
      module.allocations.toSorted((first, second) => first.pointer - second.pointer),
    )
    expect(module.live.size).toBe(0)
  })

  it('protocol test 57: returns a copy of the response bytes in a fresh ArrayBuffer of exactly their length', () => {
    const expected = responseFrame({
      id: 1,
      result: {
        type: 'engine_info',
        protocol_version: 1,
        engine_version: '0.1.0',
        format_version: 1,
        kernel: 'OCCT 7.9.3',
      },
    })
    const module = engineInfoModule()
    const response = callCore(module, requestFrame(1, { type: 'engine_info' }))
    expect(response).toBeInstanceOf(ArrayBuffer)
    expect(response.byteLength).toBe(expected.byteLength)
    expect(new Uint8Array(response)).toEqual(new Uint8Array(expected))
    expect(response).not.toBe(module.memory().buffer)
  })

  it('passes the request bytes to ps_call unchanged', () => {
    const module = engineInfoModule()
    const request = requestFrame(7, { type: 'engine_info' })
    const copy = new Uint8Array(request.slice(0))
    callCore(module, request)
    expect(module.requests).toHaveLength(1)
    expect(new Uint8Array(module.requests[0] ?? new ArrayBuffer(0))).toEqual(copy)
  })

  it('reads the response from the memory as it is after ps_call grew it', () => {
    const module = engineInfoModule()
    const before = module.memory().buffer
    const response = callCore(module, requestFrame(1, { type: 'engine_info' }))
    expect(module.memory().buffer).not.toBe(before)
    expect(readResponse(response).response.id).toBe(1)
  })

  it('throws CoreOutOfMemoryError when ps_alloc returns 0', () => {
    const module = engineInfoModule()
    module.failAllocation = true
    expect(() => callCore(module, requestFrame(1, { type: 'engine_info' }))).toThrow(
      CoreOutOfMemoryError,
    )
  })

  it('throws CoreOutOfMemoryError when ps_call returns 0', () => {
    const module = engineInfoModule()
    module.failResponse = true
    expect(() => callCore(module, requestFrame(1, { type: 'engine_info' }))).toThrow(
      CoreOutOfMemoryError,
    )
  })
})
