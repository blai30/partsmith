import { clsx } from 'clsx/lite'
import { CircleCheck, X } from 'lucide-react'
import { useRef } from 'react'
import { dismiss, pauseNotification, resumeNotification } from '../app/actions/notifications'
import { useApp, useAppState } from '../app/AppProvider'
import { Button } from '../components/Button'
import { IconButton } from '../components/IconButton'
import { SeverityIcon } from '../print/SeverityIcon'
import type { Notification } from '../state/store'
import { COLOR_CLASSES } from '../theme/roles'

// Notifications (ui.md 3.22): a stack at the viewport's bottom-right, newest at the bottom. The
// stack is the shell's last region, placed in the viewport's grid area, so it comes last in the tab
// order (3.24). Errors are announced through an alert region and the others through a status
// region; both regions stay mounted, so a message added to them is announced, which a region
// inserted with its text often is not. A notification's countdown pauses while the pointer is over
// it or focus is inside it.

function NotificationIcon({ severity }: { readonly severity: Notification['severity'] }) {
  if (severity === 'success') {
    return (
      <CircleCheck
        aria-hidden="true"
        strokeWidth={1.75}
        className={clsx('size-4 shrink-0', COLOR_CLASSES.successMark.foreground)}
      />
    )
  }
  return <SeverityIcon severity={severity} />
}

function NotificationCard({ notification }: { readonly notification: Notification }) {
  const app = useApp()
  // Whether the pointer is over the card and whether focus is inside it; either holds the countdown.
  const holds = useRef({ pointer: false, focus: false })

  function hold(kind: 'pointer' | 'focus', held: boolean): void {
    const wasHeld = holds.current.pointer || holds.current.focus
    holds.current = { ...holds.current, [kind]: held }
    const isHeld = holds.current.pointer || holds.current.focus
    if (isHeld && !wasHeld) {
      pauseNotification(app, notification.id)
    } else if (!isHeld && wasHeld) {
      resumeNotification(app, notification.id)
    }
  }

  return (
    <div
      data-testid="notification"
      data-severity={notification.severity}
      data-code={notification.code}
      onPointerEnter={() => {
        hold('pointer', true)
      }}
      onPointerLeave={() => {
        hold('pointer', false)
      }}
      onFocus={() => {
        hold('focus', true)
      }}
      onBlur={(event) => {
        if (
          !(
            event.relatedTarget instanceof Node && event.currentTarget.contains(event.relatedTarget)
          )
        ) {
          hold('focus', false)
        }
      }}
      className={clsx(
        'pointer-events-auto flex w-90 max-w-full items-start gap-3 rounded-md border p-3 text-sm shadow-lg transition-opacity duration-150 ease-out starting:opacity-0 motion-reduce:transition-none',
        COLOR_CLASSES.surfaceRaised.background,
        COLOR_CLASSES.border.border,
        COLOR_CLASSES.text.foreground,
      )}
    >
      <span className="flex h-5 items-center">
        <NotificationIcon severity={notification.severity} />
      </span>
      <div className="flex min-w-0 flex-1 flex-col gap-2">
        <p className="text-left break-words">{notification.message}</p>
        {notification.actions.length === 0 ? null : (
          <div className="flex flex-wrap gap-2">
            {notification.actions.map((action) => (
              <Button
                key={action.label}
                onClick={() => {
                  dismiss(app, notification.id)
                  action.run()
                }}
              >
                {action.label}
              </Button>
            ))}
          </div>
        )}
      </div>
      <IconButton
        label="Dismiss"
        icon={X}
        size="compact"
        onClick={() => {
          dismiss(app, notification.id)
        }}
      />
    </div>
  )
}

/** The messages of the notifications a live region announces. */
function Announcements({ notifications }: { readonly notifications: readonly Notification[] }) {
  return notifications.map((notification) => <p key={notification.id}>{notification.message}</p>)
}

export function Notifications() {
  const notifications = useAppState((state) => state.notifications)
  const errors = notifications.filter((notification) => notification.severity === 'error')
  const others = notifications.filter((notification) => notification.severity !== 'error')
  return (
    <>
      <div role="alert" data-testid="notification-alerts" className="sr-only">
        <Announcements notifications={errors} />
      </div>
      <output data-testid="notification-status" className="sr-only">
        <Announcements notifications={others} />
      </output>
      {notifications.length === 0 ? null : (
        <div className="pointer-events-none relative z-30 col-start-2 row-start-2 flex min-h-0 min-w-0 flex-col items-end justify-end gap-2 overflow-hidden p-4">
          {notifications.map((notification) => (
            <NotificationCard key={notification.id} notification={notification} />
          ))}
        </div>
      )}
    </>
  )
}
