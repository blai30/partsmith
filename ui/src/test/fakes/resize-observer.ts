import { vi } from 'vitest'

// jsdom has no ResizeObserver. This stand-in records what is observed and calls back only when a
// test fires it, as a layout change would.

export type FakeResizeObservers = {
  /** The elements every live observer watches, in the order they were observed. */
  observed(): readonly Element[]
  /** Calls the callback of every live observer that watches something. */
  fire(): void
}

/** Stubs the global `ResizeObserver` with the stand-in; `vi.unstubAllGlobals()` removes it. */
export function stubResizeObserver(): FakeResizeObservers {
  const live = new Set<FakeResizeObserver>()

  class FakeResizeObserver implements ResizeObserver {
    readonly targets = new Set<Element>()
    readonly callback: ResizeObserverCallback

    constructor(callback: ResizeObserverCallback) {
      this.callback = callback
      live.add(this)
    }

    observe(target: Element): void {
      this.targets.add(target)
    }

    unobserve(target: Element): void {
      this.targets.delete(target)
    }

    disconnect(): void {
      this.targets.clear()
      live.delete(this)
    }
  }

  vi.stubGlobal('ResizeObserver', FakeResizeObserver)
  return {
    observed: () => Array.from(live).flatMap((observer) => Array.from(observer.targets)),
    fire() {
      for (const observer of live) {
        if (observer.targets.size > 0) {
          observer.callback([], observer)
        }
      }
    },
  }
}
