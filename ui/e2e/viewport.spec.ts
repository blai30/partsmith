/// <reference lib="dom" />
import { expect, test, type Page } from '@playwright/test'
import type { Sample } from './pages/viewport-harness'
import { serveTestPage } from './support/test-pages'

// What e2e/pages/viewport.html puts on window: the harness around the real WebGL2 backend, served by Vite from src/.
type HarnessModule = typeof import('./pages/viewport-harness')

// Pick ids span all four bytes, so the tests cover the whole RGBA8 encoding of the ID pass.
const PICK_BASE = 0x01020300
// Face order of the test cube: +X, -X, +Y, -Y, +Z, -Z.
const FRONT_FACE = 3
const TOP_FACE = 4
const CANVAS_WIDTH = 320
const CANVAS_HEIGHT = 240
const PIXEL_RATIO = 2

const harnessPage = serveTestPage('viewport.html')

// Opens the test page and waits until the harness is on window.
async function openHarness(page: Page): Promise<void> {
  const pageErrors: string[] = []
  page.on('pageerror', (error) => pageErrors.push(error.message))
  page.on('console', (message) => {
    if (message.type() === 'error') {
      pageErrors.push(message.text())
    }
  })
  await page.goto(harnessPage.url)
  const ready = await page
    .locator('body[data-ready=true]')
    .waitFor({ timeout: 15_000 })
    .then(
      () => true,
      () => false,
    )
  expect(ready, `The test page did not load the harness: ${pageErrors.join('\n')}`).toBe(true)
}

function distinctIds(samples: readonly Sample[]): number[] {
  return [...new Set(samples.map((sample) => sample.pickId))]
}

test('the WebGL2 backend renders a cube and reads back the pick id of the face under the cursor', async ({
  page,
}) => {
  await openHarness(page)
  const result = await page.evaluate(
    ({ pickBase, width, height, pixelRatio }) => {
      const harness = (window as unknown as { partsmithViewport: HarnessModule }).partsmithViewport
      const { backend, drawingBuffer, frame, renderPixels, pickAt } = harness.createHarness(
        width,
        height,
        pixelRatio,
      )
      // A 20 mm cube centered on the origin.
      backend.upload('cube', harness.cubeGeometry(10))
      const draws = [harness.drawCall('cube', { pickBase })]
      const colorAt = (pixels: Uint8ClampedArray, x: number, y: number): number[] =>
        Array.from(
          pixels.slice((y * drawingBuffer.width + x) * 4, (y * drawingBuffer.width + x + 1) * 4),
        )

      const front = frame(harness.cameraFor('front'), draws, Math.sqrt(300))
      const frontPixels = renderPixels(front)
      const centerColor = colorAt(frontPixels, drawingBuffer.width / 2, drawingBuffer.height / 2)
      const cornerColor = colorAt(frontPixels, 8, 8)
      const frontPick = pickAt(front, width / 2, height / 2, 1)
      const missPick = pickAt(front, 4, 4, 1)
      const top = frame(harness.cameraFor('top'), draws, Math.sqrt(300))
      renderPixels(top)
      const topPick = pickAt(top, width / 2, height / 2, 1)
      const kind = backend.kind
      backend.dispose()
      return { kind, drawingBuffer, centerColor, cornerColor, frontPick, missPick, topPick }
    },
    { pickBase: PICK_BASE, width: CANVAS_WIDTH, height: CANVAS_HEIGHT, pixelRatio: PIXEL_RATIO },
  )

  expect(result.kind).toBe('webgl2')
  // The drawing buffer is the CSS size times the pixel ratio (3.12).
  expect(result.drawingBuffer).toEqual({
    width: CANVAS_WIDTH * PIXEL_RATIO,
    height: CANVAS_HEIGHT * PIXEL_RATIO,
  })
  // The shaded cube covers the center and the background the corner.
  expect(result.centerColor).not.toEqual(result.cornerColor)

  // The front view looks at the -Y face, 190 mm in front of the eye.
  expect(distinctIds(result.frontPick)).toEqual([PICK_BASE + FRONT_FACE])
  expect(Math.min(...result.frontPick.map((sample) => sample.distance))).toBeLessThan(1)
  // Depth is optional, but when the backend reads it, it is the view-space distance.
  expect(
    result.frontPick.filter(
      (sample) => sample.depth !== null && Math.abs(sample.depth - 190) >= 0.5,
    ),
  ).toEqual([])
  // Nothing is drawn near the canvas corner.
  expect(result.missPick.filter((sample) => sample.pickId !== 0)).toEqual([])
  // After the camera changes, the ID pass follows the new frame: the top view looks at the +Z face.
  expect(distinctIds(result.topPick)).toEqual([PICK_BASE + TOP_FACE])
})

