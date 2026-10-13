import type { OriginPlane } from '../core/labels'
import { MILLIMETERS_PER_UNIT } from '../expression/format'
import type { Units } from '../generated/protocol'
import type { SectionState } from '../state/store'
import { boundsCenter, dot, scale, type Bounds, type Vector3 } from './math'
import { ORIGIN_PLANE_PLACEMENTS, ORIGIN_PLANES } from './planes'

/** The origin plane a section cuts along (3.15). */
export type SectionPlane = OriginPlane

export const SECTION_PLANES: readonly SectionPlane[] = ORIGIN_PLANES

/** The unflipped plane normal, the origin plane's normal: XY +Z, XZ -Y, YZ +X (3.15). */
export function sectionNormal(plane: SectionPlane): Vector3 {
  return ORIGIN_PLANE_PLACEMENTS[plane].normal
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

function isUnit(word: string): word is Units {
  return Object.hasOwn(MILLIMETERS_PER_UNIT, word)
}

/**
 * The offset in mm of an offset text that is a plain number with an optional length unit word (in
 * the document unit without one), or null for any other expression, which is evaluated elsewhere.
 */
export function sectionOffsetMillimeters(text: string, units: Units): number | null {
  const match = /^\s*([+-]?(?:\d+(?:\.\d*)?|\.\d+))\s*(mm|cm|m|in)?\s*$/.exec(text)
  if (match === null) {
    return null
  }
  const [, number = '', word = ''] = match
  return Number(number) * MILLIMETERS_PER_UNIT[isUnit(word) ? word : units]
}

/** The clip of a section at an offset in mm (`sectionClip`) with every -0 written 0, as a frame carries it. */
export function sectionFrameClip(
  section: SectionState,
  millimeters: number,
): { readonly normal: Vector3; readonly offset: number } {
  const clip = sectionClip(section.plane, millimeters, section.flipped)
  return {
    normal: [clip.normal[0] + 0, clip.normal[1] + 0, clip.normal[2] + 0],
    offset: clip.offset + 0,
  }
}
