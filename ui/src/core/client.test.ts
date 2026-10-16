import { afterEach, describe, expect, it, vi } from 'vitest'
import type { Command, Event as ProtocolEvent, FixAction, Request } from '../generated/protocol'
import { emptyDocument } from '../test/fakes/documents'
import { readRequest, responseFrame, type ResponseBody } from '../test/fakes/protocol'
import { CoreError, createCoreClient } from './client'
import type { CoreEvent } from './events'
import { decodeFrame } from './frame'
import { CoreFatalError, type Transport } from './transport'

type ScriptedTransport = Transport & {
  /** Every request frame sent, in order. */
  readonly frames: ArrayBuffer[]
  /** Simulates the core dying outside the caller's control. */
  die(message: string): void
}

/** A transport that answers each request frame with `answer(request)`, encoded as a response frame. */
function scriptedTransport(
  answer: (request: Request) => ArrayBuffer | ResponseBody,
): ScriptedTransport {
  const frames: ArrayBuffer[] = []
  const fatalListeners = new Set<(error: CoreFatalError) => void>()
  return {
    kind: 'worker',
    frames,
    start: async () => ({ protocolVersion: 1, engineVersion: '0.1.0' }),
    async send(frame) {
      frames.push(frame)
      const request = readRequest(frame)
      const answered = answer(request)
      return answered instanceof ArrayBuffer
        ? answered
        : responseFrame({ ...answered, id: request.id })
    },
    stop: () => undefined,
    onFatal(listener) {
      fatalListeners.add(listener)
      return () => {
        fatalListeners.delete(listener)
      }
    },
    die(message) {
      for (const listener of fatalListeners) {
        listener(new CoreFatalError(message))
      }
    },
  }
}

const UNDO: Command = { type: 'undo', doc: 1 }

function undoAnswer(): ResponseBody {
  return { result: { type: 'undo', revision: 6, label: 'Change height' } }
}

function documentChanged(revision: number): ProtocolEvent {
  return {
    type: 'document_changed',
    doc: 1,
    revision,
    document: emptyDocument(),
    dirty: true,
    undo: 'Change height',
    transaction_open: false,
  }
}

function recomputed(revision: number): ProtocolEvent {
  return { type: 'recomputed', doc: 1, revision, features: [], parameters: [], diagnostics: [] }
}

async function rejectionOf(promise: Promise<unknown>): Promise<unknown> {
  try {
    await promise
  } catch (error) {
    return error
  }
  throw new Error('The promise resolved.')
}

afterEach(() => {
  vi.restoreAllMocks()
})

