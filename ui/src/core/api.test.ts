import { describe, expect, it } from 'vitest'
import type { Command, Edit, FixRef } from '../generated/protocol'
import {
  CUBE_DOCUMENT,
  CUBE_TOP_FACE,
  documentText,
  NC09_DOCUMENT,
  RECT_SKETCH_DOCUMENT,
  RECT_SKETCH_SOLUTION,
} from '../test/fakes/documents'
import { withoutNulls } from '../test/fakes/protocol'
import { createFakeTransport, type FakeTransport } from '../test/fakes/transport'
import { createCoreApi, rebuildsDocument, rebuiltDocument, type CoreApi } from './api'
import { CoreError, createCoreClient } from './client'

type Setup = { readonly api: CoreApi; readonly transport: FakeTransport }

async function setUp(): Promise<Setup> {
  const transport = createFakeTransport({ solutions: { 'f-sk1': RECT_SKETCH_SOLUTION } })
  await transport.start()
  return { api: createCoreApi(createCoreClient(transport)), transport }
}

function lastCommand(transport: FakeTransport): unknown {
  return withoutNulls(transport.commands.at(-1))
}

const SUPERSEDED = { code: 'superseded', message: 'A newer request replaced this one.' } as const

const RENAME_EDITS: readonly Edit[] = [{ type: 'update_body', body: { id: 'b-cube', name: 'Box' } }]

