// Turning a thrown value into text. This module has no imports, so the worker bundle can use it.

/**
 * The message of a thrown Error, also one from another realm such as a DOMException under jsdom,
 * or the thrown value as text.
 */
export function errorMessage(error: unknown): string {
  if (error instanceof Error) {
    return error.message
  }
  const message: unknown =
    typeof error === 'object' && error !== null ? Reflect.get(error, 'message') : undefined
  return typeof message === 'string' ? message : String(error)
}
