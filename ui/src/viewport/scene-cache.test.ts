import { describe, expect, it, vi } from 'vitest'
import type { SketchSolution } from '../generated/protocol'
import { readyState, rectSketchDocumentState } from '../layout/fixtures.testing'
import { tessellateArc } from '../sketch/geometry'
import { createAppStore, type AppState } from '../state/store'
import { createFakeBackend } from '../test/fakes/backend'
import { RECT_SKETCH_SOLUTION } from '../test/fakes/documents'
import { DEFAULT_CAMERA, orbit } from './camera'
import { createScene } from './scene'

// The scene keeps the tessellation, bounds and upload signature of an unchanged sketch (3.12).

vi.mock('../sketch/geometry', async (importOriginal) => {
  const actual = await importOriginal<typeof import('../sketch/geometry')>()
  return {
    ...actual,
    tessellateArc: vi.fn<typeof actual.tessellateArc>(actual.tessellateArc),
  }
})

const SIZE = { width: 800, height: 600, pixelRatio: 1 }

const CIRCLE_SOLUTION: SketchSolution = {
  ...RECT_SKETCH_SOLUTION,
  entities: [
    ...RECT_SKETCH_SOLUTION.entities,
    {
      id: 'e-c1',
      type: 'circle',
      center: [20, 10],
      radius: 5,
      constrained: 'under_constrained',
    },
  ],
}

function circleState(solution: SketchSolution = CIRCLE_SOLUTION): AppState {
  const document = rectSketchDocumentState({
    sketches: { 'f-sk1': { solution, display: null } },
  })
  return createAppStore(readyState(document)).getState()
}

describe('sketch caches', () => {
  it('tessellates an unchanged sketch once while the camera orbits and the bounds are read', () => {
    const backend = createFakeBackend()
    const scene = createScene(backend)
    const state = circleState()
    scene.frame(state, DEFAULT_CAMERA, SIZE)
    const tessellate = vi.mocked(tessellateArc)
    tessellate.mockClear()
    let camera = DEFAULT_CAMERA
    for (let i = 0; i < 5; i++) {
      camera = orbit(camera, 20, 5, camera.target)
      scene.frame(state, camera, SIZE)
      scene.visibleBounds(state)
    }
    expect(tessellate).not.toHaveBeenCalled()
  })

  it('draws a new solution of the sketch again', () => {
    const backend = createFakeBackend()
    const scene = createScene(backend)
    scene.frame(circleState(), DEFAULT_CAMERA, SIZE)
    const curves = backend.geometries.get('sketch:f-sk1:curves')
    const moved: SketchSolution = {
      ...CIRCLE_SOLUTION,
      entities: [
        ...RECT_SKETCH_SOLUTION.entities,
        {
          id: 'e-c1',
          type: 'circle',
          center: [50, 10],
          radius: 5,
          constrained: 'under_constrained',
        },
      ],
    }
    scene.frame(circleState(moved), DEFAULT_CAMERA, SIZE)
    const redrawn = backend.geometries.get('sketch:f-sk1:curves')
    expect(redrawn).toBeDefined()
    expect(redrawn).not.toBe(curves)
    expect(scene.visibleBounds(circleState(moved))?.max[0]).toBeCloseTo(55, 1)
  })
})
