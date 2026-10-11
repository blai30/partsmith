/** A span as expressions.md reports it: UTF-8 byte offsets into the source, end exclusive. */
export type Utf8Span = { readonly start: number; readonly end: number }

/** UTF-8 length of a code point; a lone surrogate gets 3, the length of the U+FFFD `TextEncoder` writes for it. */
function utf8Length(codePoint: number): number {
  if (codePoint < 0x80) {
    return 1
  }
  if (codePoint < 0x800) {
    return 2
  }
  return codePoint < 0x10000 ? 3 : 4
}

/** The UTF-16 index of the code point that starts at a UTF-8 byte offset, or null when none does. */
function utf16Index(source: string, byteOffset: number): number | null {
  if (!Number.isInteger(byteOffset) || byteOffset < 0) {
    return null
  }
  let bytes = 0
  let index = 0
  while (index < source.length) {
    if (bytes === byteOffset) {
      return index
    }
    if (bytes > byteOffset) {
      return null
    }
    const codePoint = source.codePointAt(index) ?? 0
    bytes += utf8Length(codePoint)
    // A code point above U+FFFF is a surrogate pair; a lone surrogate is one unit.
    index += codePoint > 0xffff ? 2 : 1
  }
  return bytes === byteOffset ? source.length : null
}

/** The same range as UTF-16 indices into `source`, or null when a bound is past the end or inside a character's UTF-8 encoding. */
export function utf16Range(
  source: string,
  span: Utf8Span,
): { readonly start: number; readonly end: number } | null {
  if (span.end < span.start) {
    return null
  }
  const start = utf16Index(source, span.start)
  const end = utf16Index(source, span.end)
  return start === null || end === null ? null : { start, end }
}
