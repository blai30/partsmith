import { cleanup, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { installPopoverPolyfill } from '../components/popover-polyfill.testing'
import type { Diagnostic, Document, FeatureStatus, FixAction, FixRef } from '../generated/protocol'
import { cubeDocumentState, documentState, readyState } from '../layout/fixtures.testing'
import { createModelApp, renderWithApp, stubMatchMedia } from '../layout/test-app.testing'
import { MESSAGES } from '../messages'
import { INITIAL_STATE, type ActiveCommand, type DocumentState } from '../state/store'
import { coreErrorFor } from '../test/fakes/core-api'
import { CUBE_FEATURES_DOCUMENT } from '../test/fakes/documents'
import {
  lostEdgeRepair,
  R16_DOCUMENT,
  R16_STATUS,
  R16_USE_FRONT,
  r16Fix,
} from '../test/fakes/repairs'

beforeEach(installPopoverPolyfill)

beforeEach(() => {
  stubMatchMedia()
})

afterEach(() => {
  cleanup()
  vi.unstubAllGlobals()
})

const SMALLER_FIX: FixRef = {
  feature: 'f-fil1',
  code: 'fillet_too_large',
  field: '/radius',
  cause: false,
  index: 0,
  revision: 4,
}

/** "Use 2.3 mm" of the fillet message, as the core offers it (engine.md 3.22). */
const USE_SMALLER: FixAction = { label: 'Use 2.3 mm', run: { kind: 'apply', fix: SMALLER_FIX } }

const TOO_LARGE_MESSAGE: Diagnostic = {
  severity: 'error',
  code: 'fillet_too_large',
  message: 'Fillet radius 15 mm is too large for the selected edges. Try 2.3 mm or less.',
  field: '/radius',
  suggestion: '2.3 mm',
  actions: [USE_SMALLER],
}

const FILLET_TOO_LARGE: FeatureStatus = {
  id: 'f-fil1',
  status: 'error',
  messages: [TOO_LARGE_MESSAGE],
  repairs: [],
}

const SHELL_WARNING: FeatureStatus = {
  id: 'f-sh1',
  status: 'warning',
  messages: [
    {
      severity: 'warning',
      code: 'no_effect',
      message: "This feature does not change body 'Cube'.",
    },
  ],
  repairs: [],
}

function withProblems(
  statuses: readonly FeatureStatus[],
  diagnostics: DocumentState['diagnostics'] = [],
): DocumentState {
  const document = documentState(CUBE_FEATURES_DOCUMENT)
  return {
    ...document,
    featureStatus: {
      ...document.featureStatus,
      ...Object.fromEntries(statuses.map((status) => [status.id, status])),
    },
    diagnostics,
  }
}

const EXTRUDE_COMMAND: ActiveCommand = {
  featureType: 'extrude',
  featureId: 'f-new1',
  editing: false,
  title: 'Extrude',
  params: { extent: { kind: 'distance', expr: '10 mm' }, op: 'join', body: 'b-cube' },
  activeField: 'profile',
  fieldErrors: {},
  applied: false,
  newBody: null,
  helper: null,
  changedPaths: [],
  history: [],
  future: [],
  preview: 'none',
}

function renderPanel(document: DocumentState, command: ActiveCommand | null = null) {
  const test = createModelApp({
    state: readyState(document, {
      panels: { ...INITIAL_STATE.panels, browserOpen: true, side: 'problems' },
      command,
    }),
  })
  renderWithApp(test.app)
  return { ...test, user: userEvent.setup(), panel: screen.getByTestId('problems-panel') }
}

describe('the Problems panel (3.9)', () => {
  it('says "No problems." when every feature is ok', () => {
    const { panel } = renderPanel(cubeDocumentState())
    expect(panel).toHaveTextContent('No problems.')
  })

  it('pads its body on every side, so the first row clears the header rule (3.23)', () => {
    const { panel } = renderPanel(withProblems([FILLET_TOO_LARGE]))
    expect(within(panel).getByRole('list').parentElement).toHaveClass('p-4')
  })

  it('lists features with warnings or errors in timeline order, with their messages', () => {
    const { panel } = renderPanel(withProblems([SHELL_WARNING, FILLET_TOO_LARGE]))
    const fillet = within(panel).getByRole('button', { name: 'Fillet 1' })
    const shell = within(panel).getByRole('button', { name: 'Shell 1' })
    expect(fillet.compareDocumentPosition(shell) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy()
    expect(panel).toHaveTextContent(FILLET_TOO_LARGE.messages[0]?.message ?? '')
    expect(panel).toHaveTextContent(SHELL_WARNING.messages[0]?.message ?? '')
    expect(panel.querySelector('.lucide-circle-x')).not.toBeNull()
    expect(panel.querySelector('.lucide-triangle-alert')).not.toBeNull()
  })

  it('lists document diagnostics after the features', () => {
    const { panel } = renderPanel(
      withProblems(
        [FILLET_TOO_LARGE],
        [
          {
            severity: 'warning',
            code: 'unknown_printer_profile',
            message:
              'Printer profile "my-printer" is not installed, so the default FDM profile is used.',
          },
        ],
      ),
    )
    expect(panel.textContent?.indexOf('Fillet 1')).toBeLessThan(
      panel.textContent?.indexOf('Printer profile "my-printer"') ?? -1,
    )
  })

  it('selects a feature with its label button', async () => {
    const { app, user, panel } = renderPanel(withProblems([FILLET_TOO_LARGE]))
    await user.click(within(panel).getByRole('button', { name: 'Fillet 1' }))
    expect(app.store.getState().selection.map((item) => item.key)).toEqual(['feature:f-fil1'])
  })

  it('edits a feature with its Edit button', async () => {
    const { core, user, panel } = renderPanel(withProblems([FILLET_TOO_LARGE]))
    await user.click(within(panel).getByRole('button', { name: 'Edit' }))
    await waitFor(() => {
      expect(core.callsTo('beginTransaction')).toEqual([[1, 'Edit Fillet 1']])
    })
  })

  it(`U-110 offers "Show what 'Fillet 1' lost" and "Edit" for a lost edge, and no "Re-pick" (4.2)`, async () => {
    const lost: FeatureStatus = { ...FILLET_TOO_LARGE, repairs: [lostEdgeRepair()] }
    const { app, user, panel } = renderPanel(withProblems([lost]))
    expect(within(panel).getByRole('button', { name: 'Edit' })).toBeInTheDocument()
    expect(within(panel).queryByRole('button', { name: /^Re-pick/ })).toBeNull()
    await user.click(within(panel).getByRole('button', { name: "Show what 'Fillet 1' lost" }))
    await waitFor(() => {
      expect(app.store.getState().command?.activeField).toBe('edges')
    })
  })

  it('offers no "Show what … lost" for a feature without repairs', () => {
    const { panel } = renderPanel(withProblems([FILLET_TOO_LARGE]))
    expect(within(panel).queryByRole('button', { name: /^Show what/ })).toBeNull()
  })

  it("runs a message's apply action through applyFix at rest, outside a transaction (4.2)", async () => {
    const { core, user, panel } = renderPanel(withProblems([FILLET_TOO_LARGE]))
    await user.click(within(panel).getByRole('button', { name: 'Use 2.3 mm' }))
    await waitFor(() => {
      expect(core.callsTo('applyFix')).toEqual([[1, SMALLER_FIX]])
    })
    expect(core.callsTo('beginTransaction')).toEqual([])
    expect(core.callsTo('applyEdits')).toEqual([])
  })

  it('during a feature command, an apply action waits: aria-disabled, it says why and sends nothing (3.7)', async () => {
    const { app, core, user, panel } = renderPanel(
      withProblems([FILLET_TOO_LARGE]),
      EXTRUDE_COMMAND,
    )
    const use = within(panel).getByRole('button', { name: 'Use 2.3 mm' })
    expect(use).toHaveAttribute('aria-disabled', 'true')
    await user.click(use)
    expect(core.callsTo('applyFix')).toEqual([])
    expect(app.store.getState().notifications).toMatchObject([
      {
        severity: 'info',
        message: MESSAGES.finish_command_first('Extrude'),
        code: 'finish_command_first',
      },
    ])
  })

  it('makes no fix of its own from a suggestion: a message without actions has no Use button (open issue 26)', () => {
    const withoutActions: FeatureStatus = {
      ...FILLET_TOO_LARGE,
      messages: [{ ...TOO_LARGE_MESSAGE, actions: [] }],
    }
    const { panel } = renderPanel(withProblems([withoutActions, SHELL_WARNING]))
    expect(within(panel).queryByRole('button', { name: /^Use / })).toBeNull()
  })

  it('marks an info diagnostic with the info icon, not the hazard sign', () => {
    const { panel } = renderPanel(
      withProblems(
        [],
        [{ severity: 'info', code: 'example_info', message: 'The document uses inches.' }],
      ),
    )
    expect(within(panel).getByLabelText('Info')).toHaveClass('lucide-info')
    expect(panel.querySelector('.lucide-triangle-alert')).toBeNull()
  })
})

/** A datum plane, a sketch on it and its extrude, a shell and a chamfer, for grouping by root (U-129). */
const GROUPED_DOCUMENT: Document = {
  format: 'partsmith',
  format_version: 2,
  units: 'mm',
  parameters: [],
  bodies: [{ id: 'b-1', name: 'Body 1' }],
  features: [
    { id: 'f-dp1', type: 'datum_plane', base: { kind: 'origin', plane: 'XY' }, offset: '10 mm' },
    {
      id: 'f-sk3',
      type: 'sketch',
      plane: { kind: 'datum', feature: 'f-dp1' },
      entities: [],
      constraints: [],
    },
    {
      id: 'f-ext3',
      type: 'extrude',
      body: 'b-1',
      profile: { sketch: 'f-sk3', regions: [] },
      extent: { kind: 'distance', expr: '5 mm' },
      op: 'new',
    },
    { id: 'f-sh1', type: 'shell', body: 'b-1', remove_faces: [], thickness: '2 mm' },
    {
      id: 'f-cha1',
      type: 'chamfer',
      body: 'b-1',
      edges: [],
      distance: { kind: 'equal', expr: '1 mm' },
    },
  ],
}

function errorStatus(id: string, message: Diagnostic): FeatureStatus {
  return { id, status: 'error', messages: [message], repairs: [] }
}

const DATUM_LOST = errorStatus('f-dp1', {
  severity: 'error',
  code: 'reference_not_found',
  message: "The top of 'Box 1' selected for Base plane was cut away by 'Extrude 1'.",
  field: '/base/face',
})

function upstreamFailed(id: string): FeatureStatus {
  return errorStatus(id, {
    severity: 'error',
    code: 'upstream_failed',
    message: "This feature uses 'Datum plane 1', which failed.",
    root: 'f-dp1',
  })
}

const SHELL_UNKNOWN: FeatureStatus = {
  id: 'f-sh1',
  status: 'warning',
  messages: [
    {
      severity: 'warning',
      code: 'unknown_params',
      message: "Shell 1 has a setting this version does not know, 'thikness', so it was ignored.",
    },
  ],
  repairs: [],
}

const FOLLOWED: Diagnostic = {
  severity: 'info',
  code: 'reference_followed',
  message:
    "The front top edge of 'Extrude 1' selected for Edges now follows the edge that replaced it.",
  field: '/edges/0',
  actions: [
    {
      label: 'Keep this match',
      run: {
        kind: 'apply',
        fix: {
          feature: 'f-cha1',
          code: 'reference_followed',
          field: '/edges/0',
          cause: false,
          index: 0,
          revision: 1,
        },
      },
    },
    { label: 'Pick an edge again', run: { kind: 'pick', feature: 'f-cha1', path: '/edges/0' } },
  ],
}

const CHAMFER_NOTE: FeatureStatus = {
  id: 'f-cha1',
  status: 'ok',
  messages: [FOLLOWED],
  repairs: [],
}

function groupedDocument(statuses: readonly FeatureStatus[]): DocumentState {
  const document = documentState(GROUPED_DOCUMENT)
  return {
    ...document,
    featureStatus: {
      ...document.featureStatus,
      ...Object.fromEntries(statuses.map((status) => [status.id, status])),
    },
  }
}

function problemRowsIn(panel: HTMLElement): HTMLElement[] {
  return within(panel).getAllByTestId('problem-row')
}

describe('Problems grouped by root (3.9, U-129)', () => {
  const ALL = [
    DATUM_LOST,
    upstreamFailed('f-sk3'),
    upstreamFailed('f-ext3'),
    SHELL_UNKNOWN,
    CHAMFER_NOTE,
  ]

  it('U-129 nests the features that failed because of a root under its row', () => {
    const { panel } = renderPanel(groupedDocument(ALL))
    const rows = problemRowsIn(panel)
    const topLevel = rows.filter((row) => !row.hasAttribute('data-nested'))
    const nested = rows.filter((row) => row.getAttribute('data-nested') === 'true')
    expect(topLevel.map((row) => row.getAttribute('data-feature-id'))).toEqual(['f-dp1', 'f-sh1'])
    expect(nested.map((row) => row.getAttribute('data-feature-id'))).toEqual(['f-sk3', 'f-ext3'])
    const [first, second] = topLevel
    if (first === undefined || second === undefined) {
      throw new Error('The panel lacks a top-level row.')
    }
    for (const row of nested) {
      expect(first.compareDocumentPosition(row) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy()
      expect(row.compareDocumentPosition(second) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy()
    }
  })

  it('U-129 gives each nested row its message and the single action "Show \'Datum plane 1\'"', () => {
    const { panel } = renderPanel(groupedDocument(ALL))
    const nested = problemRowsIn(panel).filter((row) => row.getAttribute('data-nested') === 'true')
    const labels: Record<string, string> = { 'f-sk3': 'Sketch 1', 'f-ext3': 'Extrude 1' }
    for (const row of nested) {
      expect(row).toHaveTextContent("This feature uses 'Datum plane 1', which failed.")
      const label = labels[row.getAttribute('data-feature-id') ?? ''] ?? ''
      const actions = within(row)
        .getAllByRole('button')
        .map((button) => button.textContent ?? '')
        .filter((name) => name !== label)
      expect(actions).toEqual(["Show 'Datum plane 1'"])
    }
  })

  it('U-129 "Show \'Datum plane 1\'" selects the root', async () => {
    const { app, user, panel } = renderPanel(groupedDocument(ALL))
    const nested = problemRowsIn(panel).find((row) => row.getAttribute('data-nested') === 'true')
    await user.click(within(nested ?? panel).getByRole('button', { name: "Show 'Datum plane 1'" }))
    expect(app.store.getState().selection.map((item) => item.key)).toEqual(['feature:f-dp1'])
  })

  it('U-129 counts one problem per root: the problems button reads "2 problems"', () => {
    renderPanel(groupedDocument(ALL))
    expect(screen.getByTestId('timeline-problems')).toHaveTextContent('2 problems')
  })

  it('U-129 lists notes under a collapsed "Notes" disclosure with their count and actions', async () => {
    const { user, panel } = renderPanel(groupedDocument(ALL))
    const notes = within(panel).getByTestId('problems-notes')
    expect(notes).toHaveTextContent(/^Notes\s*1$/)
    expect(notes).toHaveAttribute('aria-expanded', 'false')
    expect(within(panel).queryByText(FOLLOWED.message)).toBeNull()
    await user.click(notes)
    expect(notes).toHaveAttribute('aria-expanded', 'true')
    expect(panel).toHaveTextContent(FOLLOWED.message)
    expect(within(panel).getByRole('button', { name: 'Keep this match' })).toBeInTheDocument()
  })

  it('U-129 "Keep this match" of a note runs through applyFix', async () => {
    const { core, user, panel } = renderPanel(groupedDocument(ALL))
    await user.click(within(panel).getByTestId('problems-notes'))
    await user.click(within(panel).getByRole('button', { name: 'Keep this match' }))
    await waitFor(() => {
      expect(core.callsTo('applyFix')).toEqual([
        [
          1,
          {
            feature: 'f-cha1',
            code: 'reference_followed',
            field: '/edges/0',
            cause: false,
            index: 0,
            revision: 1,
          },
        ],
      ])
    })
  })

  it('U-129 a feature with only a note stays ok in the timeline with the info mark', () => {
    renderPanel(groupedDocument(ALL))
    const item = screen
      .getAllByTestId('timeline-item')
      .find((element) => element.getAttribute('data-feature-id') === 'f-cha1')
    expect(item).toHaveAttribute('data-status', 'ok')
    expect(item).toHaveAttribute('data-notes', '1')
    expect(item?.querySelector('.lucide-info')).not.toBeNull()
  })

  it('U-129 with only a note: "No problems.", the Notes, and no problems button', () => {
    const { panel } = renderPanel(groupedDocument([CHAMFER_NOTE]))
    expect(panel).toHaveTextContent('No problems.')
    expect(within(panel).getByTestId('problems-notes')).toBeInTheDocument()
    expect(screen.queryByTestId('timeline-problems')).toBeNull()
  })

  it('U-129 a feature without notes has data-notes 0', () => {
    renderPanel(groupedDocument(ALL))
    const item = screen
      .getAllByTestId('timeline-item')
      .find((element) => element.getAttribute('data-feature-id') === 'f-sh1')
    expect(item).toHaveAttribute('data-notes', '0')
  })
})

describe('a candidate at rest (4.2, U-128)', () => {
  it('U-128 clicking A0 in the Problems panel calls applyFix with its FixRef outside a transaction', async () => {
    const document = documentState(R16_DOCUMENT)
    const { core, user, panel } = renderPanel({
      ...document,
      featureStatus: { ...document.featureStatus, 'f-fil1': R16_STATUS },
    })
    await user.click(within(panel).getByRole('button', { name: R16_USE_FRONT.label }))
    await waitFor(() => {
      expect(core.callsTo('applyFix')).toEqual([[1, r16Fix(0)]])
    })
    expect(core.callsTo('beginTransaction')).toEqual([])
  })
})

describe('fixes that no longer apply (4.2, U-128)', () => {
  const STALE =
    'This fix no longer applies because the document changed after it was offered. Look at the problem again.'

  it('U-128 a stale_fix error shows the core message with "Show problems" and nothing else', async () => {
    const { app, core, user, panel } = renderPanel(withProblems([FILLET_TOO_LARGE]))
    const stale = await coreErrorFor({ code: 'stale_fix', message: STALE })
    core.stub('applyFix', () => Promise.reject(stale))
    await user.click(within(panel).getByRole('button', { name: 'Use 2.3 mm' }))
    await waitFor(() => {
      expect(app.store.getState().notifications).toHaveLength(1)
    })
    const [notification] = app.store.getState().notifications
    expect(notification).toMatchObject({ severity: 'error', message: STALE, code: 'stale_fix' })
    expect(notification?.actions.map((action) => action.label)).toEqual(['Show problems'])
  })
})

describe('Copy details in the Problems panel (4.1, U-130)', () => {
  it('U-130 is the last button of a message with a detail', () => {
    const detailed: FeatureStatus = {
      ...FILLET_TOO_LARGE,
      messages: [{ ...TOO_LARGE_MESSAGE, detail: 'fillet: StdFail_NotDone' }],
    }
    const { panel } = renderPanel(withProblems([detailed]))
    const [row] = problemRowsIn(panel)
    const names = within(row ?? panel)
      .getAllByRole('button')
      .map((button) => button.textContent ?? '')
    expect(names.indexOf('Copy details')).toBeGreaterThan(names.indexOf('Use 2.3 mm'))
    expect(names.indexOf('Copy details')).toBeLessThan(names.indexOf('Edit'))
  })

  it('U-130 a message without a detail has no Copy details button', () => {
    const { panel } = renderPanel(withProblems([FILLET_TOO_LARGE]))
    expect(within(panel).queryByRole('button', { name: 'Copy details' })).toBeNull()
  })
})

describe('repair mode from a Problems row (3.7, 3.9)', () => {
  const LOST: FeatureStatus = { ...FILLET_TOO_LARGE, repairs: [lostEdgeRepair()] }

  it('shows repair mode while focus is inside the row of a feature with repairs, and ends it when focus leaves', async () => {
    const { app, user, panel } = renderPanel(withProblems([LOST, SHELL_WARNING]))
    const [filletRow, shellRow] = problemRowsIn(panel)
    await user.click(within(filletRow ?? panel).getByRole('button', { name: 'Fillet 1' }))
    expect(app.store.getState().repair).toEqual({ feature: 'f-fil1' })
    await user.click(within(shellRow ?? panel).getByRole('button', { name: 'Shell 1' }))
    expect(app.store.getState().repair).toBeNull()
  })
})
