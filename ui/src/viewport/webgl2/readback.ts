import type { PickSample } from '../backend'
import { decodePickId } from '../picking'

/** A rectangle of drawing buffer pixels in WebGL coordinates: origin at the bottom-left, rows upward. */
export type PixelBlock = {
  readonly left: number
  readonly bottom: number
  readonly width: number
  readonly height: number
}

/**
 * The drawing buffer pixels that can lie within `radius` CSS pixels of the point (x, y) in CSS pixels (origin top-left), clipped to the buffer; null when none do.
 */
export function pickBlock(
  x: number,
  y: number,
  radius: number,
  pixelRatio: number,
  bufferWidth: number,
  bufferHeight: number,
): PixelBlock | null {
  const reach = Math.max(radius, 0) * pixelRatio
  const centerX = x * pixelRatio
  const centerY = y * pixelRatio
  const left = Math.max(0, Math.floor(centerX - reach))
  const right = Math.min(bufferWidth - 1, Math.floor(centerX + reach))
  const top = Math.max(0, Math.floor(centerY - reach))
  const bottomRow = Math.min(bufferHeight - 1, Math.floor(centerY + reach))
  if (left > right || top > bottomRow) {
    return null
  }
  return {
    left,
    bottom: bufferHeight - 1 - bottomRow,
    width: right - left + 1,
    height: bottomRow - top + 1,
  }
}

/**
 * The pick ids of an ID pass block within `radius` CSS pixels of (x, y), one sample per id at its nearest pixel, nearest first (then by id). A pixel's distance is from the point to the nearest point of the pixel's square, so the pixel under the point is at 0. `ids` and `depths` hold RGBA8 pixels as `readPixels` returns them, bottom row first; `depths` holds the float bits of the view depth, or is null.
 */
export function collectPickSamples(input: {
  readonly ids: Uint8Array
  readonly depths: Uint8Array | null
  readonly block: PixelBlock
  readonly bufferHeight: number
  readonly pixelRatio: number
  readonly x: number
  readonly y: number
  readonly radius: number
}): PickSample[] {
  const { ids, depths, block, bufferHeight, pixelRatio, x, y, radius } = input
  const nearest = new Map<number, PickSample>()
  // A depth pixel holds the bits of a float32, least significant byte first like a pick id.
  const depthBits =
    depths === null ? null : new DataView(depths.buffer, depths.byteOffset, depths.byteLength)
  for (let row = 0; row < block.height; row++) {
    const topDownRow = bufferHeight - 1 - (block.bottom + row)
    const top = topDownRow / pixelRatio
    const offsetY = Math.max(top - y, 0, y - (topDownRow + 1) / pixelRatio)
    for (let column = 0; column < block.width; column++) {
      const offset = (row * block.width + column) * 4
      const pickId = decodePickId(
        ids[offset] ?? 0,
        ids[offset + 1] ?? 0,
        ids[offset + 2] ?? 0,
        ids[offset + 3] ?? 0,
      )
      if (pickId === 0) {
        continue
      }
      const bufferColumn = block.left + column
      const offsetX = Math.max(
        bufferColumn / pixelRatio - x,
        0,
        x - (bufferColumn + 1) / pixelRatio,
      )
      const distance = Math.hypot(offsetX, offsetY)
      if (distance > radius) {
        continue
      }
      const known = nearest.get(pickId)
      if (known !== undefined && known.distance <= distance) {
        continue
      }
      const depth = depthBits === null ? null : depthBits.getFloat32(offset, true)
      nearest.set(pickId, {
        pickId,
        distance,
        depth: depth !== null && Number.isFinite(depth) ? depth : null,
      })
    }
  }
  return [...nearest.values()].toSorted(
    (first, second) => first.distance - second.distance || first.pickId - second.pickId,
  )
}
