import { createOneAtATime } from '../../core/queue'
import type { AppStore } from '../../state/store'
import type { App } from '../createApp'
import { confirmRunning, confirmSettled } from './command'
import { inputLanded, inputOnItsWay } from './selection'

// Starts of ui.md 3.7 step 1, one at a time per store: feature commands, edits and the ways into
// sketch mode (3.16.1). What acts on what a start opens (Esc, Enter, the Model and Print contexts,
// Finish sketch, 3.5) takes its turn after the starts asked for before it.

const turnsOneAtATime = createOneAtATime<AppStore>()

/** Per store: the starts asked for and not run yet, and how many have been asked for in all. */
type Starts = { inProgress: number; asked: number }

const startsOfStores = new WeakMap<AppStore, Starts>()

function startsOf(app: App): Starts {
  let starts = startsOfStores.get(app.store)
  if (starts === undefined) {
    starts = { inProgress: 0, asked: 0 }
    startsOfStores.set(app.store, starts)
  }
  return starts
}

/**
 * Runs `turn` once the turns asked for before it have run, and after OK when it runs then, so it
 * acts on what they left (3.7).
 */
function inTurn(app: App, turn: () => void | Promise<void>): Promise<void> {
  return turnsOneAtATime(app.store, async () => {
    if (confirmRunning(app)) {
      await confirmSettled(app)
    }
    await turn()
  })
}

/**
 * A start of 3.7 step 1, run in its turn: after any start still in progress (a double-click, a held
 * key), so the second never begins a transaction while the first is opening one, after OK while it
 * runs, and once the clicks and selection changes made before it have landed (3.6), so it ends a
 * command and reads the selection as they leave them, however late the core answers them.
 */
export function startInTurn(app: App, start: () => Promise<void>): Promise<void> {
  const starts = startsOf(app)
  starts.asked += 1
  starts.inProgress += 1
  // With no input on its way, the start runs at once.
  const inputBefore = inputOnItsWay(app) ? inputLanded(app) : null
  return inTurn(app, async () => {
    try {
      if (inputBefore !== null) {
        await inputBefore
      }
      await start()
    } finally {
      starts.inProgress -= 1
    }
  })
}

/** Whether a start has been asked for and has not run yet (3.5, 3.7 step 1). */
export function startInProgress(app: App): boolean {
  return (startsOfStores.get(app.store)?.inProgress ?? 0) > 0
}

/** How many starts have been asked for so far, so an Enter tells whether one came after it (3.5). */
export function startsAsked(app: App): number {
  return startsOfStores.get(app.store)?.asked ?? 0
}

/**
 * Runs `action` at once, or, while a start is in progress, in its turn after it, so Esc, Enter,
 * the Model and Print contexts and Finish sketch act on what the start opened (3.5, 3.16.1).
 */
export function afterStarts(app: App, action: () => void | Promise<void>): Promise<void> {
  if (startInProgress(app)) {
    return inTurn(app, action)
  }
  return Promise.resolve(action())
}
