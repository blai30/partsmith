import type {
  BoundingBox,
  Diagnostic,
  DocHandle,
  Document,
  Event,
  FeatureStatus,
  MeshBuffers,
  ParameterValue,
  Placement,
  PrintReport,
  SketchDisplay,
  SketchSolution,
} from '../generated/protocol'
import type { BufferView } from './frame'
import { CoreFatalError } from './transport'

// Protocol events as the UI uses them (ui.md 2.7). This is the only module that names events. Buffer
// indices become typed array views into the frame (no copies).

export type CoreEvent =
  | {
      readonly type: 'document_changed'
      readonly documentHandle: DocHandle
      readonly revision: number
      readonly document: Document
      readonly dirty: boolean
      readonly undo: string | null
      readonly redo: string | null
      readonly transactionOpen: boolean
      readonly transaction: string | null
      readonly focus: string | null
    }
  | {
      readonly type: 'recomputed'
      readonly documentHandle: DocHandle
      readonly revision: number
      readonly features: readonly FeatureStatus[]
      readonly parameters: readonly ParameterValue[]
      readonly diagnostics: readonly Diagnostic[]
    }
  | {
      readonly type: 'sketch_solved'
      readonly documentHandle: DocHandle
      readonly revision: number
      readonly solution: SketchSolution
      readonly display: SketchDisplayData | null
    }
  | {
      readonly type: 'datum_plane'
      readonly documentHandle: DocHandle
      readonly revision: number
      readonly feature: string
      readonly placement: Placement | null
    }
  | {
      readonly type: 'body_mesh'
      readonly documentHandle: DocHandle
      readonly revision: number
      readonly body: string
      readonly layer: 'model' | 'preview'
      readonly meshId: number
      readonly mesh: BodyMeshData | null
    }
  | {
      readonly type: 'print_health'
      readonly documentHandle: DocHandle
      readonly revision: number
      readonly report: PrintReport
      readonly overhang: Readonly<Record<string, Float32Array>>
    }

/** Mesh buffers of one body in one layer (protocol.md MeshBuffers, 3.4 invariants). */
export type BodyMeshData = {
  readonly positions: Float32Array
  readonly normals: Float32Array
  readonly indices: Uint32Array
  readonly faceIds: Uint32Array
  readonly edgePositions: Float32Array
  readonly edgeIds: Uint32Array
  readonly vertexPositions: Float32Array
  /** Display vertex id of each vertex marker (the `index` of a vertex pick). */
  readonly vertexIds: Uint32Array
  readonly faceCount: number
  readonly edgeCount: number
  readonly vertexCount: number
  readonly boundingBox: BoundingBox
}

/** Region fill buffers of a sketch solution (protocol.md SketchDisplay). */
export type SketchDisplayData = {
  readonly regionPositions: Float32Array
  readonly regionIndices: Uint32Array
  readonly regionIds: Uint32Array
}

function bufferAt(buffers: readonly BufferView[], index: number): BufferView {
  const buffer = buffers[index]
  if (buffer === undefined) {
    throw new CoreFatalError(
      `The geometry core referred to buffer ${String(index)}, but the frame has ${String(buffers.length)} buffers.`,
    )
  }
  return buffer
}

/** The `f32` buffer at `index`; throws CoreFatalError when it is missing or of another type. */
export function floatBuffer(buffers: readonly BufferView[], index: number): Float32Array {
  const buffer = bufferAt(buffers, index)
  if (!(buffer instanceof Float32Array)) {
    throw new CoreFatalError(`The geometry core sent buffer ${String(index)} with the wrong type.`)
  }
  return buffer
}

/** The `u32` buffer at `index`; throws CoreFatalError when it is missing or of another type. */
export function integerBuffer(buffers: readonly BufferView[], index: number): Uint32Array {
  const buffer = bufferAt(buffers, index)
  if (!(buffer instanceof Uint32Array)) {
    throw new CoreFatalError(`The geometry core sent buffer ${String(index)} with the wrong type.`)
  }
  return buffer
}

