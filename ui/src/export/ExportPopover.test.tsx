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
import type { AppState, DocumentState } from '../state/store'
import { TWO_BODIES_DOCUMENT } from '../test/fakes/documents'
import type { FakeFileService } from '../test/fakes/files'

let uninstallPopovers: () => void

beforeEach(() => {
  uninstallPopovers = installPopoverPolyfill()
  stubMatchMedia()
})

afterEach(() => {
  cleanup()
  uninstallPopovers()
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
    expect(within(formats).getAllByRole('radio').map((radio) => radio.textContent?.trim() || radio.getAttribute('aria-label'))).toEqual([
      '3MF',
      'STL',
      'OBJ',
      'STEP',
    ])
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
    const { core, user, popover } = await openExport(twoBodies(), { selection: [bodyItem('b-peg', 'Peg')] })
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
})
