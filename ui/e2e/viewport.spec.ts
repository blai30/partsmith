/// <reference lib="dom" />
import { fileURLToPath } from 'node:url'
import { expect, test } from '@playwright/test'
import { createServer, type ViteDevServer } from 'vite'
import type { DrawCall, Geometry, RenderFrame, ViewportTheme } from '../src/viewport/backend'
import type { Camera, StandardView, Vector3 } from '../src/viewport/camera'

// What e2e/pages/viewport.html puts on window: the real viewport modules, served by Vite from src/.
type ViewportModules = {
  readonly backend: typeof import('../src/viewport/backend')
  readonly camera: typeof import('../src/viewport/camera')
}

type Sample = { readonly pickId: number; readonly distance: number; readonly depth: number | null }

// Pick ids span all four bytes, so the test covers the whole RGBA8 encoding of the ID pass.
const PICK_BASE = 0x01020300
// Face order of the test cube: +X, -X, +Y, -Y, +Z, -Z.
const FRONT_FACE = 3
const TOP_FACE = 4
const CANVAS_WIDTH = 320
const CANVAS_HEIGHT = 240
const PIXEL_RATIO = 2

const uiDirectory = fileURLToPath(new URL('..', import.meta.url))
let server: ViteDevServer | null = null
let pageUrl = ''

// The production preview serves only the app, so this test serves its own page from a Vite dev server.
test.beforeAll(async () => {
  server = await createServer({
    root: uiDirectory,
    logLevel: 'error',
    clearScreen: false,
    server: { port: 0, hmr: false },
    optimizeDeps: { entries: ['e2e/pages/viewport.html'] },
  })
  await server.listen()
  const base = server.resolvedUrls?.local[0]
  if (base === undefined) {
    throw new Error('The Vite dev server reported no local URL.')
  }
  pageUrl = new URL('e2e/pages/viewport.html', base).href
})

test.afterAll(async () => {
  await server?.close()
})

function distinctIds(samples: readonly Sample[]): number[] {
  return [...new Set(samples.map((sample) => sample.pickId))]
}

test('the WebGL2 backend renders a cube and reads back the pick id of the face under the cursor', async ({
  page,
}) => {
  const pageErrors: string[] = []
  page.on('pageerror', (error) => pageErrors.push(error.message))
  page.on('console', (message) => {
    if (message.type() === 'error') {
      pageErrors.push(message.text())
    }
  })
  await page.goto(pageUrl)
  const ready = await page
    .locator('body[data-ready=true]')
    .waitFor({ timeout: 15_000 })
    .then(
      () => true,
      () => false,
    )
  expect(ready, `The test page did not load the viewport modules: ${pageErrors.join('\n')}`).toBe(
    true,
  )

  const result = await page.evaluate(
    ({ pickBase, width, height, pixelRatio }) => {
      const modules = (window as unknown as { partsmithViewport: ViewportModules })
        .partsmithViewport
      const canvas = document.querySelector('canvas')
      if (!canvas) {
        throw new Error('The test page has no canvas.')
      }
      const backend = modules.backend.createWebGl2Backend(canvas)
      if (!backend) {
        throw new Error('createWebGl2Backend returned null although Chromium supports WebGL2.')
      }
      backend.resize(width, height, pixelRatio)

      // A 20 mm cube centered on the origin: per face a normal and two in-face axes whose cross product is the normal, so corners run counterclockwise seen from outside.
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
          positions.push(
            10 * (normal[0] + alongTangent * tangent[0] + alongBitangent * bitangent[0]),
            10 * (normal[1] + alongTangent * tangent[1] + alongBitangent * bitangent[1]),
            10 * (normal[2] + alongTangent * tangent[2] + alongBitangent * bitangent[2]),
          )
          normals.push(...normal)
          elementIds.push(face)
        }
        indices.push(first, first + 1, first + 2, first, first + 2, first + 3)
      })
      const cube: Geometry = {
        kind: 'triangles',
        positions: new Float32Array(positions),
        normals: new Float32Array(normals),
        indices: new Uint32Array(indices),
        elementIds: new Uint32Array(elementIds),
        elementCount: faces.length,
      }
      backend.upload('cube', cube)

      const draw: DrawCall = {
        id: 'cube',
        layer: 'opaque',
        color: [0.35, 0.35, 0.38],
        elementColors: null,
        opacity: 1,
        size: 1,
        dashed: false,
        shading: 'matcap',
        depthTest: true,
        clipped: false,
        pickBase,
        pickSize: 0,
        analysis: false,
      }
      const theme: ViewportTheme = {
        backgroundTop: [1, 1, 1],
        backgroundBottom: [0.91, 0.91, 0.92],
        highlight: [0.91, 0.07, 0],
        problem: [0.53, 0, 0],
        edgeDark: [0.01, 0.01, 0.01],
        edgeLight: [0.78, 0.78, 0.8],
      }
      const frameFor = (standardView: StandardView): RenderFrame => {
        const camera: Camera = {
          ...modules.camera.DEFAULT_CAMERA,
          rotation: modules.camera.standardViewRotation(standardView),
        }
        const range = modules.camera.depthRange(camera, [0, 0, 0], Math.sqrt(300))
        return {
          view: modules.camera.viewMatrix(camera),
          projection: modules.camera.projectionMatrix(
            camera,
            width / height,
            range.near,
            range.far,
          ),
          eye: modules.camera.eye(camera),
          orthographic: false,
          pixelRatio,
          uiScale: 1,
          theme,
          draws: [draw],
          section: null,
          analysis: null,
        }
      }
      // Reads drawing buffer pixels in the task that rendered them, before the canvas is composited.
      const colorAt = (x: number, y: number): number[] => {
        const probe = document.createElement('canvas')
        probe.width = canvas.width
        probe.height = canvas.height
        const context = probe.getContext('2d')
        if (!context) {
          throw new Error('The test page cannot create a 2D context.')
        }
        context.drawImage(canvas, 0, 0)
        return Array.from(context.getImageData(x, y, 1, 1).data)
      }
      // Picks at a point in CSS pixels, copying the samples into plain objects for the return trip.
      const pickAt = (frame: RenderFrame, x: number, y: number) =>
        backend
          .pick(frame, x, y, 1)
          .map(({ pickId, distance, depth }) => ({ pickId, distance, depth }))

      const front = frameFor('front')
      backend.render(front)
      const centerColor = colorAt(canvas.width / 2, canvas.height / 2)
      const cornerColor = colorAt(8, 8)
      const frontPick = pickAt(front, width / 2, height / 2)
      const missPick = pickAt(front, 4, 4)
      const top = frameFor('top')
      backend.render(top)
      const topPick = pickAt(top, width / 2, height / 2)
      const drawingBuffer = { width: canvas.width, height: canvas.height }
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
  // The shaded cube covers the center and the background gradient the corner.
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
