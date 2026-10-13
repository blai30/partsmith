// Test support: the Web Locks API of one browser profile, shared by its pages, for the recovery
// store's running sessions (ui.md 2.15). Each page has its own LockManager. A lock is held until
// the promise its callback returned settles or until its page ends, as the browser releases the
// locks of a closed or crashed page. Grants run on promises only, so tests may use fake timers.

/** One page (a tab or window) of the browser. */
export type FakeLockPage = {
  /** The page's `navigator.locks`. */
  readonly locks: LockManager
  /** The names this page passed to `request`, oldest first. */
  readonly requested: readonly string[]
  /** Ends the page as closing or crashing its tab does: its locks are released and its waiting requests dropped. */
  end(): void
}

export type FakeLocks = {
  /** Opens a page. With `failQuery`, its `query()` rejects, as a lock manager that is not available. */
  page(options?: { readonly failQuery?: boolean }): FakeLockPage
  /** The names of the locks held now, by any page, in the order they were granted. */
  held(): readonly string[]
}

type Grant = {
  readonly name: string
  readonly mode: LockMode
  readonly clientId: string
}

type Waiting = Grant & { readonly start: () => void }

function lockInfo(grant: Grant): LockInfo {
  return { name: grant.name, mode: grant.mode, clientId: grant.clientId }
}

function compatible(held: readonly Grant[], name: string, mode: LockMode): boolean {
  return held.every(
    (grant) => grant.name !== name || (grant.mode === 'shared' && mode === 'shared'),
  )
}

/** Creates the lock state of one browser profile. */
export function createFakeLocks(): FakeLocks {
  const held: Grant[] = []
  const waiting: Waiting[] = []
  let pageCount = 0

  function release(grant: Grant): void {
    const index = held.indexOf(grant)
    if (index < 0) {
      return
    }
    held.splice(index, 1)
    grantWaiting()
  }

  // Starts the waiting requests that can be granted now, in request order.
  function grantWaiting(): void {
    let index = 0
    while (index < waiting.length) {
      const next = waiting[index]
      if (next !== undefined && compatible(held, next.name, next.mode)) {
        waiting.splice(index, 1)
        next.start()
      } else {
        index += 1
      }
    }
  }

  function page(options: { readonly failQuery?: boolean } = {}): FakeLockPage {
    pageCount += 1
    const clientId = `page-${String(pageCount)}`
    const requested: string[] = []
    let ended = false

    function request(
      name: string,
      optionsOrCallback: LockOptions | LockGrantedCallback<unknown>,
      maybeCallback?: LockGrantedCallback<unknown>,
    ): Promise<unknown> {
      const callback = typeof optionsOrCallback === 'function' ? optionsOrCallback : maybeCallback
      const lockOptions = typeof optionsOrCallback === 'function' ? {} : optionsOrCallback
      if (callback === undefined) {
        return Promise.reject(new TypeError('A lock request needs a callback.'))
      }
      if (ended) {
        return Promise.reject(new DOMException('The page has ended.', 'InvalidStateError'))
      }
      if (lockOptions.steal === true || lockOptions.signal !== undefined) {
        return Promise.reject(
          new DOMException('The fake lock manager has no steal or signal.', 'NotSupportedError'),
        )
      }
      requested.push(name)
      const mode = lockOptions.mode ?? 'exclusive'
      const grant: Grant = { name, mode, clientId }
      return new Promise((resolve, reject) => {
        const start = (): void => {
          held.push(grant)
          Promise.resolve()
            .then(() => callback({ name, mode }))
            .then(
              (result) => {
                release(grant)
                resolve(result)
              },
              (error: unknown) => {
                release(grant)
                reject(error)
              },
            )
        }
        if (compatible(held, name, mode)) {
          start()
        } else if (lockOptions.ifAvailable === true) {
          Promise.resolve()
            .then(() => callback(null))
            .then(resolve, reject)
        } else {
          waiting.push({ ...grant, start })
        }
      })
    }

    const locks = {
      request,
      query: async (): Promise<LockManagerSnapshot> => {
        if (options.failQuery === true) {
          throw new DOMException('The lock manager is not available.', 'SecurityError')
        }
        return { held: held.map(lockInfo), pending: waiting.map(lockInfo) }
      },
    }

    return {
      // The fake has the members the app uses, not all of LockManager's overloads.
      locks: locks as unknown as LockManager,
      requested,
      end() {
        ended = true
        // The request promises of an ended page never settle: nothing is left to hear them.
        for (const grant of held.filter((candidate) => candidate.clientId === clientId)) {
          held.splice(held.indexOf(grant), 1)
        }
        for (const dropped of waiting.filter((candidate) => candidate.clientId === clientId)) {
          waiting.splice(waiting.indexOf(dropped), 1)
        }
        grantWaiting()
      },
    }
  }

  return {
    page,
    held: () => held.map((grant) => grant.name),
  }
}
