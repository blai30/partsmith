// Ordering helpers for core requests (ui.md 2.5).

/** Runs tasks strictly one after another, in submission order. */
export type MutationQueue = {
  run<Result>(task: () => Promise<Result>): Promise<Result>
  readonly pending: number
}

export function createMutationQueue(): MutationQueue {
  let tail: Promise<void> = Promise.resolve()
  let pending = 0

  function finished(): void {
    pending -= 1
  }

  return {
    run(task) {
      pending += 1
      const result = tail.then(task)
      tail = result.then(finished, finished)
      return result
    },
    get pending() {
      return pending
    },
  }
}

/** Sends at most one input at a time; while one is in flight, newer inputs replace the waiting one. */
export type LatestOnly<Input> = {
  submit(input: Input): void
  /** Drops the waiting input and ignores the result of the one in flight. */
  cancel(): void
  /** Sends a waiting input now if idle and resolves when nothing is waiting or in flight. */
  flush(): Promise<void>
  readonly busy: boolean
}

type Outcome<Output> =
  | { readonly ok: true; readonly output: Output }
  | { readonly ok: false; readonly error: unknown }

export function createLatestOnly<Input, Output>(
  send: (input: Input) => Promise<Output>,
  onResult: (output: Output, input: Input) => void,
  onError: (error: unknown, input: Input) => void,
): LatestOnly<Input> {
  let waiting: { readonly input: Input } | null = null
  let inFlight = false
  let ignoreInFlight = false
  let idleWaiters: (() => void)[] = []

  function settleIdle(): void {
    if (inFlight || waiting !== null) {
      return
    }
    const waiters = idleWaiters
    idleWaiters = []
    for (const resolve of waiters) {
      resolve()
    }
  }

  function report(outcome: Outcome<Output>, input: Input): void {
    try {
      if (outcome.ok) {
        onResult(outcome.output, input)
      } else {
        onError(outcome.error, input)
      }
    } catch (error) {
      console.error('A request callback failed.', error)
    }
  }

  function start(input: Input): void {
    inFlight = true
    ignoreInFlight = false
    let request: Promise<Output>
    try {
      request = send(input)
    } catch (error) {
      request = Promise.reject(error)
    }
    request.then(
      (output) => {
        finish({ ok: true, output }, input)
      },
      (error: unknown) => {
        finish({ ok: false, error }, input)
      },
    )
  }

  function finish(outcome: Outcome<Output>, input: Input): void {
    inFlight = false
    const ignored = ignoreInFlight
    ignoreInFlight = false
    if (waiting !== null) {
      // A newer input replaces this result.
      const next = waiting.input
      waiting = null
      start(next)
      return
    }
    if (!ignored) {
      report(outcome, input)
    }
    settleIdle()
  }

  return {
    submit(input) {
      if (inFlight) {
        waiting = { input }
        return
      }
      start(input)
    },
    cancel() {
      waiting = null
      if (inFlight) {
        ignoreInFlight = true
      }
    },
    flush() {
      if (!inFlight && waiting !== null) {
        const next = waiting.input
        waiting = null
        start(next)
      }
      if (!inFlight && waiting === null) {
        return Promise.resolve()
      }
      return new Promise<void>((resolve) => {
        idleWaiters.push(resolve)
      })
    },
    get busy() {
      return inFlight || waiting !== null
    },
  }
}
