import { afterEach, describe, expect, it, vi } from 'vitest'
import { fileDigest, fileDigestOrNull } from './digest'

// The digest Save compares to see a change made elsewhere (ui.md 2.15, 3.19, ADR 0037 decision 1).

function utf8(text: string): Uint8Array {
  // Copied into this realm's Uint8Array: under jsdom, TextEncoder returns Node's.
  return Uint8Array.from(new TextEncoder().encode(text))
}

afterEach(() => {
  vi.restoreAllMocks()
})

describe('fileDigest (2.15)', () => {
  it('U-216 is the SHA-256 of the bytes as 64 lowercase hex digits', async () => {
    await expect(fileDigest(utf8('hello'))).resolves.toBe(
      '2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824',
    )
  })

  it('U-216 of no bytes is the digest of the empty input', async () => {
    await expect(fileDigest(new Uint8Array(0))).resolves.toBe(
      'e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855',
    )
  })

  it('U-216 of a view digests only the bytes the view covers', async () => {
    const buffer = new Uint8Array(16).map((_, index) => index * 7)
    const view = buffer.subarray(1, 6)
    await expect(fileDigest(view)).resolves.toBe(await fileDigest(Uint8Array.from(view)))
    await expect(fileDigest(view)).resolves.not.toBe(await fileDigest(buffer))
  })

  it('fileDigestOrNull is the digest, and null with one warning naming the file where Web Crypto cannot digest', async () => {
    await expect(fileDigestOrNull(utf8('hello'), 'cube.ptsm')).resolves.toBe(
      '2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824',
    )
    const warn = vi.spyOn(console, 'warn').mockImplementation(() => undefined)
    vi.spyOn(crypto.subtle, 'digest').mockRejectedValue(
      new DOMException('Web Crypto needs a secure context.', 'NotSupportedError'),
    )
    await expect(fileDigestOrNull(utf8('hello'), 'cube.ptsm')).resolves.toBeNull()
    expect(warn).toHaveBeenCalledTimes(1)
    expect(warn.mock.calls[0]?.[0]).toBe(
      'Partsmith could not compute the digest of cube.ptsm, so Save does not check it for changes made elsewhere.',
    )
  })

  it('rejects when Web Crypto cannot digest', async () => {
    vi.spyOn(crypto.subtle, 'digest').mockRejectedValue(
      new DOMException('Web Crypto needs a secure context.', 'NotSupportedError'),
    )
    await expect(fileDigest(utf8('hello'))).rejects.toThrow('Web Crypto needs a secure context.')
  })
})
