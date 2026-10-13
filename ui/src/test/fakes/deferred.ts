/** A promise with its resolve and reject functions, for answering a call later. */
export type Deferred<Value> = {
  readonly promise: Promise<Value>
  readonly resolve: (value: Value) => void
  readonly reject: (error: unknown) => void
}

function unset(): void {
  // Replaced by the promise executor, which runs before `deferred` returns.
}

/** A promise the test settles by hand. */
export function deferred<Value>(): Deferred<Value> {
  let resolve: (value: Value) => void = unset
  let reject: (error: unknown) => void = unset
  const promise = new Promise<Value>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise
    reject = rejectPromise
  })
  return { promise, resolve, reject }
}
