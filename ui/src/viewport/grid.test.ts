import { describe, expect, it } from 'vitest'
import type { Placement } from '../generated/protocol'
import { cubeDocumentState, readyState } from '../layout/fixtures.testing'
import { createAppStore, type AppState, type SketchSession } from '../state/store'
import { DEFAULT_CAMERA, visibleHeightAtTarget } from './camera'
import {
  drawnGridSpacing,
  gridCorners,
  gridHalfSide,
  gridPlacement,
  gridSegments,
  gridSpacing,
} from './grid'

const GROUND: Placement = { origin: [0, 0, 0], x_axis: [1, 0, 0], normal: [0, 0, 1] }

const EDITING_SKETCH: SketchSession = {
  sketchId: 'f-sk1',
  tool: null,
  construction: false,
  showConstraints: true,
  selection: [],
  dragging: false,
  cameraBefore: null,
}

function stateWith(overrides: Partial<AppState> = {}): AppState {
  return createAppStore(readyState(cubeDocumentState(), overrides)).getState()
}

describe('gridSpacing', () => {
  it('U-120 gridSpacing(0.05, mm) is 1', () => {
    expect(gridSpacing(0.05, 'mm')).toBe(1)
  })

  it('U-120 gridSpacing(0.5, mm) is 10', () => {
    expect(gridSpacing(0.5, 'mm')).toBe(10)
  })

  it('U-120 gridSpacing(5, mm) is 100', () => {
    expect(gridSpacing(5, 'mm')).toBe(100)
  })

  it('U-120 gridSpacing(0.05, in) is 2.54', () => {
    expect(gridSpacing(0.05, 'in')).toBe(2.54)
  })

  it('takes the step whose lines are exactly 8 pixels apart', () => {
    expect(gridSpacing(1.25, 'mm')).toBe(10)
  })

  it('uses the millimeter steps in centimeter and meter documents', () => {
    expect(gridSpacing(0.5, 'cm')).toBe(10)
    expect(gridSpacing(0.5, 'm')).toBe(10)
  })

  it('uses the largest step when every step is closer than 8 pixels', () => {
    expect(gridSpacing(1000, 'mm')).toBe(1000)
    expect(gridSpacing(1000, 'in')).toBe(2540)
  })
})

describe('gridPlacement', () => {
  it('lies on the XY plane in Model context', () => {
    expect(gridPlacement(stateWith())).toEqual(GROUND)
  })

  it('lies on the sketch plane while a sketch is edited', () => {
    const state = stateWith({ context: 'sketch', sketch: EDITING_SKETCH })
    expect(gridPlacement(state)).toBe(state.document?.sketches['f-sk1']?.solution.placement)
  })

  it('lies on the XY plane while a plane is picked for a new sketch', () => {
    const state = stateWith({ context: 'sketch', sketch: { ...EDITING_SKETCH, sketchId: null } })
    expect(gridPlacement(state)).toEqual(GROUND)
  })

  it('is null in Print, which draws the bed outline', () => {
    expect(gridPlacement(stateWith({ context: 'print' }))).toBeNull()
  })

  it('is null while the grid is turned off', () => {
    const state = stateWith()
    expect(gridPlacement({ ...state, view: { ...state.view, grid: false } })).toBeNull()
  })
})

describe('drawnGridSpacing', () => {
  it('follows the scale at the camera target times the UI scale', () => {
    const state = stateWith()
    const height = 600
    const millimetersPerPixel = visibleHeightAtTarget(DEFAULT_CAMERA) / height
    expect(drawnGridSpacing(state, DEFAULT_CAMERA, height)).toBe(
      gridSpacing(millimetersPerPixel, 'mm'),
    )
    const scaled = { ...state, settings: { ...state.settings, uiScale: 3 as const } }
    expect(drawnGridSpacing(scaled, DEFAULT_CAMERA, height)).toBe(
      gridSpacing(millimetersPerPixel * 3, 'mm'),
    )
  })

  it('is null when no grid is drawn', () => {
    expect(drawnGridSpacing(stateWith({ context: 'print' }), DEFAULT_CAMERA, 600)).toBeNull()
  })
})

describe('the grid square', () => {
  it('is at least 200 mm across, rounded out to whole major cells', () => {
    expect(gridHalfSide(null, 1)).toBe(100)
    expect(gridHalfSide(null, 100)).toBe(1000)
    expect(gridHalfSide(null, 2.54)).toBeCloseTo(101.6, 9)
  })

  it('is twice the diagonal of the visible bounds when that is larger', () => {
    // A diagonal of 300 mm gives a side of 600 mm.
    expect(gridHalfSide({ min: [0, 0, 0], max: [300, 0, 0] }, 10)).toBe(300)
  })

  it('has every tenth line major, one each way through the origin', () => {
    const segments = gridSegments(GROUND, 10, 100)
    // 21 offsets from -100 to 100; 3 of them (-100, 0, 100) major; two lines per offset.
    expect(segments.major.length / 6).toBe(6)
    expect(segments.minor.length / 6).toBe(36)
    // The lines through the origin, after the two of offset -100.
    expect(Array.from(segments.major.slice(12, 24))).toEqual([
      -100, 0, 0, 100, 0, 0, 0, -100, 0, 0, 100, 0,
    ])
  })

  it('lies on the sketch plane along its axes', () => {
    const front: Placement = { origin: [0, 0, 5], x_axis: [1, 0, 0], normal: [0, -1, 0] }
    const segments = gridSegments(front, 10, 100)
    for (let i = 1; i < segments.major.length; i += 3) {
      expect(segments.major[i]).toBe(0)
    }
    expect(gridCorners(front, 100)).toEqual([
      [-100, 0, -95],
      [100, 0, -95],
      [100, 0, 105],
      [-100, 0, 105],
    ])
  })
})
