import { act, cleanup, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { COLOR_CLASSES } from '../theme/roles'
import { installPopoverPolyfill } from './popover-polyfill.testing'
import { Tooltip } from './Tooltip'

let uninstallPopovers: () => void

beforeEach(() => {
  vi.useFakeTimers()
  uninstallPopovers = installPopoverPolyfill()
})

afterEach(() => {
  cleanup()
  uninstallPopovers()
  vi.useRealTimers()
})

function setUp() {
  const user = userEvent.setup({
    advanceTimers: (delay) => {
      vi.advanceTimersByTime(delay)
    },
  })
  render(
    <>
      <button type="button">Before</button>
      <Tooltip label="Save" shortcut="Ctrl+S">
        <button type="button">Save file</button>
      </Tooltip>
    </>,
  )
  return { user, trigger: screen.getByRole('button', { name: 'Save file' }) }
}

function advance(milliseconds: number): void {
  act(() => {
    vi.advanceTimersByTime(milliseconds)
  })
}

// The tooltip shown to the user: in the accessibility tree, so not a closed popover.
function shownTooltip(): HTMLElement | null {
  return screen.queryByRole('tooltip')
}

describe('Tooltip', () => {
  it('is hidden at first', () => {
    setUp()
    expect(shownTooltip()).toBeNull()
  })

  it('appears after 500 ms of hover, not before', async () => {
    const { user, trigger } = setUp()
    await user.hover(trigger)
    advance(499)
    expect(shownTooltip()).toBeNull()
    advance(1)
    expect(shownTooltip()).not.toBeNull()
  })

  it('shows the command title and the formatted shortcut', async () => {
    const { user, trigger } = setUp()
    await user.hover(trigger)
    advance(500)
    const tooltip = shownTooltip()
    expect(tooltip).toHaveTextContent('Save')
    expect(tooltip).toHaveTextContent('Ctrl+S')
  })

  it('is referenced by aria-describedby of its trigger', async () => {
    const { user, trigger } = setUp()
    await user.hover(trigger)
    advance(500)
    const tooltip = shownTooltip()
    expect(tooltip?.id).toBeTruthy()
    expect(trigger.getAttribute('aria-describedby')?.split(' ')).toContain(tooltip?.id)
    expect(trigger).toHaveAccessibleDescription(/Save/)
  })

  it('appears immediately on keyboard focus', async () => {
    const { user, trigger } = setUp()
    await user.tab()
    await user.tab()
    expect(trigger).toHaveFocus()
    expect(shownTooltip()).not.toBeNull()
  })

  it('closes on Esc', async () => {
    const { user, trigger } = setUp()
    await user.tab()
    await user.tab()
    expect(trigger).toHaveFocus()
    expect(shownTooltip()).not.toBeNull()
    await user.keyboard('{Escape}')
    advance(0)
    expect(shownTooltip()).toBeNull()
    expect(trigger).toHaveFocus()
  })

  it('closes when the pointer leaves', async () => {
    const { user, trigger } = setUp()
    await user.hover(trigger)
    advance(500)
    expect(shownTooltip()).not.toBeNull()
    await user.unhover(trigger)
    advance(1000)
    expect(shownTooltip()).toBeNull()
  })

  it('closes when focus leaves', async () => {
    const { user } = setUp()
    await user.tab()
    await user.tab()
    expect(shownTooltip()).not.toBeNull()
    await user.tab({ shift: true })
    advance(1000)
    expect(shownTooltip()).toBeNull()
  })

  it('does not appear when the pointer leaves before 500 ms', async () => {
    const { user, trigger } = setUp()
    await user.hover(trigger)
    advance(300)
    await user.unhover(trigger)
    advance(1000)
    expect(shownTooltip()).toBeNull()
  })

  it('is a raised surface with the text role', async () => {
    const { user, trigger } = setUp()
    await user.hover(trigger)
    advance(500)
    const tooltip = shownTooltip()
    expect(tooltip).toHaveClass(COLOR_CLASSES.surfaceRaised.background)
    expect(tooltip).toHaveClass(COLOR_CLASSES.border.border)
  })
})
