import { describe, expect, it, vi } from 'vitest'
import type { DrawCall, Geometry, RenderFrame } from '../../viewport/backend'
import { createFakeBackend, geometryLayoutProblem, type PickHandler } from './backend'

type Triangles = Extract<Geometry, { readonly kind: 'triangles' }>
type Lines = Extract<Geometry, { readonly kind: 'lines' }>

// One quad of two triangles, both of element 0.
function quad(): Triangles {
  return {
    kind: 'triangles',
    positions: new Float32Array([0, 0, 0, 1, 0, 0, 1, 1, 0, 0, 1, 0]),
    normals: new Float32Array([0, 0, 1, 0, 0, 1, 0, 0, 1, 0, 0, 1]),
    indices: new Uint32Array([0, 1, 2, 0, 2, 3]),
    elementIds: new Uint32Array([0, 0, 0, 0]),
    elementCount: 1,
  }
}

// Two line segments, elements 0 and 1.
function segments(): Lines {
  return {
    kind: 'lines',
    positions: new Float32Array([0, 0, 0, 1, 0, 0, 1, 0, 0, 1, 1, 0]),
    elementIds: new Uint32Array([0, 1]),
    elementCount: 2,
  }
}

function draw(id: string, pickBase: number | null): DrawCall {
  return {
    id,
    layer: 'opaque',
    color: [0.5, 0.5, 0.5],
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
}

function frame(draws: readonly DrawCall[]): RenderFrame {
  return {
    view: new Float32Array(16),
    projection: new Float32Array(16),
    eye: [0, -200, 0],
    orthographic: false,
    pixelRatio: 1,
    uiScale: 1,
    theme: {
      backgroundTop: [1, 1, 1],
      backgroundBottom: [0.9, 0.9, 0.9],
      highlight: [0.9, 0.1, 0],
      problem: [0.5, 0, 0],
      edgeDark: [0, 0, 0],
      edgeLight: [0.8, 0.8, 0.8],
    },
    draws,
    section: null,
    analysis: null,
  }
}

describe('geometryLayoutProblem', () => {
  it('accepts valid triangles and lines', () => {
    expect(geometryLayoutProblem(quad())).toBeNull()
    expect(geometryLayoutProblem(segments())).toBeNull()
  })

  it('rejects triangles without one element id per vertex', () => {
    expect(geometryLayoutProblem({ ...quad(), elementIds: new Uint32Array([0, 0]) })).toMatch(
      /one per vertex/,
    )
  })

  it('rejects an index that is not below the vertex count', () => {
    expect(geometryLayoutProblem({ ...quad(), indices: new Uint32Array([0, 1, 4]) })).toMatch(
      /vertex count/,
    )
  })

  it('rejects an element id that is not below elementCount', () => {
    expect(geometryLayoutProblem({ ...segments(), elementIds: new Uint32Array([0, 2]) })).toMatch(
      /elementCount/,
    )
  })

  it('rejects lines that are not two points per segment', () => {
    const geometry: Geometry = {
      kind: 'lines',
      positions: new Float32Array([0, 0, 0, 1, 0, 0, 1, 1, 0]),
      elementIds: new Uint32Array([0]),
      elementCount: 1,
    }
    expect(geometryLayoutProblem(geometry)).toMatch(/2 points per segment/)
  })

  it('rejects normals whose length differs from the positions', () => {
    expect(geometryLayoutProblem({ ...quad(), normals: new Float32Array(3) })).toMatch(/normals/)
  })
})

describe('createFakeBackend', () => {
  it('keeps uploaded geometry by reference and records the call', () => {
    const backend = createFakeBackend()
    const geometry = quad()
    backend.upload('body', geometry)
    expect(backend.geometries.get('body')).toBe(geometry)
    expect(backend.calls).toEqual([{ method: 'upload', id: 'body' }])
  })

  it('throws on an upload that breaks the buffer layout', () => {
    const backend = createFakeBackend()
    expect(() => {
      backend.upload('body', { ...quad(), elementCount: 0 })
    }).toThrow(/FakeBackend.upload\("body"\)/)
  })

  it('stores a copy of the element states and requires one byte per element', () => {
    const backend = createFakeBackend()
    backend.upload('edges', segments())
    const states = new Uint8Array([1, 2])
    backend.setElementStates('edges', states)
    states[0] = 0
    expect(Array.from(backend.elementStates.get('edges') ?? [])).toEqual([1, 2])
    expect(() => {
      backend.setElementStates('edges', new Uint8Array(3))
    }).toThrow(/3 states for 2 elements/)
  })

  it('requires one scalar per vertex and clears the field with null', () => {
    const backend = createFakeBackend()
    backend.upload('body', quad())
    backend.setScalarField('body', new Float32Array([0, 1, 2, 3]))
    expect(backend.scalarFields.has('body')).toBe(true)
    backend.setScalarField('body', null)
    expect(backend.scalarFields.has('body')).toBe(false)
    expect(() => {
      backend.setScalarField('body', new Float32Array(2))
    }).toThrow(/2 values for 4 vertices/)
  })

  it('drops the states and scalar field of a geometry uploaded again', () => {
    const backend = createFakeBackend()
    backend.upload('body', quad())
    backend.setElementStates('body', new Uint8Array([1]))
    backend.setScalarField('body', new Float32Array(4))
    backend.upload('body', quad())
    expect(backend.elementStates.has('body')).toBe(false)
    expect(backend.scalarFields.has('body')).toBe(false)
  })

  it('records rendered frames and rejects a draw without uploaded geometry', () => {
    const backend = createFakeBackend()
    backend.upload('body', quad())
    const rendered = frame([draw('body', 1)])
    backend.render(rendered)
    expect(backend.lastFrame()).toBe(rendered)
    expect(() => {
      backend.render(frame([draw('missing', null)]))
    }).toThrow(/no uploaded geometry/)
  })

  it('rejects draws whose pick id ranges overlap', () => {
    const backend = createFakeBackend()
    backend.upload('edges', segments())
    backend.upload('body', quad())
    backend.render(frame([draw('edges', 1), draw('body', 3)]))
    expect(() => {
      backend.render(frame([draw('edges', 1), draw('body', 2)]))
    }).toThrow(/overlapping pick ids/)
  })

  it('answers picks through the pick handler with the request', () => {
    const backend = createFakeBackend()
    const handler = vi.fn<PickHandler>(() => [{ pickId: 3, distance: 1, depth: null }])
    backend.setPickHandler(handler)
    const rendered = frame([])
    expect(backend.pick(rendered, 10, 20, 4)).toEqual([{ pickId: 3, distance: 1, depth: null }])
    expect(handler).toHaveBeenCalledWith({ frame: rendered, x: 10, y: 20, radius: 4 })
    expect(backend.calls).toEqual([{ method: 'pick', x: 10, y: 20, radius: 4 }])
  })

  it('answers no picks by default', () => {
    expect(createFakeBackend().pick(frame([]), 0, 0, 4)).toEqual([])
  })

  it('drops every resource on context loss and notifies the listeners in order', () => {
    const backend = createFakeBackend()
    const events: string[] = []
    backend.onContextLost(() => events.push('lost'))
    backend.onContextRestored(() => events.push('restored'))
    backend.upload('body', quad())
    backend.loseContext()
    expect(backend.contextLost).toBe(true)
    expect(backend.geometries.size).toBe(0)
    backend.restoreContext()
    expect(backend.contextLost).toBe(false)
    expect(events).toEqual(['lost', 'restored'])
  })

  it('ignores uploads while the context is lost', () => {
    const backend = createFakeBackend()
    backend.loseContext()
    backend.upload('body', quad())
    expect(backend.geometries.size).toBe(0)
  })

  it('stops calling a listener after it unsubscribes', () => {
    const backend = createFakeBackend()
    const listener = vi.fn<() => void>()
    const unsubscribe = backend.onContextLost(listener)
    unsubscribe()
    backend.loseContext()
    expect(listener).not.toHaveBeenCalled()
  })

  it('throws on any call after dispose', () => {
    const backend = createFakeBackend()
    backend.dispose()
    expect(backend.disposed).toBe(true)
    expect(() => {
      backend.render(frame([]))
    }).toThrow(/after dispose/)
  })

  it('records resize and exposes the size', () => {
    const backend = createFakeBackend()
    backend.resize(800, 600, 2)
    expect(backend.size).toEqual({ width: 800, height: 600, pixelRatio: 2 })
  })
})
