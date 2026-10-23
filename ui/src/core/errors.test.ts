import { describe, expect, it } from 'vitest'
import { errorMessage } from './errors'

describe('errorMessage', () => {
  it('is the message of an Error', () => {
    expect(errorMessage(new TypeError('The disk is full.'))).toBe('The disk is full.')
  })

  it('is the message of an error object that is not an Error of this realm', () => {
    expect(
      errorMessage({ name: 'SecurityError', message: 'Must be handling a user gesture.' }),
    ).toBe('Must be handling a user gesture.')
  })

  it('is any other thrown value as text', () => {
    expect(errorMessage('lost')).toBe('lost')
    expect(errorMessage(42)).toBe('42')
    expect(errorMessage(undefined)).toBe('undefined')
  })
})
