import { describe, expect, it } from 'vitest'
import { deferred } from './deferred'
import { createFakeLocks } from './locks'
import { settle } from './settle'

describe('createFakeLocks', () => {
  it('holds an exclusive lock until the promise its callback returned settles', async () => {
    const locks = createFakeLocks()
    const page = locks.page()
    const done = deferred<string>()
    const granted = page.locks.request('a', () => done.promise)
    await settle(2)
    expect(locks.held()).toEqual(['a'])
    done.resolve('finished')
    await expect(granted).resolves.toBe('finished')
    expect(locks.held()).toEqual([])
  })

  it('makes a second page wait for a held lock, and reports both in query()', async () => {
    const locks = createFakeLocks()
    const first = locks.page()
    const second = locks.page()
    const release = deferred<void>()
    void first.locks.request('a', () => release.promise)
    let secondGranted = false
    void second.locks.request('a', { mode: 'exclusive' }, () => {
      secondGranted = true
    })
    await settle(2)
    expect(secondGranted).toBe(false)
    const snapshot = await second.locks.query()
    expect(snapshot.held?.map((lock) => lock.name)).toEqual(['a'])
    expect(snapshot.pending?.map((lock) => lock.name)).toEqual(['a'])
    release.resolve()
    await settle(2)
    expect(secondGranted).toBe(true)
  })

  it('passes null to the callback of an ifAvailable request for a held lock', async () => {
    const locks = createFakeLocks()
    const holder = locks.page()
    void holder.locks.request('a', () => new Promise(() => undefined))
    await settle(2)
    const answer = await locks
      .page()
      .locks.request('a', { ifAvailable: true }, (lock) => (lock === null ? 'taken' : 'free'))
    expect(answer).toBe('taken')
  })

  it("releases an ended page's locks, drops its waiting requests and grants the next page's", async () => {
    const locks = createFakeLocks()
    const ending = locks.page()
    const other = locks.page()
    void ending.locks.request('a', () => new Promise(() => undefined))
    void ending.locks.request('b', () => new Promise(() => undefined))
    let otherGranted = false
    void other.locks.request('a', () => {
      otherGranted = true
      return new Promise(() => undefined)
    })
    await settle(2)
    expect(locks.held()).toEqual(['a', 'b'])
    ending.end()
    await settle(2)
    expect(otherGranted).toBe(true)
    expect(locks.held()).toEqual(['a'])
    await expect(ending.locks.request('c', () => undefined)).rejects.toMatchObject({
      name: 'InvalidStateError',
    })
  })

  it('records the names each page requested and can refuse query()', async () => {
    const locks = createFakeLocks()
    const page = locks.page({ failQuery: true })
    await page.locks.request('a', () => undefined)
    await page.locks.request('b', () => undefined)
    expect(page.requested).toEqual(['a', 'b'])
    await expect(page.locks.query()).rejects.toMatchObject({ name: 'SecurityError' })
  })
})
