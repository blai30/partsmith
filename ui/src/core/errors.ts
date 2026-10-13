// Turning a thrown value into text. This module has no imports, so the worker bundle can use it.

/** The message of a thrown Error, or the thrown value as text. */
export function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error)
}
