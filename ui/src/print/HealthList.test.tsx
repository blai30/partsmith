import { render, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it, vi } from 'vitest'
import type { Finding, Suggestion } from '../generated/protocol'
import type { AppState, FindingKey } from '../state/store'
import { COLOR_CLASSES } from '../theme/roles'
import {
  bodyPrint,
  documentState,
  finding,
  printReport,
  printState,
  stateWith,
} from './fixtures.testing'
import { healthRows, type HealthRow } from './health'
import { HealthList, type HealthListProps } from './HealthList'

const PROFILE = 'Generic FDM 0.4 mm'

const ROWS: readonly HealthRow[] = [
  {
    key: { body: 'b-slab', check: 'bed_fit', index: 0 },
    severity: 'error',
    message: 'The part is 400 × 400 mm, but the bed is 256 × 256 mm. Split it or scale it down.',
    suggestion: null,
    bodyName: 'Slab',
  },
  {
    key: { body: 'b-mushroom', check: 'overhangs', index: 1 },
    severity: 'warning',
    message: 'Overhang steeper than 45 degrees over 500 mm². Add supports.',
    suggestion: null,
    bodyName: 'Mushroom',
  },
  {
    key: { body: 'b-mushroom', check: 'trapped_volumes', index: 0 },
    severity: 'info',
    message: 'The body encloses a hollow of 120 mm³. Slicers fill it with infill.',
    suggestion: null,
    bodyName: 'Mushroom',
  },
]

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
    expect(within(error).getByText('Slab')).toHaveClass(COLOR_CLASSES.textMuted.foreground)
  })

  it.each([
    {
      severity: 'error',
      icon: 'lucide-circle-x',
      iconClasses: [COLOR_CLASSES.errorMark.foreground],
    },
    {
      severity: 'warning',
      icon: 'lucide-triangle-alert',
      iconClasses: [COLOR_CLASSES.warningMark.fill, COLOR_CLASSES.warningMark.foreground],
    },
    {
      severity: 'info',
      icon: 'lucide-info',
      iconClasses: [COLOR_CLASSES.textMuted.foreground],
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
    expect(item(1)).toHaveClass(COLOR_CLASSES.selection.background)
    // The state outline that forced colors draw (3.23).
    expect(item(1)).toHaveClass('outline', '-outline-offset-1', COLOR_CLASSES.forcedEdge.outline)
    expect(item(0)).toHaveClass(COLOR_CLASSES.forcedEdge.outline)
    expect(item(0)).not.toHaveClass('outline')
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

describe('HealthList and Lay on face (3.17)', () => {
  it('offers "Lay on a face" at the end of overhang, first layer and island rows only', async () => {
    const onLayOnFace = vi.fn<() => void>()
    const { onFocusFinding } = renderList({ onLayOnFace })
    const buttons = screen.getAllByRole('button', { name: 'Lay on a face' })
    expect(buttons).toHaveLength(1)
    const row = item(1).closest('li')
    expect(row).not.toBeNull()
    expect(within(row ?? document.body).getByRole('button', { name: 'Lay on a face' })).toBe(
      buttons[0],
    )
    // The finding text is its own button, so the two never nest.
    expect(item(1).contains(buttons[0] ?? null)).toBe(false)
    await userEvent.click(buttons[0] ?? document.body)
    expect(onLayOnFace).toHaveBeenCalledTimes(1)
    expect(onFocusFinding).not.toHaveBeenCalled()
  })

  it('ends "Lay on a face" 1rem from the panel edge, in line with the header (3.17)', () => {
    renderList({ onLayOnFace: vi.fn<() => void>() })
    const button = screen.getByRole('button', { name: 'Lay on a face' })
    // The list's p-2 and the button's mr-2 make the header block's p-4.
    expect(button).toHaveClass('mr-2')
    expect(screen.getByRole('list', { name: 'Findings' })).toHaveClass('p-2')
    expect(screen.getByTestId('health-list').firstElementChild).toHaveClass('p-4')
  })

  it('offers no "Lay on a face" without the handler', () => {
    renderList()
    expect(screen.queryByRole('button', { name: 'Lay on a face' })).toBeNull()
  })

  it('says the picked face rests on the bed under the header, with "Print as modeled"', async () => {
    const onPrintAsModeled = vi.fn<() => void>()
    const { list } = renderList({ orientationBody: 'Mushroom', onPrintAsModeled })
    expect(list).toHaveTextContent('Printing with the picked face of Mushroom on the bed.')
    await userEvent.click(within(list).getByRole('button', { name: 'Print as modeled' }))
    expect(onPrintAsModeled).toHaveBeenCalledTimes(1)
  })

  it('has no orientation line while printing as modeled', () => {
    const { list } = renderList({ orientationBody: null, onPrintAsModeled: vi.fn<() => void>() })
    expect(within(list).queryByRole('button', { name: 'Print as modeled' })).toBeNull()
  })
})

// The report of U-235: Slab with a split, Mushroom without a suggestion, Peg with a split without planes.
const SUGGESTION_SLAB = { id: 'b-slab', name: 'Slab' }
const SUGGESTION_MUSHROOM = { id: 'b-mushroom', name: 'Mushroom' }
const SUGGESTION_PEG = { id: 'b-peg', name: 'Peg' }
const SLAB_MESSAGE =
  'The part footprint is 400 x 400 mm, larger than the 256 x 256 mm bed. Split the part or scale it down.'
const MUSHROOM_MESSAGE =
  'Overhang steeper than 45 degrees over 500 mm². Add supports, add a chamfer, or reorient the part.'
const PEG_MESSAGE =
  'The part footprint is 300 x 10 mm, larger than the 256 x 256 mm bed. Split the part or scale it down.'
const SLAB_WORDS = 'Split at x = 0 mm and y = 0 mm'

function bedFitWith(message: string, suggestion: Suggestion): Finding {
  return finding({
    check: 'bed_fit',
    code: 'footprint_too_large',
    severity: 'error',
    message,
    location: { kind: 'bounds', min: [-200, -200, 0], max: [200, 200, 10] },
    suggestion,
  })
}

const SUGGESTION_REPORT = printReport([
  bodyPrint({
    body: SUGGESTION_SLAB.id,
    meshId: 1,
    findings: {
      bed_fit: [
        bedFitWith(SLAB_MESSAGE, {
          kind: 'split',
          planes: [
            { origin: [0, 0, 5], normal: [1, 0, 0] },
            { origin: [0, 0, 5], normal: [0, 1, 0] },
          ],
        }),
      ],
    },
  }),
  bodyPrint({
    body: SUGGESTION_MUSHROOM.id,
    meshId: 2,
    findings: {
      overhangs: [
        finding({
          check: 'overhangs',
          code: 'overhang',
          severity: 'warning',
          message: MUSHROOM_MESSAGE,
        }),
      ],
    },
  }),
  bodyPrint({
    body: SUGGESTION_PEG.id,
    meshId: 3,
    findings: { bed_fit: [bedFitWith(PEG_MESSAGE, { kind: 'split', planes: [] })] },
  }),
])

/** Print context with the U-235 report and the document's units set to `units`. */
function suggestionReportState(units: 'mm' | 'in' = 'mm'): AppState {
  const state = stateWith({
    context: 'print',
    document: documentState({
      revision: 3,
      bodies: [SUGGESTION_SLAB, SUGGESTION_MUSHROOM, SUGGESTION_PEG],
    }),
    print: printState({ report: SUGGESTION_REPORT, reportRevision: 3 }),
  })
  return state.document === null
    ? state
    : { ...state, document: { ...state.document, recipe: { ...state.document.recipe, units } } }
}

/** The rendered row whose text holds `message`. */
function rowWith(message: string): HTMLElement {
  const found = items().find((candidate) => candidate.textContent?.includes(message) === true)
  if (found === undefined) {
    throw new Error(`There is no row with ${message}`)
  }
  return found
}

describe('U-235 rows with suggestions', () => {
  it('U-235 gives the rows the words of their suggestions, null without words', () => {
    const rows = healthRows(suggestionReportState())
    expect(rows.map((listed) => [listed.bodyName, listed.suggestion])).toEqual([
      ['Slab', SLAB_WORDS],
      ['Peg', null],
      ['Mushroom', null],
    ])
  })

  it('U-235 shows the words after the message and before the body name', () => {
    renderList({ rows: healthRows(suggestionReportState()) })
    const slab = rowWith(SLAB_MESSAGE)
    expect(within(slab).getByTestId('health-suggestion')).toHaveTextContent(SLAB_WORDS)
    const text = slab.textContent ?? ''
    expect(text.indexOf(SLAB_MESSAGE)).toBeGreaterThanOrEqual(0)
    expect(text.indexOf(SLAB_MESSAGE)).toBeLessThan(text.indexOf(SLAB_WORDS))
    expect(text.indexOf(SLAB_WORDS)).toBeLessThan(text.lastIndexOf('Slab'))
  })

  it('U-235 names a row with its suggestion between the message and the body name', () => {
    renderList({ rows: healthRows(suggestionReportState()) })
    expect(rowWith(SLAB_MESSAGE)).toHaveAccessibleName(`Error: ${SLAB_MESSAGE} ${SLAB_WORDS}. Slab`)
  })

  it('U-235 gives rows without a suggestion, or with a split without planes, no suggestion line', () => {
    renderList({ rows: healthRows(suggestionReportState()) })
    const mushroom = rowWith(MUSHROOM_MESSAGE)
    expect(within(mushroom).queryByTestId('health-suggestion')).toBeNull()
    expect(mushroom).toHaveAccessibleName(`Warning: ${MUSHROOM_MESSAGE} Mushroom`)
    const peg = rowWith(PEG_MESSAGE)
    expect(within(peg).queryByTestId('health-suggestion')).toBeNull()
    expect(peg).toHaveAccessibleName(`Error: ${PEG_MESSAGE} Peg`)
  })

  it('U-235 keeps the words in millimeters in a document in inches', () => {
    renderList({ rows: healthRows(suggestionReportState('in')) })
    expect(within(rowWith(SLAB_MESSAGE)).getByTestId('health-suggestion')).toHaveTextContent(
      SLAB_WORDS,
    )
  })
})
