// Captured before any test installs fake timers, so settle() always waits on the real clock.
const realSetTimeout = globalThis.setTimeout

/**
 * Lets pending promise chains (requests through a fake transport, their events and the code awaiting
 * them) run to completion. Works with real and fake timers alike: it waits for real macrotasks and
 * never advances fake time. Each of the `rounds` waits is one real macrotask.
 */
export async function settle(rounds = 10): Promise<void> {
  await new Promise<void>((resolve) => {
    realSetTimeout(resolve, 0)
  })
  if (rounds > 1) {
    await settle(rounds - 1)
  }
}
