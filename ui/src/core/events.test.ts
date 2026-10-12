import { describe, expect, it } from 'vitest'
import type { BoundingBox, Event as ProtocolEvent, MeshBuffers } from '../generated/protocol'
import { emptyDocument, RECT_SKETCH_SOLUTION } from '../test/fakes/documents'
import { meshParts, readResponse, responseFrame } from '../test/fakes/protocol'
import { decodeEvent } from './events'
import type { BufferView } from './frame'
import { CoreFatalError } from './transport'

const BOUNDS: BoundingBox = { min: [-10, -10, 0], max: [10, 10, 20] }

/** An event and the buffers of the frame it arrived in, decoded the way the client decodes them. */
function received(
  event: ProtocolEvent,
  buffers: readonly BufferView[] = [],
): { readonly event: ProtocolEvent; readonly buffers: readonly BufferView[]; readonly frame: ArrayBuffer } {
  const frame = responseFrame({
    id: 1,
    result: { type: 'close_document' },
    events: [event],
    buffers,
  })
  const decoded = readResponse(frame)
  const [first] = decoded.events
  if (first === undefined) {
    throw new Error('The frame lost its event.')
  }
  return { event: first, buffers: decoded.buffers, frame }
}

function bodyMesh(mesh: MeshBuffers | null): ProtocolEvent {
  return {
    type: 'body_mesh',
    doc: 1,
    revision: 4,
    body: 'b-1',
    layer: 'model',
    mesh_id: 12,
    ...(mesh === null ? {} : { mesh }),
  }
}

