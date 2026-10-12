import { describe, expect, it } from 'vitest'
import type { BodyMeshData } from '../core/events'
import type { Document } from '../generated/protocol'
import {
  bodyMeshEvent,
  boxMesh,
  CUBE_BOUNDS,
  cubeDocumentState,
  documentState,
  meshSummary,
  PEG_BOUNDS,
  readyState,
} from '../layout/fixtures.testing'
import { DEFAULT_SETTINGS } from '../settings/settings'
import {
  createAppStore,
  type ActiveCommand,
  type AppState,
  type DocumentState,
} from '../state/store'
import { createFakeBackend, type FakeBackend } from '../test/fakes/backend'
import { CUBE_DOCUMENT, TWO_BODIES_DOCUMENT } from '../test/fakes/documents'
import { linearFromHex, VIEWPORT_COLORS, viewportTheme } from '../theme/roles'
import type { DrawCall, Geometry, RenderFrame } from './backend'
import { DEFAULT_CAMERA, eye, standardViewRotation, viewMatrix, type Camera } from './camera'
import { createScene, type Scene } from './scene'

const SIZE = { width: 800, height: 600, pixelRatio: 1 }
const CUBE_MESH = boxMesh([-10, -10, 0], [10, 10, 20])
const PEG_MESH = boxMesh([-4, -4, 10], [4, 4, 30])

type Fixture = { readonly backend: FakeBackend; readonly scene: Scene }

function stateOf(document: DocumentState | null, overrides: Partial<AppState> = {}): AppState {
  return createAppStore(readyState(document, overrides)).getState()
}

function cubeState(overrides: Partial<AppState> = {}, document = cubeDocumentState()): AppState {
  return stateOf(document, overrides)
}

function cubeScene(): Fixture {
  const backend = createFakeBackend()
  const scene = createScene(backend)
  scene.applyEvent(bodyMeshEvent({ body: 'b-cube', meshId: 1, mesh: CUBE_MESH }))
  return { backend, scene }
}

/** Builds a frame and renders it, as the controller does. */
function render(fixture: Fixture, state: AppState, camera: Camera = DEFAULT_CAMERA): RenderFrame {
  const frame = fixture.scene.frame(state, camera, SIZE)
  fixture.backend.render(frame)
  return frame
}

/** The id under which a geometry was uploaded, found by its buffer (meshes are never copied). */
function uploadedId(backend: FakeBackend, positions: Float32Array): string {
  const found = [...backend.geometries].find(([, geometry]) => geometry.positions === positions)
  if (found === undefined) {
    throw new Error('No geometry was uploaded with these positions.')
  }
  return found[0]
}

function drawOf(frame: RenderFrame, backend: FakeBackend, positions: Float32Array): DrawCall {
  const id = uploadedId(backend, positions)
  const draw = frame.draws.find((candidate) => candidate.id === id)
  if (draw === undefined) {
    throw new Error(`The frame does not draw ${id}.`)
  }
  return draw
}

function geometryOf(backend: FakeBackend, positions: Float32Array): Geometry {
  const geometry = backend.geometries.get(uploadedId(backend, positions))
  if (geometry === undefined) {
    throw new Error('No geometry.')
  }
  return geometry
}

function faces(fixture: Fixture, frame: RenderFrame, mesh: BodyMeshData = CUBE_MESH): DrawCall {
  return drawOf(frame, fixture.backend, mesh.positions)
}

function edges(fixture: Fixture, frame: RenderFrame, mesh: BodyMeshData = CUBE_MESH): DrawCall {
  return drawOf(frame, fixture.backend, mesh.edgePositions)
}

function vertices(fixture: Fixture, frame: RenderFrame, mesh: BodyMeshData = CUBE_MESH): DrawCall {
  return drawOf(frame, fixture.backend, mesh.vertexPositions)
}

function pickId(draw: DrawCall, index: number): number {
  if (draw.pickBase === null) {
    throw new Error(`${draw.id} is not pickable.`)
  }
  return draw.pickBase + index
}

