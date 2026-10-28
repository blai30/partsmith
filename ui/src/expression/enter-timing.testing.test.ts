import { afterEach, describe, expect, it, vi } from 'vitest'
import { RESULT_TIMEOUT_MS, whenResultShows } from './enter-timing.testing'

// The timing helper fails with a reason, and leaves the act environment as it found it, when the
// result it waits for cannot show.

type ActEnvironment = { IS_REACT_ACT_ENVIRONMENT?: boolean | undefined }

afterEach(() => {
  vi.useRealTimers()
})

function resultShowing(text: string): HTMLElement {
  const result = document.createElement('span')
  result.textContent = text
  return result
}

describe('whenResultShows', () => {
  it('fails at once when the result already shows the text', async () => {
    const settleEvaluation = vi.fn<() => void>()
    const action = vi.fn<() => void>()
    await expect(
      whenResultShows(resultShowing('mm'), 'mm', settleEvaluation, action),
    ).rejects.toThrow("The result already shows 'mm'")
    expect(settleEvaluation).not.toHaveBeenCalled()
    expect(action).not.toHaveBeenCalled()
  })

  it('fails after its timeout when the result never shows the text, and restores the act environment', async () => {
    vi.useFakeTimers()
    const environment = globalThis as ActEnvironment
    const actEnvironment = environment.IS_REACT_ACT_ENVIRONMENT
    const action = vi.fn<() => void>()
    const failure = whenResultShows(resultShowing(''), 'mm', () => undefined, action).catch(
      (error: unknown) => error,
    )
    expect(environment.IS_REACT_ACT_ENVIRONMENT).toBe(false)
    await vi.advanceTimersByTimeAsync(RESULT_TIMEOUT_MS)
    expect(await failure).toEqual(new Error("The result never showed 'mm'."))
    expect(action).not.toHaveBeenCalled()
    expect(environment.IS_REACT_ACT_ENVIRONMENT).toBe(actEnvironment)
  })
})
