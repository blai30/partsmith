import { act, cleanup, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { COLOR_CLASSES } from '../theme/roles'
import { installPopoverPolyfill } from './popover-polyfill.testing'
import { Tooltip } from './Tooltip'

beforeEach(() => {
  vi.useFakeTimers()
  // Testing Library's asyncWrapper, which user-event calls, waits for a setTimeout(0) and advances
  // fake timers only through a `jest` global; without it every user-event call hangs.
  vi.stubGlobal('jest', {
    advanceTimersByTime: (milliseconds: number) => {
      vi.advanceTimersByTime(milliseconds)
    },
  })
})

beforeEach(installPopoverPolyfill)

afterEach(() => {
  cleanup()
  vi.unstubAllGlobals()
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

describe('Tooltip pointer and focus details', () => {
  it('stays open while the pointer moves onto the tooltip', async () => {
    const { user, trigger } = setUp()
    await user.hover(trigger)
    advance(500)
    const tooltip = shownTooltip()
    expect(tooltip).not.toBeNull()
    await user.unhover(trigger)
    if (tooltip !== null) {
      await user.hover(tooltip)
    }
    advance(1000)
    expect(shownTooltip()).not.toBeNull()
  })

  it('does not appear when a click focuses the trigger', async () => {
    const { user, trigger } = setUp()
    await user.click(trigger)
    expect(trigger).toHaveFocus()
    advance(1000)
    expect(shownTooltip()).toBeNull()
  })

  it('stays open on keyboard focus when the pointer leaves', async () => {
    const { user, trigger } = setUp()
    await user.tab()
    await user.tab()
    expect(shownTooltip()).not.toBeNull()
    await user.hover(trigger)
    await user.unhover(trigger)
    advance(1000)
    expect(shownTooltip()).not.toBeNull()
  })

  it('keeps a description the trigger already has', () => {
    render(
      <>
        <p id="state">Edited</p>
        <Tooltip label="Save" shortcut={null}>
          <button type="button" aria-describedby="state">
            Save file
          </button>
        </Tooltip>
      </>,
    )
    const ids = screen.getByRole('button', { name: 'Save file' }).getAttribute('aria-describedby')
    expect(ids?.split(' ')).toHaveLength(2)
    expect(ids?.split(' ')[0]).toBe('state')
  })
})

describe('Tooltip with a description (3.24)', () => {
  it('shows the description on a second line in the muted text role', async () => {
    const user = userEvent.setup({
      advanceTimers: (delay) => {
        vi.advanceTimersByTime(delay)
      },
    })
    render(
      <Tooltip label="Shell" description="Hollows out a body.">
        <button type="button">Shell</button>
      </Tooltip>,
    )
    await user.hover(screen.getByRole('button', { name: 'Shell' }))
    advance(500)
    const description = screen.getByText('Hollows out a body.')
    expect(shownTooltip()).toContainElement(description)
    expect(description).toHaveClass(COLOR_CLASSES.textMuted.foreground)
    expect(description.parentElement).toBe(shownTooltip())
    expect(shownTooltip()?.firstElementChild).toHaveTextContent('Shell')
  })
})

describe('Tooltip without a label', () => {
  it('shows nothing and adds no description, keeping the same trigger element when the label comes and goes', async () => {
    const user = userEvent.setup({
      advanceTimers: (delay) => {
        vi.advanceTimersByTime(delay)
      },
    })
    const { rerender } = render(
      <Tooltip label="New sketch">
        <button type="button">Sketch</button>
      </Tooltip>,
    )
    const trigger = screen.getByRole('button', { name: 'Sketch' })
    expect(trigger).toHaveAccessibleDescription('New sketch')
    rerender(
      <Tooltip label={null}>
        <button type="button">Sketch</button>
      </Tooltip>,
    )
    expect(screen.getByRole('button', { name: 'Sketch' })).toBe(trigger)
    expect(trigger).not.toHaveAttribute('aria-describedby')
    await user.hover(trigger)
    advance(500)
    expect(shownTooltip()).toBeNull()
    expect(document.querySelector('[role="tooltip"]')).toBeNull()
  })
})

/** A tooltip on a "Sketch" button. */
function tooltipFor(label: string | null) {
  return (
    <Tooltip label={label}>
      <button type="button">Sketch</button>
    </Tooltip>
  )
}

describe('Tooltip whose label goes away while shown', () => {
  it('closes, stays closed when a label returns, and opens on the next hover', async () => {
    const user = userEvent.setup({
      advanceTimers: (delay) => {
        vi.advanceTimersByTime(delay)
      },
    })
    const { rerender } = render(tooltipFor('New sketch'))
    await user.hover(screen.getByRole('button', { name: 'Sketch' }))
    advance(500)
    expect(shownTooltip()).toHaveTextContent('New sketch')
    rerender(tooltipFor(null))
    expect(shownTooltip()).toBeNull()
    rerender(tooltipFor('Edit Sketch 1'))
    advance(500)
    expect(shownTooltip()).toBeNull()
    const trigger = screen.getByRole('button', { name: 'Sketch' })
    await user.unhover(trigger)
    advance(100)
    await user.hover(trigger)
    advance(500)
    expect(shownTooltip()).toHaveTextContent('Edit Sketch 1')
  })
})