const FILLET_EDGES_COMMAND: ActiveCommand = {
  featureType: 'fillet',
  featureId: 'f-fil9',
  editing: false,
  title: 'Fillet',
  params: { radius: '2 mm' },
  activeField: 'edges',
  fieldErrors: {},
  applied: false,
  newBody: null,
  preview: 'none',
}

describe('uploads', () => {
  it('uploads a model-layer mesh as triangles, lines and points that alias the event buffers', () => {
    const fixture = cubeScene()
    render(fixture, cubeState())
    const triangles = geometryOf(fixture.backend, CUBE_MESH.positions)
    expect(triangles).toMatchObject({ kind: 'triangles', elementCount: 6 })
    expect(triangles.kind === 'triangles' && triangles.normals).toBe(CUBE_MESH.normals)
    expect(triangles.kind === 'triangles' && triangles.indices).toBe(CUBE_MESH.indices)
    expect(triangles.elementIds).toBe(CUBE_MESH.faceIds)
    const lines = geometryOf(fixture.backend, CUBE_MESH.edgePositions)
    expect(lines).toMatchObject({ kind: 'lines', elementCount: 12 })
    expect(lines.elementIds).toBe(CUBE_MESH.edgeIds)
    const points = geometryOf(fixture.backend, CUBE_MESH.vertexPositions)
    expect(points).toMatchObject({ kind: 'points', elementCount: 8 })
    expect(points.elementIds).toBe(CUBE_MESH.vertexIds)
  })

  it('removes a body’s draws when its model-layer mesh becomes null', () => {
    const fixture = cubeScene()
    render(fixture, cubeState())
    const id = uploadedId(fixture.backend, CUBE_MESH.positions)
    fixture.scene.applyEvent(bodyMeshEvent({ body: 'b-cube', meshId: 2, mesh: null, revision: 2 }))
    const frame = render(fixture, cubeState({}, cubeDocumentState({ meshes: {} })))
    expect(fixture.backend.geometries.has(id)).toBe(false)
    expect(frame.draws.some((draw) => draw.id === id)).toBe(false)
  })

  it('replaces a body’s mesh when a new model-layer mesh arrives', () => {
    const fixture = cubeScene()
    render(fixture, cubeState())
    const replacement = boxMesh([-10, -10, 0], [10, 10, 30])
    fixture.scene.applyEvent(
      bodyMeshEvent({ body: 'b-cube', meshId: 2, mesh: replacement, revision: 2 }),
    )
    const frame = render(
      fixture,
      cubeState({}, cubeDocumentState({ meshes: { 'b-cube': meshSummary(2, CUBE_BOUNDS) } })),
    )
    expect(faces(fixture, frame, replacement).pickBase).not.toBeNull()
    expect(
      [...fixture.backend.geometries.values()].some(
        (geometry) => geometry.positions === CUBE_MESH.positions,
      ),
    ).toBe(false)
  })

  it('restore() uploads every drawable again after a lost context', () => {
    const fixture = cubeScene()
    render(fixture, cubeState())
    fixture.backend.loseContext()
    fixture.backend.restoreContext()
    fixture.scene.restore()
    expect(geometryOf(fixture.backend, CUBE_MESH.positions).elementIds).toBe(CUBE_MESH.faceIds)
    expect(geometryOf(fixture.backend, CUBE_MESH.edgePositions).kind).toBe('lines')
    expect(() => render(fixture, cubeState())).not.toThrow()
  })
})

