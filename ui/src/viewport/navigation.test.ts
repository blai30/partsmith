import { describe, expect, it } from 'vitest'
import {
  gestureFor,
  keyNavigation,
  NAVIGATION_PRESETS,
  navigationLegend,
  type Gesture,
  type NavigationPreset,
} from './navigation'

// PointerEvent.buttons bits.
const LEFT = 1
const RIGHT = 2
const MIDDLE = 4

type Case = {
  readonly preset: NavigationPreset
  readonly pressed: string
  readonly buttons: number
  readonly shift: boolean
  readonly gesture: Gesture
}

const CASES: readonly Case[] = [
  { preset: 'fusion', pressed: 'middle', buttons: MIDDLE, shift: false, gesture: 'pan' },
  { preset: 'fusion', pressed: 'Shift + middle', buttons: MIDDLE, shift: true, gesture: 'orbit' },
  { preset: 'fusion', pressed: 'left', buttons: LEFT, shift: false, gesture: 'none' },
  { preset: 'fusion', pressed: 'Shift + left', buttons: LEFT, shift: true, gesture: 'none' },
  { preset: 'fusion', pressed: 'right', buttons: RIGHT, shift: false, gesture: 'orbit' },
  { preset: 'fusion', pressed: 'Shift + right', buttons: RIGHT, shift: true, gesture: 'pan' },
  {
    preset: 'fusion',
    pressed: 'left + right',
    buttons: LEFT | RIGHT,
    shift: false,
    gesture: 'none',
  },
  {
    preset: 'fusion',
    pressed: 'middle + left',
    buttons: MIDDLE | LEFT,
    shift: false,
    gesture: 'none',
  },
  {
    preset: 'fusion',
    pressed: 'middle + right',
    buttons: MIDDLE | RIGHT,
    shift: false,
    gesture: 'none',
  },
  { preset: 'fusion', pressed: 'no button', buttons: 0, shift: false, gesture: 'none' },
  { preset: 'freecad', pressed: 'middle', buttons: MIDDLE, shift: false, gesture: 'pan' },
  {
    preset: 'freecad',
    pressed: 'middle + left',
    buttons: MIDDLE | LEFT,
    shift: false,
    gesture: 'orbit',
  },
  {
    preset: 'freecad',
    pressed: 'middle + right',
    buttons: MIDDLE | RIGHT,
    shift: false,
    gesture: 'orbit',
  },
  { preset: 'freecad', pressed: 'left', buttons: LEFT, shift: false, gesture: 'none' },
  { preset: 'freecad', pressed: 'Shift + left', buttons: LEFT, shift: true, gesture: 'none' },
  { preset: 'freecad', pressed: 'right', buttons: RIGHT, shift: false, gesture: 'orbit' },
  { preset: 'freecad', pressed: 'Shift + right', buttons: RIGHT, shift: true, gesture: 'pan' },
  {
    preset: 'freecad',
    pressed: 'Shift + middle',
    buttons: MIDDLE,
    shift: true,
    gesture: 'pan',
  },
  {
    preset: 'freecad',
    pressed: 'left + right',
    buttons: LEFT | RIGHT,
    shift: false,
    gesture: 'none',
  },
  { preset: 'freecad', pressed: 'no button', buttons: 0, shift: false, gesture: 'none' },
]

describe('gestureFor', () => {
  it.each(CASES)('$preset with $pressed is $gesture', ({ preset, buttons, shift, gesture }) => {
    expect(gestureFor(preset, buttons, { shift })).toBe(gesture)
  })
})

describe('gestureFor with the right button (U-97)', () => {
  it('U-97 right alone orbits in both presets', () => {
    expect(gestureFor('fusion', RIGHT, { shift: false })).toBe('orbit')
    expect(gestureFor('freecad', RIGHT, { shift: false })).toBe('orbit')
  })

  it('U-97 Shift with the right button pans in both presets', () => {
    expect(gestureFor('fusion', RIGHT, { shift: true })).toBe('pan')
    expect(gestureFor('freecad', RIGHT, { shift: true })).toBe('pan')
  })

  it('U-97 FreeCAD middle with right orbits, and Fusion left alone does nothing', () => {
    expect(gestureFor('freecad', MIDDLE | RIGHT, { shift: false })).toBe('orbit')
    expect(gestureFor('fusion', LEFT, { shift: false })).toBe('none')
  })
})

