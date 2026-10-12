/// <reference lib="dom" />
// What e2e/pages/viewport.html puts on window for viewport.spec.ts: the real WebGL2 backend on the page's canvas, with helpers to build frames and read pixels.
import {
  createWebGl2Backend,
  type DrawCall,
  type Geometry,
  type PickSample,
  type RenderBackend,
  type RenderFrame,
  type ViewportTheme,
} from '../../src/viewport/backend'
import {
  DEFAULT_CAMERA,
  depthRange,
  eye,
  projectionMatrix,
  standardViewRotation,
  viewMatrix,
  type Camera,
  type StandardView,
  type Vector3,
} from '../../src/viewport/camera'

/** Linear colors: a white background and a saturated highlight, so blends are easy to read back. */
export const THEME: ViewportTheme = {
  backgroundTop: [1, 1, 1],
  backgroundBottom: [1, 1, 1],
  highlight: [1, 0, 0],
  problem: [0.53, 0, 0],
  edgeDark: [0.01, 0.01, 0.01],
  edgeLight: [0.78, 0.78, 0.8],
}

/** A draw with the defaults of an opaque, not pickable body; `overrides` change any field. */
export function drawCall(id: string, overrides: Partial<DrawCall> = {}): DrawCall {
  return {
    id,
    layer: 'opaque',
    color: [0.35, 0.35, 0.38],
    elementColors: null,
    opacity: 1,
    size: 1,
    dashed: false,
    shading: 'matcap',
    depthTest: true,
    clipped: false,
    pickBase: null,
    pickSize: 0,
    analysis: false,
    ...overrides,
  }
}

/** A cube of side `2 × half` centered on the origin, one element per face in the order +X, -X, +Y, -Y, +Z, -Z. */
export function cubeGeometry(half: number): Geometry {
  // Per face a normal and two in-face axes whose cross product is the normal, so corners run counterclockwise seen from outside.
  const faces: readonly (readonly [Vector3, Vector3, Vector3])[] = [
    [
      [1, 0, 0],
      [0, 1, 0],
      [0, 0, 1],
    ],
    [
      [-1, 0, 0],
      [0, -1, 0],
      [0, 0, 1],
    ],
    [
      [0, 1, 0],
      [-1, 0, 0],
      [0, 0, 1],
    ],
    [
      [0, -1, 0],
      [1, 0, 0],
      [0, 0, 1],
    ],
    [
      [0, 0, 1],
      [1, 0, 0],
      [0, 1, 0],
    ],
    [
      [0, 0, -1],
      [1, 0, 0],
      [0, -1, 0],
    ],
  ]
  const corners: readonly (readonly [number, number])[] = [
    [-1, -1],
    [1, -1],
    [1, 1],
    [-1, 1],
  ]
  const positions: number[] = []
  const normals: number[] = []
  const indices: number[] = []
  const elementIds: number[] = []
  faces.forEach(([normal, tangent, bitangent], face) => {
    const first = positions.length / 3
    for (const [alongTangent, alongBitangent] of corners) {
      for (const axis of [0, 1, 2] as const) {
        positions.push(
          half * (normal[axis] + alongTangent * tangent[axis] + alongBitangent * bitangent[axis]),
        )
      }
      normals.push(...normal)
      elementIds.push(face)
    }
    indices.push(first, first + 1, first + 2, first, first + 2, first + 3)
  })
  return {
    kind: 'triangles',
    positions: new Float32Array(positions),
    normals: new Float32Array(normals),
    indices: new Uint32Array(indices),
    elementIds: new Uint32Array(elementIds),
    elementCount: faces.length,
  }
}

