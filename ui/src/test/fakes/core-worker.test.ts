import { describe, expect, it } from 'vitest'
import { createCoreClient } from '../../core/client'
import { CoreFatalError, createWorkerTransport } from '../../core/transport'
import { createFakeCoreWorkers } from './core-worker'
import { settle } from './settle'

describe('createFakeCoreWorkers', () => {
  it('boots a core per worker that answers the transport through the client', async () => {
    const workers = createFakeCoreWorkers()
    const transport = createWorkerTransport({ createWorker: workers.createWorker })
    await expect(transport.start()).resolves.toMatchObject({ engineVersion: '0.1.0' })
    const client = createCoreClient(transport)
    const reply = await client.request({ type: 'new_document', units: 'mm' })
    expect(reply.result.type).toBe('new_document')
    expect(workers.workers).toHaveLength(1)
    expect(workers.workers[0]?.commands.map((command) => command.type)).toEqual(['new_document'])
  })

  it('never answers a stalled call or the calls after it, until the worker is terminated', async () => {
    const workers = createFakeCoreWorkers()
    const transport = createWorkerTransport({ createWorker: workers.createWorker })
    await transport.start()
    const client = createCoreClient(transport)
    workers.stallWhen((command) => command.type === 'feature_catalog')
    const outcomes: string[] = []
    const stuck = client.request({ type: 'feature_catalog' }).catch((error: unknown) => {
      outcomes.push(error instanceof CoreFatalError ? 'stuck call stopped' : 'other error')
    })
    const behind = client.request({ type: 'list_printer_profiles' }).catch(() => {
      outcomes.push('call behind it stopped')
    })
    await settle()
    expect(outcomes).toEqual([])
    expect(workers.workers[0]?.commands.map((command) => command.type)).toEqual([
      'feature_catalog',
      'list_printer_profiles',
    ])
    transport.stop()
    await Promise.all([stuck, behind])
    expect(outcomes).toEqual(['stuck call stopped', 'call behind it stopped'])
    expect(workers.workers[0]?.terminated).toBe(true)
  })

  it('starts a fresh worker with a fresh core after a stop', async () => {
    const workers = createFakeCoreWorkers()
    const transport = createWorkerTransport({ createWorker: workers.createWorker })
    await transport.start()
    transport.stop()
    await transport.start()
    const reply = await createCoreClient(transport).request({ type: 'new_document', units: 'mm' })
    expect(reply.result).toMatchObject({ type: 'new_document', doc: 1 })
    expect(workers.workers).toHaveLength(2)
  })
})
