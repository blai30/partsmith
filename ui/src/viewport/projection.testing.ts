import type { RenderFrame } from './backend'

// Where a frame draws world points, for tests that check positions on the canvas.

/** A column-major 4 by 4 matrix times a 4-vector. */
function applyMatrix(matrix: Float32Array, vector: readonly number[]): number[] {
  return [0, 1, 2, 3].map((row) =>
    [0, 1, 2, 3].reduce(
      (sum, column) => sum + (matrix[column * 4 + row] ?? 0) * (vector[column] ?? 0),
      0,
    ),
  )
}

/** Where `frame`'s view and projection draw the world point `point` on a canvas of `size`, in CSS pixels. */
export function worldToCanvas(
  frame: Pick<RenderFrame, 'view' | 'projection'>,
  point: readonly number[],
  size: { readonly width: number; readonly height: number },
): { x: number; y: number } {
  const clip = applyMatrix(frame.projection, applyMatrix(frame.view, [...point, 1]))
  const homogeneous = clip[3] ?? 1
  return {
    x: (((clip[0] ?? 0) / homogeneous + 1) / 2) * size.width,
    y: ((1 - (clip[1] ?? 0) / homogeneous) / 2) * size.height,
  }
}
