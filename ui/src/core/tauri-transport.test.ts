import { describe, expect, it, vi } from 'vitest'
import { PROTOCOL_VERSION, type Request } from '../generated/protocol'
import { readRequest, readResponse, requestFrame, responseFrame } from '../test/fakes/protocol'
import { settle } from '../test/fakes/settle'
import { createTauriTransport, type TauriGlobal } from './tauri-transport'
import { CoreFatalError } from './transport'

/** One `invoke` call, answered later by the test. */
type Invocation = {
  readonly command: string
  readonly request: Request
  answer(frame: ArrayBuffer): void
  fail(error: Error): void
}

type FakeTauri = { readonly tauri: TauriGlobal; readonly invocations: Invocation[] }

function engineInfoResult(protocolVersion: number): ArrayBuffer {
  return responseFrame({
    id: 0,
    result: {
      type: 'engine_info',
      protocol_version: protocolVersion,
      engine_version: '0.1.0',
      format_version: 1,
      kernel: 'OCCT 7.9.3',
    },
  })
}

/** A Tauri global whose `invoke` answers engine_info at once (with `protocolVersion`) and holds every other call. */
function fakeTauri(protocolVersion: number = PROTOCOL_VERSION): FakeTauri {
  const invocations: Invocation[] = []
  const tauri: TauriGlobal = {
    core: {
      invoke: (command, payload) =>
        new Promise<ArrayBuffer>((resolve, reject) => {
          const request = readRequest(payload)
          invocations.push({ command, request, answer: resolve, fail: reject })
          if (request.command.type === 'engine_info' && request.id === 0) {
            resolve(engineInfoResult(protocolVersion))
          }
        }),
    },
  }
  return { tauri, invocations }
}

function drag(id: number): ArrayBuffer {
  return requestFrame(id, {
    type: 'drag_sketch',
    doc: 1,
    feature: 'f-sk1',
    point: 'e-1.end',
    to: [20, 5],
  })
}

function dragAnswer(id: number): ArrayBuffer {
  return responseFrame({
    id,
    result: {
      type: 'drag_sketch',
      solution: {
        feature: 'f-sk1',
        status: 'under_constrained',
        failed: false,
        dof: 1,
        redundant: [],
        conflicting: [],
        placement: { origin: [0, 0, 0], x_axis: [1, 0, 0], normal: [0, 0, 1] },
        entities: [],
        points: [],
        constraints: [],
        regions: [],
        profile_findings: [],
      },
    },
  })
}

describe('createTauriTransport', () => {
  it('has kind tauri', () => {
    expect(createTauriTransport(fakeTauri().tauri).kind).toBe('tauri')
  })

  it('U-6 start() invokes call with an engine_info request frame of id 0 and resolves the versions', async () => {
    const { tauri, invocations } = fakeTauri()
    const transport = createTauriTransport(tauri)
    await expect(transport.start()).resolves.toEqual({
      protocolVersion: PROTOCOL_VERSION,
      engineVersion: '0.1.0',
    })
    expect(invocations).toHaveLength(1)
    expect(invocations[0]?.command).toBe('call')
    expect(invocations[0]?.request).toEqual({ id: 0, command: { type: 'engine_info' } })
  })

  it('U-6 a core reporting another protocol version rejects start() with the reinstall message', async () => {
    const transport = createTauriTransport(fakeTauri(PROTOCOL_VERSION + 1).tauri)
    const error = await transport.start().then(
      () => null,
      (rejection: unknown) => rejection,
    )
    expect(error).toBeInstanceOf(CoreFatalError)
    expect(error instanceof Error ? error.message : '').toBe(
      'This app and its core speak different protocol versions. Reinstall Partsmith.',
    )
  })

  it('send() invokes call with the request frame and resolves with the returned frame', async () => {
    const { tauri, invocations } = fakeTauri()
    const transport = createTauriTransport(tauri)
    await transport.start()
    const reply = transport.send(drag(1))
    await settle()
    expect(invocations.map((invocation) => invocation.request.id)).toEqual([0, 1])
    expect(invocations[1]?.command).toBe('call')
    const answer = dragAnswer(1)
    invocations[1]?.answer(answer)
    await expect(reply).resolves.toBe(answer)
  })

  it('U-7 of three drags of f-sk1 sent while the first is in flight, only the first and third reach invoke; the second is superseded', async () => {
    const { tauri, invocations } = fakeTauri()
    const transport = createTauriTransport(tauri)
    await transport.start()
    const first = transport.send(drag(1))
    await settle()
    const second = transport.send(drag(2))
    const third = transport.send(drag(3))
    await settle()
    expect(invocations.map((invocation) => invocation.request.id)).toEqual([0, 1])
    invocations[1]?.answer(dragAnswer(1))
    await expect(first).resolves.toBeInstanceOf(ArrayBuffer)
    const superseded = readResponse(await second).response
    expect(superseded.id).toBe(2)
    expect(superseded.result).toEqual({
      kind: 'error',
      code: 'superseded',
      message: 'A newer request replaced this one.',
    })
    await settle()
    expect(invocations.map((invocation) => invocation.request.id)).toEqual([0, 1, 3])
    invocations[2]?.answer(dragAnswer(3))
    expect(readResponse(await third).response.id).toBe(3)
  })

  it('runs one invoke at a time, in send order', async () => {
    const { tauri, invocations } = fakeTauri()
    const transport = createTauriTransport(tauri)
    await transport.start()
    const first = transport.send(requestFrame(1, { type: 'undo', doc: 1 }))
    const second = transport.send(requestFrame(2, { type: 'redo', doc: 1 }))
    await settle()
    expect(invocations.map((invocation) => invocation.request.id)).toEqual([0, 1])
    invocations[1]?.answer(responseFrame({ id: 1, result: { type: 'undo', revision: 1, label: 'Edit' } }))
    await first
    await settle()
    expect(invocations.map((invocation) => invocation.request.id)).toEqual([0, 1, 2])
    invocations[2]?.answer(responseFrame({ id: 2, result: { type: 'redo', revision: 2, label: 'Edit' } }))
    expect(readResponse(await second).response.id).toBe(2)
  })

  it('U-8 an invoke rejection resolves the send with an internal_error response frame for that request id', async () => {
    const { tauri, invocations } = fakeTauri()
    const transport = createTauriTransport(tauri)
    const fatal = vi.fn<(error: CoreFatalError) => void>()
    transport.onFatal(fatal)
    await transport.start()
    const reply = transport.send(requestFrame(7, { type: 'undo', doc: 1 }))
    await settle()
    invocations[1]?.fail(new Error('boom'))
    const { response, events } = readResponse(await reply)
    expect(events).toEqual([])
    expect(response.id).toBe(7)
    expect(response.result.kind).toBe('error')
    if (response.result.kind === 'error') {
      expect(response.result.code).toBe('internal_error')
      expect(response.result.message).toContain('boom')
    }
    expect(fatal).not.toHaveBeenCalled()
  })

  it('keeps answering after an invoke rejection', async () => {
    const { tauri, invocations } = fakeTauri()
    const transport = createTauriTransport(tauri)
    await transport.start()
    const failed = transport.send(requestFrame(1, { type: 'undo', doc: 1 }))
    const next = transport.send(requestFrame(2, { type: 'undo', doc: 1 }))
    await settle()
    invocations[1]?.fail(new Error('boom'))
    await failed
    await settle()
    invocations[2]?.answer(responseFrame({ id: 2, result: { type: 'undo', revision: 2, label: 'Edit' } }))
    expect(readResponse(await next).response.id).toBe(2)
  })
})
