// Test support for an Enter pressed in an expression field as its result shows: after the commit
// that shows the result and before React runs that render's passive effects, the window in which
// E-13 lost its Enter on WebKit (ui.md 3.5). Nothing here is part of the app.

import { settle } from '../test/fakes/settle'

type ActEnvironment = { IS_REACT_ACT_ENVIRONMENT?: boolean | undefined }

/**
 * Runs `settleEvaluation`, which lets the field's evaluation finish, presses Enter in `input` as
 * soon as `result` shows `text`, then lets the app settle. Outside an act environment React
 * schedules its work as in a browser, so the press comes right after the commit that shows the
 * result and before the passive effects of that render, which React runs in a later task.
 */
export async function pressEnterAsResultShows(
  input: HTMLElement,
  result: HTMLElement,
  text: string,
  settleEvaluation: () => void,
): Promise<void> {
  const environment = globalThis as ActEnvironment
  const actEnvironment = environment.IS_REACT_ACT_ENVIRONMENT
  environment.IS_REACT_ACT_ENVIRONMENT = false
  try {
    const pressed = new Promise<void>((resolve) => {
      const observer = new MutationObserver(() => {
        if (result.textContent !== text) {
          return
        }
        observer.disconnect()
        input.dispatchEvent(
          new KeyboardEvent('keydown', { key: 'Enter', bubbles: true, cancelable: true }),
        )
        resolve()
      })
      observer.observe(result, { childList: true, characterData: true, subtree: true })
    })
    settleEvaluation()
    await pressed
    await settle()
  } finally {
    environment.IS_REACT_ACT_ENVIRONMENT = actEnvironment
  }
}