/** The `u8` buffer at `index`; throws CoreFatalError when it is missing or of another type. */
export function byteBuffer(buffers: readonly BufferView[], index: number): Uint8Array {
  const buffer = bufferAt(buffers, index)
  if (!(buffer instanceof Uint8Array)) {
    throw new CoreFatalError(`The geometry core sent buffer ${String(index)} with the wrong type.`)
  }
  return buffer
}

/** The region fill buffers a solution's `display` names, or null without them. */
export function sketchDisplayData(
  display: SketchDisplay | null | undefined,
  buffers: readonly BufferView[],
): SketchDisplayData | null {
  if (display === null || display === undefined) {
    return null
  }
  return {
    regionPositions: floatBuffer(buffers, display.region_positions),
    regionIndices: integerBuffer(buffers, display.region_indices),
    regionIds: integerBuffer(buffers, display.region_ids),
  }
}

function meshData(mesh: MeshBuffers, buffers: readonly BufferView[]): BodyMeshData {
  return {
    positions: floatBuffer(buffers, mesh.positions),
    normals: floatBuffer(buffers, mesh.normals),
    indices: integerBuffer(buffers, mesh.indices),
    faceIds: integerBuffer(buffers, mesh.face_ids),
    edgePositions: floatBuffer(buffers, mesh.edge_positions),
    edgeIds: integerBuffer(buffers, mesh.edge_ids),
    vertexPositions: floatBuffer(buffers, mesh.vertex_positions),
    vertexIds: integerBuffer(buffers, mesh.vertex_ids),
    faceCount: mesh.face_count,
    edgeCount: mesh.edge_count,
    vertexCount: mesh.vertex_count,
    boundingBox: mesh.bounding_box,
  }
}

function overhangMaps(
  report: PrintReport,
  buffers: readonly BufferView[],
): Readonly<Record<string, Float32Array>> {
  const maps: Record<string, Float32Array> = {}
  for (const body of report.bodies) {
    const index = body.overhang ?? null
    if (index !== null) {
      maps[body.body] = floatBuffer(buffers, index)
    }
  }
  return maps
}

/** The UI form of a protocol event, or null for an event type this UI does not know. */
export function decodeEvent(event: Event, buffers: readonly BufferView[]): CoreEvent | null {
  switch (event.type) {
    case 'document_changed':
      return {
        type: 'document_changed',
        documentHandle: event.doc,
        revision: event.revision,
        document: event.document,
        dirty: event.dirty,
        undo: event.undo ?? null,
        redo: event.redo ?? null,
        transactionOpen: event.transaction_open,
        transaction: event.transaction ?? null,
        focus: event.focus ?? null,
      }
    case 'recomputed':
      return {
        type: 'recomputed',
        documentHandle: event.doc,
        revision: event.revision,
        features: event.features,
        parameters: event.parameters,
        diagnostics: event.diagnostics,
      }
    case 'sketch_solved':
      return {
        type: 'sketch_solved',
        documentHandle: event.doc,
        revision: event.revision,
        solution: event.solution,
        display: sketchDisplayData(event.solution.display, buffers),
      }
    case 'datum_plane':
      return {
        type: 'datum_plane',
        documentHandle: event.doc,
        revision: event.revision,
        feature: event.feature,
        placement: event.placement ?? null,
      }
    case 'body_mesh':
      return {
        type: 'body_mesh',
        documentHandle: event.doc,
        revision: event.revision,
        body: event.body,
        layer: event.layer,
        meshId: event.mesh_id,
        mesh: event.mesh === null || event.mesh === undefined ? null : meshData(event.mesh, buffers),
      }
    case 'print_health':
      return {
        type: 'print_health',
        documentHandle: event.doc,
        revision: event.revision,
        report: event.report,
        overhang: overhangMaps(event.report, buffers),
      }
    default:
      // Clients ignore events of unknown types (protocol.md 3.6).
      return null
  }
}
