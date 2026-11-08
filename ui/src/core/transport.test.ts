import { afterEach, describe, expect, it, vi } from 'vitest'
import { readResponse, requestFrame, responseFrame } from '../test/fakes/protocol'
import { settle } from '../test/fakes/settle'
import { FakeWorker } from '../test/fakes/worker'
import type { TauriGlobal } from './tauri-transport'
import { chooseTransport, CoreFatalError, createWorkerTransport, type Transport } from './transport'

type WorkerSetup = { readonly transport: Transport; readonly worker: FakeWorker }

function setUp(options: { readonly startTimeoutMilliseconds?: number } = {}): WorkerSetup {
  const worker = new FakeWorker()
  const transport = createWorkerTransport({
    createWorker: () => worker as unknown as Worker,
    ...options,
  })
  return { transport, worker }
}

async function started(): Promise<WorkerSetup> {
  const setup = setUp()
  const ready = setup.transport.start()
  await settle()
  setup.worker.emit({ type: 'ready', protocolVersion: 1, engineVersion: '0.1.0' })
  await ready
  return setup
}

function engineInfoFrame(id: number): ArrayBuffer {
  return requestFrame(id, { type: 'engine_info' })
}

function undoResponse(id: number): ArrayBuffer {
  return responseFrame({ id, result: { type: 'undo', revision: id, label: 'Edit' } })
}

/** Tracks a promise's state without awaiting it. */
function track<Value>(promise: Promise<Value>): {
  readonly state: () => 'pending' | 'resolved' | 'rejected'
} {
  let state: 'pending' | 'resolved' | 'rejected' = 'pending'
  promise.then(
    () => {
      state = 'resolved'
    },
    () => {
      state = 'rejected'
    },
  )
  return { state: () => state }
}

afterEach(() => {
  vi.useRealTimers()
  vi.unstubAllGlobals()
})