describe('pick ids', () => {
  it('allocates consecutive pick id ranges from 1, each as long as its draw’s element count', () => {
    const fixture = cubeScene()
    const frame = render(fixture, cubeState())
    const ranges = frame.draws
      .filter((draw) => draw.pickBase !== null)
      .map((draw) => ({
        first: draw.pickBase ?? 0,
        count: fixture.backend.geometries.get(draw.id)?.elementCount ?? 0,
      }))
      .toSorted((first, second) => first.first - second.first)
    expect(ranges[0]?.first).toBe(1)
    for (let i = 1; i < ranges.length; i++) {
      const previous = ranges[i - 1]
      expect(ranges[i]?.first).toBe((previous?.first ?? 0) + (previous?.count ?? 0))
    }
    expect(ranges.map((range) => range.count).toSorted((first, second) => first - second)).toEqual([
      6, 8, 12,
    ])
  })

  it('maps a face pick id back to the mesh face, an area', () => {
    const fixture = cubeScene()
    const frame = render(fixture, cubeState())
    expect(fixture.scene.targetFor(pickId(faces(fixture, frame), 3))).toEqual({
      result: expect.objectContaining({
        kind: 'mesh',
        body: 'b-cube',
        meshId: 1,
        element: 'face',
        index: 3,
        key: 'mesh:1:face:3',
      }),
      pickClass: 'area',
    })
  })

  it('maps an edge pick id to a curve and a vertex pick id to a point', () => {
    const fixture = cubeScene()
    const frame = render(fixture, cubeState())
    expect(fixture.scene.targetFor(pickId(edges(fixture, frame), 11))).toEqual({
      result: expect.objectContaining({
        kind: 'mesh',
        element: 'edge',
        index: 11,
        key: 'mesh:1:edge:11',
      }),
      pickClass: 'curve',
    })
    expect(fixture.scene.targetFor(pickId(vertices(fixture, frame), 0))).toEqual({
      result: expect.objectContaining({
        kind: 'mesh',
        element: 'vertex',
        index: 0,
        key: 'mesh:1:vertex:0',
      }),
      pickClass: 'point',
    })
  })

  it('maps 0 and ids past every range to nothing', () => {
    const fixture = cubeScene()
    render(fixture, cubeState())
    expect(fixture.scene.targetFor(0)).toBeNull()
    expect(fixture.scene.targetFor(1000)).toBeNull()
  })
})

describe('pickable kinds (3.6 and the ID pass contract of DrawCall.pickBase)', () => {
  it('picks faces, edges and vertices with the default filter', () => {
    const fixture = cubeScene()
    const frame = render(fixture, cubeState())
    expect(faces(fixture, frame).pickBase).not.toBeNull()
    expect(edges(fixture, frame).pickBase).not.toBeNull()
    expect(vertices(fixture, frame).pickBase).not.toBeNull()
  })

  it('leaves edges out of the ID pass when the filter rejects them, so they cannot hide faces', () => {
    const fixture = cubeScene()
    const state = cubeState({
      view: { ...cubeState().view, filter: { body: false, face: true, edge: false, vertex: true } },
    })
    const frame = render(fixture, state)
    expect(edges(fixture, frame).pickBase).toBeNull()
    expect(faces(fixture, frame).pickBase).not.toBeNull()
    expect(vertices(fixture, frame).pickBase).not.toBeNull()
  })

  it('leaves faces out with Faces and Bodies off, and keeps them with Bodies on (a face picks its body)', () => {
    const fixture = cubeScene()
    const view = cubeState().view
    const off = render(
      fixture,
      cubeState({
        view: { ...view, filter: { body: false, face: false, edge: true, vertex: true } },
      }),
    )
    expect(faces(fixture, off).pickBase).toBeNull()
    const bodies = render(
      fixture,
      cubeState({
        view: { ...view, filter: { body: true, face: false, edge: true, vertex: true } },
      }),
    )
    expect(faces(fixture, bodies).pickBase).not.toBeNull()
  })

  it('makes only what the active selection field accepts pickable during a feature command', () => {
    const fixture = cubeScene()
    const frame = render(fixture, cubeState({ command: FILLET_EDGES_COMMAND }))
    expect(edges(fixture, frame).pickBase).not.toBeNull()
    expect(faces(fixture, frame).pickBase).toBeNull()
    expect(vertices(fixture, frame).pickBase).toBeNull()
  })

  it('makes nothing pickable during a feature command without an active field', () => {
    const fixture = cubeScene()
    const frame = render(
      fixture,
      cubeState({ command: { ...FILLET_EDGES_COMMAND, activeField: null } }),
    )
    expect(frame.draws.every((draw) => draw.pickBase === null)).toBe(true)
  })

  it('draws locked bodies but keeps them out of the ID pass', () => {
    const fixture = cubeScene()
    const frame = render(fixture, cubeState({ view: { ...cubeState().view, locked: ['b-cube'] } }))
    expect(faces(fixture, frame).pickBase).toBeNull()
    expect(edges(fixture, frame).pickBase).toBeNull()
    expect(vertices(fixture, frame).pickBase).toBeNull()
  })

  it('draws the preview layer as shaded bodies that cannot be picked', () => {
    const fixture = cubeScene()
    const preview = boxMesh([-10, -10, 0], [10, 10, 25])
    fixture.scene.applyEvent(
      bodyMeshEvent({ body: 'b-cube', meshId: 9, mesh: preview, layer: 'preview', revision: 2 }),
    )
    const frame = render(
      fixture,
      cubeState({ command: { ...FILLET_EDGES_COMMAND, preview: 'shown' } }),
    )
    const draw = faces(fixture, frame, preview)
    expect(draw.pickBase).toBeNull()
    expect(draw.shading).toBe('matcap')
  })
})

