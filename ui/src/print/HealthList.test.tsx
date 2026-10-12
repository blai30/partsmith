import { cleanup, render, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'
import type { FindingKey } from '../state/store'
import { COLOR_CLASSES } from '../theme/roles'
import type { HealthRow } from './health'
import { HealthList, type HealthListProps } from './HealthList'

afterEach(() => {
  cleanup()
})

const PROFILE = 'Generic FDM 0.4 mm'

const ROWS: readonly HealthRow[] = [
  {
    key: { body: 'b-slab', check: 'bed_fit', index: 0 },
    severity: 'error',
    message: 'The part is 400 × 400 mm, but the bed is 256 × 256 mm. Split it or scale it down.',
    bodyName: 'Slab',
  },
  {
    key: { body: 'b-mushroom', check: 'overhangs', index: 1 },
    severity: 'warning',
    message: 'Overhang steeper than 45 degrees over 500 mm². Add supports.',
    bodyName: 'Mushroom',
  },
  {
    key: { body: 'b-mushroom', check: 'trapped_volumes', index: 0 },
    severity: 'info',
    message: 'The body encloses a hollow of 120 mm³. Slicers fill it with infill.',
    bodyName: 'Mushroom',
  },
]

function classes(roleClasses: string): string[] {
  return roleClasses.split(' ')
}

function renderList(overrides: Partial<HealthListProps> = {}) {
  const onCheckAgain = vi.fn<() => void>()
  const onFocusFinding = vi.fn<(finding: FindingKey) => void>()
  render(
    <HealthList
      profileName={PROFILE}
      rows={ROWS}
      hasGeometry
      onCheckAgain={onCheckAgain}
      onFocusFinding={onFocusFinding}
      {...overrides}
    />,
  )
  return { list: screen.getByTestId('health-list'), onCheckAgain, onFocusFinding }
}

function items(): HTMLElement[] {
  return screen.getAllByTestId('health-item')
}

function item(index: number): HTMLElement {
  const found = items()[index]
  if (found === undefined) {
    throw new Error(`There is no health item ${String(index)}.`)
  }
  return found
}

describe('HealthList', () => {
  it('reads "Print check: {profile name}" in its heading', () => {
    const { list } = renderList()
    expect(within(list).getByRole('heading', { level: 2 })).toHaveTextContent(
      'Print check: Generic FDM 0.4 mm',
    )
  })

  it('gives its heading the id the surrounding region is labeled by', () => {
    const { list } = renderList({ headingId: 'print-check-title' })
    expect(within(list).getByRole('heading', { level: 2 })).toHaveAttribute(
      'id',
      'print-check-title',
    )
  })

  it('runs the analysis again from "Check again"', async () => {
    const { list, onCheckAgain } = renderList()
    await userEvent.click(within(list).getByRole('button', { name: 'Check again' }))
    expect(onCheckAgain).toHaveBeenCalledTimes(1)
  })

  it('lists one health-item per row, in order, with data-check and data-severity', () => {
    const { list } = renderList()
    const listed = within(list).getAllByTestId('health-item')
    expect(listed.map((row) => row.dataset.check)).toEqual([
      'bed_fit',
      'overhangs',
      'trapped_volumes',
    ])
    expect(listed.map((row) => row.dataset.severity)).toEqual(['error', 'warning', 'info'])
  })

  it('shows the message verbatim and the body name in textMuted', () => {
    renderList()
    const error = item(0)
    expect(within(error).getByText(ROWS[0]?.message ?? '')).toBeInTheDocument()
    expect(within(error).getByText('Slab')).toHaveClass(
      ...classes(COLOR_CLASSES.textMuted.foreground),
    )
  })

  it.each([
    {
      severity: 'error',
      icon: 'lucide-circle-x',
      iconClasses: classes(COLOR_CLASSES.errorMark.foreground),
    },
    {
      severity: 'warning',
      icon: 'lucide-triangle-alert',
      iconClasses: [
        ...classes(COLOR_CLASSES.warningMark.fill),
        ...classes(COLOR_CLASSES.warningMark.foreground),
      ],
    },
    {
      severity: 'info',
      icon: 'lucide-info',
      iconClasses: classes(COLOR_CLASSES.textMuted.foreground),
    },
  ])('marks $severity rows with their severity icon', ({ severity, icon, iconClasses }) => {
    renderList()
    const row = items().find((candidate) => candidate.dataset.severity === severity)
    expect(row?.querySelector('svg')).toHaveClass(icon, ...iconClasses)
  })

  it('focuses the finding of a clicked row', async () => {
    const { onFocusFinding } = renderList()
    await userEvent.click(item(1))
    expect(onFocusFinding).toHaveBeenCalledTimes(1)
    expect(onFocusFinding).toHaveBeenCalledWith({
      body: 'b-mushroom',
      check: 'overhangs',
      index: 1,
    })
  })

  it('focuses the finding of a row on Enter', async () => {
    const { onFocusFinding } = renderList()
    const bedFit = item(0)
    bedFit.focus()
    expect(bedFit).toHaveFocus()
    await userEvent.keyboard('{Enter}')
    expect(onFocusFinding).toHaveBeenCalledTimes(1)
    expect(onFocusFinding).toHaveBeenCalledWith({ body: 'b-slab', check: 'bed_fit', index: 0 })
  })

  it('says "No print issues found with {profile name}." without rows', () => {
    const { list } = renderList({ rows: [] })
    expect(within(list).queryAllByTestId('health-item')).toEqual([])
    expect(list).toHaveTextContent('No print issues found with Generic FDM 0.4 mm.')
  })

  it('asks for a body when no body has geometry', () => {
    const { list } = renderList({ rows: [], hasGeometry: false })
    expect(list).toHaveTextContent('Add a body to check it for printing.')
    expect(list).not.toHaveTextContent('No print issues found')
  })
})

describe('HealthList details', () => {
  it('marks the row of the focused finding as current', () => {
    renderList({ focusedFinding: { body: 'b-mushroom', check: 'overhangs', index: 1 } })
    expect(items().map((row) => row.getAttribute('aria-current'))).toEqual([null, 'true', null])
    expect(item(1)).toHaveClass(...classes(COLOR_CLASSES.selection.background))
  })

  it('marks no row without a focused finding', () => {
    renderList()
    expect(items().filter((row) => row.hasAttribute('aria-current'))).toEqual([])
  })

  it('names each row with its severity, message and body for screen readers', () => {
    renderList()
    expect(item(0)).toHaveAccessibleName(`Error: ${ROWS[0]?.message ?? ''} Slab`)
  })

  it('closes from its close button when it can be closed', async () => {
    const onClose = vi.fn<() => void>()
    const { list } = renderList({ onClose })
    await userEvent.click(within(list).getByRole('button', { name: 'Close print check' }))
    expect(onClose).toHaveBeenCalledTimes(1)
  })

  it('has no close button otherwise', () => {
    const { list } = renderList()
    expect(within(list).queryByRole('button', { name: 'Close print check' })).toBeNull()
  })
})

describe('HealthList without a report', () => {
  it('says the model is being checked while the first analysis runs', () => {
    const { list } = renderList({ rows: [], missingReport: 'checking' })
    expect(list).toHaveTextContent('Checking the model for Generic FDM 0.4 mm.')
    expect(list).not.toHaveTextContent('No print issues found')
  })

  it('says the model is not checked yet before any analysis', () => {
    const { list } = renderList({ rows: [], missingReport: 'unchecked' })
    expect(list).toHaveTextContent('Not checked yet.')
    expect(list).not.toHaveTextContent('No print issues found')
  })

  it('asks for a body before anything else', () => {
    const { list } = renderList({ rows: [], hasGeometry: false, missingReport: 'checking' })
    expect(list).toHaveTextContent('Add a body to check it for printing.')
    expect(list).not.toHaveTextContent('Checking')
  })
})
