import { act, cleanup, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { describedBy } from './described-by'
import { HelpLabel } from './HelpLabel'
import { installPopoverPolyfill } from './popover-polyfill.testing'

beforeEach(() => {
  vi.useFakeTimers()
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

function renderField(help: string | null) {
  render(
    <>
      <HelpLabel help={help} helpId="width-help">
        <label htmlFor="width" className={help === null ? undefined : 'cursor-help'}>
          Width
        </label>
      </HelpLabel>
      <input id="width" aria-describedby={describedBy(help === null ? null : 'width-help')} />
    </>,
  )
}

describe('HelpLabel (3.7)', () => {
  it('shows the help in a tooltip after the pointer rests on the label', async () => {
    renderField('How wide the part is.')
    const user = userEvent.setup({
      advanceTimers: (delay) => {
        vi.advanceTimersByTime(delay)
      },
    })
    await user.hover(screen.getByText('Width'))
    act(() => {
      vi.advanceTimersByTime(500)
    })
    expect(screen.getByRole('tooltip')).toHaveTextContent('How wide the part is.')
  })

  it('describes the control with a visually hidden copy of the help', () => {
    renderField('How wide the part is.')
    expect(screen.getByRole('textbox', { name: 'Width' })).toHaveAccessibleDescription(
      'How wide the part is.',
    )
    expect(document.getElementById('width-help')).toHaveClass('sr-only')
  })

  it('renders the label alone without help', () => {
    renderField(null)
    expect(document.getElementById('width-help')).toBeNull()
    expect(screen.getByText('Width')).not.toHaveAttribute('aria-describedby')
    expect(screen.getByRole('textbox', { name: 'Width' })).not.toHaveAttribute('aria-describedby')
  })
})
