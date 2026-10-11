// Buffer and texture layout shared by the WebGL2 backend and its shaders. Pure, so it is unit-tested without WebGL.

/** Attribute locations of surface (triangles, fill) and point programs, fixed so one vertex array serves both passes. */
export const SURFACE_ATTRIBUTES = { position: 0, normal: 1, elementId: 2, scalar: 3 } as const
/** Attribute locations of line programs: one instance per segment, plus the shared quad corner. */
export const LINE_ATTRIBUTES = { start: 0, end: 1, elementId: 2, startDistance: 3, corner: 4 } as const
/** Attribute locations of point programs: one instance per point, plus the shared quad corner. */
export const POINT_ATTRIBUTES = { center: 0, elementId: 2, corner: 4 } as const

/** Bytes per float and per position (3 floats). */
export const FLOAT_BYTES = 4
export const POSITION_BYTES = 3 * FLOAT_BYTES
/** Lines hold two positions per segment: start at byte 0, end at byte 12 of a 24-byte stride. */
export const SEGMENT_BYTES = 2 * POSITION_BYTES

/** Corners of a segment quad as a triangle strip: x 0 at the start and 1 at the end, y the side. */
export const LINE_CORNERS = new Float32Array([0, -1, 0, 1, 1, -1, 1, 1])
/** Corners of a point quad as a triangle strip, each coordinate -1 or 1. */
export const POINT_CORNERS = new Float32Array([-1, -1, 1, -1, -1, 1, 1, 1])

/** Element textures hold one texel per element, row by row, this many texels wide. */
export const ELEMENT_TEXTURE_WIDTH = 1024

/** Width and line widths in CSS pixels, multiplied by the UI scale and the pixel ratio when drawn (3.6, 3.12). */
export const EMPHASIS_LINE_WIDTH = 2.5
export const EMPHASIS_POINT_SIZE = 7
export const DASH_LENGTH = 6
export const DASH_GAP = 4
/** Width of the anti-aliasing smoothstep falloff in device pixels (3.12 pass 4). */
export const ANTIALIAS_WIDTH = 1

/** Texture size for one texel per element; at least one row so empty geometry still has a valid texture. */
export function elementTextureSize(elementCount: number): {
  readonly width: number
  readonly height: number
} {
  return {
    width: ELEMENT_TEXTURE_WIDTH,
    height: Math.max(1, Math.ceil(elementCount / ELEMENT_TEXTURE_WIDTH)),
  }
}

/** One byte per element padded to the element texture size (R8UI). */
export function packElementStates(states: Uint8Array, elementCount: number): Uint8Array {
  const { width, height } = elementTextureSize(elementCount)
  const packed = new Uint8Array(width * height)
  packed.set(states.subarray(0, Math.min(states.length, elementCount)))
  return packed
}

/** Three floats per element expanded to RGBA (alpha 1) and padded to the element texture size (RGBA32F). */
export function packElementColors(colors: Float32Array, elementCount: number): Float32Array {
  const { width, height } = elementTextureSize(elementCount)
  const packed = new Float32Array(width * height * 4)
  const count = Math.min(elementCount, Math.floor(colors.length / 3))
  for (let i = 0; i < count; i++) {
    packed[i * 4] = colors[i * 3] ?? 0
    packed[i * 4 + 1] = colors[i * 3 + 1] ?? 0
    packed[i * 4 + 2] = colors[i * 3 + 2] ?? 0
    packed[i * 4 + 3] = 1
  }
  return packed
}

/**
 * The distance in mm along its polyline at which each segment starts, for dash phases: a segment that starts where the previous one ended continues its polyline, any other segment starts a new one at 0.
 */
export function lineStartDistances(positions: Float32Array): Float32Array {
  const segmentCount = Math.floor(positions.length / 6)
  const distances = new Float32Array(segmentCount)
  let running = 0
  for (let i = 0; i < segmentCount; i++) {
    const offset = i * 6
    if (i > 0) {
      const previous = offset - 6
      const continues =
        positions[offset] === positions[previous + 3] &&
        positions[offset + 1] === positions[previous + 4] &&
        positions[offset + 2] === positions[previous + 5]
      running = continues
        ? running +
          Math.hypot(
            (positions[previous + 3] ?? 0) - (positions[previous] ?? 0),
            (positions[previous + 4] ?? 0) - (positions[previous + 1] ?? 0),
            (positions[previous + 5] ?? 0) - (positions[previous + 2] ?? 0),
          )
        : 0
    }
    distances[i] = running
  }
  return distances
}

/** Why a geometry's array lengths do not match its kind, or null. Element id ranges are not scanned. */
export function geometryLengthProblem(geometry: {
  readonly kind: 'triangles' | 'lines' | 'points' | 'fill'
  readonly positions: Float32Array
  readonly normals?: Float32Array
  readonly indices?: Uint32Array
  readonly elementIds: Uint32Array
}): string | null {
  const { kind, positions, elementIds } = geometry
  if (kind === 'lines') {
    return positions.length % 6 === 0 && elementIds.length === positions.length / 6
      ? null
      : 'lines need 6 position floats and one element id per segment'
  }
  if (positions.length % 3 !== 0 || elementIds.length !== positions.length / 3) {
    return `${kind} need 3 position floats and one element id per vertex`
  }
  if (kind === 'triangles' && geometry.normals?.length !== positions.length) {
    return 'triangles need one normal per vertex'
  }
  if ((kind === 'triangles' || kind === 'fill') && (geometry.indices?.length ?? 0) % 3 !== 0) {
    return `${kind} need 3 indices per triangle`
  }
  return null
}