test('a line with element colors draws on its first render, also after it is uploaded again', async ({
  page,
}) => {
  await openHarness(page)
  const result = await page.evaluate(
    ({ width, height, pixelRatio }) => {
      const harness = (window as unknown as { partsmithViewport: HarnessModule }).partsmithViewport
      const { backend, drawingBuffer, frame, renderPixels, takeErrors } = harness.createHarness(
        width,
        height,
        pixelRatio,
      )
      const line = harness.segmentGeometry([-40, 0, 0], [40, 0, 0])
      // A sketch line in the overlay layer, blue from its element colors (sketch DOF colors).
      const draws = [
        harness.drawCall('line', {
          layer: 'overlay',
          color: [0.5, 0.5, 0.5],
          elementColors: new Float32Array([0, 0, 1]),
          size: 6,
          depthTest: false,
        }),
      ]
      const view = frame(harness.cameraFor('front'), draws, 50)
      const center = (drawingBuffer.height / 2) * drawingBuffer.width + drawingBuffer.width / 2
      const centerColor = (pixels: Uint8ClampedArray): number[] =>
        Array.from(pixels.slice(center * 4, center * 4 + 4))
      takeErrors()
      backend.upload('line', line)
      const first = centerColor(renderPixels(view))
      const firstErrors = takeErrors()
      backend.upload('line', line)
      const afterUpload = centerColor(renderPixels(view))
      const afterUploadErrors = takeErrors()
      backend.dispose()
      return { first, firstErrors, afterUpload, afterUploadErrors }
    },
    { width: CANVAS_WIDTH, height: CANVAS_HEIGHT, pixelRatio: PIXEL_RATIO },
  )

  expect(result.firstErrors).toEqual([])
  expect(result.first).toEqual([0, 0, 255, 255])
  expect(result.afterUploadErrors).toEqual([])
  expect(result.afterUpload).toEqual([0, 0, 255, 255])
})

test('the ID pass depth-tests overlay draws, which the color pass draws on top', async ({
  page,
}) => {
  await openHarness(page)
  const result = await page.evaluate(
    ({ width, height, pixelRatio }) => {
      const harness = (window as unknown as { partsmithViewport: HarnessModule }).partsmithViewport
      const { backend, drawingBuffer, frame, renderPixels, pickAt, takeErrors } =
        harness.createHarness(width, height, pixelRatio)
      // From the top: an opaque face at z = 0, a sketch line on it at y = 10 and one 5 mm below it at y = -10.
      backend.upload('face', harness.squareGeometry(30, 0))
      backend.upload('onFace', harness.segmentGeometry([-20, 10, 0], [20, 10, 0]))
      backend.upload('belowFace', harness.segmentGeometry([-20, -10, -5], [20, -10, -5]))
      const sketchLine = { layer: 'overlay', depthTest: false, size: 2, pickSize: 7 } as const
      const draws = [
        harness.drawCall('face', { pickBase: 1 }),
        harness.drawCall('onFace', { ...sketchLine, color: [0, 1, 0], pickBase: 20 }),
        harness.drawCall('belowFace', { ...sketchLine, color: [0, 0, 1], pickBase: 30 }),
      ]
      const camera = harness.cameraFor('top', { distance: 100 })
      const view = frame(camera, draws, 50)
      const pixels = renderPixels(view)
      // The CSS pixel of a world point in the plane z = 0 seen from the top (+Y up on screen).
      const millimetersPerPixel = (2 * 100 * Math.tan(camera.verticalFieldOfView / 2)) / height
      const screenY = (worldY: number): number => height / 2 - worldY / millimetersPerPixel
      const colorAt = (x: number, y: number): number[] => {
        const index = Math.round(y * pixelRatio) * drawingBuffer.width + Math.round(x * pixelRatio)
        return Array.from(pixels.slice(index * 4, index * 4 + 4))
      }
      // The line below the face is 105 mm away, so its screen position shrinks by 100 / 105.
      const belowY = height / 2 + (10 / millimetersPerPixel) * (100 / 105)
      const readings = {
        belowColor: colorAt(width / 2, belowY),
        belowPick: pickAt(view, width / 2, belowY, 1),
        onFacePick: pickAt(view, width / 2, screenY(10), 1),
        errors: takeErrors(),
      }
      backend.dispose()
      return readings
    },
    { width: CANVAS_WIDTH, height: CANVAS_HEIGHT, pixelRatio: PIXEL_RATIO },
  )

  expect(result.errors).toEqual([])
  // The color pass draws the overlay without a depth test (3.12 pass 7): the hidden line shows on top of the face.
  expect(result.belowColor).toEqual([0, 0, 255, 255])
  // The ID pass is depth-tested (3.6): the face hides the line below it.
  expect(distinctIds(result.belowPick)).toEqual([1])
  // The face's polygon offset lets the coplanar sketch line win.
  expect(distinctIds(result.onFacePick)).toEqual([20])
})

