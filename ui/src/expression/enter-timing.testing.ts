// Test support for input that arrives as an expression field's result shows: after the commit
// that shows the result and before React runs that render's passive effects, the window in which
// E-13 lost its Enter on WebKit (ui.md 2.13, 3.5). Nothing here is part of the app.

import { deferred } from '../test/fakes/deferred'
import { settle } from '../test/fakes/settle'

/** The global flag through which React tells whether updates run inside `act`. */
export type ActEnvironment = { IS_REACT_ACT_ENVIRONMENT?: boolean | undefined }

/** How long the result may take to show, well within a test's timeout so the failure says why. */
export const RESULT_TIMEOUT_MILLISECONDS = 2_000

/** Presses Enter in `input` as a browser delivers it: a `keydown` that bubbles and can be canceled. */
export function pressEnter(input: HTMLElement): void {
  input.dispatchEvent(
    new KeyboardEvent('keydown', { key: 'Enter', bubbles: true, cancelable: true }),
  )
}

/**
 * Runs `settleEvaluation`, which lets the field's evaluation finish, runs `action` as soon as
 * `result` shows `text`, then lets the app settle. Outside an act environment React schedules its
 * work as in a browser, so the action comes right after the commit that shows the result and
 * before the passive effects of that render, which React runs in a later task. Fails at once when
 * `result` already shows `text`, and after `RESULT_TIMEOUT_MILLISECONDS` when it never does.
 */
export async function whenResultShows(
  result: HTMLElement,
  text: string,
  settleEvaluation: () => void,
  action: () => void,
): Promise<void> {
  if (result.textContent === text) {
    throw new Error(`The result already shows '${text}', so no commit can show it.`)
  }
  const shown = deferred<undefined>()
  const observer = new MutationObserver(() => {
    if (result.textContent === text) {
      shown.resolve(undefined)
    }
  })
  const timeout = setTimeout(() => {
    shown.reject(new Error(`The result never showed '${text}'.`))
  }, RESULT_TIMEOUT_MILLISECONDS)
  const environment = globalThis as ActEnvironment
  const actEnvironment = environment.IS_REACT_ACT_ENVIRONMENT
  environment.IS_REACT_ACT_ENVIRONMENT = false
  try {
    observer.observe(result, { childList: true, characterData: true, subtree: true })
    settleEvaluation()
    // The observer resolves in the microtask after the commit that shows the text, and this
    // continuation runs in the same microtask checkpoint, before the task of the passive effects.
    await shown.promise
    action()
    await settle()
  } finally {
    observer.disconnect()
    clearTimeout(timeout)
    environment.IS_REACT_ACT_ENVIRONMENT = actEnvironment
  }
}
