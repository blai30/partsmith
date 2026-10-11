import type { Vector3 } from './math'

export { createWebGl2Backend } from './webgl2/backend'

/** Linear RGB, each 0 to 1: the space shading is computed in. */
export type Rgb = readonly [number, number, number]
/** sRGB-encoded RGB, each 0 to 1: a '#rrggbb' value divided by 255. */
export type EncodedRgb = readonly [number, number, number]

/**
 * Display geometry in the protocol's buffer layout (protocol.md 3.4): triangles and fills carry one element id per vertex, lines two points (6 floats) and one element id per segment, points one element id per point. Element ids are below `elementCount`.
 */
export type Geometry =
  | {
      readonly kind: 'triangles'
      readonly positions: Float32Array
      readonly normals: Float32Array
      readonly indices: Uint32Array
      readonly elementIds: Uint32Array
      readonly elementCount: number
    }
  | {
      readonly kind: 'lines'
      readonly positions: Float32Array
      readonly elementIds: Uint32Array
      readonly elementCount: number
    }
  | {
      readonly kind: 'points'
      readonly positions: Float32Array
      readonly elementIds: Uint32Array
      readonly elementCount: number
    }
  | {
      readonly kind: 'fill'
      readonly positions: Float32Array
      readonly indices: Uint32Array
      readonly elementIds: Uint32Array
      readonly elementCount: number
    }

export type DrawCall = {
  readonly id: string
  readonly layer: 'opaque' | 'translucent' | 'overlay'
  readonly color: Rgb
  /** Optional per-element color, 3 floats per element (sketch DOF colors). */
  readonly elementColors: Float32Array | null
  readonly opacity: number
  /** Lines: width in CSS pixels; points: size in CSS pixels. */
  readonly size: number
  /** Lines only: dashed (construction geometry). */
  readonly dashed: boolean
  readonly shading: 'matcap' | 'flat'
  readonly depthTest: boolean
  readonly clipped: boolean
  /** First pick id of this draw, or null when not pickable. Element i has pick id pickBase + i. */
  readonly pickBase: number | null
  /** Pick footprint in CSS pixels for lines and points in the ID pass. */
  readonly pickSize: number
  /** Whether this draw colors its scalar field with RenderFrame.analysis. */
  readonly analysis: boolean
}

export type RenderFrame = {
  readonly view: Float32Array
  readonly projection: Float32Array
  readonly eye: Vector3
  readonly orthographic: boolean
  readonly pixelRatio: number
  readonly uiScale: number
  readonly theme: ViewportTheme
  readonly draws: readonly DrawCall[]
  readonly section: { readonly normal: Vector3; readonly offset: number } | null
  /** The analysis ramp ends are sRGB-encoded because the ramp is interpolated before linearizing (3.12 pass 3). */
  readonly analysis: {
    readonly low: EncodedRgb
    readonly high: EncodedRgb
    readonly threshold: number
    readonly maximum: number
  } | null
}

/** The theme colors the backend itself uses, linearized from the viewport roles of 2.16.4 by `viewportTheme`. */
export type ViewportTheme = {
  readonly backgroundTop: Rgb
  readonly backgroundBottom: Rgb
  /** Hover and selection tint. */
  readonly highlight: Rgb
  readonly problem: Rgb
  readonly edgeDark: Rgb
  readonly edgeLight: Rgb
}

export type PickSample = {
  readonly pickId: number
  /** Distance from the queried point in CSS pixels. */
  readonly distance: number
  /** View-space distance in mm when the backend can read it, else null. */
  readonly depth: number | null
}

/** Bits of `RenderBackend.setElementStates`. */
export const ELEMENT_STATE = { hover: 1, selected: 2, problem: 4 } as const

/** Renders display geometry and answers picks (ADR 0007). A WebGPU backend will implement the same interface. */
export type RenderBackend = {
  readonly kind: 'webgl2'
  /** Sets the canvas size in CSS pixels and the device pixel ratio. */
  resize(width: number, height: number, pixelRatio: number): void
  /** Uploads geometry under an id, replacing earlier geometry with that id. */
  upload(id: string, geometry: Geometry): void
  remove(id: string): void
  /** One byte per element: bit 0 hover, bit 1 selected, bit 2 problem. */
  setElementStates(id: string, states: Uint8Array): void
  /** One float per vertex, or null to clear. */
  setScalarField(id: string, values: Float32Array | null): void
  render(frame: RenderFrame): void
  /** Renders the ID pass if stale and reads every pick id within `radius` CSS pixels of the point (x, y) in CSS pixels. */
  pick(frame: RenderFrame, x: number, y: number, radius: number): readonly PickSample[]
  onContextLost(listener: () => void): () => void
  onContextRestored(listener: () => void): () => void
  dispose(): void
}