describe('navigationLegend (U-97)', () => {
  it('U-97 gives the Fusion legend of 2.11', () => {
    expect(navigationLegend('fusion')).toBe(
      'Orbit: Shift+middle-drag, right-drag or drag the view cube. Pan: middle-drag or Shift+right-drag. Zoom: wheel or pinch.',
    )
  })

  it('U-97 gives the FreeCAD legend of 2.11', () => {
    expect(navigationLegend('freecad')).toBe(
      'Orbit: middle+left-drag, right-drag or drag the view cube. Pan: middle-drag or Shift+right-drag. Zoom: wheel or pinch.',
    )
  })
})

// The navigation table of ui.md 3.13 as data: action, Fusion column, FreeCAD column.
const NAVIGATION_TABLE: readonly (readonly [string, string, string])[] = [
  ['Select', 'Left click', 'Left click'],
  ['Pan', 'Middle drag', 'Middle drag'],
  [
    'Orbit',
    'Shift + middle drag',
    'Middle + left drag, or middle + right drag (press middle first)',
  ],
  [
    'Orbit without a middle button (also a two-finger press and drag on a macOS trackpad)',
    'Right drag',
    'Right drag',
  ],
  ['Pan without a middle button', 'Shift + right drag', 'Shift + right drag'],
  ['Zoom', 'Wheel at the cursor', 'Wheel at the cursor'],
  ['Zoom with a trackpad pinch', 'At the cursor', 'At the cursor'],
  ['Fit to view', 'Middle double-click', 'None'],
  ['Orbit with the view cube', 'Left drag on the cube', 'Left drag on the cube'],
  [
    'Box selection',
    'Left drag on empty space, sketch mode only',
    'Left drag on empty space, sketch mode only',
  ],
]

describe('NAVIGATION_PRESETS', () => {
  it('lists Fusion first, the only preset where a middle double-click fits the view', () => {
    expect(NAVIGATION_PRESETS.map((preset) => preset.id)).toEqual(['fusion', 'freecad'])
    expect(NAVIGATION_PRESETS.map((preset) => preset.middleDoubleClickFits)).toEqual([true, false])
  })

  it.each([
    ['fusion', 1],
    ['freecad', 2],
  ] as const)(
    'U-97 %s has one row per row of the 3.13 table with that preset’s column',
    (id, column) => {
      const preset = NAVIGATION_PRESETS.find((entry) => entry.id === id)
      expect(preset?.rows).toEqual(
        NAVIGATION_TABLE.map((row) => ({ action: row[0], gesture: row[column] })),
      )
    },
  )
})

describe('keyNavigation', () => {
  const none = { shift: false, control: false, meta: false, alt: false }
  // 15 degrees at the orbit rate of 0.4 degrees per pixel.
  const orbitStep = 15 / 0.4

  it('orbits 15 degrees with the arrow keys, as a drag in their direction would', () => {
    expect(keyNavigation('ArrowLeft', none)).toEqual({
      kind: 'orbit',
      deltaX: -orbitStep,
      deltaY: 0,
    })
    expect(keyNavigation('ArrowRight', none)).toEqual({
      kind: 'orbit',
      deltaX: orbitStep,
      deltaY: 0,
    })
    expect(keyNavigation('ArrowUp', none)).toEqual({ kind: 'orbit', deltaX: 0, deltaY: -orbitStep })
    expect(keyNavigation('ArrowDown', none)).toEqual({
      kind: 'orbit',
      deltaX: 0,
      deltaY: orbitStep,
    })
  })

  it('pans by 10 percent of the viewport height with Shift and an arrow key', () => {
    expect(keyNavigation('ArrowRight', { ...none, shift: true })).toEqual({
      kind: 'pan',
      deltaX: 0.1,
      deltaY: 0,
    })
    expect(keyNavigation('ArrowUp', { ...none, shift: true })).toEqual({
      kind: 'pan',
      deltaX: 0,
      deltaY: -0.1,
    })
  })

  it('zooms in with = or + and out with - by 1.25', () => {
    expect(keyNavigation('=', none)).toEqual({ kind: 'zoom', factor: 1 / 1.25 })
    expect(keyNavigation('+', { ...none, shift: true })).toEqual({ kind: 'zoom', factor: 1 / 1.25 })
    expect(keyNavigation('-', none)).toEqual({ kind: 'zoom', factor: 1.25 })
  })

  it('leaves other keys and keys with Ctrl, Cmd or Alt to the shortcuts', () => {
    expect(keyNavigation('a', none)).toBeNull()
    expect(keyNavigation('ArrowLeft', { ...none, control: true })).toBeNull()
    expect(keyNavigation('ArrowLeft', { ...none, meta: true })).toBeNull()
    expect(keyNavigation('-', { ...none, alt: true })).toBeNull()
  })
})
