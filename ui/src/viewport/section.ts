import { boundsCenter, dot, scale, type Bounds, type Vector3 } from './math'

/** The origin plane a section cuts along (3.15). */
export type SectionPlane = 'XY' | 'XZ' | 'YZ'

export const SECTION_PLANES: readonly SectionPlane[] = ['XY', 'XZ', 'YZ']

const NORMALS: Readonly<Record<SectionPlane, Vector3>> = {
  XY: [0, 0, 1],
  XZ: [0, -1, 0],
  YZ: [1, 0, 0],
}

/** The unflipped plane normal: XY +Z, XZ -Y, YZ +X (3.15). */
export function sectionNormal(plane: SectionPlane): Vector3 {
  return NORMALS[plane]
}

/**
 * The section of `RenderFrame.section`: it removes the half-space where dot(normal, p) > offset. Flip negates both the normal and the offset (3.15).
 */
export function sectionClip(
  plane: SectionPlane,
  offset: number,
  flipped: boolean,
): { readonly normal: Vector3; readonly offset: number } {
  const normal = sectionNormal(plane)
  return flipped ? { normal: scale(normal, -1), offset: -offset } : { normal, offset }
}

/** The default offset: the center of the visible bounds along the unflipped plane normal, or 0 with nothing visible. */
export function defaultSectionOffset(plane: SectionPlane, bounds: Bounds | null): number {
  if (bounds === null) {
    return 0
  }
  // Adding 0 turns -0 into 0.
  return dot(sectionNormal(plane), boundsCenter(bounds)) + 0
}
