import { describe, expect, it, vi } from 'vitest'
import { settle } from '../test/fakes/settle'
import { createLatestOnly, createMutationQueue } from './queue'

type Deferred<Value> = {
  readonly promise: Promise<Value>
  resolve(value: Value): void
  reject(error: unknown): void
}

function deferred<Value>(): Deferred<Value> {
  const settlers: { resolve?: (value: Value) => void; reject?: (error: unknown) => void } = {}
  const promise = new Promise<Value>((resolve, reject) => {
    settlers.resolve = resolve
    settlers.reject = reject
  })
  return {
    promise,
    resolve: (value) => settlers.resolve?.(value),
    reject: (error) => settlers.reject?.(error),
  }
}

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
