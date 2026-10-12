// Captured before any test installs fake timers, so settle() always waits on the real clock.
const realSetTimeout = globalThis.setTimeout

/**
 * Lets pending promise chains (requests through a fake transport, their events and the code awaiting
 * them) run to completion. Works with real and fake timers alike: it waits for real macrotasks and
 * never advances fake time.
 */
export async function settle(): Promise<void> {
  for (let round = 0; round < 10; round += 1) {
    await new Promise<void>((resolve) => {
      realSetTimeout(resolve, 0)
    })
  }
}
