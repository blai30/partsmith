import { describe, expect, it } from 'vitest'
import {
  gestureFor,
  keyNavigation,
  NAVIGATION_PRESETS,
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
  { preset: 'fusion', pressed: 'right', buttons: RIGHT, shift: false, gesture: 'none' },
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
  { preset: 'freecad', pressed: 'right', buttons: RIGHT, shift: false, gesture: 'none' },
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

describe('NAVIGATION_PRESETS', () => {
  it('lists Fusion first, the only preset where a middle double-click fits the view', () => {
    expect(NAVIGATION_PRESETS.map((preset) => preset.id)).toEqual(['fusion', 'freecad'])
    expect(NAVIGATION_PRESETS.map((preset) => preset.middleDoubleClickFits)).toEqual([true, false])
  })
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
