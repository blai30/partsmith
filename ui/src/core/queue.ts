// Ordering helpers for core requests (ui.md 2.5).

/**
 * What a task does, for a core that is stopped before it ends (4.4): a `change` to a document, or
 * `other` work (an open, a new document, a save, an export) whose action says itself that it was
 * not done.
 */
export type TaskKind = 'change' | 'other'

/** Runs tasks strictly one after another, in submission order. */
export type MutationQueue = {
  /** Runs `task` after every task submitted before it; `kind` is `change` unless given. */
  run<Result>(task: () => Promise<Result>, kind?: TaskKind): Promise<Result>
  /** Rejects every task that has not started with `error`, without running it, and returns how many it rejected (4.4). */
  discard(error: Error): number
  /** The started task and the waiting ones. */
  readonly pending: number
  /** Whether the started task or a waiting one is a `change`. */
  readonly changing: boolean
}

export function createMutationQueue(): MutationQueue {
  let tail: Promise<void> = Promise.resolve()
  let pending = 0
  // The kind of the started task, while one runs.
  let started: TaskKind | null = null
  // The tasks that have not started yet: their rejecters, with their kinds.
  const waiting = new Map<(error: Error) => void, TaskKind>()

  return {
    run<Result>(task: () => Promise<Result>, kind: TaskKind = 'change') {
      pending += 1
      return new Promise<Result>((resolve, reject) => {
        waiting.set(reject, kind)
        tail = tail.then(async () => {
          // A discarded task was rejected already and never runs.
          if (!waiting.delete(reject)) {
            return
          }
          started = kind
          try {
            resolve(await task())
          } catch (error) {
            reject(error)
          } finally {
            started = null
            pending -= 1
          }
        })
      })
    },
    discard(error) {
      const discarded = Array.from(waiting.keys())
      waiting.clear()
      pending -= discarded.length
      for (const reject of discarded) {
        reject(error)
      }
      return discarded.length
    },
    get pending() {
      return pending
    },
    get changing() {
      return started === 'change' || Array.from(waiting.values()).includes('change')
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

/**
 * Wraps `latest` so that an input `same` as the one submitted last is dropped while `latest` is
 * busy, since the answer that comes next already answers it.
 */
export function skipRepeats<Input>(
  latest: LatestOnly<Input>,
  same: (first: Input, second: Input) => boolean,
): LatestOnly<Input> {
  // The input submitted last: while `latest` is busy, the waiting one, else the one in flight.
  let submitted: { readonly input: Input } | null = null
  return {
    submit(input) {
      if (latest.busy && submitted !== null && same(submitted.input, input)) {
        return
      }
      submitted = { input }
      latest.submit(input)
    },
    cancel() {
      submitted = null
      latest.cancel()
    },
    flush: () => latest.flush(),
    get busy() {
      return latest.busy
    },
  }
}