describe('createCoreClient', () => {
  it('U-10 numbers requests 1, 2, 3 and encodes the first frame header exactly', async () => {
    const transport = scriptedTransport(undoAnswer)
    const client = createCoreClient(transport)
    await client.request(UNDO)
    await client.request(UNDO)
    await client.request(UNDO)
    expect(transport.frames.map((frame) => readRequest(frame).id)).toEqual([1, 2, 3])
    const first = transport.frames[0]
    expect(first).toBeDefined()
    expect(decodeFrame(first ?? new ArrayBuffer(0)).header).toEqual({
      protocol_version: 1,
      messages: [{ kind: 'request', id: 1, command: UNDO }],
      buffers: [],
    })
  })

  it('resolves an ok outcome with the result and the frame buffers', async () => {
    const transport = scriptedTransport(() => ({
      result: {
        type: 'export',
        files: [{ name: 'model.stl', media_type: 'model/stl', data: 0 }],
        warnings: [],
      },
      buffers: [Uint8Array.of(1, 2, 3)],
    }))
    const client = createCoreClient(transport)
    const reply = await client.request({
      type: 'export',
      doc: 1,
      format: { kind: 'stl', binary: true, per_body: false },
    })
    expect(reply.result).toEqual({
      type: 'export',
      files: [{ name: 'model.stl', media_type: 'model/stl', data: 0 }],
      warnings: [],
    })
    expect(reply.buffers).toHaveLength(1)
    expect(Array.from(reply.buffers[0] ?? [])).toEqual([1, 2, 3])
  })

  it('U-11 dispatches the events of the response frame, in order, before the request settles', async () => {
    const transport = scriptedTransport(() => ({
      ...undoAnswer(),
      events: [documentChanged(6), recomputed(6)],
    }))
    const client = createCoreClient(transport)
    const log: string[] = []
    const seen: CoreEvent[] = []
    client.onEvent((event) => {
      log.push(`event:${event.type}`)
      seen.push(event)
    })
    await client.request(UNDO).then(() => {
      log.push('resolved')
    })
    expect(log).toEqual(['event:document_changed', 'event:recomputed', 'resolved'])
    expect(seen[0]).toMatchObject({
      type: 'document_changed',
      documentHandle: 1,
      revision: 6,
      dirty: true,
      undo: 'Change height',
      redo: null,
      transactionOpen: false,
      transaction: null,
      focus: null,
    })
    expect(seen[1]).toMatchObject({ type: 'recomputed', documentHandle: 1, revision: 6 })
  })

  it('dispatches the events of an error response before rejecting', async () => {
    const transport = scriptedTransport(() => ({
      error: { code: 'nothing_to_undo', message: 'There is nothing to undo.' },
      events: [documentChanged(3)],
    }))
    const client = createCoreClient(transport)
    const log: string[] = []
    client.onEvent((event) => {
      log.push(`event:${event.type}`)
    })
    await client.request(UNDO).catch(() => {
      log.push('rejected')
    })
    expect(log).toEqual(['event:document_changed', 'rejected'])
  })

  it('U-12 an error outcome rejects with CoreError carrying the code, the message verbatim and the edit index', async () => {
    const transport = scriptedTransport(() => ({
      error: { code: 'edit_rejected', message: 'There is no feature with id f-x.', edit_index: 1 },
    }))
    const client = createCoreClient(transport)
    const error = await rejectionOf(client.request(UNDO))
    expect(error).toBeInstanceOf(CoreError)
    if (!(error instanceof CoreError)) {
      return
    }
    expect(error.code).toBe('edit_rejected')
    expect(error.message).toBe('There is no feature with id f-x.')
    expect(error.editIndex).toBe(1)
    expect(error.span).toBeNull()
    expect(error.detail).toBeNull()
    expect(error.actions).toEqual([])
  })

  it('U-147 an error with a detail and actions rejects with a CoreError carrying both', async () => {
    const finish: FixAction = {
      label: 'Finish "Fillet" and save',
      run: { kind: 'finish_transaction' },
    }
    const transport = scriptedTransport(() => ({
      error: {
        code: 'transaction_open',
        message: 'Finish or cancel "Fillet" first.',
        detail: 'boom',
        actions: [finish],
      },
    }))
    const client = createCoreClient(transport)
    const error = await rejectionOf(client.request(UNDO))
    expect(error).toBeInstanceOf(CoreError)
    if (!(error instanceof CoreError)) {
      return
    }
    expect(error.detail).toBe('boom')
    expect(error.actions).toEqual([finish])
  })

  it('a CoreError without an edit index has editIndex null, and carries an expression error span', async () => {
    const transport = scriptedTransport(() => ({
      error: {
        code: 'expression_error',
        message: "Unknown name 'wal'.",
        span: { start: 4, end: 7 },
      },
    }))
    const client = createCoreClient(transport)
    const error = await rejectionOf(client.request({ type: 'evaluate', doc: 1, expr: '2 * wal' }))
    expect(error).toBeInstanceOf(CoreError)
    if (!(error instanceof CoreError)) {
      return
    }
    expect(error.editIndex).toBeNull()
    expect(error.span).toEqual({ start: 4, end: 7 })
  })

  it('U-13 a response with another id rejects with CoreFatalError and notifies the fatal listeners', async () => {
    const transport = scriptedTransport(() =>
      responseFrame({ id: 99, result: { type: 'undo', revision: 1, label: 'Edit' } }),
    )
    const client = createCoreClient(transport)
    const fatal = vi.fn<(error: CoreFatalError) => void>()
    client.onFatal(fatal)
    const error = await rejectionOf(client.request(UNDO))
    expect(error).toBeInstanceOf(CoreFatalError)
    expect(error instanceof Error ? error.message : '').toMatch(
      /^The geometry core sent a response the app cannot read: /,
    )
    expect(fatal).toHaveBeenCalledTimes(1)
    expect(fatal.mock.calls[0]?.[0]).toBeInstanceOf(CoreFatalError)
  })

  it('a frame that fails to decode rejects with CoreFatalError and notifies the fatal listeners', async () => {
    const transport = scriptedTransport(() => Uint8Array.of(0x50, 0x53, 0x4d).buffer)
    const client = createCoreClient(transport)
    const fatal = vi.fn<(error: CoreFatalError) => void>()
    client.onFatal(fatal)
    const error = await rejectionOf(client.request(UNDO))
    expect(error).toBeInstanceOf(CoreFatalError)
    expect(error instanceof Error ? error.message : '').toMatch(
      /^The geometry core sent a response the app cannot read: /,
    )
    expect(fatal).toHaveBeenCalledTimes(1)
  })

  it('passes a transport fatal error to the fatal listeners', () => {
    const transport = scriptedTransport(undoAnswer)
    const client = createCoreClient(transport)
    const fatal = vi.fn<(error: CoreFatalError) => void>()
    client.onFatal(fatal)
    transport.die('The geometry core stopped responding.')
    expect(fatal).toHaveBeenCalledTimes(1)
    expect(fatal.mock.calls[0]?.[0].message).toBe('The geometry core stopped responding.')
  })

  it('U-14 ignores an event of unknown type and keeps dispatching the others', async () => {
    const unknown = { type: 'future_event', doc: 1, revision: 6 } as unknown as ProtocolEvent
    const transport = scriptedTransport(() => ({
      ...undoAnswer(),
      events: [unknown, recomputed(6)],
    }))
    const client = createCoreClient(transport)
    const types: string[] = []
    client.onEvent((event) => {
      types.push(event.type)
    })
    await expect(client.request(UNDO)).resolves.toMatchObject({
      result: { type: 'undo', revision: 6, label: 'Change height' },
    })
    expect(types).toEqual(['recomputed'])
  })

  it('U-14 a listener that throws is reported with console.error and stops neither the next listener nor the request', async () => {
    const consoleError = vi.spyOn(console, 'error').mockImplementation(() => undefined)
    const transport = scriptedTransport(() => ({ ...undoAnswer(), events: [recomputed(6)] }))
    const client = createCoreClient(transport)
    const next = vi.fn<(event: CoreEvent) => void>()
    client.onEvent(() => {
      throw new Error('listener bug')
    })
    client.onEvent(next)
    await expect(client.request(UNDO)).resolves.toBeDefined()
    expect(next).toHaveBeenCalledTimes(1)
    expect(consoleError).toHaveBeenCalled()
  })

  it('an unsubscribed event listener hears nothing', async () => {
    const transport = scriptedTransport(() => ({ ...undoAnswer(), events: [recomputed(6)] }))
    const client = createCoreClient(transport)
    const listener = vi.fn<(event: CoreEvent) => void>()
    client.onEvent(listener)()
    await client.request(UNDO)
    expect(listener).not.toHaveBeenCalled()
  })
})