describe('frames', () => {
  it('carries the camera, the theme and the UI scale', () => {
    const fixture = cubeScene()
    const camera: Camera = { ...DEFAULT_CAMERA, rotation: standardViewRotation('front') }
    const frame = render(
      fixture,
      cubeState({ settings: { ...DEFAULT_SETTINGS, theme: 'dark', uiScale: 2 } }),
      camera,
    )
    expect([...frame.view]).toEqual([...viewMatrix(camera)])
    expect(frame.eye).toEqual(eye(camera))
    expect(frame.orthographic).toBe(false)
    expect(frame.uiScale).toBe(2)
    expect(frame.theme).toEqual(viewportTheme('dark'))
    expect(frame.pixelRatio).toBe(1)
    expect(frame.analysis).toBeNull()
  })

  it('marks orthographic frames', () => {
    const fixture = cubeScene()
    const frame = render(fixture, cubeState(), { ...DEFAULT_CAMERA, projection: 'orthographic' })
    expect(frame.orthographic).toBe(true)
  })

  it('shades bodies opaque with matcap in the default body color', () => {
    const fixture = cubeScene()
    const draw = faces(fixture, render(fixture, cubeState()))
    expect(draw).toMatchObject({
      layer: 'opaque',
      shading: 'matcap',
      opacity: 1,
      depthTest: true,
      clipped: true,
    })
    expect(draw.color).toEqual(linearFromHex(VIEWPORT_COLORS.light.body))
  })

  it('shades a body in its own color', () => {
    const fixture = cubeScene()
    const red: Document = {
      ...CUBE_DOCUMENT,
      bodies: [{ id: 'b-cube', name: 'Cube', color: '#e7000b' }],
    }
    const draw = faces(fixture, render(fixture, cubeState({}, cubeDocumentState({ recipe: red }))))
    expect(draw.color).toEqual(linearFromHex('#e7000b'))
  })

  it('draws edges 1.25 pixels wide times the UI scale in the edge color with more contrast', () => {
    const fixture = cubeScene()
    const light = edges(fixture, render(fixture, cubeState()))
    expect(light.color).toEqual(linearFromHex(VIEWPORT_COLORS.light.edgeDark))
    expect(light.size).toBeCloseTo(1.25, 9)
    const black: Document = {
      ...CUBE_DOCUMENT,
      bodies: [{ id: 'b-cube', name: 'Cube', color: '#27272a' }],
    }
    const scaled = edges(
      fixture,
      render(
        fixture,
        cubeState(
          { settings: { ...DEFAULT_SETTINGS, theme: 'light', uiScale: 2 } },
          cubeDocumentState({ recipe: black }),
        ),
      ),
    )
    expect(scaled.color).toEqual(linearFromHex(VIEWPORT_COLORS.light.edgeLight))
    expect(scaled.size).toBeCloseTo(2.5, 9)
  })

  it('draws bodies that are not parts at 40 percent opacity in the translucent pass', () => {
    const fixture = cubeScene()
    const blocker: Document = {
      ...CUBE_DOCUMENT,
      bodies: [{ id: 'b-cube', name: 'Cube', role: 'support_blocker' }],
    }
    const draw = faces(
      fixture,
      render(fixture, cubeState({}, cubeDocumentState({ recipe: blocker }))),
    )
    expect(draw.layer).toBe('translucent')
    expect(draw.opacity).toBeCloseTo(0.4, 9)
  })

  it('does not draw hidden bodies', () => {
    const fixture = cubeScene()
    render(fixture, cubeState())
    const id = uploadedId(fixture.backend, CUBE_MESH.positions)
    const hidden: Document = {
      ...CUBE_DOCUMENT,
      bodies: [{ id: 'b-cube', name: 'Cube', visible: false }],
    }
    const frame = render(fixture, cubeState({}, cubeDocumentState({ recipe: hidden })))
    expect(frame.draws.some((draw) => draw.id === id)).toBe(false)
  })

  it('has no section without one, and the plane normal and offset with one (3.15)', () => {
    const fixture = cubeScene()
    const view = cubeState().view
    expect(render(fixture, cubeState()).section).toBeNull()
    expect(
      render(
        fixture,
        cubeState({ view: { ...view, section: { plane: 'XY', offset: '5', flipped: false } } }),
      ).section,
    ).toEqual({
      normal: [0, 0, 1],
      offset: 5,
    })
    expect(
      render(
        fixture,
        cubeState({ view: { ...view, section: { plane: 'XZ', offset: '5', flipped: false } } }),
      ).section,
    ).toEqual({
      normal: [0, -1, 0],
      offset: 5,
    })
    expect(
      render(
        fixture,
        cubeState({ view: { ...view, section: { plane: 'YZ', offset: '5', flipped: true } } }),
      ).section,
    ).toEqual({
      normal: [-1, 0, 0],
      offset: -5,
    })
  })

  it('marks the hovered face with the hover bit only', () => {
    const fixture = cubeScene()
    const frame = render(fixture, cubeState({ hover: { key: 'mesh:1:face:2', kind: 'face' } }))
    const states = fixture.backend.elementStates.get(faces(fixture, frame).id)
    expect(states === undefined ? [] : [...states]).toEqual([0, 0, 1, 0, 0, 0])
  })
})