describe('createCoreApi', () => {
  it('engineInfo returns the versions in camel case', async () => {
    const { api, transport } = await setUp()
    await expect(api.engineInfo()).resolves.toEqual({
      protocolVersion: 1,
      engineVersion: '0.1.0',
      formatVersion: 1,
      kernel: 'Fake kernel',
    })
    expect(lastCommand(transport)).toEqual({ type: 'engine_info' })
  })

  it('featureCatalog and listPrinterProfiles return the entries and profiles', async () => {
    const { api } = await setUp()
    const catalog = await api.featureCatalog()
    expect(catalog.map((entry) => entry.type)).toContain('extrude')
    const printers = await api.listPrinterProfiles()
    expect(printers.map((profile) => profile.id)).toEqual(['generic-fdm', 'generic-resin'])
  })

  it('newDocument sends the units and resolves the document handle', async () => {
    const { api, transport } = await setUp()
    await expect(api.newDocument('in')).resolves.toBe(1)
    expect(lastCommand(transport)).toEqual({ type: 'new_document', units: 'in' })
    await expect(api.newDocument()).resolves.toBe(2)
    expect(lastCommand(transport)).toEqual({ type: 'new_document' })
  })

  it('openDocument sends the text and resolves the handle with migratedFrom null', async () => {
    const { api, transport } = await setUp()
    const text = documentText(CUBE_DOCUMENT)
    await expect(api.openDocument(text)).resolves.toEqual({ documentHandle: 1, migratedFrom: null })
    const command = transport.commandsOfType('open_document')[0]
    expect(command?.text).toBe(text)
    expect(command?.assets ?? []).toEqual([])
  })

  it('openDocument passes migrated_from through as migratedFrom', async () => {
    const { api, transport } = await setUp()
    transport.handle('open_document', () => ({
      result: { type: 'open_document', doc: 4, migrated_from: 1 },
    }))
    await expect(api.openDocument('{}')).resolves.toEqual({ documentHandle: 4, migratedFrom: 1 })
  })

  it('saveDocument resolves the text and its revision; markSaved and closeDocument send their documents', async () => {
    const { api, transport } = await setUp()
    const handle = await api.openDocument(documentText(CUBE_DOCUMENT))
    const saved = await api.saveDocument(handle.documentHandle)
    expect(saved).toEqual({ text: documentText(CUBE_DOCUMENT), revision: 0 })
    await api.markSaved(handle.documentHandle, saved.revision)
    expect(lastCommand(transport)).toEqual({ type: 'mark_saved', doc: 1, revision: 0 })
    await api.closeDocument(handle.documentHandle)
    expect(lastCommand(transport)).toEqual({ type: 'close_document', doc: 1 })
  })

  it('applyEdits sends the label outside a transaction and none inside, and resolves the revision', async () => {
    const { api, transport } = await setUp()
    const { documentHandle } = await api.openDocument(documentText(CUBE_DOCUMENT))
    await expect(api.applyEdits(documentHandle, 'Rename', RENAME_EDITS)).resolves.toBe(1)
    expect(lastCommand(transport)).toEqual({
      type: 'apply_edits',
      doc: 1,
      label: 'Rename',
      edits: RENAME_EDITS,
    })
    await api.beginTransaction(documentHandle, 'Edit')
    expect(lastCommand(transport)).toEqual({ type: 'begin_transaction', doc: 1, label: 'Edit' })
    await api.applyEdits(documentHandle, null, RENAME_EDITS)
    expect(lastCommand(transport)).toEqual({ type: 'apply_edits', doc: 1, edits: RENAME_EDITS })
  })

  it('applyFix sends apply_fix with its FixRef and resolves the revision (2.6)', async () => {
    const fix: FixRef = {
      feature: 'f-ext1',
      code: 'out_of_range',
      field: '/extent/expr',
      cause: false,
      index: 0,
      revision: 0,
    }
    const transport = createFakeTransport({
      fixes: [
        {
          feature: 'f-ext1',
          code: 'out_of_range',
          field: '/extent/expr',
          index: 0,
          label: 'Use 20 mm',
          edits: RENAME_EDITS,
        },
      ],
    })
    await transport.start()
    const api = createCoreApi(createCoreClient(transport))
    const { documentHandle } = await api.openDocument(documentText(CUBE_DOCUMENT))
    await expect(api.applyFix(documentHandle, fix)).resolves.toBe(1)
    expect(lastCommand(transport)).toEqual({ type: 'apply_fix', doc: 1, fix })
  })

  it('applyFix rejects a fix the core no longer offers with its stale_fix CoreError', async () => {
    const { api } = await setUp()
    const { documentHandle } = await api.openDocument(documentText(CUBE_DOCUMENT))
    const refused = await api
      .applyFix(documentHandle, {
        feature: 'f-ext1',
        code: 'no_effect',
        cause: false,
        index: 0,
        revision: 0,
      })
      .catch((error: unknown) => error)
    expect(refused).toBeInstanceOf(CoreError)
    expect(refused).toMatchObject({ code: 'stale_fix' })
  })

  it('setDocumentUnits sends set_document_units with keep_sizes and resolves the revision and kept (2.6)', async () => {
    const { api, transport } = await setUp()
    const { documentHandle } = await api.openDocument(documentText(NC09_DOCUMENT))
    await expect(api.setDocumentUnits(documentHandle, 'in', true)).resolves.toEqual({
      revision: 1,
      kept: 4,
    })
    expect(lastCommand(transport)).toEqual({
      type: 'set_document_units',
      doc: 1,
      units: 'in',
      keep_sizes: true,
    })
    await expect(api.setDocumentUnits(documentHandle, 'cm', false)).resolves.toEqual({
      revision: 2,
      kept: 0,
    })
    expect(lastCommand(transport)).toEqual({
      type: 'set_document_units',
      doc: 1,
      units: 'cm',
      keep_sizes: false,
    })
  })

  it('maps the transaction commands', async () => {
    const { api, transport } = await setUp()
    const { documentHandle } = await api.openDocument(documentText(CUBE_DOCUMENT))
    await api.beginTransaction(documentHandle, 'Edit Extrude 1')
    await api.setFocus(documentHandle, 'f-ext1')
    expect(lastCommand(transport)).toEqual({ type: 'set_focus', doc: 1, feature: 'f-ext1' })
    await api.setFocus(documentHandle, null)
    expect(lastCommand(transport)).toEqual({ type: 'set_focus', doc: 1 })
    await api.applyEdits(documentHandle, null, RENAME_EDITS)
    const committed = await api.commitTransaction(documentHandle)
    expect(committed.recorded).toBe(true)
    expect(typeof committed.revision).toBe('number')
    expect(lastCommand(transport)).toEqual({ type: 'commit_transaction', doc: 1 })
    await api.beginTransaction(documentHandle, 'Roll back')
    await expect(api.cancelTransaction(documentHandle)).resolves.toEqual(expect.any(Number))
    expect(lastCommand(transport)).toEqual({ type: 'cancel_transaction', doc: 1 })
  })

  it('undo and redo resolve the revision and the label', async () => {
    const { api, transport } = await setUp()
    const { documentHandle } = await api.openDocument(documentText(CUBE_DOCUMENT))
    await api.applyEdits(documentHandle, 'Rename', RENAME_EDITS)
    await expect(api.undo(documentHandle)).resolves.toEqual({ revision: 2, label: 'Rename' })
    expect(lastCommand(transport)).toEqual({ type: 'undo', doc: 1 })
    await expect(api.redo(documentHandle)).resolves.toEqual({ revision: 3, label: 'Rename' })
    expect(lastCommand(transport)).toEqual({ type: 'redo', doc: 1 })
  })

  it('renameParameter sends rename_parameter and resolves the revision', async () => {
    const { api, transport } = await setUp()
    const { documentHandle } = await api.openDocument(
      documentText({
        ...CUBE_DOCUMENT,
        parameters: [{ id: 'p-h', name: 'height', expr: '20 mm' }],
      }),
    )
    await expect(api.renameParameter(documentHandle, 'p-h', 'h')).resolves.toBe(1)
    expect(lastCommand(transport)).toEqual({
      type: 'rename_parameter',
      doc: 1,
      parameter: 'p-h',
      name: 'h',
    })
  })

  it('solveSketch sends the candidate and resolves the solution with display null without fill buffers', async () => {
    const { api, transport } = await setUp()
    const { documentHandle } = await api.openDocument(documentText(RECT_SKETCH_DOCUMENT))
    const solved = await api.solveSketch(documentHandle, 'f-sk1')
    expect(solved).toEqual({ solution: RECT_SKETCH_SOLUTION, display: null })
    expect(lastCommand(transport)).toEqual({ type: 'solve_sketch', doc: 1, feature: 'f-sk1' })
    const candidate = {
      plane: { kind: 'origin', plane: 'XY' },
      entities: [],
      constraints: [],
    } as const
    await api.solveSketch(documentHandle, 'f-sk1', {
      plane: { kind: 'origin', plane: 'XY' },
      entities: [],
      constraints: [],
    })
    expect(lastCommand(transport)).toEqual({
      type: 'solve_sketch',
      doc: 1,
      feature: 'f-sk1',
      candidate,
    })
  })

  it('dragSketch and endSketchDrag send the point, the target and the commit flag', async () => {
    const { api, transport } = await setUp()
    const { documentHandle } = await api.openDocument(documentText(RECT_SKETCH_DOCUMENT))
    await api.dragSketch(documentHandle, 'f-sk1', 'e-1.end', [45, 0])
    expect(lastCommand(transport)).toEqual({
      type: 'drag_sketch',
      doc: 1,
      feature: 'f-sk1',
      point: 'e-1.end',
      to: [45, 0],
    })
    await api.endSketchDrag(documentHandle, 'f-sk1', true)
    expect(lastCommand(transport)).toEqual({
      type: 'end_sketch_drag',
      doc: 1,
      feature: 'f-sk1',
      commit: true,
    })
  })

  it.each<[string, (api: CoreApi) => Promise<unknown>, Command['type']]>([
    ['solveSketch', (api) => api.solveSketch(1, 'f-sk1'), 'solve_sketch'],
    ['dragSketch', (api) => api.dragSketch(1, 'f-sk1', 'e-1.end', [1, 2]), 'drag_sketch'],
    [
      'analyzePrint',
      (api) => api.analyzePrint(1, { orientation: { kind: 'as_modeled' } }),
      'analyze_print',
    ],
  ])('%s resolves null when the core answers superseded', async (_name, call, type) => {
    const { api, transport } = await setUp()
    await api.openDocument(documentText(RECT_SKETCH_DOCUMENT))
    transport.handle(type, () => ({ error: SUPERSEDED }))
    await expect(call(api)).resolves.toBeNull()
  })

  it('every other method rejects a superseded answer with that CoreError', async () => {
    const { api, transport } = await setUp()
    const { documentHandle } = await api.openDocument(documentText(CUBE_DOCUMENT))
    transport.handle('undo', () => ({ error: SUPERSEDED }))
    const error = await api.undo(documentHandle).catch((rejection: unknown) => rejection)
    expect(error).toBeInstanceOf(CoreError)
    expect(error instanceof CoreError ? error.code : null).toBe('superseded')
  })

  it('evaluate resolves the value and dimension', async () => {
    const { api, transport } = await setUp()
    const { documentHandle } = await api.openDocument(documentText(CUBE_DOCUMENT))
    await expect(api.evaluate(documentHandle, '2 * 5 mm')).resolves.toEqual({
      value: 10,
      dimension: { length: 1, angle: 0 },
    })
    expect(lastCommand(transport)).toEqual({ type: 'evaluate', doc: 1, expr: '2 * 5 mm' })
  })

  it('listElements, pickReference and resolveReferences send their arguments and resolve the core data', async () => {
    const { api, transport } = await setUp()
    const { documentHandle } = await api.openDocument(documentText(CUBE_DOCUMENT))
    await api.listElements(documentHandle, 'b-cube', 'face')
    expect(lastCommand(transport)).toEqual({
      type: 'list_elements',
      doc: 1,
      body: 'b-cube',
      kind: 'face',
    })
    transport.handle('pick_reference', () => ({
      result: { type: 'pick_reference', element: CUBE_TOP_FACE },
    }))
    await expect(
      api.pickReference(documentHandle, { mesh_id: 2, kind: 'face', index: 3 }),
    ).resolves.toEqual(CUBE_TOP_FACE)
    expect(lastCommand(transport)).toEqual({
      type: 'pick_reference',
      doc: 1,
      pick: { mesh_id: 2, kind: 'face', index: 3 },
    })
    const references = [{ body: 'b-cube', reference: CUBE_TOP_FACE.reference }]
    await expect(api.resolveReferences(documentHandle, references)).resolves.toEqual([
      { status: 'resolved', matches: [] },
    ])
    expect(lastCommand(transport)).toEqual({ type: 'resolve_references', doc: 1, references })
  })

  it('analyzePrint sends the orientation and the profile only when given', async () => {
    const { api, transport } = await setUp()
    const { documentHandle } = await api.openDocument(documentText(CUBE_DOCUMENT))
    await expect(
      api.analyzePrint(documentHandle, { orientation: { kind: 'as_modeled' } }),
    ).resolves.toBe('ok')
    expect(lastCommand(transport)).toEqual({
      type: 'analyze_print',
      doc: 1,
      orientation: { kind: 'as_modeled' },
    })
    await api.analyzePrint(documentHandle, {
      profile: 'generic-resin',
      orientation: { kind: 'as_modeled' },
    })
    expect(lastCommand(transport)).toEqual({
      type: 'analyze_print',
      doc: 1,
      profile: 'generic-resin',
      orientation: { kind: 'as_modeled' },
    })
  })

  it('exportBodies copies each file out of the frame and maps its media type', async () => {
    const { api, transport } = await setUp()
    const { documentHandle } = await api.openDocument(documentText(CUBE_DOCUMENT))
    const exported = await api.exportBodies(documentHandle, { kind: '3mf' }, ['b-cube'])
    expect(lastCommand(transport)).toEqual({
      type: 'export',
      doc: 1,
      format: { kind: '3mf' },
      bodies: ['b-cube'],
    })
    expect(exported.warnings).toEqual([])
    expect(exported.files).toHaveLength(1)
    const [file] = exported.files
    expect(file?.name).toBe('model.3mf')
    expect(file?.mediaType).toBe('model/3mf')
    expect(Array.from(file?.bytes ?? [])).toEqual([0x50, 0x4b, 0x03, 0x04])
    expect(file?.bytes.byteOffset).toBe(0)
    expect(file?.bytes.buffer.byteLength).toBe(4)
  })

  it('measure sends its one target and resolves the measurement', async () => {
    const { api, transport } = await setUp()
    const { documentHandle } = await api.openDocument(documentText(CUBE_DOCUMENT))
    const measured = await api.measure(documentHandle, { kind: 'body', body: 'b-cube' })
    expect(lastCommand(transport)).toEqual({
      type: 'measure',
      doc: 1,
      targets: [{ kind: 'body', body: 'b-cube' }],
    })
    expect(measured.volume).toBe(8000)
  })

  it('exportBodies sends the orientation only when given', async () => {
    const { api, transport } = await setUp()
    const { documentHandle } = await api.openDocument(documentText(CUBE_DOCUMENT))
    const orientation = {
      kind: 'face_down',
      body: 'b-cube',
      face: CUBE_TOP_FACE.reference,
    } as const
    await api.exportBodies(documentHandle, { kind: '3mf' }, ['b-cube'], orientation)
    expect(lastCommand(transport)).toEqual({
      type: 'export',
      doc: 1,
      format: { kind: '3mf' },
      bodies: ['b-cube'],
      orientation: withoutNulls(orientation),
    })
  })

  it('exportBodies without bodies sends none', async () => {
    const { api, transport } = await setUp()
    const { documentHandle } = await api.openDocument(documentText(CUBE_DOCUMENT))
    await api.exportBodies(documentHandle, { kind: 'stl', binary: true, per_body: false })
    expect(lastCommand(transport)).toEqual({
      type: 'export',
      doc: 1,
      format: { kind: 'stl', binary: true, per_body: false },
    })
  })
})

