import type { Geometry, PickSample, RenderBackend, RenderFrame } from '../../viewport/backend'

/** One recorded backend call. */
export type FakeBackendCall =
  | {
      readonly method: 'resize'
      readonly width: number
      readonly height: number
      readonly pixelRatio: number
    }
  | {
      readonly method: 'upload' | 'remove' | 'setElementStates' | 'setScalarField'
      readonly id: string
    }
  | { readonly method: 'render' }
  | { readonly method: 'pick'; readonly x: number; readonly y: number; readonly radius: number }
  | { readonly method: 'dispose' }

export type PickRequest = {
  readonly frame: RenderFrame
  readonly x: number
  readonly y: number
  readonly radius: number
}

export type PickHandler = (request: PickRequest) => readonly PickSample[]

function answerNothing(): readonly PickSample[] {
  return []
}

/**
 * A `RenderBackend` for jsdom tests. It records every call, keeps uploaded geometry by reference (meshes are never copied), and throws on calls that break the buffer layout of `Geometry`, so scene and controller bugs surface in tests.
 */
export type FakeBackend = RenderBackend & {
  /** Every call so far, in call order. */
  readonly calls: readonly FakeBackendCall[]
  /** Geometry by id as uploaded; dropped by `remove`, context loss and `dispose`. */
  readonly geometries: ReadonlyMap<string, Geometry>
  /** Copies of the last element states by id; dropped when the geometry is uploaded again or removed. */
  readonly elementStates: ReadonlyMap<string, Uint8Array>
  /** Copies of the last scalar field by id; dropped when the geometry is uploaded again or removed. */
  readonly scalarFields: ReadonlyMap<string, Float32Array>
  /** Every rendered frame, oldest first. */
  readonly frames: readonly RenderFrame[]
  /** The last `resize` arguments, or null before the first. */
  readonly size: {
    readonly width: number
    readonly height: number
    readonly pixelRatio: number
  } | null
  readonly contextLost: boolean
  readonly disposed: boolean
  lastFrame(): RenderFrame | null
  /** The id under which a geometry was uploaded, found by its positions buffer (meshes are never copied); throws without one. */
  idOf(positions: Float32Array): string
  /** Answers later `pick` calls; the default answers nothing. */
  setPickHandler(handler: PickHandler): void
  /** Simulates `webglcontextlost`: drops every GPU resource and notifies the lost listeners. */
  loseContext(): void
  /** Simulates `webglcontextrestored`: notifies the restored listeners, which must upload everything again. */
  restoreContext(): void
  /** Forgets the recorded calls and frames, keeping the uploaded state. */
  clearCalls(): void
}

/** The number of vertices of a geometry: `positions` holds 3 floats per vertex. */
function vertexCountOf(geometry: Geometry): number {
  return geometry.positions.length / 3
}

function indexProblem(indices: Uint32Array, vertexCount: number): string | null {
  if (indices.length % 3 !== 0) {
    return `indices has ${String(indices.length)} values, not 3 per triangle`
  }
  const outOfRange = indices.findIndex((index) => index >= vertexCount)
  if (outOfRange !== -1) {
    return `index ${String(indices[outOfRange])} at ${String(outOfRange)} is not below the vertex count ${String(vertexCount)}`
  }
  return null
}

function elementIdProblem(geometry: Geometry, expectedLength: number, per: string): string | null {
  if (geometry.elementIds.length !== expectedLength) {
    return `elementIds has ${String(geometry.elementIds.length)} values, not one per ${per} (${String(expectedLength)})`
  }
  const outOfRange = geometry.elementIds.findIndex((id) => id >= geometry.elementCount)
  if (outOfRange !== -1) {
    return `element id ${String(geometry.elementIds[outOfRange])} at ${String(outOfRange)} is not below elementCount ${String(geometry.elementCount)}`
  }
  return null
}

/**
 * Why a geometry breaks the buffer layout, or null when it is valid. Triangles and fills carry one element id per vertex (protocol.md 3.4 `face_ids`), lines carry two points (6 floats) and one element id per segment (`edge_positions`, `edge_ids`), points one element id per point (`vertex_ids`).
 */