describe('visibleBounds', () => {
  it('bounds the visible bodies', () => {
    const fixture = cubeScene()
    expect(fixture.scene.visibleBounds(cubeState())).toEqual(CUBE_BOUNDS)
  })

  it('unites several bodies', () => {
    const fixture = cubeScene()
    fixture.scene.applyEvent(bodyMeshEvent({ body: 'b-peg', meshId: 2, mesh: PEG_MESH }))
    const state = stateOf(
      documentState(TWO_BODIES_DOCUMENT, {
        meshes: { 'b-cube': meshSummary(1, CUBE_BOUNDS), 'b-peg': meshSummary(2, PEG_BOUNDS) },
      }),
    )
    expect(fixture.scene.visibleBounds(state)).toEqual({ min: [-10, -10, 0], max: [10, 10, 30] })
  })

  it('is null when nothing is visible', () => {
    const fixture = cubeScene()
    const hidden: Document = {
      ...CUBE_DOCUMENT,
      bodies: [{ id: 'b-cube', name: 'Cube', visible: false }],
    }
    const state = cubeState(
      { view: { ...cubeState().view, sessionVisible: { 'feature:f-sk1': false } } },
      cubeDocumentState({ recipe: hidden }),
    )
    expect(fixture.scene.visibleBounds(state)).toBeNull()
  })

  it('includes a visible sketch', () => {
    const backend = createFakeBackend()
    const scene = createScene(backend)
    const state = cubeState(
      { view: { ...cubeState().view, sessionVisible: { 'feature:f-sk1': true } } },
      cubeDocumentState({ meshes: {} }),
    )
    expect(scene.visibleBounds(state)).toEqual({ min: [-10, -10, 0], max: [10, 10, 0] })
  })
})

