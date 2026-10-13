import { describe, expect, it, vi } from 'vitest'
import { FileServiceError } from '../../files/file-service'
import type { Body, Diagnostic, ExportFormat } from '../../generated/protocol'
import { MESSAGES } from '../../messages'
import {
  CUBE,
  CUBE_BOUNDS,
  DOCUMENT_HANDLE,
  EMPTY,
  GHOST,
  PEG,
  PEG_BOUNDS,
  THREE_MF_BYTES,
  createPrintApp,
  documentState,
  exportedFile,
  meshSummary,
  type PrintApp,
} from '../../print/fixtures.testing'
import type { AppState, SelectionItem } from '../../state/store'
import { TEST_CATALOG } from '../../test/fakes/catalog'
import { coreErrorFor } from '../../test/fakes/core-api'
import { createFakeFileHandle } from '../../test/fakes/files'
import { bodyItem } from '../../test/fakes/selection'
import { exportModel, type ExportRequest } from './export'

const NOTHING_TO_EXPORT = 'There is no visible body with geometry to export.'

// The document "cube" with, in browser order, a visible body with geometry, a hidden body with
// geometry, a visible body without geometry and a second visible body with geometry.
function exportApp(overrides: Partial<AppState> = {}): PrintApp {
  return createPrintApp({
    document: documentState({
      name: 'cube',
      bodies: [CUBE, GHOST, EMPTY, PEG],
      meshes: {
        [CUBE.id]: meshSummary(1, CUBE_BOUNDS),
        [GHOST.id]: meshSummary(2, CUBE_BOUNDS),
        [PEG.id]: meshSummary(3, PEG_BOUNDS),
      },
    }),
    ...overrides,
  })
}

function faceItem(body: string): SelectionItem {
  return {
    key: `element:${body}:face-1`,
    kind: 'face',
    label: 'Face of Cube',
    body,
    feature: null,
    element: null,
    sketchItem: null,
    origin: null,
  }
}

function notifications(harness: PrintApp): { severity: string; message: string }[] {
  return harness.app.store
    .getState()
    .notifications.map(({ severity, message }) => ({ severity, message }))
}

function warning(message: string): Diagnostic {
  return { severity: 'warning', code: 'export_warning', message }
}

const THREE_MF: ExportRequest = { format: '3mf', scope: 'visible' }

const FORMATS: readonly [ExportRequest['format'], ExportFormat][] = [
  ['3mf', { kind: '3mf' }],
  ['stl', { kind: 'stl', binary: true, per_body: false }],
  ['obj', { kind: 'obj' }],
  ['step', { kind: 'step' }],
]

describe('exportModel without anything to export', () => {
  it('U-80 with no visible bodies shows nothing_to_export and calls no exportBodies', async () => {
    const harness = createPrintApp({
      document: documentState({
        bodies: [GHOST, EMPTY],
        meshes: { [GHOST.id]: meshSummary(2, CUBE_BOUNDS) },
      }),
    })
    await exportModel(harness.app, THREE_MF)
    expect(harness.core.callsTo('exportBodies')).toEqual([])
    expect(notifications(harness)).toEqual([{ severity: 'error', message: NOTHING_TO_EXPORT }])
  })

  it('U-80 a document without bodies shows nothing_to_export and calls no exportBodies', async () => {
    const harness = createPrintApp({ document: documentState({ bodies: [], meshes: {} }) })
    await exportModel(harness.app, THREE_MF)
    expect(harness.core.callsTo('exportBodies')).toEqual([])
    expect(notifications(harness)).toEqual([{ severity: 'error', message: NOTHING_TO_EXPORT }])
  })

  it('asks for no save location when there is nothing to export', async () => {
    const harness = createPrintApp({
      document: documentState({
        bodies: [GHOST],
        meshes: { [GHOST.id]: meshSummary(2, CUBE_BOUNDS) },
      }),
    })
    await exportModel(harness.app, THREE_MF)
    expect(harness.files.saveRequests).toEqual([])
    expect(harness.files.writes).toEqual([])
  })

  it('shows nothing_to_export for the selected scope when no body is selected', async () => {
    const harness = exportApp({ selection: [faceItem(CUBE.id)] })
    await exportModel(harness.app, { format: '3mf', scope: 'selected' })
    expect(harness.core.callsTo('exportBodies')).toEqual([])
    expect(notifications(harness)).toEqual([{ severity: 'error', message: NOTHING_TO_EXPORT }])
  })
})

