import { afterEach, describe, expect, it, vi } from 'vitest'
import { reportFailure } from './report'

describe('reportFailure', () => {
  afterEach(() => {
    vi.restoreAllMocks()
  })

  it('logs "{action} failed." with the error', () => {
    const log = vi.spyOn(console, 'error').mockImplementation(() => undefined)
    const error = new Error('The disk is full.')
    reportFailure('Renaming the body')(error)
    expect(log).toHaveBeenCalledExactlyOnceWith('Renaming the body failed.', error)
  })
})