test('a line receding in depth keeps its screen width along its length', async ({ page }) => {
  await openHarness(page)
  const result = await page.evaluate(
    ({ width, height, pixelRatio }) => {
      const harness = (window as unknown as { partsmithViewport: HarnessModule }).partsmithViewport
      const { backend, drawingBuffer, frame, renderPixels, takeErrors } = harness.createHarness(
        width,
        height,
        pixelRatio,
      )
      // Seen from the front (eye at y = -200), the line runs from view depth 50 to 600 at x = 0, so it is vertical on screen.
      backend.upload('line', harness.segmentGeometry([0, -150, -10], [0, 400, 100]))
      const draws = [harness.drawCall('line', { color: [0, 0, 0], size: 6 })]
      const pixels = renderPixels(frame(harness.cameraFor('front'), draws, 800))
      // Coverage summed across each row: the line's width in device pixels at that row.
      const rowWidths: number[] = []
      for (let row = 0; row < drawingBuffer.height; row++) {
        let covered = 0
        for (let column = 0; column < drawingBuffer.width; column++) {
          covered += (255 - (pixels[(row * drawingBuffer.width + column) * 4] ?? 255)) / 255
        }
        if (covered > 0) {
          rowWidths.push(covered)
        }
      }
      const errors = takeErrors()
      backend.dispose()
      return { rowWidths, errors }
    },
    { width: CANVAS_WIDTH, height: CANVAS_HEIGHT, pixelRatio: PIXEL_RATIO },
  )

  expect(result.errors).toEqual([])
  // Leaves out the rows of the line's rounded-off ends.
  const inner = result.rowWidths.slice(8, -8)
  expect(inner.length).toBeGreaterThan(100)
  const expected = 6 * PIXEL_RATIO
  expect(Math.min(...inner)).toBeGreaterThan(expected - 0.5)
  expect(Math.max(...inner)).toBeLessThan(expected + 0.5)
})

test('hover draws B-rep edges in highlight at 70 percent and sketch curves in full highlight', async ({
  page,
}) => {
  await openHarness(page)
  const result = await page.evaluate(
    ({ width, height, pixelRatio }) => {
      const harness = (window as unknown as { partsmithViewport: HarnessModule }).partsmithViewport
      const { backend, drawingBuffer, frame, renderPixels, takeErrors } = harness.createHarness(
        width,
        height,
        pixelRatio,
      )
      backend.upload('edge', harness.segmentGeometry([-40, 0, 10], [40, 0, 10]))
      backend.upload('sketch', harness.segmentGeometry([-40, 0, -10], [40, 0, -10]))
      backend.setElementStates('edge', new Uint8Array([1]))
      backend.setElementStates('sketch', new Uint8Array([1]))
      const draws = [
        harness.drawCall('edge', { color: [0, 0, 0], size: 4 }),
        harness.drawCall('sketch', {
          layer: 'overlay',
          depthTest: false,
          color: [0, 0, 0],
          elementColors: new Float32Array([0, 0, 1]),
          size: 4,
        }),
      ]
      const camera = harness.cameraFor('front')
      const pixels = renderPixels(frame(camera, draws, 50))
      const millimetersPerPixel = (2 * 200 * Math.tan(camera.verticalFieldOfView / 2)) / height
      const colorAtHeight = (worldZ: number): number[] => {
        const row = Math.round((height / 2 - worldZ / millimetersPerPixel) * pixelRatio)
        const index = row * drawingBuffer.width + drawingBuffer.width / 2
        return Array.from(pixels.slice(index * 4, index * 4 + 4))
      }
      const colors = { edge: colorAtHeight(10), sketch: colorAtHeight(-10), errors: takeErrors() }
      backend.dispose()
      return colors
    },
    { width: CANVAS_WIDTH, height: CANVAS_HEIGHT, pixelRatio: PIXEL_RATIO },
  )

  expect(result.errors).toEqual([])
  // Black mixed 70 percent toward the red highlight in linear space, then sRGB-encoded: 255 × encode(0.7) ≈ 217.85, within one step of the GPU's rounding.
  const encodedSeventyPercent = 255 * (1.055 * 0.7 ** (1 / 2.4) - 0.055)
  const [edgeRed, ...edgeRest] = result.edge
  expect(Math.abs((edgeRed ?? 0) - encodedSeventyPercent)).toBeLessThan(1)
  expect(edgeRest).toEqual([0, 0, 255])
  // The blue sketch line becomes the highlight itself.
  expect(result.sketch).toEqual([255, 0, 0, 255])
})