describe('rebuildsDocument and rebuiltDocument', () => {
  it('count edits, fixes, transactions, undo, opening and units as rebuilds, but not saving, exporting or queries', () => {
    const rebuilding: readonly Command[] = [
      { type: 'apply_edits', doc: 1, label: 'Rename', edits: [...RENAME_EDITS] },
      {
        type: 'apply_fix',
        doc: 1,
        fix: { feature: 'f-ext1', code: 'no_effect', cause: false, index: 0, revision: 1 },
      },
      { type: 'commit_transaction', doc: 1 },
      { type: 'undo', doc: 1 },
      { type: 'open_document', text: '{}' },
      { type: 'set_document_units', doc: 1, units: 'in', keep_sizes: true },
    ]
    const quiet: readonly Command[] = [
      { type: 'save_document', doc: 1 },
      { type: 'mark_saved', doc: 1, revision: 2 },
      { type: 'begin_transaction', doc: 1, label: 'Extrude' },
      { type: 'export', doc: 1, format: { kind: 'step' } },
      { type: 'engine_info' },
    ]
    expect(rebuilding.map(rebuildsDocument)).toEqual([true, true, true, true, true, true])
    expect(quiet.map(rebuildsDocument)).toEqual([false, false, false, false, false])
  })

  it('names the document from the command, or from the result of open_document', () => {
    expect(
      rebuiltDocument({ type: 'undo', doc: 3 }, { type: 'undo', revision: 4, label: 'Edit' }),
    ).toBe(3)
    expect(
      rebuiltDocument(
        { type: 'open_document', text: '{}' },
        { type: 'open_document', doc: 7, migrated_from: null },
      ),
    ).toBe(7)
    expect(
      rebuiltDocument(
        { type: 'save_document', doc: 3 },
        { type: 'save_document', text: '', assets: [], revision: 1 },
      ),
    ).toBeNull()
  })
})
