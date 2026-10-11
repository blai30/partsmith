import { describe, expect, it } from 'vitest'
import { encodePickId } from '../picking'
import { collectPickSamples, pickBlock, type PixelBlock } from './readback'

// The 4 little-endian bytes of a float32, as the ID pass writes view depths.
function floatBytes(value: number): number[] {
  const view = new DataView(new ArrayBuffer(4))
  view.setFloat32(0, value, true)
  return [view.getUint8(0), view.getUint8(1), view.getUint8(2), view.getUint8(3)]
}

// RGBA8 pixels as readPixels returns them: bottom row first.
function pixels(rows: readonly (readonly number[])[], bytes: (value: number) => number[]) {
  return new Uint8Array(rows.toReversed().flatMap((row) => row.flatMap(bytes)))
}

describe('pickBlock', () => {
  it('covers the radius around the point in drawing buffer pixels, bottom row first', () => {
    expect(pickBlock(10, 10, 1, 2, 100, 80)).toEqual({
      left: 18,
      bottom: 57,
      width: 5,
      height: 5,
    })
  })

  it('is the single pixel under the point for radius 0', () => {
    expect(pickBlock(10.5, 3.25, 0, 1, 100, 80)).toEqual({
      left: 10,
      bottom: 76,
      width: 1,
      height: 1,
    })
  })

  it('is clipped to the buffer', () => {
    expect(pickBlock(0, 0, 2, 1, 100, 80)).toEqual({ left: 0, bottom: 77, width: 3, height: 3 })
    expect(pickBlock(99.5, 79.5, 2, 1, 100, 80)).toEqual({
      left: 97,
      bottom: 0,
      width: 3,
      height: 3,
    })
  })

  it('is null when the point and its radius lie outside the buffer', () => {
    expect(pickBlock(-10, 10, 1, 2, 100, 80)).toBeNull()
    expect(pickBlock(10, 200, 1, 2, 100, 80)).toBeNull()
  })
})

describe('collectPickSamples', () => {
  // A 3 × 3 block whose top-left pixel is column 4, row 2 (top-down) of a buffer 10 rows high.
  const block: PixelBlock = { left: 4, bottom: 5, width: 3, height: 3 }
  // Pick ids in reading order (top row first) for the block above.
  const idsTopDown = [
    [9, 0, 0],
    [9, 7, 0],
    [0, 0, 3],
  ]
  const depthsTopDown = [
    [12, 0, 0],
    [11, 10, 0],
    [0, 0, 30],
  ]

  const ids = pixels(idsTopDown, (id) => [...encodePickId(id)])
  const depths = pixels(depthsTopDown, floatBytes)
  // The center of the block's middle pixel: column 5, row 3.
  const query = { block, bufferHeight: 10, pixelRatio: 1, x: 5.5, y: 3.5 }

  it('gives one sample per id at its nearest pixel, nearest first, with its view depth', () => {
    expect(collectPickSamples({ ...query, ids, depths, radius: 1.5 })).toEqual([
      { pickId: 7, distance: 0, depth: 10 },
      { pickId: 9, distance: 0.5, depth: 11 },
      { pickId: 3, distance: Math.hypot(0.5, 0.5), depth: 30 },
    ])
  })

  it('leaves out pixels beyond the radius', () => {
    expect(
      collectPickSamples({ ...query, ids, depths, radius: 0.6 }).map((sample) => sample.pickId),
    ).toEqual([7, 9])
  })

  it('reports null depths when the backend cannot read them', () => {
    const samples = collectPickSamples({ ...query, ids, depths: null, radius: 0 })
    expect(samples).toEqual([{ pickId: 7, distance: 0, depth: null }])
  })

  it('reports a null depth for bits that are not a finite float', () => {
    const notANumber = pixels(
      depthsTopDown.map((row) => row.map(() => Number.NaN)),
      floatBytes,
    )
    const samples = collectPickSamples({ ...query, ids, depths: notANumber, radius: 0 })
    expect(samples).toEqual([{ pickId: 7, distance: 0, depth: null }])
  })

  it('measures distances in CSS pixels at a pixel ratio of 2', () => {
    // At ratio 2 the block's middle pixel spans CSS (2.5, 1.5) to (3, 2).
    const samples = collectPickSamples({
      ...query,
      ids,
      depths: null,
      pixelRatio: 2,
      x: 2,
      y: 1.5,
      radius: 1.2,
    })
    expect(samples).toEqual([
      { pickId: 9, distance: 0, depth: null },
      { pickId: 7, distance: 0.5, depth: null },
      { pickId: 3, distance: Math.hypot(1, 0.5), depth: null },
    ])
  })

  it('orders samples at equal distance by pick id', () => {
    const tied = pixels(
      [
        [0, 0, 0],
        [8, 0, 5],
        [0, 0, 0],
      ],
      (id) => [...encodePickId(id)],
    )
    const samples = collectPickSamples({ ...query, ids: tied, depths: null, radius: 1 })
    expect(samples.map((sample) => sample.pickId)).toEqual([5, 8])
  })

  it('decodes ids that use all four bytes', () => {
    const large = pixels(
      [
        [0, 0, 0],
        [0, 0xfedcba98, 0],
        [0, 0, 0],
      ],
      (id) => [...encodePickId(id)],
    )
    expect(collectPickSamples({ ...query, ids: large, depths: null, radius: 0 })).toEqual([
      { pickId: 0xfedcba98, distance: 0, depth: null },
    ])
  })
})