export function geometryLayoutProblem(geometry: Geometry): string | null {
  if (!Number.isInteger(geometry.elementCount) || geometry.elementCount < 0) {
    return `elementCount ${String(geometry.elementCount)} is not a non-negative integer`
  }
  if (geometry.positions.length % 3 !== 0) {
    return `positions has ${String(geometry.positions.length)} values, not 3 per vertex`
  }
  const vertexCount = vertexCountOf(geometry)
  if (geometry.kind === 'triangles') {
    if (geometry.normals.length !== geometry.positions.length) {
      return `normals has ${String(geometry.normals.length)} values, positions ${String(geometry.positions.length)}`
    }
    return (
      indexProblem(geometry.indices, vertexCount) ??
      elementIdProblem(geometry, vertexCount, 'vertex')
    )
  }
  if (geometry.kind === 'fill') {
    return (
      indexProblem(geometry.indices, vertexCount) ??
      elementIdProblem(geometry, vertexCount, 'vertex')
    )
  }
  if (geometry.kind === 'lines') {
    if (geometry.positions.length % 6 !== 0) {
      return `positions has ${String(geometry.positions.length)} values, not 2 points per segment`
    }
    return elementIdProblem(geometry, geometry.positions.length / 6, 'segment')
  }
  return elementIdProblem(geometry, vertexCount, 'point')
}

/** Why a frame cannot be drawn from the uploaded geometry, or null when it can. */
function frameProblem(
  frame: RenderFrame,
  geometries: ReadonlyMap<string, Geometry>,
): string | null {
  if (frame.view.length !== 16 || frame.projection.length !== 16) {
    return 'view and projection must be 4 x 4 matrices'
  }
  const ranges: { readonly id: string; readonly first: number; readonly end: number }[] = []
  for (const draw of frame.draws) {
    const geometry = geometries.get(draw.id)
    if (!geometry) {
      return `draw "${draw.id}" has no uploaded geometry`
    }
    if (draw.elementColors !== null && draw.elementColors.length !== 3 * geometry.elementCount) {
      return `draw "${draw.id}" has ${String(draw.elementColors.length)} element color values, not 3 per element`
    }
    if (draw.pickBase === null || geometry.elementCount === 0) {
      continue
    }
    if (!Number.isInteger(draw.pickBase) || draw.pickBase < 1) {
      return `draw "${draw.id}" has pickBase ${String(draw.pickBase)}; pick ids start at 1`
    }
    const end = draw.pickBase + geometry.elementCount
    if (end - 1 > 0xffffffff) {
      return `draw "${draw.id}" has pick ids beyond 32 bits`
    }
    const pickBase = draw.pickBase
    const overlapping = ranges.find(
      (range) =>
        !(range.id === draw.id && range.first === pickBase) &&
        pickBase < range.end &&
        range.first < end,
    )
    if (overlapping) {
      return `draws "${overlapping.id}" and "${draw.id}" have overlapping pick ids`
    }
    ranges.push({ id: draw.id, first: pickBase, end })
  }
  return null
}

