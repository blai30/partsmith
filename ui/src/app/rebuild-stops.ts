import type { CoreEvent } from '../core/events'
import { featureLabel } from '../core/labels'
import { MESSAGES } from '../messages'
import { dismiss, notify } from './actions/notifications'
import { reportFailure } from './report'
import type { App } from './createApp'

// The rebuild_stopped notification (ui.md 4.4, ADR 0063): a response whose recompute stopped
// offers Rebuild, and a later response with every feature built takes it back.

/** The id of the shown rebuild_stopped notification, by app. */
const shownNotifications = new WeakMap<App, string>()

/**
 * Follows the `recomputed` event of one response. `stoppedByApp` is true for a preview the app
 * stopped itself, for a newer value or Esc, which shows no notification.
 */
export function noteRebuildStops(
  app: App,
  event: Extract<CoreEvent, { type: 'recomputed' }>,
  stoppedByApp: boolean,
): void {
  const notBuilt = event.features.filter((status) => status.status === 'not_run')
  const [first] = notBuilt
  const shown = shownNotifications.get(app)
  if (first === undefined) {
    if (shown !== undefined) {
      shownNotifications.delete(app)
      dismiss(app, shown)
    }
    return
  }
  if (stoppedByApp) {
    return
  }
  const { catalog, document } = app.store.getState()
  const feature = document === null ? first.id : featureLabel(document.recipe, first.id, catalog)
  const message =
    notBuilt.length === 1
      ? MESSAGES.rebuild_stopped_one(feature)
      : MESSAGES.rebuild_stopped(feature, notBuilt.length)
  const { documentHandle } = event
  if (shown !== undefined) {
    dismiss(app, shown)
  }
  shownNotifications.set(
    app,
    notify(app, {
      severity: 'info',
      message,
      actions: [
        {
          label: 'Rebuild',
          run: () => {
            app.mutations
              .run(() => app.core.rebuild(documentHandle))
              .catch(reportFailure('Rebuilding the model'))
          },
        },
      ],
    }),
  )
}
