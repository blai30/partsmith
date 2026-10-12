import type { AppStore, Notification, PickHint } from '../../state/store'
import { nowOf } from '../connection'
import type { App } from '../createApp'

// Notifications (ui.md 3.22): at most three shown, info and success dismissed after 5 s, warnings
// after 8 s, errors kept until dismissed. Timers pause while the user points at or focuses one.

/** Milliseconds a notification stays, or null to keep it until dismissed. */
const LIFETIMES: Readonly<Record<Notification['severity'], number | null>> = {
  info: 5_000,
  success: 5_000,
  warning: 8_000,
  error: null,
}

/** The most notifications shown at once. */
const MAXIMUM_SHOWN = 3

type Countdown = {
  /** Milliseconds left when the countdown is paused, else when it last started. */
  remaining: number
  startedAt: number
  timer: ReturnType<typeof setTimeout> | null
}

type NotificationTimers = {
  nextNumber: number
  readonly countdowns: Map<string, Countdown>
  /** Notifications kept until acted on, which never count down. */
  readonly persistent: Set<string>
}

const timersByStore = new WeakMap<AppStore, NotificationTimers>()

function timersOf(store: AppStore): NotificationTimers {
  let timers = timersByStore.get(store)
  if (timers === undefined) {
    timers = { nextNumber: 1, countdowns: new Map(), persistent: new Set() }
    timersByStore.set(store, timers)
  }
  return timers
}

function stopCountdown(store: AppStore, id: string): void {
  const timers = timersOf(store)
  const countdown = timers.countdowns.get(id)
  if (countdown?.timer !== null && countdown?.timer !== undefined) {
    clearTimeout(countdown.timer)
  }
  timers.countdowns.delete(id)
}

function runCountdown(store: AppStore, id: string, countdown: Countdown): void {
  countdown.startedAt = Date.now()
  countdown.timer = setTimeout(() => {
    countdown.timer = null
    removeNotification(store, id)
  }, countdown.remaining)
}

function startCountdown(store: AppStore, notification: Notification): void {
  stopCountdown(store, notification.id)
  const lifetime = LIFETIMES[notification.severity]
  if (lifetime === null || timersOf(store).persistent.has(notification.id)) {
    return
  }
  const countdown: Countdown = { remaining: lifetime, startedAt: 0, timer: null }
  timersOf(store).countdowns.set(notification.id, countdown)
  runCountdown(store, notification.id, countdown)
}

function removeNotification(store: AppStore, id: string): void {
  stopCountdown(store, id)
  timersOf(store).persistent.delete(id)
  store.setState((state) => {
    const notifications = state.notifications.filter((notification) => notification.id !== id)
    return notifications.length === state.notifications.length ? {} : { notifications }
  })
}

/** The notification to drop to make room: the oldest that is not an error, else the oldest. */
function oldestRemovable(notifications: readonly Notification[]): Notification | undefined {
  return notifications.find((notification) => notification.severity !== 'error') ?? notifications[0]
}

export type NotifyOptions = {
  /** Keeps an info, success or warning notification until the user acts on it or dismisses it. */
  readonly persistent?: boolean
}

/**
 * Shows a notification and returns its id. A message already shown is not shown twice: that one
 * takes the new actions, code and named features (they belong to the latest occurrence, such as the
 * command a Restore reopens) and restarts its timer (3.22).
 */
export function notify(
  app: App,
  notification: Omit<Notification, 'id' | 'createdAt'>,
  options: NotifyOptions = {},
): string {
  const { store } = app
  const persistent = options.persistent ?? false
  const timers = timersOf(store)
  const shown = store
    .getState()
    .notifications.find((candidate) => candidate.message === notification.message)
  if (shown !== undefined) {
    const latest: Notification = { ...notification, id: shown.id, createdAt: shown.createdAt }
    if (persistent) {
      timers.persistent.add(shown.id)
    }
    store.setState((state) => ({
      notifications: state.notifications.map((candidate) =>
        candidate.id === shown.id ? latest : candidate,
      ),
    }))
    startCountdown(store, latest)
    return shown.id
  }
  const added: Notification = {
    ...notification,
    id: `notification-${String(timers.nextNumber)}`,
    createdAt: nowOf(app),
  }
  timers.nextNumber += 1
  if (persistent) {
    timers.persistent.add(added.id)
  }
  let notifications = [...store.getState().notifications, added]
  while (notifications.length > MAXIMUM_SHOWN) {
    const dropped = oldestRemovable(notifications)
    if (dropped === undefined) {
      break
    }
    stopCountdown(store, dropped.id)
    timers.persistent.delete(dropped.id)
    notifications = notifications.filter((candidate) => candidate.id !== dropped.id)
  }
  store.setState({ notifications })
  if (notifications.includes(added)) {
    startCountdown(store, added)
  }
  return added.id
}

/** Removes a notification; an id that is not shown is ignored. */
export function dismiss(app: App, id: string): void {
  removeNotification(app.store, id)
}

/**
 * Dismisses each failure notification once none of the features it names is in error anymore,
 * after an undo, a fix or a deletion (3.22, 4.2). Returns the function that stops it.
 */
export function installFailureDismissal(app: App): () => void {
  return app.store.subscribe((state, previous) => {
    const statuses = state.document?.featureStatus
    if (statuses === previous.document?.featureStatus) {
      return
    }
    for (const notification of state.notifications) {
      const features = notification.features
      if (features !== undefined && features.every((id) => statuses?.[id]?.status !== 'error')) {
        removeNotification(app.store, notification.id)
      }
    }
  })
}

/** Holds a notification's countdown while the pointer is over it or focus is inside it. */
export function pauseNotification(app: App, id: string): void {
  const countdown = timersOf(app.store).countdowns.get(id)
  if (countdown === undefined || countdown.timer === null) {
    return
  }
  clearTimeout(countdown.timer)
  countdown.timer = null
  countdown.remaining = Math.max(0, countdown.remaining - (Date.now() - countdown.startedAt))
}

/** Continues a paused countdown with the time it had left. */
export function resumeNotification(app: App, id: string): void {
  const countdown = timersOf(app.store).countdowns.get(id)
  if (countdown === undefined || countdown.timer !== null) {
    return
  }
  runCountdown(app.store, id, countdown)
}

/** How long the hint about a refused pick stays (3.7). */
const PICK_HINT_MILLISECONDS = 3_000

/**
 * Shows the hint about a refused pick for 3 s: under the active command's selection field `field`,
 * or in the sketch hint for null (3.7, 3.16.1). A newer hint replaces it and gets its own 3 s.
 */
export function showPickHint(app: App, message: string, field: string | null): void {
  const hint: PickHint = { message, field }
  app.store.setState({ pickHint: hint })
  setTimeout(() => {
    app.store.setState((state) => (state.pickHint === hint ? { pickHint: null } : {}))
  }, PICK_HINT_MILLISECONDS)
}

/** Says a message through the polite live region, such as "Undid Rename" (3.20, 3.24). */
export function announce(app: App, message: string): void {
  app.store.setState((state) => ({
    announcement: { id: (state.announcement?.id ?? 0) + 1, message },
  }))
}
