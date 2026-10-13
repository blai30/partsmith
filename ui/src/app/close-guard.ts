import type { TauriInvokeGlobal } from '../files/desktop-file-service'
import { selectUnsaved } from '../state/selectors'
import type { AppStore } from '../state/store'

// The UI half of the desktop close guard (desktop.md 3.6 and 3.7): the host asks before closing a
// window only while the UI reports unsaved changes.

/** Keeps the desktop host told whether closing the window would lose unsaved changes. Returns a function that stops it. */
export function installCloseGuard(store: AppStore, tauri: TauriInvokeGlobal): () => void {
  function report(unsaved: boolean): void {
    tauri.core.invoke('set_unsaved', { unsaved }).catch((error: unknown) => {
      console.warn('Partsmith could not tell the desktop host about unsaved changes.', error)
    })
  }

  // The last value sent, whether or not the host received it: a failure is not retried until the next change.
  let reported = selectUnsaved(store.getState())
  report(reported)
  return store.subscribe((state) => {
    const unsaved = selectUnsaved(state)
    if (unsaved !== reported) {
      reported = unsaved
      report(unsaved)
    }
  })
}
