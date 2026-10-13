import { describe, expect, it, vi } from 'vitest'
import { deferred, type Deferred } from '../test/fakes/deferred'
import { settle } from '../test/fakes/settle'
import { createLatestOnly, createMutationQueue } from './queue'

describe('createMutationQueue', () => {
  it('U-15 starts the second task only after the first settles', async () => {
    const queue = createMutationQueue()
    const first = deferred<string>()
    const log: string[] = []
    const firstResult = queue.run(() => {
      log.push('first started')
      return first.promise
    })
    const secondResult = queue.run(async () => {
      log.push('second started')
      return 'second'
    })
    await settle()
    expect(log).toEqual(['first started'])
    first.resolve('first')
    await expect(firstResult).resolves.toBe('first')
    await expect(secondResult).resolves.toBe('second')
    expect(log).toEqual(['first started', 'second started'])
  })

  it('U-15 a rejected task rejects its own promise and does not block the next', async () => {
    const queue = createMutationQueue()
    const failure = new Error('core error')
    const failed = queue.run(async () => {
      throw failure
    })
    const next = queue.run(async () => 'next')
    await expect(failed).rejects.toBe(failure)
    await expect(next).resolves.toBe('next')
  })

  it('U-15 pending counts queued and running tasks', async () => {
    const queue = createMutationQueue()
    expect(queue.pending).toBe(0)
    const first = deferred<void>()
    const second = deferred<void>()
    const firstDone = queue.run(() => first.promise)
    const secondDone = queue.run(() => second.promise)
    expect(queue.pending).toBe(2)
    first.resolve()
    await firstDone
    await settle()
    expect(queue.pending).toBe(1)
    second.resolve()
    await secondDone
    await settle()
    expect(queue.pending).toBe(0)
  })

  it('runs tasks in submission order', async () => {
    const queue = createMutationQueue()
    const order: number[] = []
    await Promise.all(
      [1, 2, 3, 4].map((number) =>
        queue.run(async () => {
          await settle()
          order.push(number)
        }),
      ),
    )
    expect(order).toEqual([1, 2, 3, 4])
  })
})

describe('MutationQueue.discard (4.4)', () => {
  it('U-160 rejects the waiting tasks without running them and leaves the started task to settle', async () => {
    const queue = createMutationQueue()
    const first = deferred<string>()
    const started: number[] = []
    const firstResult = queue.run(() => {
      started.push(1)
      return first.promise
    })
    const secondResult = queue.run(async () => {
      started.push(2)
      return 'second'
    })
    const thirdResult = queue.run(async () => {
      started.push(3)
      return 'third'
    })
    // The rejections are observed before discard causes them.
    const waiting = Promise.allSettled([secondResult, thirdResult])
    await settle()
    expect(started).toEqual([1])
    expect(queue.pending).toBe(3)
    const stopped = new Error('The geometry core was stopped.')
    expect(queue.discard(stopped)).toBe(2)
    expect(queue.pending).toBe(1)
    expect(await waiting).toEqual([
      { status: 'rejected', reason: stopped },
      { status: 'rejected', reason: stopped },
    ])
    first.resolve('first')
    await expect(firstResult).resolves.toBe('first')
    await settle()
    expect(started).toEqual([1])
    expect(queue.pending).toBe(0)
  })

  it('U-160 runs a task submitted after discard once the started task has settled', async () => {
    const queue = createMutationQueue()
    const first = deferred<void>()
    const started: string[] = []
    const firstResult = queue.run(() => {
      started.push('first')
      return first.promise
    })
    const waiting = queue.run(async () => {
      started.push('discarded')
    })
    const discarded = waiting.catch(() => 'rejected')
    await settle()
    expect(queue.discard(new Error('stopped'))).toBe(1)
    const later = queue.run(async () => {
      started.push('later')
      return 'later'
    })
    expect(queue.pending).toBe(2)
    await settle()
    expect(started).toEqual(['first'])
    first.resolve()
    await firstResult
    await expect(later).resolves.toBe('later')
    await expect(discarded).resolves.toBe('rejected')
    expect(started).toEqual(['first', 'later'])
    expect(queue.pending).toBe(0)
  })

  it('U-160 returns 0 and changes nothing when no task is waiting', async () => {
    const queue = createMutationQueue()
    expect(queue.discard(new Error('stopped'))).toBe(0)
    expect(queue.pending).toBe(0)
    await expect(queue.run(async () => 'ran')).resolves.toBe('ran')
  })
})