describe('exportModel request', () => {
  it.each(FORMATS)(
    'U-80 the format %s maps to its protocol export format',
    async (format, expected) => {
      const harness = exportApp()
      await exportModel(harness.app, { format, scope: 'visible' })
      const calls = harness.core.callsTo('exportBodies')
      expect(calls).toHaveLength(1)
      expect(calls[0]?.[0]).toBe(DOCUMENT_HANDLE)
      expect(calls[0]?.[1]).toEqual(expected)
    },
  )

  it('exports the visible bodies with model-layer geometry, in document order', async () => {
    const harness = exportApp()
    await exportModel(harness.app, THREE_MF)
    expect(harness.core.callsTo('exportBodies')).toEqual([
      [DOCUMENT_HANDLE, { kind: '3mf' }, [CUBE.id, PEG.id]],
    ])
  })

  it('exports the selected bodies for the selected scope', async () => {
    const harness = exportApp({ selection: [faceItem(CUBE.id), bodyItem(PEG.id, PEG.name)] })
    await exportModel(harness.app, { format: 'stl', scope: 'selected' })
    expect(harness.core.callsTo('exportBodies')).toEqual([
      [DOCUMENT_HANDLE, { kind: 'stl', binary: true, per_body: false }, [PEG.id]],
    ])
  })
})

describe('exportModel saving', () => {
  it('U-80 the suggested name is "{document name}.3mf"', async () => {
    const harness = exportApp()
    await exportModel(harness.app, THREE_MF)
    expect(harness.files.saveRequests).toEqual([{ suggestedName: 'cube.3mf', kind: '3mf' }])
  })

  it.each(FORMATS)('suggests "{document name}.%s" with the matching file kind', async (format) => {
    const harness = exportApp()
    await exportModel(harness.app, { format, scope: 'visible' })
    expect(harness.files.saveRequests).toEqual([{ suggestedName: `cube.${format}`, kind: format }])
  })

  it('writes the bytes of the returned file to the chosen target', async () => {
    const harness = exportApp()
    await exportModel(harness.app, THREE_MF)
    expect(harness.files.writes).toEqual([
      { target: { kind: 'download', name: 'cube.3mf' }, kind: '3mf', bytes: THREE_MF_BYTES },
    ])
  })

  it('announces success with "Exported {file name}."', async () => {
    const harness = exportApp()
    await exportModel(harness.app, THREE_MF)
    const shown = notifications(harness)
    expect(shown).toHaveLength(1)
    expect(shown[0]?.message).toBe('Exported cube.3mf.')
    // ui.md 3.18 calls it an info notification and 4.3 gives `exported` the success severity.
    expect(['info', 'success']).toContain(shown[0]?.severity)
  })

  it('names the file the save picker chose in the success message', async () => {
    const harness = exportApp()
    const target = {
      kind: 'handle',
      name: 'bracket.3mf',
      handle: createFakeFileHandle('bracket.3mf'),
    } as const
    harness.files.nextSaveTarget = target
    await exportModel(harness.app, THREE_MF)
    expect(harness.files.writes).toEqual([{ target, kind: '3mf', bytes: THREE_MF_BYTES }])
    expect(notifications(harness).map((shown) => shown.message)).toEqual(['Exported bracket.3mf.'])
  })

  it('asks where to save before exporting, so a slow export cannot cost the dialog its click (3.18)', async () => {
    const harness = exportApp()
    let asked = -1
    harness.core.stub('exportBodies', () => {
      asked = harness.files.saveRequests.length
      return Promise.resolve({ files: [exportedFile('model.3mf', THREE_MF_BYTES)], warnings: [] })
    })
    await exportModel(harness.app, THREE_MF)
    expect(asked).toBe(1)
    expect(harness.files.writes).toHaveLength(1)
  })

  it('exports nothing when the save picker is canceled', async () => {
    const harness = exportApp()
    harness.files.nextSaveTarget = null
    await exportModel(harness.app, THREE_MF)
    expect(harness.core.callsTo('exportBodies')).toEqual([])
    expect(harness.files.writes).toEqual([])
    expect(notifications(harness)).toEqual([])
  })

  it('shows each warning diagnostic as a warning notification', async () => {
    const harness = exportApp()
    const first = 'Peg has no color, so it is exported in the default color.'
    const second = 'Cube has 2 open edges. Slicers may repair them differently.'
    harness.core.stub('exportBodies', () =>
      Promise.resolve({
        files: [exportedFile('model.3mf', THREE_MF_BYTES)],
        warnings: [warning(first), warning(second)],
      }),
    )
    await exportModel(harness.app, THREE_MF)
    expect(notifications(harness)).toEqual(
      expect.arrayContaining([
        { severity: 'warning', message: first },
        { severity: 'warning', message: second },
      ]),
    )
  })

  it('shows the message of a CoreError as an error notification and saves nothing', async () => {
    const harness = exportApp()
    const message = 'Export failed. The disk is full.'
    const error = await coreErrorFor({ code: 'export_failed', message })
    harness.core.stub('exportBodies', () => Promise.reject(error))
    await exportModel(harness.app, THREE_MF)
    expect(harness.files.writes).toEqual([])
    expect(notifications(harness)).toEqual([{ severity: 'error', message }])
  })
})

