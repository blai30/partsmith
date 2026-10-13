import { cleanup, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { installPopoverPolyfill } from '../components/popover-polyfill.testing'
import {
  bodyItem,
  CUBE_BOUNDS,
  cubeDocumentState,
  documentState,
  meshSummary,
  PEG_BOUNDS,
  readyState,
} from '../layout/fixtures.testing'
import { createModelApp, renderWithApp, stubMatchMedia } from '../layout/test-app.testing'
import { MESSAGES } from '../messages'
import { INITIAL_STATE, type AppState, type DocumentState } from '../state/store'
import { CUBE_FEATURES_DOCUMENT, CUBE_TOP_FACE, TWO_BODIES_DOCUMENT } from '../test/fakes/documents'
import type { FakeFileService } from '../test/fakes/files'

beforeEach(installPopoverPolyfill)

beforeEach(() => {
  stubMatchMedia()
})

afterEach(() => {
  cleanup()
  vi.unstubAllGlobals()
})

function twoBodies(): DocumentState {
  return documentState(TWO_BODIES_DOCUMENT, {
    name: 'two-bodies',
    meshes: { 'b-cube': meshSummary(1, CUBE_BOUNDS), 'b-peg': meshSummary(2, PEG_BOUNDS) },
  })
}

async function openExport(document: DocumentState = twoBodies(), state: Partial<AppState> = {}) {
  const test = createModelApp({ state: readyState(document, state) })
  test.core.stub('exportBodies', () =>
    Promise.resolve({
      files: [{ name: 'model.3mf', mediaType: 'model/3mf', bytes: new Uint8Array([1, 2, 3]) }],
      warnings: [],
    }),
  )
  renderWithApp(test.app)
  const user = userEvent.setup()
  await user.click(within(screen.getByTestId('top-bar')).getByRole('button', { name: 'Export' }))
  const popover = screen.getByTestId('export-popover')
  return { ...test, user, popover, files: test.app.files as FakeFileService }
}

describe('the export popover (3.18)', () => {
  it('offers the formats 3MF, STL, OBJ and STEP with 3MF chosen', async () => {
    const { popover } = await openExport()
    const formats = within(popover).getByRole('radiogroup', { name: 'Format' })
    const radios = within(formats).getAllByRole('radio')
    expect(radios).toHaveLength(4)
    for (const [index, name] of ['3MF', 'STL', 'OBJ', 'STEP'].entries()) {
      expect(radios[index]).toHaveAccessibleName(name)
    }
    expect(within(formats).getByRole('radio', { name: '3MF' })).toBeChecked()
  })

  it('offers all visible bodies or the selected bodies, the second only with bodies selected', async () => {
    const { popover } = await openExport()
    const bodies = within(popover).getByRole('radiogroup', { name: 'Bodies' })
    expect(within(bodies).getByRole('radio', { name: 'All visible bodies' })).toBeChecked()
    expect(within(bodies).getByRole('radio', { name: 'Selected bodies' })).toBeDisabled()
    cleanup()
    const selected = await openExport(twoBodies(), { selection: [bodyItem('b-peg', 'Peg')] })
    const enabled = within(selected.popover).getByRole('radiogroup', { name: 'Bodies' })
    expect(within(enabled).getByRole('radio', { name: 'Selected bodies' })).toBeEnabled()
  })

  it('exports 3MF of the visible bodies with geometry and saves it as "{document name}.3mf"', async () => {
    const { core, files, user, popover } = await openExport()
    await user.click(within(popover).getByRole('button', { name: 'Export' }))
    await waitFor(() => {
      expect(files.writes).toHaveLength(1)
    })
    expect(core.callsTo('exportBodies')).toEqual([[1, { kind: '3mf' }, ['b-cube', 'b-peg']]])
    expect(files.saveRequests).toEqual([{ suggestedName: 'two-bodies.3mf', kind: '3mf' }])
    expect(files.writes[0]?.kind).toBe('3mf')
    expect([...(files.writes[0]?.bytes ?? [])]).toEqual([1, 2, 3])
  })

  it('exports binary STL of one file for all bodies', async () => {
    const { core, user, popover } = await openExport()
    await user.click(within(popover).getByRole('radio', { name: 'STL' }))
    await user.click(within(popover).getByRole('button', { name: 'Export' }))
    await waitFor(() => {
      expect(core.callsTo('exportBodies')).toEqual([
        [1, { kind: 'stl', binary: true, per_body: false }, ['b-cube', 'b-peg']],
      ])
    })
  })

  it('exports only the selected bodies when asked', async () => {
    const { core, user, popover } = await openExport(twoBodies(), {
      selection: [bodyItem('b-peg', 'Peg')],
    })
    await user.click(within(popover).getByRole('radio', { name: 'Selected bodies' }))
    await user.click(within(popover).getByRole('radio', { name: 'STEP' }))
    await user.click(within(popover).getByRole('button', { name: 'Export' }))
    await waitFor(() => {
      expect(core.callsTo('exportBodies')).toEqual([[1, { kind: 'step' }, ['b-peg']]])
    })
  })

  it('shows nothing_to_export without calling the core when no visible body has geometry', async () => {
    const { app, core, user, popover } = await openExport(cubeDocumentState({ meshes: {} }))
    await user.click(within(popover).getByRole('button', { name: 'Export' }))
    await waitFor(() => {
      expect(app.store.getState().notifications.map((entry) => entry.message)).toContain(
        MESSAGES.nothing_to_export(),
      )
    })
    expect(core.callsTo('exportBodies')).toEqual([])
  })

  it('exports 3MF of all visible bodies directly from the Print toolbar', async () => {
    const test = createModelApp({ state: readyState(twoBodies(), { context: 'print' }) })
    test.core.stub('exportBodies', () =>
      Promise.resolve({
        files: [{ name: 'model.3mf', mediaType: 'model/3mf', bytes: new Uint8Array([1]) }],
        warnings: [],
      }),
    )
    renderWithApp(test.app)
    const user = userEvent.setup()
    const toolbar = screen.getByRole('toolbar', { name: 'Print tools' })
    await user.click(within(toolbar).getByRole('button', { name: 'Export 3MF' }))
    await waitFor(() => {
      expect(test.core.callsTo('exportBodies')).toEqual([[1, { kind: '3mf' }, ['b-cube', 'b-peg']]])
    })
    expect(screen.queryByTestId('export-popover')).toBeNull()
  })
})

/** cube-features.ptsm with Fillet 1 in error. */
function failingFillet(): DocumentState {
  const document = documentState(CUBE_FEATURES_DOCUMENT, {
    name: 'cube',
    meshes: { 'b-cube': meshSummary(1, CUBE_BOUNDS) },
  })
  return {
    ...document,
    featureStatus: {
      ...document.featureStatus,
      'f-fil1': {
        id: 'f-fil1',
        status: 'error',
        messages: [{ severity: 'error', code: 'fillet_too_large', message: 'Too large.' }],
        repairs: [],
      },
    },
  }
}

function statusLines(popover: HTMLElement): readonly HTMLElement[] {
  return within(popover).getAllByTestId('export-status')
}

describe('the export status lines (3.18)', () => {
  it('names the failed feature and an unchecked print with "Show problems" and "Check print"', async () => {
    const { popover } = await openExport(failingFillet())
    const lines = statusLines(popover)
    expect(lines.map((line) => line.textContent)).toEqual([
      expect.stringContaining('Fillet 1 failed, so it is not in the export.'),
      expect.stringContaining('Print check: Not checked'),
    ])
    expect(within(lines[0] ?? popover).getByRole('button', { name: 'Show problems' })).toBeVisible()
    expect(within(lines[1] ?? popover).getByRole('button', { name: 'Check print' })).toBeVisible()
    for (const line of lines) {
      expect(line).toHaveClass('text-xs')
    }
  })

  it('"Show problems" opens the Problems panel and closes the popover', async () => {
    const { app, user, popover } = await openExport(failingFillet())
    await user.click(within(popover).getByRole('button', { name: 'Show problems' }))
    expect(app.store.getState().panels.side).toBe('problems')
    expect(screen.queryByTestId('export-popover')).toBeNull()
  })

  it('"Check print" runs the analysis and opens the print check panel', async () => {
    const { app, core, user, popover } = await openExport(failingFillet())
    await user.click(within(popover).getByRole('button', { name: 'Check print' }))
    await waitFor(() => {
      expect(core.callsTo('analyzePrint')).toHaveLength(1)
    })
    expect(app.store.getState().panels.side).toBe('health')
  })

  it('shows no print check line for OBJ', async () => {
    const { user, popover } = await openExport(failingFillet())
    await user.click(within(popover).getByRole('radio', { name: 'OBJ' }))
    expect(statusLines(popover).map((line) => line.textContent)).toEqual([
      expect.stringContaining('Fillet 1 failed'),
    ])
  })

  it('names the Lay on face orientation, and "Print as modeled" clears it', async () => {
    const orientation = {
      kind: 'face_down',
      body: 'b-cube',
      face: CUBE_TOP_FACE.reference,
    } as const
    const { app, user, popover } = await openExport(cubeDocumentState(), {
      print: { ...INITIAL_STATE.print, orientation },
    })
    const line = statusLines(popover).at(-1) ?? popover
    expect(line).toHaveTextContent('Printing with the picked face of Cube on the bed.')
    await user.click(within(line).getByRole('button', { name: 'Print as modeled' }))
    expect(app.store.getState().print.orientation).toBeNull()
    expect(screen.getByTestId('export-popover')).toBeInTheDocument()
  })
})
