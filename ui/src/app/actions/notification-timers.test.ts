import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { createTestApp, notificationMessages } from '../../test/fakes/app'
import { notify, pauseNotification, resumeNotification } from './notifications'

// Countdown pauses and persistent notifications (ui.md 3.22, 3.19).

beforeEach(() => {
  vi.useFakeTimers()
})

afterEach(() => {
  vi.useRealTimers()
})

describe('pauseNotification and resumeNotification', () => {
  it('hold the countdown while paused and continue with the time that was left', () => {
    const app = createTestApp()
    const id = notify(app, { severity: 'info', message: 'Exported cube.3mf.', actions: [] })
    vi.advanceTimersByTime(3_000)
    pauseNotification(app, id)
    vi.advanceTimersByTime(60_000)
    expect(notificationMessages(app)).toEqual(['Exported cube.3mf.'])
    resumeNotification(app, id)
    vi.advanceTimersByTime(1_999)
    expect(notificationMessages(app)).toEqual(['Exported cube.3mf.'])
    vi.advanceTimersByTime(1)
    expect(notificationMessages(app)).toEqual([])
  })

  it('ignore errors, which never count down', () => {
    const app = createTestApp()
    const id = notify(app, { severity: 'error', message: 'It failed.', actions: [] })
    pauseNotification(app, id)
    resumeNotification(app, id)
    vi.advanceTimersByTime(60_000)
    expect(notificationMessages(app)).toEqual(['It failed.'])
  })
})

describe('persistent notifications', () => {
  it('stay until dismissed, also when shown again', () => {
    const app = createTestApp()
    const message = 'Partsmith kept unsaved changes to cube from Oct 12, 2026, 9:30 AM.'
    notify(app, { severity: 'info', message, actions: [] }, { persistent: true })
    vi.advanceTimersByTime(60_000)
    notify(app, { severity: 'info', message, actions: [] })
    vi.advanceTimersByTime(60_000)
    expect(notificationMessages(app)).toEqual([message])
  })
})