describe('createWorkerTransport', () => {
  it('has kind worker', () => {
    expect(setUp().transport.kind).toBe('worker')
  })

  it("U-1 start() resolves with the versions of the worker's ready message", async () => {
    const { transport, worker } = setUp()
    const ready = transport.start()
    await settle()
    worker.emit({ type: 'ready', protocolVersion: 1, engineVersion: '0.1.0' })
    await expect(ready).resolves.toEqual({ protocolVersion: 1, engineVersion: '0.1.0' })
  })

  it('U-2 a fatal message before ready rejects start() with CoreFatalError carrying the message', async () => {
    const { transport, worker } = setUp()
    const ready = transport.start()
    await settle()
    worker.emit({ type: 'fatal', message: 'HTTP 404' })
    const error = await ready.then(
      () => null,
      (rejection: unknown) => rejection,
    )
    expect(error).toBeInstanceOf(CoreFatalError)
    expect(error instanceof Error ? error.message : '').toContain('HTTP 404')
  })

  it('a worker error event before ready rejects start() with CoreFatalError', async () => {
    const { transport, worker } = setUp()
    const ready = transport.start()
    await settle()
    worker.fail('The worker script could not be loaded.')
    await expect(ready).rejects.toBeInstanceOf(CoreFatalError)
  })

  it('U-3 without a ready message start() rejects after exactly 30,000 ms', async () => {
    vi.useFakeTimers()
    const { transport } = setUp()
    const ready = transport.start()
    const outcome = track(ready)
    ready.catch(() => undefined)
    await vi.advanceTimersByTimeAsync(29_999)
    await settle()
    expect(outcome.state()).toBe('pending')
    await vi.advanceTimersByTimeAsync(1)
    await settle()
    expect(outcome.state()).toBe('rejected')
    await expect(ready).rejects.toThrow(
      new CoreFatalError('The geometry core did not start within 30 seconds.'),
    )
    await expect(ready).rejects.toBeInstanceOf(CoreFatalError)
  })

  it('names a custom start timeout in seconds', async () => {
    vi.useFakeTimers()
    const { transport } = setUp({ startTimeoutMilliseconds: 5_000 })
    const ready = transport.start()
    ready.catch(() => undefined)
    await vi.advanceTimersByTimeAsync(5_000)
    await expect(ready).rejects.toThrow('The geometry core did not start within 5 seconds.')
  })

  it('U-4 sends post call messages that transfer their frames, and frame replies resolve them in order', async () => {
    const { transport, worker } = await started()
    const first = engineInfoFrame(1)
    const second = engineInfoFrame(2)
    const firstReply = transport.send(first, 1)
    const secondReply = transport.send(second, 2)
    expect(worker.posted).toEqual([
      { message: { type: 'call', frame: first }, transfer: [first] },
      { message: { type: 'call', frame: second }, transfer: [second] },
    ])
    const firstResponse = undoResponse(1)
    const secondResponse = undoResponse(2)
    worker.emit({ type: 'frame', frame: firstResponse })
    worker.emit({ type: 'frame', frame: secondResponse })
    await expect(firstReply).resolves.toBe(firstResponse)
    await expect(secondReply).resolves.toBe(secondResponse)
  })

  it('resolves the oldest pending send with each frame reply', async () => {
    const { transport, worker } = await started()
    const firstReply = transport.send(engineInfoFrame(1), 1)
    const firstTracked = track(firstReply)
    const secondReply = transport.send(engineInfoFrame(2), 2)
    const secondTracked = track(secondReply)
    worker.emit({ type: 'frame', frame: undoResponse(1) })
    await settle()
    expect(firstTracked.state()).toBe('resolved')
    expect(secondTracked.state()).toBe('pending')
    worker.emit({ type: 'frame', frame: undoResponse(2) })
    await expect(secondReply).resolves.toBeInstanceOf(ArrayBuffer)
  })

  it('U-5 a worker error after start rejects every pending send, reports fatal exactly once, and later sends reject at once', async () => {
    const { transport, worker } = await started()
    const fatal = vi.fn<(error: CoreFatalError) => void>()
    transport.onFatal(fatal)
    const firstReply = transport.send(engineInfoFrame(1), 1)
    const secondReply = transport.send(engineInfoFrame(2), 2)
    worker.fail('RuntimeError: unreachable')
    worker.fail('RuntimeError: unreachable')
    await expect(firstReply).rejects.toBeInstanceOf(CoreFatalError)
    await expect(secondReply).rejects.toBeInstanceOf(CoreFatalError)
    expect(fatal).toHaveBeenCalledTimes(1)
    expect(fatal.mock.calls[0]?.[0]).toBeInstanceOf(CoreFatalError)
    const later = transport.send(engineInfoFrame(3), 3)
    await expect(later).rejects.toBeInstanceOf(CoreFatalError)
    expect(worker.posted).toHaveLength(2)
  })

  it('a fatal message after start rejects pending sends with its message', async () => {
    const { transport, worker } = await started()
    const fatal = vi.fn<(error: CoreFatalError) => void>()
    transport.onFatal(fatal)
    const reply = transport.send(engineInfoFrame(1), 1)
    worker.emit({ type: 'fatal', message: 'The core ran out of memory.' })
    await expect(reply).rejects.toThrow('The core ran out of memory.')
    expect(fatal).toHaveBeenCalledTimes(1)
  })

  it('a messageerror event after start is fatal', async () => {
    const { transport, worker } = await started()
    const fatal = vi.fn<(error: CoreFatalError) => void>()
    transport.onFatal(fatal)
    const reply = transport.send(engineInfoFrame(1), 1)
    worker.failMessage()
    await expect(reply).rejects.toBeInstanceOf(CoreFatalError)
    expect(fatal).toHaveBeenCalledTimes(1)
  })

  it('an unsubscribed fatal listener hears nothing', async () => {
    const { transport, worker } = await started()
    const fatal = vi.fn<(error: CoreFatalError) => void>()
    transport.onFatal(fatal)()
    worker.fail('boom')
    await settle()
    expect(fatal).not.toHaveBeenCalled()
  })

  it('stop() terminates the worker and rejects pending and later sends with "The geometry core was stopped."', async () => {
    const { transport, worker } = await started()
    const fatal = vi.fn<(error: CoreFatalError) => void>()
    transport.onFatal(fatal)
    const pending = transport.send(engineInfoFrame(1), 1)
    transport.stop()
    expect(worker.terminated).toBe(true)
    await expect(pending).rejects.toThrow(new CoreFatalError('The geometry core was stopped.'))
    await expect(transport.send(engineInfoFrame(2), 2)).rejects.toBeInstanceOf(CoreFatalError)
    expect(fatal).not.toHaveBeenCalled()
  })

  it('decodes nothing itself: the reply frame is handed back as received', async () => {
    const { transport, worker } = await started()
    const reply = transport.send(engineInfoFrame(4), 4)
    const response = undoResponse(4)
    worker.emit({ type: 'frame', frame: response })
    const received = await reply
    expect(readResponse(received).response.id).toBe(4)
  })
})

/** Starts `transport` on `worker`, which reports ready. */
async function startOn(transport: Transport, worker: FakeWorker): Promise<void> {
  const ready = transport.start()
  await settle()
  worker.emit({ type: 'ready', protocolVersion: 1, engineVersion: '0.1.0' })
  await ready
}

/** The cancel word the transport posted to `worker` (protocol.md 2.11 rule 8), or null. */
function postedWord(worker: FakeWorker): SharedArrayBuffer | null {
  for (const { message } of worker.posted) {
    if (
      typeof message === 'object' &&
      message !== null &&
      Reflect.get(message, 'type') === 'cancel_word'
    ) {
      const word: unknown = Reflect.get(message, 'word')
      return word instanceof SharedArrayBuffer ? word : null
    }
  }
  return null
}

