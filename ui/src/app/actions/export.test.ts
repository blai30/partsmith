import { describe, expect, it } from 'vitest'
import type { Diagnostic, ExportFormat } from '../../generated/protocol'
import {
  CUBE,
  CUBE_BOUNDS,
  DOCUMENT_HANDLE,
  EMPTY,
  GHOST,
  PEG,
  PEG_BOUNDS,
  THREE_MF_BYTES,
  coreError,
  createTestApp,
  documentState,
  exportedFile,
  meshSummary,
  type TestApp,
} from '../../print/test-app.testing'
import type { AppState, SelectionItem } from '../../state/store'
import { exportModel, type ExportRequest } from './export'

const NOTHING_TO_EXPORT = 'There is no visible body with geometry to export.'

// The document "cube" with, in browser order, a visible body with geometry, a hidden body with
// geometry, a visible body without geometry and a second visible body with geometry.
function exportApp(overrides: Partial<AppState> = {}): TestApp {
  return createTestApp({
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

function bodyItem(id: string, name: string): SelectionItem {
  return {
    key: `body:${id}`,
    kind: 'body',
    label: name,
    body: id,
    feature: null,
    element: null,
    sketchItem: null,
    origin: null,
  }
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

function notifications(harness: TestApp): { severity: string; message: string }[] {
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
    const harness = createTestApp({
      document: documentState({
        bodies: [GHOST, EMPTY],
        meshes: { [GHOST.id]: meshSummary(2, CUBE_BOUNDS) },
      }),
    })
    await exportModel(harness.app, THREE_MF)
    expect(harness.core.exportBodies).not.toHaveBeenCalled()
    expect(notifications(harness)).toEqual([{ severity: 'error', message: NOTHING_TO_EXPORT }])
  })

  it('U-80 a document without bodies shows nothing_to_export and calls no exportBodies', async () => {
    const harness = createTestApp({ document: documentState({ bodies: [], meshes: {} }) })
    await exportModel(harness.app, THREE_MF)
    expect(harness.core.exportBodies).not.toHaveBeenCalled()
    expect(notifications(harness)).toEqual([{ severity: 'error', message: NOTHING_TO_EXPORT }])
  })

  it('asks for no save location when there is nothing to export', async () => {
    const harness = createTestApp({
      document: documentState({ bodies: [GHOST], meshes: { [GHOST.id]: meshSummary(2, CUBE_BOUNDS) } }),
    })
    await exportModel(harness.app, THREE_MF)
    expect(harness.files.chooseSaveTarget).not.toHaveBeenCalled()
    expect(harness.files.write).not.toHaveBeenCalled()
  })

  it('shows nothing_to_export for the selected scope when no body is selected', async () => {
    const harness = exportApp({ selection: [faceItem(CUBE.id)] })
    await exportModel(harness.app, { format: '3mf', scope: 'selected' })
    expect(harness.core.exportBodies).not.toHaveBeenCalled()
    expect(notifications(harness)).toEqual([{ severity: 'error', message: NOTHING_TO_EXPORT }])
  })
})

describe('exportModel request', () => {
  it.each(FORMATS)('U-80 the format %s maps to its protocol export format', async (format, expected) => {
    const harness = exportApp()
    await exportModel(harness.app, { format, scope: 'visible' })
    expect(harness.core.exportBodies).toHaveBeenCalledTimes(1)
    expect(harness.core.exportBodies).toHaveBeenCalledWith(
      DOCUMENT_HANDLE,
      expected,
      expect.anything(),
    )
  })

  it('exports the visible bodies with model-layer geometry, in document order', async () => {
    const harness = exportApp()
    await exportModel(harness.app, THREE_MF)
    expect(harness.core.exportBodies).toHaveBeenCalledWith(DOCUMENT_HANDLE, { kind: '3mf' }, [
      CUBE.id,
      PEG.id,
    ])
  })

  it('exports the selected bodies for the selected scope', async () => {
    const harness = exportApp({ selection: [faceItem(CUBE.id), bodyItem(PEG.id, PEG.name)] })
    await exportModel(harness.app, { format: 'stl', scope: 'selected' })
    expect(harness.core.exportBodies).toHaveBeenCalledWith(
      DOCUMENT_HANDLE,
      { kind: 'stl', binary: true, per_body: false },
      [PEG.id],
    )
  })
})

describe('exportModel saving', () => {
  it('U-80 the suggested name is "{document name}.3mf"', async () => {
    const harness = exportApp()
    await exportModel(harness.app, THREE_MF)
    expect(harness.files.chooseSaveTarget).toHaveBeenCalledWith('cube.3mf', '3mf')
  })

  it.each(FORMATS)(
    'suggests "{document name}.%s" with the matching file kind',
    async (format) => {
      const harness = exportApp()
      await exportModel(harness.app, { format, scope: 'visible' })
      expect(harness.files.chooseSaveTarget).toHaveBeenCalledWith(`cube.${format}`, format)
    },
  )

  it('writes the bytes of the returned file to the chosen target', async () => {
    const harness = exportApp()
    await exportModel(harness.app, THREE_MF)
    expect(harness.files.write).toHaveBeenCalledTimes(1)
    expect(harness.files.write).toHaveBeenCalledWith(
      { kind: 'download', name: 'cube.3mf' },
      '3mf',
      THREE_MF_BYTES,
    )
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
    const handle = { kind: 'file', name: 'bracket.3mf' } as unknown as FileSystemFileHandle
    const target = { kind: 'handle', name: 'bracket.3mf', handle } as const
    harness.files.chooseSaveTarget.mockResolvedValue(target)
    await exportModel(harness.app, THREE_MF)
    expect(harness.files.write).toHaveBeenCalledWith(target, '3mf', THREE_MF_BYTES)
    expect(notifications(harness).map((shown) => shown.message)).toEqual(['Exported bracket.3mf.'])
  })

  it('does nothing more when the save picker is canceled', async () => {
    const harness = exportApp()
    harness.files.chooseSaveTarget.mockResolvedValue(null)
    await exportModel(harness.app, THREE_MF)
    expect(harness.files.write).not.toHaveBeenCalled()
    expect(notifications(harness)).toEqual([])
  })

  it('shows each warning diagnostic as a warning notification', async () => {
    const harness = exportApp()
    const first = 'Peg has no color, so it is exported in the default color.'
    const second = 'Cube has 2 open edges. Slicers may repair them differently.'
    harness.core.exportBodies.mockResolvedValue({
      files: [exportedFile('model.3mf', THREE_MF_BYTES)],
      warnings: [warning(first), warning(second)],
    })
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
    harness.core.exportBodies.mockRejectedValue(await coreError('export_failed', message))
    await exportModel(harness.app, THREE_MF)
    expect(harness.files.write).not.toHaveBeenCalled()
    expect(notifications(harness)).toEqual([{ severity: 'error', message }])
  })
})
