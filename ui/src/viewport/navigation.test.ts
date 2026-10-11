import { describe, expect, it } from 'vitest'
import { gestureFor, type Gesture, type NavigationPreset } from './navigation'

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
