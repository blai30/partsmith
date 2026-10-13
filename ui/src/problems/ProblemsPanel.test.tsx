import { cleanup, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { installPopoverPolyfill } from '../components/popover-polyfill.testing'
import type { FeatureStatus } from '../generated/protocol'
import { cubeDocumentState, documentState, readyState } from '../layout/fixtures.testing'
import { createModelApp, renderWithApp, stubMatchMedia } from '../layout/test-app.testing'
import { MESSAGES } from '../messages'
import { INITIAL_STATE, type ActiveCommand, type DocumentState } from '../state/store'
import { CUBE_FEATURES_DOCUMENT, featuresOf } from '../test/fakes/documents'

beforeEach(installPopoverPolyfill)

beforeEach(() => {
  stubMatchMedia()
})

afterEach(() => {
  cleanup()
  vi.unstubAllGlobals()
})

const FILLET_TOO_LARGE: FeatureStatus = {
  id: 'f-fil1',
  status: 'error',
  messages: [
    {
      severity: 'error',
      code: 'fillet_too_large',
      message: 'Fillet radius 15 mm is too large for the selected edges. Try 2.3 mm or less.',
      field: '/radius',
      suggestion: '2.3 mm',
    },
  ],
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

  it('U-110 reads "Re-pick Edges" for a lost edge and edits the feature with Edges active (4.2)', async () => {
    const lost: FeatureStatus = {
      ...FILLET_TOO_LARGE,
      repairs: [
        {
          kind: 'element',
          path: '/edges/0',
          body: 'b-cube',
          reference: { q: 'edge(top,front)', fp: 'fp-edge' },
          resolution: 'missing',
          centroid: [0, 0, 20],
        },
      ],
    }
    const { app, user, panel } = renderPanel(withProblems([lost]))
    expect(within(panel).queryByRole('button', { name: 'Edit' })).toBeNull()
    await user.click(within(panel).getByRole('button', { name: 'Re-pick Edges' }))
    await waitFor(() => {
      expect(app.store.getState().command?.activeField).toBe('edges')
    })
  })

  it('offers "Use {suggestion}", which changes the value at the message field at rest', async () => {
    const { core, user, panel } = renderPanel(withProblems([FILLET_TOO_LARGE]))
    await user.click(within(panel).getByRole('button', { name: 'Use 2.3 mm' }))
    await waitFor(() => {
      expect(core.callsTo('applyEdits')).toHaveLength(1)
    })
    const [[documentHandle, label, edits]] = core.callsTo('applyEdits') as [
      [number, string, readonly unknown[]],
    ]
    expect(documentHandle).toBe(1)
    expect(label).toBe('Change Fillet 1')
    expect(edits).toEqual([
      {
        type: 'update_feature',
        feature: { ...featuresOf(CUBE_FEATURES_DOCUMENT)[2], radius: '2.3 mm' },
      },
    ])
  })

  it('during a feature command, "Use {suggestion}" waits: aria-disabled, it says why and sends nothing (3.7)', async () => {
    const { app, core, user, panel } = renderPanel(
      withProblems([FILLET_TOO_LARGE]),
      EXTRUDE_COMMAND,
    )
    const use = within(panel).getByRole('button', { name: 'Use 2.3 mm' })
    expect(use).toHaveAttribute('aria-disabled', 'true')
    await user.click(use)
    expect(core.callsTo('applyEdits')).toEqual([])
    expect(app.store.getState().notifications).toMatchObject([
      {
        severity: 'info',
        message: MESSAGES.finish_command_first('Extrude'),
        code: 'finish_command_first',
      },
    ])
  })

  it('offers no suggestion for a message without one', () => {
    const { panel } = renderPanel(withProblems([SHELL_WARNING]))
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
