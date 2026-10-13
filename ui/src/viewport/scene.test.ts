import { describe, expect, it } from 'vitest'
import type { BodyMeshData } from '../core/events'
import type { Document, SketchSolution } from '../generated/protocol'
import {
  bodyMeshEvent,
  boxMesh,
  cubeDocumentState,
  documentState,
  FILLET_EDGES_COMMAND,
  readyState,
  rectSketchDocumentState,
} from '../layout/fixtures.testing'
import { DEFAULT_SETTINGS } from '../settings/settings'
import {
  createAppStore,
  type ActiveCommand,
  type AppState,
  type DocumentState,
  type SelectionItem,
  type SketchSession,
} from '../state/store'
import { createFakeBackend, type FakeBackend } from '../test/fakes/backend'
import { CUBE_DOCUMENT, RECT_SKETCH_SOLUTION, TWO_BODIES_DOCUMENT } from '../test/fakes/documents'
import { acceptsTarget } from '../inspector/selection-values'
import { CUBE_BOUNDS, meshSummary, PEG_BOUNDS } from '../test/fakes/state'
import { linearFromHex, VIEWPORT_COLORS, viewportTheme } from '../theme/roles'
import type { DrawCall, Geometry, RenderFrame } from './backend'
import {
  DEFAULT_CAMERA,
  eye,
  standardViewRotation,
  viewMatrix,
  type Camera,
  type Vector3,
} from './camera'
import { dot, subtract } from './math'
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

function drawOf(frame: RenderFrame, backend: FakeBackend, positions: Float32Array): DrawCall {
  const id = backend.idOf(positions)
  const draw = frame.draws.find((candidate) => candidate.id === id)
  if (draw === undefined) {
    throw new Error(`The frame does not draw ${id}.`)
  }
  return draw
}

