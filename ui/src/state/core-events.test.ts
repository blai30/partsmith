import { describe, expect, it } from 'vitest'
import type { BodyMeshData, CoreEvent } from '../core/events'
import type { Document } from '../generated/protocol'
import { CUBE_DOCUMENT, CUBE_SKETCH_SOLUTION, emptyDocument } from '../test/fakes/documents'
import { checkResult, finding, printReport } from '../test/fakes/state'
import { adoptDocument, applyCoreEvent, releaseDocuments } from './core-events'
import { createAppStore, type AppStore } from './store'

const DETAILS = { name: 'cube', file: null, localUnsaved: false } as const

const MESH: BodyMeshData = {
  positions: new Float32Array(9),
  normals: new Float32Array(9),
  indices: Uint32Array.of(0, 1, 2),
  faceIds: new Uint32Array(3),
  edgePositions: new Float32Array(0),
  edgeIds: new Uint32Array(0),
  vertexPositions: new Float32Array(0),
  vertexIds: new Uint32Array(0),
  faceCount: 1,
  edgeCount: 0,
  vertexCount: 0,
  boundingBox: { min: [-10, -10, 0], max: [10, 10, 20] },
}

function documentChanged(
  revision: number,
  document: Document = CUBE_DOCUMENT,
  handle = 1,
): CoreEvent {
  return {
    type: 'document_changed',
    documentHandle: handle,
    revision,
    document,
    dirty: revision > 0,
    undo: revision > 0 ? 'Edit' : null,
    redo: null,
    transactionOpen: false,
    transaction: null,
    focus: null,
  }
}

function bodyMesh(
  revision: number,
  meshId: number,
  layer: 'model' | 'preview' = 'model',
  mesh: BodyMeshData | null = MESH,
): CoreEvent {
  return { type: 'body_mesh', documentHandle: 1, revision, body: 'b-cube', layer, meshId, mesh }
}

/** A store showing cube.ptsm (handle 1) at revision 3. */
function shownCube(): AppStore {
  const store = createAppStore()
  applyCoreEvent(store, documentChanged(3))
  adoptDocument(store, 1, DETAILS)
  return store
}

describe('adoptDocument', () => {
  it('shows a document the core reported before it was adopted, with the details only the UI knows', () => {
    const store = createAppStore()
    applyCoreEvent(store, documentChanged(0))
    applyCoreEvent(store, bodyMesh(0, 4))
    expect(store.getState().document).toBeNull()
    adoptDocument(store, 1, { name: 'cube', file: null, localUnsaved: true })
    expect(store.getState().document).toMatchObject({
      documentHandle: 1,
      name: 'cube',
      revision: 0,
      recipe: CUBE_DOCUMENT,
      localUnsaved: true,
      meshes: { 'b-cube': { meshId: 4 } },
    })
  })

  it('shows an adopted document with its first document_changed when it had none yet', () => {
    const store = createAppStore()
    adoptDocument(store, 2, DETAILS)
    expect(store.getState().document).toBeNull()
    applyCoreEvent(store, bodyMesh(1, 4))
    expect(store.getState().document).toBeNull()
    applyCoreEvent(store, documentChanged(1, CUBE_DOCUMENT, 2))
    expect(store.getState().document).toMatchObject({
      documentHandle: 2,
      name: 'cube',
      revision: 1,
    })
  })

  it('keeps showing the current document while another one is reported', () => {
    const store = shownCube()
    applyCoreEvent(store, documentChanged(0, emptyDocument(), 2))
    expect(store.getState().document?.documentHandle).toBe(1)
    expect(store.getState().document?.recipe).toEqual(CUBE_DOCUMENT)
  })

  it('releaseDocuments forgets the shown and the reported documents', () => {
    const store = shownCube()
    applyCoreEvent(store, documentChanged(0, emptyDocument(), 2))
    releaseDocuments(store)
    expect(store.getState().document).toBeNull()
    adoptDocument(store, 2, DETAILS)
    expect(store.getState().document).toBeNull()
  })
})

describe('applyCoreEvent', () => {
  it('ignores events for an older revision and applies equal ones (2.7)', () => {
    const store = shownCube()
    applyCoreEvent(store, documentChanged(2, emptyDocument()))
    expect(store.getState().document?.recipe).toEqual(CUBE_DOCUMENT)
    applyCoreEvent(store, bodyMesh(3, 9))
    expect(store.getState().document?.meshes['b-cube']?.meshId).toBe(9)
  })

  it('keeps the model layer only: a preview mesh leaves the summaries alone', () => {
    const store = shownCube()
    applyCoreEvent(store, bodyMesh(3, 5))
    applyCoreEvent(store, bodyMesh(3, 6, 'preview'))
    expect(store.getState().document?.meshes['b-cube']?.meshId).toBe(5)
  })

  it('removes the summary of a body whose model mesh is empty', () => {
    const store = shownCube()
    applyCoreEvent(store, bodyMesh(3, 5))
    applyCoreEvent(store, bodyMesh(4, 6, 'model', null))
    expect(store.getState().document?.meshes).toEqual({})
  })

  it('drops meshes, solutions and datums of bodies and features the recipe no longer has', () => {
    const store = shownCube()
    applyCoreEvent(store, bodyMesh(3, 5))
    applyCoreEvent(store, {
      type: 'sketch_solved',
      documentHandle: 1,
      revision: 3,
      solution: CUBE_SKETCH_SOLUTION,
      display: null,
    })
    applyCoreEvent(store, documentChanged(4, emptyDocument()))
    const document = store.getState().document
    expect(document?.meshes).toEqual({})
    expect(document?.sketches).toEqual({})
  })

  it('stores statuses, parameter values and diagnostics of a recompute by id', () => {
    const store = shownCube()
    applyCoreEvent(store, {
      type: 'recomputed',
      documentHandle: 1,
      revision: 3,
      features: [{ id: 'f-ext1', status: 'warning', messages: [], repairs: [] }],
      parameters: [{ id: 'p-h', value: 40, dimension: { length: 1, angle: 0 } }],
      diagnostics: [],
    })
    expect(store.getState().document?.featureStatus['f-ext1']?.status).toBe('warning')
    expect(store.getState().document?.parameterValues['p-h']?.value).toBe(40)
  })

  it('stores a print report with the revision it describes', () => {
    const store = shownCube()
    const report = printReport({
      'b-cube': [checkResult('overhangs', [finding('overhangs', 'warning')])],
    })
    applyCoreEvent(store, {
      type: 'print_health',
      documentHandle: 1,
      revision: 3,
      report,
      overhang: {},
    })
    expect(store.getState().print).toMatchObject({ report, reportRevision: 3 })
  })

  it('stores a datum plane placement, or null when it failed', () => {
    const store = shownCube()
    applyCoreEvent(store, {
      type: 'datum_plane',
      documentHandle: 1,
      revision: 3,
      feature: 'f-dp1',
      placement: null,
    })
    expect(store.getState().document?.datums).toEqual({ 'f-dp1': null })
  })
})