describe('exportModel body roles', () => {
  const BLOCKER: Body = { id: 'b-blocker', name: 'Blocker', role: 'support_blocker' }

  // The cube document with a visible part body and a visible support blocker, both with geometry.
  function modifierApp(overrides: Partial<AppState> = {}): PrintApp {
    return createPrintApp({
      document: documentState({
        bodies: [CUBE, BLOCKER],
        meshes: {
          [CUBE.id]: meshSummary(1, CUBE_BOUNDS),
          [BLOCKER.id]: meshSummary(2, PEG_BOUNDS),
        },
      }),
      ...overrides,
    })
  }

  it.each(['stl', 'obj', 'step'] as const)(
    'leaves visible modifier bodies out of %s, as the core does',
    async (format) => {
      const harness = modifierApp()
      await exportModel(harness.app, { format, scope: 'visible' })
      expect(harness.core.callsTo('exportBodies')[0]?.[2]).toEqual([CUBE.id])
    },
  )

  it('passes visible modifier bodies to a 3MF export, as the core does', async () => {
    const harness = modifierApp()
    await exportModel(harness.app, THREE_MF)
    expect(harness.core.callsTo('exportBodies')[0]?.[2]).toEqual([CUBE.id, BLOCKER.id])
  })

  it('exports a selected modifier body in any format', async () => {
    const harness = modifierApp({ selection: [bodyItem(BLOCKER.id, BLOCKER.name)] })
    await exportModel(harness.app, { format: 'stl', scope: 'selected' })
    expect(harness.core.callsTo('exportBodies')[0]?.[2]).toEqual([BLOCKER.id])
  })
})

