import { describe, expect, it } from 'vitest'
import { PROTOCOL_VERSION } from '../generated/protocol'
import { readRequest, requestFrame, responseFrame } from '../test/fakes/protocol'
import { settle } from '../test/fakes/settle'
import { createCoreClient } from './client'
import { createTauriTransport } from './tauri-transport'
import { CoreFatalError, createWorkerTransport } from './transport'
import type { FromWorker } from './worker-host'

// Restarting the core after a fatal error (ui.md 4.4) starts the same transport again.

class FakeWorker extends EventTarget {
  terminated = false

  postMessage(): void {
    // The tests answer through emit.
  }

  terminate(): void {
    this.terminated = true
  }

  emit(data: FromWorker): void {
    this.dispatchEvent(new MessageEvent('message', { data }))
  }
}

describe('createWorkerTransport restarts', () => {
  it('starts a fresh worker after its core died, and sends to it', async () => {
    const workers: FakeWorker[] = []
    const transport = createWorkerTransport({
      createWorker: () => {
        const worker = new FakeWorker()
        workers.push(worker)
        return worker as unknown as Worker
      },
    })
    const first = transport.start()
    await settle()
    workers[0]?.emit({ type: 'ready', protocolVersion: 1, engineVersion: '0.1.0' })
    await first
    workers[0]?.emit({ type: 'fatal', message: 'The core trapped.' })
    await expect(transport.send(requestFrame(1, { type: 'engine_info' }))).rejects.toBeInstanceOf(
      CoreFatalError,
    )
    expect(workers[0]?.terminated).toBe(true)

    const second = transport.start()
    await settle()
    expect(workers).toHaveLength(2)
    workers[1]?.emit({ type: 'ready', protocolVersion: 1, engineVersion: '0.2.0' })
    await expect(second).resolves.toEqual({ protocolVersion: 1, engineVersion: '0.2.0' })
    const reply = transport.send(requestFrame(2, { type: 'engine_info' }))
    const frame = responseFrame({ id: 2, result: { type: 'undo', revision: 1, label: 'Edit' } })
    workers[1]?.emit({ type: 'frame', frame })
    await expect(reply).resolves.toBe(frame)
  })

  it('ignores the messages of a worker it replaced', async () => {
    const workers: FakeWorker[] = []
    const transport = createWorkerTransport({
      createWorker: () => {
        const worker = new FakeWorker()
        workers.push(worker)
        return worker as unknown as Worker
      },
    })
    const first = transport.start()
    await settle()
    workers[0]?.emit({ type: 'ready', protocolVersion: 1, engineVersion: '0.1.0' })
    await first
    transport.stop()
    const second = transport.start()
    await settle()
    workers[0]?.emit({ type: 'fatal', message: 'Late news from the old worker.' })
    workers[1]?.emit({ type: 'ready', protocolVersion: 1, engineVersion: '0.1.0' })
    await expect(second).resolves.toMatchObject({ engineVersion: '0.1.0' })
  })
})

describe('createTauriTransport restarts', () => {
  it('rejects sends after stop() and accepts them again after start()', async () => {
    const tauri = {
      core: {
        invoke: async (_command: string, payload: ArrayBuffer) => {
          const request = readRequest(payload)
          return request.command.type === 'engine_info'
            ? responseFrame({
                id: request.id,
                result: {
                  type: 'engine_info',
                  protocol_version: PROTOCOL_VERSION,
                  engine_version: '0.1.0',
                  format_version: 1,
                  kernel: 'OCCT 7.9.3',
                },
              })
            : responseFrame({
                id: request.id,
                result: { type: 'undo', revision: 1, label: 'Edit' },
              })
        },
      },
    }
    const transport = createTauriTransport(tauri)
    await transport.start()
    transport.stop()
    await expect(transport.send(requestFrame(1, { type: 'undo', doc: 1 }))).rejects.toBeInstanceOf(
      CoreFatalError,
    )
    await transport.start()
    const answer = await transport.send(requestFrame(2, { type: 'undo', doc: 1 }))
    expect(answer.byteLength).toBeGreaterThan(0)
  })
})

describe('createCoreClient close', () => {
  it('stops the transport and rejects later requests', async () => {
    let stopped = false
    const client = createCoreClient({
      kind: 'worker',
      start: async () => ({ protocolVersion: 1, engineVersion: '0.1.0' }),
      send: async (frame) =>
        responseFrame({
          id: readRequest(frame).id,
          result: { type: 'undo', revision: 1, label: 'Edit' },
        }),
      stop: () => {
        stopped = true
      },
      onFatal: () => () => undefined,
    })
    client.close()
    expect(stopped).toBe(true)
    await expect(client.request({ type: 'undo', doc: 1 })).rejects.toBeInstanceOf(CoreFatalError)
  })

  it('rejects every request after the stream became unreadable', async () => {
    let sends = 0
    const client = createCoreClient({
      kind: 'worker',
      start: async () => ({ protocolVersion: 1, engineVersion: '0.1.0' }),
      send: async () => {
        sends += 1
        return responseFrame({ id: 99, result: { type: 'undo', revision: 1, label: 'Edit' } })
      },
      stop: () => undefined,
      onFatal: () => () => undefined,
    })
    await expect(client.request({ type: 'undo', doc: 1 })).rejects.toBeInstanceOf(CoreFatalError)
    await expect(client.request({ type: 'undo', doc: 1 })).rejects.toBeInstanceOf(CoreFatalError)
    expect(sends).toBe(1)
  })
})