function geometryOf(backend: FakeBackend, positions: Float32Array): Geometry {
  const geometry = backend.geometries.get(backend.idOf(positions))
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
    const id = fixture.backend.idOf(CUBE_MESH.positions)
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
    const shellFaces: ActiveCommand = {
      ...FILLET_EDGES_COMMAND,
      featureType: 'shell',
      params: { thickness: '2 mm' },
      activeField: 'remove_faces',
    }
    const frame = render(fixture, cubeState({ command: shellFaces }))
    expect(faces(fixture, frame).pickBase).not.toBeNull()
    expect(edges(fixture, frame).pickBase).toBeNull()
    expect(vertices(fixture, frame).pickBase).toBeNull()
  })

  it('makes faces pickable for a multiple edge field, which takes a face as its edges (3.7)', () => {
    const fixture = cubeScene()
    const frame = render(fixture, cubeState({ command: FILLET_EDGES_COMMAND }))
    expect(edges(fixture, frame).pickBase).not.toBeNull()
    expect(faces(fixture, frame).pickBase).not.toBeNull()
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

  it('draws the preview of a body the command creates, which has no model-layer mesh yet', () => {
    const fixture = cubeScene()
    const base = cubeDocumentState()
    const document = documentState({
      ...base.recipe,
      bodies: [...(base.recipe.bodies ?? []), { id: 'b-new', name: 'Body 1' }],
    })
    const preview = boxMesh([30, 30, 0], [40, 40, 10])
    fixture.scene.applyEvent(
      bodyMeshEvent({ body: 'b-new', meshId: 9, mesh: preview, layer: 'preview', revision: 2 }),
    )
    const command: ActiveCommand = {
      ...FILLET_EDGES_COMMAND,
      featureType: 'box',
      featureId: 'f-box1',
      params: {},
      activeField: null,
      applied: true,
      newBody: { id: 'b-new', name: 'Body 1', path: 'body' },
      preview: 'shown',
    }
    const frame = render(fixture, stateOf(document, { command }))
    const shadedFaces = faces(fixture, frame, preview)
    expect(shadedFaces.pickBase).toBeNull()
    expect(shadedFaces.shading).toBe('matcap')
    expect(shadedFaces.opacity).toBe(1)
    const outline = edges(fixture, frame, preview)
    expect(outline.pickBase).toBeNull()
    expect(outline.size).toBeCloseTo(1.25, 9)
    expect(
      frame.draws.some((draw) => draw.id.startsWith('model:') && draw.id.includes('b-new')),
    ).toBe(false)
    const hidden = render(fixture, stateOf(document, { command: { ...command, preview: 'none' } }))
    expect(hidden.draws.some((draw) => draw.id.includes('b-new'))).toBe(false)
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
    const id = fixture.backend.idOf(CUBE_MESH.positions)
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

  describe('region fills in the Model context (3.12 pass 7)', () => {
    // One triangle standing for the rectangle's region.
    const DISPLAY = {
      regionPositions: new Float32Array([0, 0, 40, 0, 40, 20]),
      regionIndices: new Uint32Array([0, 1, 2]),
      regionIds: new Uint32Array([0, 0, 0]),
    }
    const EXTRUDE_PROFILE_COMMAND: ActiveCommand = {
      ...FILLET_EDGES_COMMAND,
      featureType: 'extrude',
      featureId: 'f-ext9',
      title: 'Extrude',
      params: { extent: { kind: 'distance', expr: '10 mm' }, op: 'new' },
      activeField: 'profile',
    }

    function regionOpacity(command: ActiveCommand | null): number | undefined {
      const backend = createFakeBackend()
      const scene = createScene(backend)
      const document = rectSketchDocumentState({
        sketches: { 'f-sk1': { solution: RECT_SKETCH_SOLUTION, display: DISPLAY } },
      })
      const frame = scene.frame(stateOf(document, { command }), DEFAULT_CAMERA, SIZE)
      return frame.draws.find((draw) => draw.id === 'sketch:f-sk1:regions')?.opacity
    }

    it('leaves the regions of a visible sketch unfilled without a command', () => {
      expect(regionOpacity(null)).toBe(0)
    })

    it('fills every region at 12 percent while the active selection field takes regions', () => {
      expect(regionOpacity(EXTRUDE_PROFILE_COMMAND)).toBe(0.12)
    })

    it('leaves them unfilled while the active field takes something else', () => {
      expect(regionOpacity({ ...EXTRUDE_PROFILE_COMMAND, activeField: null })).toBe(0)
    })
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

describe('sketch point entities', () => {
  const POINT_SOLUTION: SketchSolution = {
    ...RECT_SKETCH_SOLUTION,
    entities: [
      ...RECT_SKETCH_SOLUTION.entities,
      { id: 'e-p1', type: 'point', position: [10, 5], constrained: 'under_constrained' },
    ],
    points: [...RECT_SKETCH_SOLUTION.points, { point: 'e-p1', at: [10, 5] }],
  }
  const HOLE_POINTS_COMMAND: ActiveCommand = {
    ...FILLET_EDGES_COMMAND,
    featureType: 'hole',
    featureId: 'f-hole1',
    title: 'Hole',
    params: {},
    activeField: 'points',
  }

  function pointTarget(overrides: Partial<AppState>): SelectionItem | null {
    const scene = createScene(createFakeBackend())
    const document = rectSketchDocumentState({
      sketches: { 'f-sk1': { solution: POINT_SOLUTION, display: null } },
    })
    const state = stateOf(document, overrides)
    const frame = scene.frame(state, DEFAULT_CAMERA, SIZE)
    const draw = frame.draws.find((candidate) => candidate.id === 'sketch:f-sk1:points')
    if (draw === undefined || draw.pickBase === null) {
      return null
    }
    const target = scene.targetFor(draw.pickBase)
    return target?.result.kind === 'item' ? target.result.item : null
  }

  it('picks a point entity by its point reference, which a Hole points field accepts', () => {
    const item = pointTarget({ command: HOLE_POINTS_COMMAND })
    expect(item).toMatchObject({
      key: 'point:f-sk1:e-p1',
      kind: 'sketch_point',
      sketchItem: { kind: 'point', sketch: 'f-sk1', point: 'e-p1' },
    })
    expect(item !== null && acceptsTarget('sketch_point', item)).toBe(true)
  })

  it('selects a point entity in the Model context as the same item a field takes', () => {
    const item = pointTarget({
      view: { ...stateOf(null).view, sessionVisible: { 'feature:f-sk1': true } },
    })
    expect(item?.sketchItem).toEqual({ kind: 'point', sketch: 'f-sk1', point: 'e-p1' })
    expect(item !== null && acceptsTarget('sketch_point', item)).toBe(true)
  })
})

function gridDraws(frame: RenderFrame): readonly DrawCall[] {
  return frame.draws.filter((draw) => draw.id.startsWith('grid:'))
}

describe('the grid (3.12)', () => {
  const EDITING: SketchSession = {
    sketchId: 'f-sk1',
    tool: null,
    construction: false,
    showConstraints: true,
    selection: [],
    dragging: false,
    cameraBefore: null,
  }

  it('draws minor lines at 35 and major lines at 70 percent, 1 pixel wide, depth-tested and never picked', () => {
    const fixture = cubeScene()
    const frame = render(fixture, cubeState())
    const draws = gridDraws(frame)
    expect(draws.map((draw) => [draw.id, draw.opacity])).toEqual([
      ['grid:minor', 0.35],
      ['grid:major', 0.7],
    ])
    for (const draw of draws) {
      expect(draw.layer).toBe('opaque')
      expect(draw.color).toEqual(linearFromHex(VIEWPORT_COLORS.light.grid))
      expect(draw.size).toBe(1)
      expect(draw.depthTest).toBe(true)
      expect(draw.pickBase).toBeNull()
      expect(fixture.backend.geometries.get(draw.id)?.kind).toBe('lines')
    }
  })

  it('draws after the faces, which hide it, and before the edges and vertices, so an edge on the ground stays on top of its grid line (3.12 pass 4)', () => {
    const fixture = cubeScene()
    const frame = render(fixture, cubeState())
    const order = [
      faces(fixture, frame).id,
      'grid:minor',
      'grid:major',
      edges(fixture, frame).id,
      vertices(fixture, frame).id,
    ].map((id) => frame.draws.findIndex((draw) => draw.id === id))
    expect(order.every((index) => index !== -1)).toBe(true)
    expect(order.toSorted((first, second) => first - second)).toEqual(order)
    expect(new Set(order.map((index) => frame.draws[index]?.layer))).toEqual(new Set(['opaque']))
  })

  it('draws lines 1 pixel wide times the UI scale', () => {
    const fixture = cubeScene()
    const frame = render(
      fixture,
      cubeState({ settings: { ...DEFAULT_SETTINGS, theme: 'light', uiScale: 2 } }),
    )
    expect(gridDraws(frame).map((draw) => draw.size)).toEqual([2, 2])
  })

  it('lies on the XY plane in Model context', () => {
    const fixture = cubeScene()
    render(fixture, cubeState())
    const positions = fixture.backend.geometries.get('grid:major')?.positions ?? new Float32Array()
    expect(positions.length).toBeGreaterThan(0)
    for (let i = 2; i < positions.length; i += 3) {
      expect(positions[i]).toBe(0)
    }
  })

  it('lies on the sketch plane while a sketch is edited', () => {
    const fixture = cubeScene()
    const state = cubeState({ context: 'sketch', sketch: EDITING })
    const placement = state.document?.sketches['f-sk1']?.solution.placement
    render(fixture, state)
    const positions = fixture.backend.geometries.get('grid:major')?.positions ?? new Float32Array()
    expect(positions.length).toBeGreaterThan(0)
    for (let i = 0; i + 2 < positions.length; i += 3) {
      const point: Vector3 = [positions[i] ?? 0, positions[i + 1] ?? 0, positions[i + 2] ?? 0]
      const along = dot(
        subtract(point, placement?.origin ?? [0, 0, 0]),
        placement?.normal ?? [0, 0, 1],
      )
      expect(Math.abs(along)).toBeLessThan(1e-3)
    }
  })

  it('draws the origin and axes of the edited sketch before its curves and points, so a curve on an axis covers it in the ID pass (3.6)', () => {
    const fixture = cubeScene()
    const frame = render(fixture, cubeState({ context: 'sketch', sketch: EDITING }))
    const order = [
      'sketch:f-sk1:axes',
      'sketch:f-sk1:origin',
      'sketch:f-sk1:curves',
      'sketch:f-sk1:points',
    ].map((id) => frame.draws.findIndex((draw) => draw.id === id))
    expect(order.every((index) => index !== -1)).toBe(true)
    expect(order.toSorted((first, second) => first - second)).toEqual(order)
  })

  it('draws a profile problem halo under the curves it names, which keep their DOF color (3.16.2)', () => {
    const fixture = cubeScene()
    const base = cubeState({ context: 'sketch', sketch: EDITING })
    const sketchState = base.document?.sketches['f-sk1']
    if (base.document === null || sketchState === undefined) {
      throw new Error('The cube document has no sketch.')
    }
    const solution: SketchSolution = {
      ...sketchState.solution,
      profile_findings: [{ type: 'open_endpoint', entity: 'e-2', point: [10, 10] }],
    }
    const state: AppState = {
      ...base,
      document: {
        ...base.document,
        sketches: { 'f-sk1': { ...sketchState, solution } },
      },
    }
    const frame = render(fixture, state)
    const ids = frame.draws.map((draw) => draw.id)
    const halo = frame.draws.find((draw) => draw.id === 'sketch:f-sk1:halo')
    expect(halo).toMatchObject({ layer: 'overlay', opacity: 1, size: 9.5, pickBase: null })
    // Problem (red-700) at 30 percent over the viewport bottom (zinc-100), sRGB-encoded.
    expect(halo?.color).toEqual(linearFromHex('#e5abae'))
    expect(fixture.backend.geometries.get('sketch:f-sk1:halo')?.elementCount).toBe(1)
    expect(ids.indexOf('sketch:f-sk1:halo')).toBeLessThan(ids.indexOf('sketch:f-sk1:curves'))
    const curves = fixture.backend.elementStates.get('sketch:f-sk1:curves')
    expect([...(curves ?? [])]).toEqual([0, 0, 0, 0])
    expect(frame.draws.find((draw) => draw.id === 'sketch:f-sk1:curves')?.elementColors).toEqual(
      new Float32Array(
        Array.from({ length: 4 }, () => [
          ...linearFromHex(VIEWPORT_COLORS.light.sketchFull),
        ]).flat(),
      ),
    )
    render(fixture, base)
    expect(fixture.backend.geometries.has('sketch:f-sk1:halo')).toBe(false)
  })

  it('draws no grid in Print, which draws the bed, or while it is turned off', () => {
    const fixture = cubeScene()
    expect(gridDraws(render(fixture, cubeState({ context: 'print' })))).toEqual([])
    expect(fixture.backend.geometries.has('grid:minor')).toBe(false)
    const off = cubeState()
    expect(gridDraws(render(fixture, { ...off, view: { ...off.view, grid: false } }))).toEqual([])
  })

  it('uploads its lines again only when the spacing changes', () => {
    const fixture = cubeScene()
    render(fixture, cubeState())
    const first = fixture.backend.geometries.get('grid:minor')
    render(fixture, cubeState())
    expect(fixture.backend.geometries.get('grid:minor')).toBe(first)
    render(fixture, cubeState(), { ...DEFAULT_CAMERA, distance: DEFAULT_CAMERA.distance * 20 })
    expect(fixture.backend.geometries.get('grid:minor')).not.toBe(first)
  })
})