type LatestOnlyHarness = {
  readonly sends: Map<string, Deferred<string>>
  readonly sent: string[]
  readonly results: [string, string][]
  readonly errors: [unknown, string][]
}

function harness(): LatestOnlyHarness & {
  readonly latest: ReturnType<typeof createLatestOnly<string, string>>
} {
  const sends = new Map<string, Deferred<string>>()
  const sent: string[] = []
  const results: [string, string][] = []
  const errors: [unknown, string][] = []
  const latest = createLatestOnly<string, string>(
    (input) => {
      sent.push(input)
      const pending = deferred<string>()
      sends.set(input, pending)
      return pending.promise
    },
    (output, input) => {
      results.push([output, input])
    },
    (error, input) => {
      errors.push([error, input])
    },
  )
  return { sends, sent, results, errors, latest }
}

describe('createLatestOnly', () => {
  it('sends an input when idle and reports its result', async () => {
    const { latest, sends, sent, results } = harness()
    latest.submit('a')
    await settle()
    expect(sent).toEqual(['a'])
    expect(latest.busy).toBe(true)
    sends.get('a')?.resolve('result a')
    await settle()
    expect(results).toEqual([['result a', 'a']])
    expect(latest.busy).toBe(false)
  })

  it('U-16 submitting a, b and c while a is in flight sends a then c, never b, and reports only c', async () => {
    const { latest, sends, sent, results } = harness()
    latest.submit('a')
    await settle()
    latest.submit('b')
    latest.submit('c')
    await settle()
    expect(sent).toEqual(['a'])
    sends.get('a')?.resolve('result a')
    await settle()
    expect(sent).toEqual(['a', 'c'])
    expect(results).toEqual([])
    sends.get('c')?.resolve('result c')
    await settle()
    expect(sent).toEqual(['a', 'c'])
    expect(results).toEqual([['result c', 'c']])
  })

  it('U-16 cancel() drops the waiting input and suppresses the result of the one in flight', async () => {
    const { latest, sends, sent, results, errors } = harness()
    latest.submit('a')
    await settle()
    latest.submit('b')
    latest.cancel()
    sends.get('a')?.resolve('result a')
    await settle()
    expect(sent).toEqual(['a'])
    expect(results).toEqual([])
    expect(errors).toEqual([])
  })

  it('U-16 flush() sends the waiting input and resolves after its onResult', async () => {
    const { latest, sends, sent, results } = harness()
    latest.submit('a')
    await settle()
    latest.submit('b')
    let flushed = false
    const flush = latest.flush().then(() => {
      flushed = true
    })
    sends.get('a')?.resolve('result a')
    await settle()
    expect(sent).toEqual(['a', 'b'])
    expect(flushed).toBe(false)
    sends.get('b')?.resolve('result b')
    await flush
    expect(results).toEqual([['result b', 'b']])
  })

  it('flush() resolves at once when nothing is waiting or in flight', async () => {
    const { latest } = harness()
    await expect(latest.flush()).resolves.toBeUndefined()
  })

  it('reports a failed send through onError with its input', async () => {
    const { latest, sends, errors, results } = harness()
    const failure = new Error('superseded')
    latest.submit('a')
    await settle()
    sends.get('a')?.reject(failure)
    await settle()
    expect(errors).toEqual([[failure, 'a']])
    expect(results).toEqual([])
  })

  it('discards a failure of a send that a newer waiting input replaces', async () => {
    const { latest, sends, sent, errors, results } = harness()
    latest.submit('a')
    await settle()
    latest.submit('b')
    sends.get('a')?.reject(new Error('stale'))
    await settle()
    expect(errors).toEqual([])
    expect(sent).toEqual(['a', 'b'])
    sends.get('b')?.resolve('result b')
    await settle()
    expect(results).toEqual([['result b', 'b']])
  })

  it('keeps working after cancel()', async () => {
    const { latest, sends, results } = harness()
    latest.submit('a')
    await settle()
    latest.cancel()
    sends.get('a')?.resolve('result a')
    await settle()
    latest.submit('d')
    await settle()
    sends.get('d')?.resolve('result d')
    await settle()
    expect(results).toEqual([['result d', 'd']])
  })

  it('does not call send again when nothing new was submitted', async () => {
    const send = vi.fn<(input: string) => Promise<string>>(async (input) => input)
    const latest = createLatestOnly<string, string>(
      send,
      () => undefined,
      () => undefined,
    )
    latest.submit('a')
    await latest.flush()
    await settle()
    expect(send).toHaveBeenCalledTimes(1)
  })
})