export function createFakeBackend(): FakeBackend {
  const calls: FakeBackendCall[] = []
  const frames: RenderFrame[] = []
  const geometries = new Map<string, Geometry>()
  const elementStates = new Map<string, Uint8Array>()
  const scalarFields = new Map<string, Float32Array>()
  const lostListeners = new Set<() => void>()
  const restoredListeners = new Set<() => void>()
  let size: FakeBackend['size'] = null
  let pickHandler: PickHandler = answerNothing
  let contextLost = false
  let disposed = false

  function assertUsable(method: string): void {
    if (disposed) {
      throw new Error(`FakeBackend.${method} called after dispose`)
    }
  }

  function uploadedGeometry(method: string, id: string): Geometry {
    const geometry = geometries.get(id)
    if (!geometry) {
      throw new Error(`FakeBackend.${method}("${id}"): no geometry uploaded under this id`)
    }
    return geometry
  }

  function dropResources(): void {
    geometries.clear()
    elementStates.clear()
    scalarFields.clear()
  }

  return {
    kind: 'webgl2',
    get calls() {
      return calls
    },
    get geometries() {
      return geometries
    },
    get elementStates() {
      return elementStates
    },
    get scalarFields() {
      return scalarFields
    },
    get frames() {
      return frames
    },
    get size() {
      return size
    },
    get contextLost() {
      return contextLost
    },
    get disposed() {
      return disposed
    },
    resize(width, height, pixelRatio) {
      assertUsable('resize')
      calls.push({ method: 'resize', width, height, pixelRatio })
      size = { width, height, pixelRatio }
    },
    upload(id, geometry) {
      assertUsable('upload')
      calls.push({ method: 'upload', id })
      const problem = geometryLayoutProblem(geometry)
      if (problem !== null) {
        throw new Error(`FakeBackend.upload("${id}"): ${problem}`)
      }
      // A lost context ignores GPU calls, as WebGL does.
      if (contextLost) {
        return
      }
      geometries.set(id, geometry)
      elementStates.delete(id)
      scalarFields.delete(id)
    },
    remove(id) {
      assertUsable('remove')
      calls.push({ method: 'remove', id })
      geometries.delete(id)
      elementStates.delete(id)
      scalarFields.delete(id)
    },
    setElementStates(id, states) {
      assertUsable('setElementStates')
      calls.push({ method: 'setElementStates', id })
      if (contextLost) {
        return
      }
      const geometry = uploadedGeometry('setElementStates', id)
      if (states.length !== geometry.elementCount) {
        throw new Error(
          `FakeBackend.setElementStates("${id}"): ${String(states.length)} states for ${String(geometry.elementCount)} elements`,
        )
      }
      elementStates.set(id, states.slice())
    },
    setScalarField(id, values) {
      assertUsable('setScalarField')
      calls.push({ method: 'setScalarField', id })
      if (contextLost) {
        return
      }
      const geometry = uploadedGeometry('setScalarField', id)
      if (values === null) {
        scalarFields.delete(id)
        return
      }
      if (values.length !== vertexCountOf(geometry)) {
        throw new Error(
          `FakeBackend.setScalarField("${id}"): ${String(values.length)} values for ${String(vertexCountOf(geometry))} vertices`,
        )
      }
      scalarFields.set(id, values.slice())
    },
    render(frame) {
      assertUsable('render')
      calls.push({ method: 'render' })
      frames.push(frame)
      if (contextLost) {
        return
      }
      const problem = frameProblem(frame, geometries)
      if (problem !== null) {
        throw new Error(`FakeBackend.render: ${problem}`)
      }
    },
    pick(frame, x, y, radius) {
      assertUsable('pick')
      calls.push({ method: 'pick', x, y, radius })
      if (contextLost) {
        return []
      }
      return pickHandler({ frame, x, y, radius })
    },
    onContextLost(listener) {
      lostListeners.add(listener)
      return () => {
        lostListeners.delete(listener)
      }
    },
    onContextRestored(listener) {
      restoredListeners.add(listener)
      return () => {
        restoredListeners.delete(listener)
      }
    },
    dispose() {
      assertUsable('dispose')
      calls.push({ method: 'dispose' })
      disposed = true
      dropResources()
      lostListeners.clear()
      restoredListeners.clear()
    },
    lastFrame() {
      return frames.at(-1) ?? null
    },
    idOf(positions) {
      const found = [...geometries].find(([, geometry]) => geometry.positions === positions)
      if (found === undefined) {
        throw new Error('No geometry was uploaded with these positions.')
      }
      return found[0]
    },
    setPickHandler(handler) {
      pickHandler = handler
    },
    loseContext() {
      contextLost = true
      dropResources()
      for (const listener of lostListeners) {
        listener()
      }
    },
    restoreContext() {
      contextLost = false
      for (const listener of restoredListeners) {
        listener()
      }
    },
    clearCalls() {
      calls.length = 0
      frames.length = 0
    },
  }
}
