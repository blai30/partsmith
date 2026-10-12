import { describe, expect, it } from 'vitest'
import type { DrawCall } from '../backend'
import { drawState, EDGE_HOVER_HIGHLIGHT, hoverHighlight } from './draw-state'

function drawCall(overrides: Partial<DrawCall> = {}): DrawCall {
  return {
    id: 'draw',
    layer: 'opaque',
    color: [0.5, 0.5, 0.5],
    elementColors: null,
    opacity: 1,
    size: 1.25,
    dashed: false,
    shading: 'matcap',
    depthTest: true,
    clipped: false,
    pickBase: 1,
    pickSize: 7,
    analysis: false,
    ...overrides,
  }
}

const SKETCH_CURVE = drawCall({ layer: 'overlay', depthTest: false, shading: 'flat' })

describe('drawState', () => {
  it('depth-tests overlay draws in the ID pass although the color pass draws them on top', () => {
    expect(drawState(SKETCH_CURVE, 'lines', 'color', false).depthTest).toBe(false)
    expect(drawState(SKETCH_CURVE, 'lines', 'id', false).depthTest).toBe(true)
    expect(drawState(SKETCH_CURVE, 'points', 'id', false).depthTest).toBe(true)
    expect(drawState(SKETCH_CURVE, 'fill', 'id', false).depthTest).toBe(true)
  })

  it('writes depth and never blends in the ID pass', () => {
    const translucent = drawCall({ layer: 'translucent', opacity: 0.4 })
    expect(drawState(translucent, 'triangles', 'id', false)).toMatchObject({
      depthWrite: true,
      blend: false,
    })
  })

  it('draws opaque bodies without blending and writes their depth', () => {
    expect(drawState(drawCall(), 'triangles', 'color', false)).toEqual({
      depthTest: true,
      depthWrite: true,
      blend: false,
      cullFace: true,
      polygonOffset: true,
    })
  })

  it('blends translucent bodies without writing depth', () => {
    const translucent = drawCall({ layer: 'translucent', opacity: 0.4 })
    expect(drawState(translucent, 'triangles', 'color', false)).toMatchObject({
      depthWrite: false,
      blend: true,
    })
  })

  it('blends anti-aliased edges, which do not write depth', () => {
    expect(drawState(drawCall(), 'lines', 'color', false)).toMatchObject({
      depthTest: true,
      depthWrite: false,
      blend: true,
      polygonOffset: false,
      cullFace: false,
    })
  })

  it('keeps back faces of sectioned bodies for the cut cap in both passes', () => {
    expect(drawState(drawCall(), 'triangles', 'color', true).cullFace).toBe(false)
    expect(drawState(drawCall(), 'triangles', 'id', true).cullFace).toBe(false)
  })
})

describe('hoverHighlight', () => {
  it('mixes hovered B-rep edges 70 percent toward the highlight', () => {
    expect(hoverHighlight(drawCall())).toBe(EDGE_HOVER_HIGHLIGHT)
    expect(EDGE_HOVER_HIGHLIGHT).toBe(0.7)
  })

  it('draws hovered sketch curves in the highlight itself', () => {
    expect(hoverHighlight(SKETCH_CURVE)).toBe(1)
  })
})
