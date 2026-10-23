import { writableBytes } from './file-service'

// The digest Save compares to see a change made elsewhere (ui.md 2.15, 3.19, ADR 0037 decision 1).

/**
 * The SHA-256 of the bytes (Web Crypto's `crypto.subtle.digest`) as 64 lowercase hex digits; a
 * view is digested alone, never the rest of its buffer. Rejects where Web Crypto cannot digest (no
 * `crypto.subtle` outside a secure context).
 */
export async function fileDigest(bytes: Uint8Array): Promise<string> {
  const digest = await crypto.subtle.digest('SHA-256', writableBytes(bytes))
  return Array.from(new Uint8Array(digest), (byte) => byte.toString(16).padStart(2, '0')).join('')
}

/**
 * `fileDigest` of a file's bytes as the UI keeps it (3.19): null with a warning where Web Crypto
 * cannot digest, which turns Save's change check off for that file.
 */
export async function fileDigestOrNull(
  bytes: Uint8Array,
  fileName: string,
): Promise<string | null> {
  try {
    return await fileDigest(bytes)
  } catch (error) {
    console.warn(
      `Partsmith could not compute the digest of ${fileName}, so Save does not check it for changes made elsewhere.`,
      error,
    )
    return null
  }
}
