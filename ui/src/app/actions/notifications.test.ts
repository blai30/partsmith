import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import type { Notification } from '../../state/store'
import { createTestApp } from '../../test/fakes/app'
import type { App } from '../createApp'
import { dismiss, notify } from './notifications'

type Severity = Notification['severity']

function show(app: App, severity: Severity, message: string): string {
  return notify(app, { severity, message, actions: [] })
}

function messages(app: App): string[] {
  return app.store.getState().notifications.map((notification) => notification.message)
}

beforeEach(() => {
  vi.useFakeTimers()
})

afterEach(() => {
  vi.useRealTimers()
})

describe('notify', () => {
  it('adds the notification with a new id, its actions and a creation time, and returns the id', () => {
    const app = createTestApp()
    const run = vi.fn<() => void>()
    const id = notify(app, {
      severity: 'error',
      message: 'Could not save cube.ptsm: The disk is full.',
      actions: [{ label: 'Save as', run }],
    })
    const [notification] = app.store.getState().notifications
    expect(notification).toMatchObject({
      id,
      severity: 'error',
      message: 'Could not save cube.ptsm: The disk is full.',
    })
    expect(notification?.actions.map((action) => action.label)).toEqual(['Save as'])
    expect(typeof notification?.createdAt).toBe('number')
    const otherId = show(app, 'info', 'Exported cube.3mf.')
    expect(otherId).not.toBe(id)
  })

  it('keeps the newest at the end', () => {
    const app = createTestApp()
    show(app, 'error', 'First.')
    show(app, 'error', 'Second.')
    expect(messages(app)).toEqual(['First.', 'Second.'])
  })

  it('U-72 an info notification disappears after 5 s', () => {
    const app = createTestApp()
    show(app, 'info', 'Exported cube.3mf.')
    vi.advanceTimersByTime(4_999)
    expect(messages(app)).toEqual(['Exported cube.3mf.'])
    vi.advanceTimersByTime(1)
    expect(messages(app)).toEqual([])
  })

  it('a success notification disappears after 5 s', () => {
    const app = createTestApp()
    show(app, 'success', 'Exported cube.3mf.')
    vi.advanceTimersByTime(4_999)
    expect(messages(app)).toHaveLength(1)
    vi.advanceTimersByTime(1)
    expect(messages(app)).toEqual([])
  })

  it('U-72 a warning disappears after 8 s', () => {
    const app = createTestApp()
    show(app, 'warning', 'Vertical would over-constrain the sketch: it conflicts with Horizontal.')
    vi.advanceTimersByTime(7_999)
    expect(messages(app)).toHaveLength(1)
    vi.advanceTimersByTime(1)
    expect(messages(app)).toEqual([])
  })

  it('U-72 an error stays until dismissed', () => {
    const app = createTestApp()
    const id = show(app, 'error', 'Extrude 1 failed. The profile is empty.')
    vi.advanceTimersByTime(600_000)
    expect(messages(app)).toEqual(['Extrude 1 failed. The profile is empty.'])
    dismiss(app, id)
    expect(messages(app)).toEqual([])
  })

  it('U-72 a fourth notification removes the oldest non-error one', () => {
    const app = createTestApp()
    show(app, 'error', 'Error one.')
    show(app, 'info', 'Info one.')
    show(app, 'warning', 'Warning one.')
    show(app, 'info', 'Info two.')
    expect(messages(app)).toEqual(['Error one.', 'Warning one.', 'Info two.'])
  })

  it('a fourth notification removes the oldest when all are errors', () => {
    const app = createTestApp()
    show(app, 'error', 'Error one.')
    show(app, 'error', 'Error two.')
    show(app, 'error', 'Error three.')
    show(app, 'error', 'Error four.')
    expect(messages(app)).toEqual(['Error two.', 'Error three.', 'Error four.'])
  })

  it('U-72 a duplicate of a visible message adds nothing and restarts the visible one\'s timer', () => {
    const app = createTestApp()
    show(app, 'info', 'Nothing to undo.')
    vi.advanceTimersByTime(3_000)
    show(app, 'info', 'Nothing to undo.')
    expect(messages(app)).toEqual(['Nothing to undo.'])
    vi.advanceTimersByTime(4_999)
    expect(messages(app)).toEqual(['Nothing to undo.'])
    vi.advanceTimersByTime(1)
    expect(messages(app)).toEqual([])
  })
})

describe('dismiss', () => {
  it('removes only that notification', () => {
    const app = createTestApp()
    const first = show(app, 'error', 'First.')
    show(app, 'error', 'Second.')
    dismiss(app, first)
    expect(messages(app)).toEqual(['Second.'])
  })

  it('ignores an id that is not shown', () => {
    const app = createTestApp()
    show(app, 'error', 'First.')
    dismiss(app, 'no-such-notification')
    expect(messages(app)).toEqual(['First.'])
  })

  it('a dismissed info notification does not come back when its timer would have fired', () => {
    const app = createTestApp()
    const id = show(app, 'info', 'Exported cube.3mf.')
    dismiss(app, id)
    show(app, 'error', 'Later.')
    vi.advanceTimersByTime(5_000)
    expect(messages(app)).toEqual(['Later.'])
  })
})