/** A square of side `2 × half` in the plane z = `height`, facing +Z, as one element. */
export function squareGeometry(half: number, height: number): Geometry {
  return {
    kind: 'triangles',
    positions: new Float32Array([
      -half,
      -half,
      height,
      half,
      -half,
      height,
      half,
      half,
      height,
      -half,
      half,
      height,
    ]),
    normals: new Float32Array([0, 0, 1, 0, 0, 1, 0, 0, 1, 0, 0, 1]),
    indices: new Uint32Array([0, 1, 2, 0, 2, 3]),
    elementIds: new Uint32Array([0, 0, 0, 0]),
    elementCount: 1,
  }
}

/** One line segment as one element. */
export function segmentGeometry(start: Vector3, end: Vector3): Geometry {
  return {
    kind: 'lines',
    positions: new Float32Array([...start, ...end]),
    elementIds: new Uint32Array([0]),
    elementCount: 1,
  }
}

/** The default camera turned to a standard view, with `overrides`. */
export function cameraFor(view: StandardView, overrides: Partial<Camera> = {}): Camera {
  return { ...DEFAULT_CAMERA, rotation: standardViewRotation(view), ...overrides }
}

/** A plain copy of a pick sample for the trip back to the test. */
export type Sample = {
  readonly pickId: number
  readonly distance: number
  readonly depth: number | null
}

export type Harness = {
  readonly backend: RenderBackend
  readonly gl: WebGL2RenderingContext
  /** The canvas size in device pixels. */
  readonly drawingBuffer: { readonly width: number; readonly height: number }
  /** A frame of `draws` seen by `camera`, with depths covering a sphere of `sceneRadius` around the origin. */
  frame(camera: Camera, draws: readonly DrawCall[], sceneRadius: number): RenderFrame
  /** Renders `frame` and returns the canvas pixels as RGBA rows from the top, read in the task that rendered them. */
  renderPixels(frame: RenderFrame): Uint8ClampedArray
  /** Picks at (x, y) in CSS pixels. */
  pickAt(frame: RenderFrame, x: number, y: number, radius: number): Sample[]
  /** The pending WebGL errors, oldest first, which also clears them. */
  takeErrors(): number[]
}

/** Creates the WebGL2 backend on the page's canvas at a CSS size and pixel ratio. */
export function createHarness(width: number, height: number, pixelRatio: number): Harness {
  const canvas = document.querySelector('canvas')
  if (!canvas) {
    throw new Error('The test page has no canvas.')
  }
  const backend = createWebGl2Backend(canvas)
  // The backend created the context, so this returns the same one.
  const gl = canvas.getContext('webgl2')
  if (!backend || !gl) {
    throw new Error('createWebGl2Backend returned null although Chromium supports WebGL2.')
  }
  backend.resize(width, height, pixelRatio)
  return {
    backend,
    gl,
    drawingBuffer: { width: canvas.width, height: canvas.height },
    frame(camera, draws, sceneRadius) {
      const range = depthRange(camera, [0, 0, 0], sceneRadius)
      return {
        view: viewMatrix(camera),
        projection: projectionMatrix(camera, width / height, range.near, range.far),
        eye: eye(camera),
        orthographic: camera.projection === 'orthographic',
        pixelRatio,
        uiScale: 1,
        theme: THEME,
        draws,
        section: null,
        analysis: null,
      }
    },
    renderPixels(frame) {
      backend.render(frame)
      const probe = document.createElement('canvas')
      probe.width = canvas.width
      probe.height = canvas.height
      const context = probe.getContext('2d')
      if (!context) {
        throw new Error('The test page cannot create a 2D context.')
      }
      context.drawImage(canvas, 0, 0)
      return context.getImageData(0, 0, canvas.width, canvas.height).data
    },
    pickAt(frame, x, y, radius) {
      return backend
        .pick(frame, x, y, radius)
        .map(({ pickId, distance, depth }: PickSample) => ({ pickId, distance, depth }))
    },
    takeErrors() {
      const errors: number[] = []
      for (let error = gl.getError(); error !== gl.NO_ERROR; error = gl.getError()) {
        errors.push(error)
      }
      return errors
    },
  }
}
