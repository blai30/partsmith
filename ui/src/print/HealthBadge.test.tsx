import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it, vi } from 'vitest'
import { COLOR_CLASSES } from '../theme/roles'
import { HealthBadge, type HealthBadgeProps } from './HealthBadge'

// The badge as `selectHealthBadge` (state/selectors.ts) derives it from the print state (ui.md 3.17).
type Badge = HealthBadgeProps['badge']

function renderBadge(badge: Badge, onClick = vi.fn<() => void>()) {
  render(<HealthBadge badge={badge} onClick={onClick} />)
  return { badge: screen.getByTestId('health-badge'), onClick }
}

function badgeState(overrides: Partial<Badge>): Badge {
  return { state: 'none', errors: 0, warnings: 0, stale: false, ...overrides }
}

type Row = {
  readonly title: string
  readonly badge: Badge
  readonly icon: string
  readonly text: string
  readonly name: string
}

const ROWS: readonly Row[] = [
  {
    title: 'none',
    badge: badgeState({ state: 'none' }),
    icon: 'lucide-circle-dashed',
    text: 'Check print',
    name: 'Print check: Check print',
  },
  {
    title: 'running',
    badge: badgeState({ state: 'running' }),
    icon: 'lucide-loader-circle',
    text: 'Checking',
    name: 'Print check: Checking',
  },
  {
    title: 'ok',
    badge: badgeState({ state: 'ok' }),
    icon: 'lucide-circle-check',
    text: 'Ready to print',
    name: 'Print check: Ready to print',
  },
  {
    title: 'one warning',
    badge: badgeState({ state: 'warning', warnings: 1 }),
    icon: 'lucide-triangle-alert',
    text: '1 warning',
    name: 'Print check: 1 warning',
  },
  {
    title: 'several warnings',
    badge: badgeState({ state: 'warning', warnings: 3 }),
    icon: 'lucide-triangle-alert',
    text: '3 warnings',
    name: 'Print check: 3 warnings',
  },
  {
    title: 'one error and several warnings',
    badge: badgeState({ state: 'error', errors: 1, warnings: 2 }),
    icon: 'lucide-circle-x',
    text: '1 error',
    name: 'Print check: 1 error, 2 warnings',
  },
  {
    title: 'several errors and one warning',
    badge: badgeState({ state: 'error', errors: 2, warnings: 1 }),
    icon: 'lucide-circle-x',
    text: '2 errors',
    name: 'Print check: 2 errors, 1 warning',
  },
  {
    title: 'errors without warnings',
    badge: badgeState({ state: 'error', errors: 1, warnings: 0 }),
    icon: 'lucide-circle-x',
    text: '1 error',
    name: 'Print check: 1 error',
  },
]

describe('HealthBadge', () => {
  it.each(ROWS)(
    'U-81 shows $title with its icon, text and an accessible name that starts with that text',
    (row) => {
      const { badge } = renderBadge(row.badge)
      expect(badge).toBe(screen.getByRole('button', { name: row.name }))
      expect(badge).toHaveAttribute('data-state', row.badge.state)
      expect(badge).toHaveAttribute('data-stale', 'false')
      expect(badge).toHaveTextContent(row.text)
      expect(row.name.startsWith(`Print check: ${row.text}`)).toBe(true)
      expect(badge.querySelector('svg')).toHaveClass(row.icon)
    },
  )

  it('spins the running icon unless the user prefers reduced motion', () => {
    const { badge } = renderBadge(badgeState({ state: 'running' }))
    expect(badge.querySelector('svg')).toHaveClass('animate-spin', 'motion-reduce:animate-none')
  })

  it('draws the ok check in successMark', () => {
    const { badge } = renderBadge(badgeState({ state: 'ok' }))
    expect(badge.querySelector('svg')).toHaveClass(COLOR_CLASSES.successMark.foreground)
  })

  it('draws the warning hazard sign in warningMark', () => {
    const { badge } = renderBadge(badgeState({ state: 'warning', warnings: 1 }))
    expect(badge.querySelector('svg')).toHaveClass(
      COLOR_CLASSES.warningMark.fill,
      COLOR_CLASSES.warningMark.foreground,
    )
  })

  it('draws the error mark in errorMark', () => {
    const { badge } = renderBadge(badgeState({ state: 'error', errors: 1 }))
    expect(badge.querySelector('svg')).toHaveClass(COLOR_CLASSES.errorMark.foreground)
  })

  it('U-81 marks a stale report: data-stale, muted text, a dimmed icon, "(out of date)" and ", out of date"', () => {
    const { badge } = renderBadge(badgeState({ state: 'warning', warnings: 1, stale: true }))
    expect(badge).toBe(screen.getByRole('button', { name: 'Print check: 1 warning, out of date' }))
    expect(badge).toHaveAttribute('data-state', 'warning')
    expect(badge).toHaveAttribute('data-stale', 'true')
    // The text stays readable (textMuted keeps 4.5:1, 2.16.6); only the icon is dimmed (3.17).
    expect(badge).not.toHaveClass('opacity-50')
    expect(badge).toHaveClass(COLOR_CLASSES.textMuted.foreground)
    expect(badge.querySelector('svg')?.parentElement).toHaveClass('opacity-50')
    expect(badge).toHaveTextContent('1 warning (out of date)')
  })

  it('draws a current report in the text role at full opacity', () => {
    const { badge } = renderBadge(badgeState({ state: 'ok' }))
    expect(badge).toHaveClass(COLOR_CLASSES.text.foreground)
    expect(badge.querySelector('svg')?.parentElement).not.toHaveClass('opacity-50')
  })

  it('reports a click', async () => {
    const { badge, onClick } = renderBadge(
      badgeState({ state: 'warning', warnings: 1, stale: true }),
    )
    await userEvent.click(badge)
    expect(onClick).toHaveBeenCalledTimes(1)
  })
})

describe('HealthBadge while checking', () => {
  it('is not out of date while the analysis of the current revision runs', () => {
    const { badge } = renderBadge(badgeState({ state: 'running', warnings: 2, stale: true }))
    expect(badge).toBe(screen.getByRole('button', { name: 'Print check: Checking' }))
    expect(badge).toHaveTextContent(/^Checking$/)
    expect(badge.querySelector('svg')?.parentElement).not.toHaveClass('opacity-50')
    // The attribute agrees with what is drawn.
    expect(badge).toHaveAttribute('data-stale', 'false')
  })
})