describe('exportModel save failures', () => {
  it('shows a failing save picker as an error and exports and writes nothing', async () => {
    const harness = exportApp()
    vi.spyOn(harness.files, 'chooseSaveTarget').mockRejectedValue(
      new FileServiceError('io', 'The picker could not open.'),
    )
    await exportModel(harness.app, THREE_MF)
    expect(harness.core.callsTo('exportBodies')).toEqual([])
    expect(harness.files.writes).toEqual([])
    expect(notifications(harness)).toEqual([
      { severity: 'error', message: MESSAGES.save_io('cube.3mf', 'The picker could not open.') },
    ])
  })

  it('shows a refused write with the file name and announces no export', async () => {
    const harness = exportApp()
    harness.files.writeError = new FileServiceError('permission', 'No access.')
    await exportModel(harness.app, THREE_MF)
    expect(notifications(harness)).toEqual([
      { severity: 'error', message: MESSAGES.save_permission('cube.3mf') },
    ])
  })

  it.each([
    ['permission', MESSAGES.save_permission('cube.3mf')],
    ['not_found', MESSAGES.save_not_found('cube.3mf')],
    ['io', MESSAGES.save_io('cube.3mf', 'The disk is full.')],
  ] as const)('offers "Save as" when the %s error stops the write', async (reason, message) => {
    const harness = exportApp()
    harness.files.writeError = new FileServiceError(reason, 'The disk is full.')
    await exportModel(harness.app, THREE_MF)
    const [failure] = harness.app.store.getState().notifications
    expect(failure?.message).toBe(message)
    expect(failure?.actions.map((action) => action.label)).toEqual(['Save as'])
  })

  it('"Save as" asks again where to save the same file, without exporting again', async () => {
    const harness = exportApp()
    harness.files.writeError = new FileServiceError('permission', 'No access.')
    harness.core.stub('exportBodies', () =>
      Promise.resolve({
        files: [exportedFile('model.3mf', THREE_MF_BYTES)],
        warnings: [warning('Peg has no color, so it is exported in the default color.')],
      }),
    )
    await exportModel(harness.app, THREE_MF)
    const saveAs = harness.app.store.getState().notifications[0]?.actions[0]
    harness.files.writeError = null
    harness.files.nextSaveTarget = { kind: 'download', name: 'cube-copy.3mf' }
    saveAs?.run()
    await vi.waitFor(() => {
      expect(harness.files.writes).toHaveLength(1)
    })
    expect(harness.core.callsTo('exportBodies')).toHaveLength(1)
    expect(harness.files.saveRequests).toEqual([
      { suggestedName: 'cube.3mf', kind: '3mf' },
      { suggestedName: 'cube.3mf', kind: '3mf' },
    ])
    expect(harness.files.writes[0]?.bytes).toBe(THREE_MF_BYTES)
    await vi.waitFor(() => {
      expect(notifications(harness)).toEqual([
        { severity: 'error', message: MESSAGES.save_permission('cube.3mf') },
        { severity: 'success', message: 'Exported cube-copy.3mf.' },
        {
          severity: 'warning',
          message: 'Peg has no color, so it is exported in the default color.',
        },
      ])
    })
  })

  it('rethrows a write error that is not a file service error', async () => {
    const harness = exportApp()
    const bug = new TypeError('bytes is not iterable')
    vi.spyOn(harness.files, 'write').mockRejectedValue(bug)
    await expect(exportModel(harness.app, THREE_MF)).rejects.toBe(bug)
    expect(notifications(harness)).toEqual([])
  })

  it('shows no export warnings when the file was not saved', async () => {
    const harness = exportApp()
    harness.files.nextSaveTarget = null
    harness.core.stub('exportBodies', () =>
      Promise.resolve({
        files: [exportedFile('model.3mf', THREE_MF_BYTES)],
        warnings: [warning('Peg has no color, so it is exported in the default color.')],
      }),
    )
    await exportModel(harness.app, THREE_MF)
    expect(notifications(harness)).toEqual([])
  })

  it('U-119 an export with f-fil1 in error shows the warning "Exported cube.3mf. Fillet 1 failed, so it is not in the export." with Show problems', async () => {
    const base = documentState({ name: 'cube' })
    const harness = createPrintApp({
      catalog: TEST_CATALOG,
      document: {
        ...base,
        recipe: {
          ...base.recipe,
          features: [{ id: 'f-fil1', type: 'fillet', body: CUBE.id, edges: [], radius: '15 mm' }],
        },
        featureStatus: {
          'f-fil1': {
            id: 'f-fil1',
            status: 'error',
            messages: [{ severity: 'error', code: 'fillet_too_large', message: 'Too large.' }],
            repairs: [],
          },
        },
      },
    })
    await exportModel(harness.app, THREE_MF)
    const [shown] = harness.app.store.getState().notifications
    expect(shown).toMatchObject({
      severity: 'warning',
      message: 'Exported cube.3mf. Fillet 1 failed, so it is not in the export.',
      code: 'exported_with_failures',
    })
    expect(shown?.actions.map((action) => action.label)).toEqual(['Show problems'])
    shown?.actions[0]?.run()
    expect(harness.app.store.getState().panels.side).toBe('problems')
  })
})
