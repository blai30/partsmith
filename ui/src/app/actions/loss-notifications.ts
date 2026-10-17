import type { Cause, FeatureStatus } from '../../generated/protocol'
import { MESSAGES } from '../../messages'
import { changeNotifications, type ChangeNotice } from '../../problems/problems'
import { selectFeatureLabel } from '../../state/selectors'
import type { DocumentState, Notification } from '../../state/store'
import type { App } from '../createApp'
import { editFeature } from './command'
import { undo } from './document'
import { documentOf } from './edits'
import { fixNotificationAction, showLost } from './fixes'
import { notify } from './notifications'
import { showProblemsAction } from './panels'

// The notifications of ui.md 4.2 for what a user change broke: one per root with a new loss, then
// one for the features newly in error that none of those names.

type NotificationAction = Notification['actions'][number]

/** A notification before `notify` gives it an id and a time. */
type NewNotification = Omit<Notification, 'id' | 'createdAt'>

type CheckNotice = Extract<ChangeNotice, { readonly kind: 'check' }>

type LossNotice = Extract<ChangeNotice, { readonly kind: 'lost' }>

function undoAction(app: App): NotificationAction {
  return {
    label: 'Undo',
    run: () => {
      void undo(app)
    },
  }
}

/** The first `apply` action of a cause line, offered by a loss notification (4.2), or none. */
function causeRevert(app: App, cause: Cause | null): Notification['actions'] {
  const revert = cause?.actions.find((action) => action.run.kind === 'apply')
  return cause === null || revert === undefined ? [] : [fixNotificationAction(app, revert, cause)]
}

/** The features, losses and cause line a check or loss notification names (4.2). */
function namedLosses(notice: CheckNotice | LossNotice) {
  return {
    features: notice.features,
    problems: notice.problems,
    ...(notice.cause === null ? {} : { cause: notice.cause.text }),
  }
}

/**
 * The notification of features newly in error that no loss names (4.2): the feature's own first
 * message for one, a count for several; null for none.
 */
function failureNotification(
  app: App,
  document: DocumentState,
  features: readonly string[],
): NewNotification | null {
  const [first] = features
  if (first === undefined) {
    return null
  }
  if (features.length > 1) {
    return {
      severity: 'error',
      message: MESSAGES.features_failed(features.length),
      code: 'features_failed',
      features,
      actions: [showProblemsAction(app), undoAction(app)],
    }
  }
  const message = document.featureStatus[first]?.messages[0]?.message ?? ''
  return {
    severity: 'error',
    message: MESSAGES.feature_failed(
      selectFeatureLabel(app.store.getState(), first),
      message,
    ).trimEnd(),
    code: 'feature_failed',
    features,
    actions: [
      {
        label: 'Edit',
        run: () => {
          void editFeature(app, first)
        },
      },
      undoAction(app),
    ],
  }
}

/** The notification of a root whose reference is still used but needs a check (4.2). */
function checkNotification(
  app: App,
  document: DocumentState,
  notice: CheckNotice,
): NewNotification {
  const label = selectFeatureLabel(app.store.getState(), notice.root)
  const hasRepairs = (document.featureStatus[notice.root]?.repairs ?? []).length > 0
  return {
    ...namedLosses(notice),
    severity: 'warning',
    message: MESSAGES.feature_check(label),
    code: 'feature_check',
    actions: [
      {
        label: MESSAGES.check_feature(label),
        run: () => {
          void (hasRepairs ? showLost(app, notice.root) : editFeature(app, notice.root))
        },
      },
      ...causeRevert(app, notice.cause),
      undoAction(app),
    ],
  }
}

/** The notification of a root that lost a reference or region (4.2), with the features that failed after it. */
function lossNotification(app: App, notice: LossNotice): NewNotification {
  const label = selectFeatureLabel(app.store.getState(), notice.root)
  return {
    ...namedLosses(notice),
    severity: notice.severity,
    message:
      notice.failed === 0
        ? MESSAGES.feature_lost(label, notice.description)
        : MESSAGES.features_lost(label, notice.description, notice.failed),
    code: notice.failed === 0 ? 'feature_lost' : 'features_lost',
    // Never a candidate: a replacement cannot be judged without seeing it (4.2).
    actions: [
      {
        label: MESSAGES.show_lost(label),
        run: () => {
          void showLost(app, notice.root)
        },
      },
      ...causeRevert(app, notice.cause),
      undoAction(app),
    ],
  }
}

/** The notification of one notice of a change (4.2), or null when it names nothing to show. */
function changeNotification(
  app: App,
  document: DocumentState,
  notice: ChangeNotice,
): NewNotification | null {
  switch (notice.kind) {
    case 'failed':
      return failureNotification(app, document, notice.features)
    case 'check':
      return checkNotification(app, document, notice)
    case 'lost':
      return lossNotification(app, notice)
  }
}

/**
 * Shows the notifications of 4.2 for what a user change broke, from the statuses before it and
 * now (`changeNotifications`): one per root with a new loss, then one for the features newly in
 * error that none of those names. Returns how many it showed.
 */
export function reportChange(app: App, before: Readonly<Record<string, FeatureStatus>>): number {
  const document = documentOf(app)
  if (document === null) {
    return 0
  }
  const notices = changeNotifications(before, document.featureStatus, document.recipe)
  let shown = 0
  for (const notice of notices) {
    const notification = changeNotification(app, document, notice)
    if (notification !== null) {
      notify(app, notification)
      shown += 1
    }
  }
  return shown
}
