import { describe, expect, it } from 'vitest'
import {
  defaultSectionOffset,
  sectionClip,
  sectionFrameClip,
  sectionNormal,
  sectionOffsetMillimeters,
  SECTION_PLANES,
} from './section'

const CUBE = { min: [-10, -10, 0], max: [10, 10, 20] } as const

describe('sectionNormal', () => {
  it('is +Z for XY, -Y for XZ and +X for YZ', () => {
    expect(SECTION_PLANES.map(sectionNormal)).toEqual([
      [0, 0, 1],
      [0, -1, 0],
      [1, 0, 0],
    ])
  })
})

describe('sectionClip', () => {
  it('keeps the normal and offset unflipped', () => {
    expect(sectionClip('XY', 10, false)).toEqual({ normal: [0, 0, 1], offset: 10 })
  })

  it('negates both the normal and the offset when flipped', () => {
    expect(sectionClip('XZ', 4, true)).toEqual({ normal: [-0, 1, -0], offset: -4 })
  })

  it('removes the same points as the unflipped section keeps', () => {
    const point = [0, 0, 15] as const
    const removed = (clip: ReturnType<typeof sectionClip>) =>
      clip.normal[0] * point[0] + clip.normal[1] * point[1] + clip.normal[2] * point[2] >
      clip.offset
    expect(removed(sectionClip('XY', 10, false))).toBe(true)
    expect(removed(sectionClip('XY', 10, true))).toBe(false)
  })
})

describe('defaultSectionOffset', () => {
  it('is the center of the visible bounds along the plane normal', () => {
    expect(defaultSectionOffset('XY', CUBE)).toBe(10)
    expect(defaultSectionOffset('YZ', { min: [10, 0, 0], max: [30, 1, 1] })).toBe(20)
  })

  it('follows the -Y normal of XZ and writes -0 as 0', () => {
    expect(defaultSectionOffset('XZ', { min: [0, 2, 0], max: [1, 6, 1] })).toBe(-4)
    expect(Object.is(defaultSectionOffset('XZ', CUBE), 0)).toBe(true)
  })

  it('is 0 with nothing visible', () => {
    expect(defaultSectionOffset('XY', null)).toBe(0)
  })
})

describe('sectionOffsetMillimeters', () => {
  it('reads a plain number in the document unit', () => {
    expect(sectionOffsetMillimeters(' 2.5 ', 'cm')).toBe(25)
    expect(sectionOffsetMillimeters('-.5', 'mm')).toBe(-0.5)
  })

  it('reads a length unit word over the document unit', () => {
    expect(sectionOffsetMillimeters('2 in', 'mm')).toBe(50.8)
    expect(sectionOffsetMillimeters('3m', 'in')).toBe(3000)
  })

  it('is null for any other expression', () => {
    expect(sectionOffsetMillimeters('width / 2', 'mm')).toBeNull()
    expect(sectionOffsetMillimeters('2 deg', 'mm')).toBeNull()
    expect(sectionOffsetMillimeters('', 'mm')).toBeNull()
  })
})

describe('sectionFrameClip', () => {
  it('is the section clip at the offset', () => {
    expect(sectionFrameClip({ plane: 'YZ', offset: '4', flipped: true }, 4)).toEqual({
      normal: [-1, 0, 0],
      offset: -4,
    })
  })

  it('writes every -0 as 0', () => {
    const clip = sectionFrameClip({ plane: 'XY', offset: '0', flipped: true }, 0)
    expect(clip.normal.map((component) => Object.is(component, -0))).toEqual([false, false, false])
    expect(Object.is(clip.offset, -0)).toBe(false)
  })
})
