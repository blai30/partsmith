import { describe, expect, it } from 'vitest'
import { createTestApp } from '../../test/fakes/app'
import { deferred } from '../../test/fakes/deferred'
import { landInTurn } from './selection'
import { afterStarts, startInProgress, startInTurn, startsAsked } from './starts'

describe('starts one at a time (3.7 step 1)', () => {
  it('runs a start at once when none is in progress, and counts it as asked for', () => {
    const app = createTestApp()
    let ran = false
    void startInTurn(app, () => {
      ran = true
      return Promise.resolve()
    })
    expect(ran).toBe(true)
    expect(startsAsked(app)).toBe(1)
  })

  it('runs a start once the start before it has run', async () => {
    const app = createTestApp()
    const opened = deferred<undefined>()
    const order: string[] = []
    const first = startInTurn(app, async () => {
      await opened.promise
      order.push('first')
    })
    const second = startInTurn(app, () => {
      order.push('second')
      return Promise.resolve()
    })
    expect(order).toEqual([])
    opened.resolve(undefined)
    await Promise.all([first, second])
    expect(order).toEqual(['first', 'second'])
  })

  it('runs a start once the input made before it has landed, not the input made after it', async () => {
    const app = createTestApp()
    const before = deferred<undefined>()
    const after = deferred<undefined>()
    const order: string[] = []
    const landedBefore = landInTurn(app, async () => {
      await before.promise
      order.push('before')
    })
    const started = startInTurn(app, () => {
      order.push('start')
      return Promise.resolve()
    })
    const landedAfter = landInTurn(app, async () => {
      await after.promise
      order.push('after')
    })
    await Promise.resolve()
    expect(order).toEqual([])
    before.resolve(undefined)
    await started
    expect(order).toEqual(['before', 'start'])
    after.resolve(undefined)
    await Promise.all([landedBefore, landedAfter])
    expect(order).toEqual(['before', 'start', 'after'])
  })

  it('is in progress from the ask until the start has run', async () => {
    const app = createTestApp()
    const opened = deferred<undefined>()
    const started = startInTurn(app, () => opened.promise)
    expect(startInProgress(app)).toBe(true)
    opened.resolve(undefined)
    await started
    expect(startInProgress(app)).toBe(false)
  })
})

describe('afterStarts (3.5, 3.16.1)', () => {
  it('runs the action at once when no start is in progress', () => {
    const app = createTestApp()
    let ran = false
    void afterStarts(app, () => {
      ran = true
    })
    expect(ran).toBe(true)
  })

  it('runs the action once the start in progress has run, without counting as a start', async () => {
    const app = createTestApp()
    const opened = deferred<undefined>()
    const order: string[] = []
    const started = startInTurn(app, async () => {
      await opened.promise
      order.push('start')
    })
    const acted = afterStarts(app, () => {
      order.push('action')
    })
    expect(startsAsked(app)).toBe(1)
    opened.resolve(undefined)
    await Promise.all([started, acted])
    expect(order).toEqual(['start', 'action'])
  })
})