/** The id the word holds, as the core reads it. */
function wordValue(word: SharedArrayBuffer): bigint {
  return Atomics.load(new BigInt64Array(word), 0)
}

describe('the worker transport cancel (ui.md 2.4 item 5, protocol.md 2.11 rule 8)', () => {
  it('protocol test 100: a cross-origin isolated page shares a word that stops pending request 7 until its frame comes', async () => {
    vi.stubGlobal('crossOriginIsolated', true)
    const { transport, worker } = await started()
    expect(transport.canCancel).toBe(true)
    expect(worker.posted[0]?.message).toEqual({
      type: 'cancel_word',
      word: expect.any(SharedArrayBuffer),
    })
    const word = postedWord(worker)
    expect(word?.byteLength).toBe(8)
    if (word === null) {
      return
    }
    expect(wordValue(word)).toBe(0n)

    expect(transport.cancel(7)).toBe(false)
    expect(wordValue(word)).toBe(0n)

    const reply = transport.send(engineInfoFrame(7), 7)
    expect(worker.posted.map(({ message }) => Reflect.get(Object(message), 'type'))).toEqual([
      'cancel_word',
      'call',
    ])
    expect(transport.cancel(7)).toBe(true)
    expect(wordValue(word)).toBe(7n)

    let wordWhenResolved: bigint | null = null
    const read = reply.then((frame) => {
      wordWhenResolved = wordValue(word)
      return frame
    })
    worker.emit({ type: 'frame', frame: undoResponse(7) })
    expect(wordValue(word)).toBe(0n)
    await read
    expect(wordWhenResolved).toBe(0n)
  })

  it('protocol test 100: without cross-origin isolation no word is posted and cancel stops nothing', async () => {
    vi.stubGlobal('crossOriginIsolated', false)
    const { transport, worker } = await started()
    expect(transport.canCancel).toBe(false)
    expect(postedWord(worker)).toBeNull()
    const reply = transport.send(engineInfoFrame(7), 7)
    expect(transport.cancel(7)).toBe(false)
    expect(worker.posted.map(({ message }) => Reflect.get(Object(message), 'type'))).toEqual([
      'call',
    ])
    worker.emit({ type: 'frame', frame: undoResponse(7) })
    await expect(reply).resolves.toBeInstanceOf(ArrayBuffer)
  })

  it('the frame of another request leaves a stop of a later one in place', async () => {
    vi.stubGlobal('crossOriginIsolated', true)
    const { transport, worker } = await started()
    const word = postedWord(worker)
    if (word === null) {
      throw new Error('No cancel word was posted.')
    }
    const first = transport.send(engineInfoFrame(7), 7)
    const second = transport.send(engineInfoFrame(8), 8)
    expect(transport.cancel(8)).toBe(true)
    worker.emit({ type: 'frame', frame: undoResponse(7) })
    await first
    expect(wordValue(word)).toBe(8n)
    worker.emit({ type: 'frame', frame: undoResponse(8) })
    await second
    expect(wordValue(word)).toBe(0n)
  })

  it('cancel of a request that was already answered returns false', async () => {
    vi.stubGlobal('crossOriginIsolated', true)
    const { transport, worker } = await started()
    const reply = transport.send(engineInfoFrame(7), 7)
    worker.emit({ type: 'frame', frame: undoResponse(7) })
    await reply
    expect(transport.cancel(7)).toBe(false)
    expect(wordValue(postedWord(worker) ?? new SharedArrayBuffer(8))).toBe(0n)
  })

  it('a restart makes a new worker with a new word', async () => {
    vi.stubGlobal('crossOriginIsolated', true)
    const workers = [new FakeWorker(), new FakeWorker()]
    let created = 0
    const transport = createWorkerTransport({
      createWorker: () => {
        const worker = workers[created]
        created += 1
        if (worker === undefined) {
          throw new Error('No more fake workers.')
        }
        return worker as unknown as Worker
      },
    })
    for (const worker of workers) {
      // oxlint-disable-next-line no-await-in-loop
      await startOn(transport, worker)
      transport.stop()
    }
    const [firstWord, secondWord] = workers.map(postedWord)
    expect(firstWord).toBeInstanceOf(SharedArrayBuffer)
    expect(secondWord).toBeInstanceOf(SharedArrayBuffer)
    expect(secondWord).not.toBe(firstWord)
  })
})

describe('chooseTransport', () => {
  it('U-9 picks the Tauri transport when the Tauri global has core.invoke, else the worker transport', () => {
    const tauri: TauriGlobal = {
      core: { invoke: async () => new ArrayBuffer(0) },
    }
    expect(chooseTransport({ __TAURI__: tauri }).kind).toBe('tauri')
    expect(chooseTransport({}).kind).toBe('worker')
  })
})