describe('screenRectangle', () => {
  it('covers a selected body in CSS pixels', () => {
    const fixture = cubeScene()
    render(fixture, cubeState())
    // Front view, 20 CSS pixels per mm, the body's center 10 mm above the target.
    const camera: Camera = {
      ...DEFAULT_CAMERA,
      rotation: standardViewRotation('front'),
      target: [0, 0, 10],
      projection: 'orthographic',
      orthographicHeight: 25,
    }
    const body = {
      key: 'body:b-cube',
      kind: 'body' as const,
      label: 'Cube',
      body: 'b-cube',
      feature: null,
      element: null,
      sketchItem: null,
      origin: null,
    }
    const rectangle = fixture.scene.screenRectangle([body], camera, { width: 1000, height: 500 })
    expect(rectangle?.x).toBeCloseTo(300, 6)
    expect(rectangle?.y).toBeCloseTo(50, 6)
    expect(rectangle?.width).toBeCloseTo(400, 6)
    expect(rectangle?.height).toBeCloseTo(400, 6)
  })

  it('is null for no items', () => {
    const fixture = cubeScene()
    expect(
      fixture.scene.screenRectangle([], DEFAULT_CAMERA, { width: 1000, height: 500 }),
    ).toBeNull()
  })
})

describe('sketches and datum planes', () => {
  it('draws a visible sketch in the Model context as reference lines 1.25 pixels wide, without region fill', () => {
    const fixture = cubeScene()
    const reference = linearFromHex(VIEWPORT_COLORS.light.sketchReference)
    const shown = render(
      fixture,
      cubeState({ view: { ...cubeState().view, sessionVisible: { 'feature:f-sk1': true } } }),
    )
    const sketchDraws = shown.draws.filter((draw) => draw.layer === 'overlay')
    expect(
      sketchDraws.some((draw) =>
        draw.color.every((channel, index) => channel === reference[index]),
      ),
    ).toBe(true)
    expect(
      sketchDraws.some((draw) => fixture.backend.geometries.get(draw.id)?.kind === 'fill'),
    ).toBe(false)
    const lines = sketchDraws.find(
      (draw) => fixture.backend.geometries.get(draw.id)?.kind === 'lines',
    )
    expect(lines?.size).toBeCloseTo(1.25, 9)
    expect(lines?.depthTest).toBe(false)
    expect(lines?.clipped).toBe(false)
    const hidden = render(
      fixture,
      cubeState({ view: { ...cubeState().view, sessionVisible: { 'feature:f-sk1': false } } }),
    )
    expect(hidden.draws.some((draw) => draw.layer === 'overlay')).toBe(false)
  })

  it('draws a datum plane as a translucent square at 15 percent and picks it as a datum plane', () => {
    const backend = createFakeBackend()
    const scene = createScene(backend)
    const document: Document = {
      ...CUBE_DOCUMENT,
      bodies: [],
      features: [
        {
          id: 'f-dp1',
          type: 'datum_plane',
          base: { kind: 'origin', plane: 'XY' },
          offset: '10 mm',
        },
      ],
    }
    const state = stateOf(
      documentState(document, {
        datums: { 'f-dp1': { origin: [0, 0, 10], x_axis: [1, 0, 0], normal: [0, 0, 1] } },
      }),
    )
    const frame = scene.frame(state, DEFAULT_CAMERA, SIZE)
    backend.render(frame)
    const datum = linearFromHex(VIEWPORT_COLORS.light.datum)
    const fill = frame.draws.find(
      (draw) =>
        draw.layer === 'translucent' &&
        draw.opacity === 0.15 &&
        draw.color.every((channel, index) => channel === datum[index]),
    )
    expect(fill).toBeDefined()
    expect(fill?.clipped).toBe(false)
    const target =
      fill?.pickBase === null || fill === undefined ? null : scene.targetFor(fill.pickBase)
    expect(target?.pickClass).toBe('area')
    expect(target?.result).toMatchObject({
      kind: 'item',
      item: { key: 'feature:f-dp1', kind: 'datum_plane' },
    })
  })
})