describe('decodeEvent', () => {
  it('U-17 a body_mesh becomes typed arrays that alias the frame buffer', () => {
    const parts = meshParts(BOUNDS, 0)
    const { event, buffers, frame } = received(bodyMesh(parts.mesh), parts.buffers)
    const decoded = decodeEvent(event, buffers)
    expect(decoded?.type).toBe('body_mesh')
    if (decoded?.type !== 'body_mesh') {
      return
    }
    expect(decoded).toMatchObject({
      documentHandle: 1,
      revision: 4,
      body: 'b-1',
      layer: 'model',
      meshId: 12,
    })
    const mesh = decoded.mesh
    expect(mesh).not.toBeNull()
    if (mesh === null) {
      return
    }
    const floats = [mesh.positions, mesh.normals, mesh.edgePositions, mesh.vertexPositions]
    const integers = [mesh.indices, mesh.faceIds, mesh.edgeIds, mesh.vertexIds]
    for (const view of floats) {
      expect(view).toBeInstanceOf(Float32Array)
      expect(view.buffer).toBe(frame)
    }
    for (const view of integers) {
      expect(view).toBeInstanceOf(Uint32Array)
      expect(view.buffer).toBe(frame)
    }
    expect(Array.from(mesh.positions)).toEqual(Array.from(parts.buffers[0] ?? []))
    expect(Array.from(mesh.indices)).toEqual([0, 1, 2])
    expect(Array.from(mesh.vertexIds)).toEqual([0])
    expect(mesh.faceCount).toBe(1)
    expect(mesh.edgeCount).toBe(1)
    expect(mesh.vertexCount).toBe(1)
    expect(mesh.boundingBox).toEqual(BOUNDS)
  })

  it('U-17 a buffer index out of range throws CoreFatalError', () => {
    const parts = meshParts(BOUNDS, 0)
    const { event, buffers } = received(
      bodyMesh({ ...parts.mesh, vertex_ids: 8 }),
      parts.buffers,
    )
    expect(() => decodeEvent(event, buffers)).toThrow(CoreFatalError)
  })

  it('a buffer of the wrong element type throws CoreFatalError', () => {
    const parts = meshParts(BOUNDS, 0)
    const { event, buffers } = received(
      bodyMesh({ ...parts.mesh, positions: parts.mesh.indices }),
      parts.buffers,
    )
    expect(() => decodeEvent(event, buffers)).toThrow(CoreFatalError)
  })

  it('U-17 an event of unknown type returns null', () => {
    const unknown = { type: 'future_event', doc: 1, revision: 4 } as unknown as ProtocolEvent
    expect(decodeEvent(unknown, [])).toBeNull()
  })

  it('a body_mesh without a mesh (an empty body) has mesh null', () => {
    const { event, buffers } = received(bodyMesh(null))
    expect(decodeEvent(event, buffers)).toEqual({
      type: 'body_mesh',
      documentHandle: 1,
      revision: 4,
      body: 'b-1',
      layer: 'model',
      meshId: 12,
      mesh: null,
    })
  })

  it('document_changed maps every field and turns absent options into null', () => {
    const document = emptyDocument('in')
    const { event, buffers } = received({
      type: 'document_changed',
      doc: 2,
      revision: 7,
      document,
      dirty: true,
      redo: 'Change height',
      transaction_open: true,
      transaction: 'Fillet',
      focus: 'f-fil1',
    })
    expect(decodeEvent(event, buffers)).toEqual({
      type: 'document_changed',
      documentHandle: 2,
      revision: 7,
      document,
      dirty: true,
      undo: null,
      redo: 'Change height',
      transactionOpen: true,
      transaction: 'Fillet',
      focus: 'f-fil1',
    })
  })

  it('recomputed carries features, parameters and diagnostics through', () => {
    const { event, buffers } = received({
      type: 'recomputed',
      doc: 1,
      revision: 3,
      features: [{ id: 'f-box1', status: 'ok', messages: [], repairs: [] }],
      parameters: [{ id: 'p-h', value: 50, dimension: { length: 1, angle: 0 } }],
      diagnostics: [],
    })
    expect(decodeEvent(event, buffers)).toEqual({
      type: 'recomputed',
      documentHandle: 1,
      revision: 3,
      features: [{ id: 'f-box1', status: 'ok', messages: [], repairs: [] }],
      parameters: [{ id: 'p-h', value: 50, dimension: { length: 1, angle: 0 } }],
      diagnostics: [],
    })
  })

  it('sketch_solved resolves the region fill buffers, or gives display null without them', () => {
    const fills: BufferView[] = [
      Float32Array.of(0, 0, 40, 0, 40, 20),
      Uint32Array.of(0, 1, 2),
      Uint32Array.of(0, 0, 0),
    ]
    const withDisplay = received(
      {
        type: 'sketch_solved',
        doc: 1,
        revision: 2,
        solution: {
          ...RECT_SKETCH_SOLUTION,
          display: { region_positions: 0, region_indices: 1, region_ids: 2 },
        },
      },
      fills,
    )
    const decoded = decodeEvent(withDisplay.event, withDisplay.buffers)
    expect(decoded?.type).toBe('sketch_solved')
    if (decoded?.type !== 'sketch_solved') {
      return
    }
    expect(decoded.documentHandle).toBe(1)
    expect(decoded.solution.feature).toBe('f-sk1')
    expect(decoded.display?.regionPositions).toBeInstanceOf(Float32Array)
    expect(decoded.display?.regionPositions.buffer).toBe(withDisplay.frame)
    expect(Array.from(decoded.display?.regionIndices ?? [])).toEqual([0, 1, 2])
    expect(decoded.display?.regionIds).toBeInstanceOf(Uint32Array)

    const withoutDisplay = received({
      type: 'sketch_solved',
      doc: 1,
      revision: 2,
      solution: RECT_SKETCH_SOLUTION,
    })
    const plain = decodeEvent(withoutDisplay.event, withoutDisplay.buffers)
    expect(plain?.type === 'sketch_solved' ? plain.display : 'not sketch_solved').toBeNull()
  })

  it('datum_plane gives the placement, or null when the datum failed', () => {
    const placement = { origin: [0, 0, 10], x_axis: [1, 0, 0], normal: [0, 0, 1] }
    const solved = received({ type: 'datum_plane', doc: 1, revision: 2, feature: 'f-dp1', placement: { origin: [0, 0, 10], x_axis: [1, 0, 0], normal: [0, 0, 1] } })
    expect(decodeEvent(solved.event, solved.buffers)).toEqual({
      type: 'datum_plane',
      documentHandle: 1,
      revision: 2,
      feature: 'f-dp1',
      placement,
    })
    const failed = received({ type: 'datum_plane', doc: 1, revision: 2, feature: 'f-dp1' })
    expect(decodeEvent(failed.event, failed.buffers)).toMatchObject({ placement: null })
  })

  it('print_health maps each body with an overhang buffer to its Float32Array', () => {
    const overhang = Float32Array.of(0, 0.9, 1.2)
    const { event, buffers, frame } = received(
      {
        type: 'print_health',
        doc: 1,
        revision: 5,
        report: {
          profile: 'generic-fdm',
          orientation: { kind: 'as_modeled' },
          summary: { health: 'ok', errors: 0, warnings: 0, infos: 0, label: 'Ready to print' },
          bodies: [
            {
              body: 'b-1',
              transform: [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 128, 128, 0, 1],
              mesh_id: 3,
              overhang: 0,
              report: {
                profile: 'generic-fdm',
                rotation: { rows: [[1, 0, 0], [0, 1, 0], [0, 0, 1]] },
                health: 'ok',
                checks: [],
              },
            },
            {
              body: 'b-2',
              transform: [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1],
              report: {
                profile: 'generic-fdm',
                rotation: { rows: [[1, 0, 0], [0, 1, 0], [0, 0, 1]] },
                health: 'ok',
                checks: [],
              },
            },
          ],
        },
      },
      [overhang],
    )
    const decoded = decodeEvent(event, buffers)
    expect(decoded?.type).toBe('print_health')
    if (decoded?.type !== 'print_health') {
      return
    }
    expect(decoded.documentHandle).toBe(1)
    expect(decoded.revision).toBe(5)
    expect(decoded.report.profile).toBe('generic-fdm')
    expect(Object.keys(decoded.overhang)).toEqual(['b-1'])
    expect(decoded.overhang['b-1']).toBeInstanceOf(Float32Array)
    expect(decoded.overhang['b-1']?.buffer).toBe(frame)
    expect(Array.from(decoded.overhang['b-1'] ?? [])).toEqual(Array.from(overhang))
  })
})