test('an edge left out of the ID pass no longer hides the face next to it', async ({ page }) => {
  await openHarness(page)
  const result = await page.evaluate(
    ({ width, height, pixelRatio }) => {
      const harness = (window as unknown as { partsmithViewport: HarnessModule }).partsmithViewport
      const { backend, frame, renderPixels, pickAt, takeErrors } = harness.createHarness(
        width,
        height,
        pixelRatio,
      )
      // From the top: a face at z = 0 and an edge on it along y = 0, drawn 7 pixels wide in the ID pass (3.6).
      backend.upload('face', harness.squareGeometry(30, 0))
      backend.upload('edge', harness.segmentGeometry([-20, 0, 0], [20, 0, 0]))
      const camera = harness.cameraFor('top', { distance: 100 })
      const face = harness.drawCall('face', { pickBase: 1 })
      const edge = { color: [0, 0, 0], size: 1.25, pickSize: 7 } as const
      // Two CSS pixels beside the edge, inside its 7-pixel ID footprint.
      const beside = { x: width / 2, y: height / 2 + 2 }
      const accepted = frame(
        camera,
        [face, harness.drawCall('edge', { ...edge, pickBase: 20 })],
        50,
      )
      renderPixels(accepted)
      const withEdge = pickAt(accepted, beside.x, beside.y, 0)
      // What the scene draws when the selection filter rejects edges.
      const rejected = frame(
        camera,
        [face, harness.drawCall('edge', { ...edge, pickBase: null })],
        50,
      )
      renderPixels(rejected)
      const withoutEdge = pickAt(rejected, beside.x, beside.y, 0)
      const errors = takeErrors()
      backend.dispose()
      return { withEdge, withoutEdge, errors }
    },
    { width: CANVAS_WIDTH, height: CANVAS_HEIGHT, pixelRatio: PIXEL_RATIO },
  )

  expect(result.errors).toEqual([])
  expect(distinctIds(result.withEdge)).toEqual([20])
  expect(distinctIds(result.withoutEdge)).toEqual([1])
})

test('E-147 the backend clears to the viewport color of each theme, written as sRGB', async ({
  page,
}) => {
  await openHarness(page)
  const result = await page.evaluate(
    ({ width, height, pixelRatio }) => {
      const harness = (window as unknown as { partsmithViewport: HarnessModule }).partsmithViewport
      const { backend, drawingBuffer, frame, renderPixels } = harness.createHarness(
        width,
        height,
        pixelRatio,
      )
      const last = { x: drawingBuffer.width - 1, y: drawingBuffer.height - 1 }
      const points = [
        [drawingBuffer.width / 2, drawingBuffer.height / 2],
        [0, 0],
        [last.x, 0],
        [0, last.y],
        [last.x, last.y],
      ] as const
      const colors = (['light', 'dark'] as const).map((theme) => {
        const pixels = renderPixels({
          ...frame(harness.cameraFor('front'), [], 10),
          theme: harness.viewportTheme(theme),
        })
        return points.map(([x, y]) =>
          Array.from(
            pixels.slice((y * drawingBuffer.width + x) * 4, (y * drawingBuffer.width + x + 1) * 4),
          ),
        )
      })
      backend.dispose()
      return colors
    },
    { width: CANVAS_WIDTH, height: CANVAS_HEIGHT, pixelRatio: PIXEL_RATIO },
  )

  // zinc-100 (244, 244, 245) and zinc-950 (9, 9, 11), the viewport role of each theme (2.16.4).
  const expected = [
    [244, 244, 245, 255],
    [9, 9, 11, 255],
  ]
  for (const [index, colors] of result.entries()) {
    for (const color of colors) {
      expect(color).toHaveLength(4)
      for (const [channel, value] of color.entries()) {
        expect(Math.abs(value - (expected[index]?.[channel] ?? Number.NaN))).toBeLessThanOrEqual(1)
      }
    }
  }
})
