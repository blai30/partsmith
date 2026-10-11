export type NavigationPreset = 'fusion' | 'freecad'
export type Gesture = 'orbit' | 'pan' | 'none'

/** The navigation presets of M1 (3.13), in menu order. */
export const NAVIGATION_PRESETS: readonly {
  readonly id: NavigationPreset
  readonly label: string
  /** Whether a middle-button double-click fits the view. */
  readonly middleDoubleClickFits: boolean
}[] = [
  { id: 'fusion', label: 'Fusion', middleDoubleClickFits: true },
  { id: 'freecad', label: 'FreeCAD', middleDoubleClickFits: false },
]

// PointerEvent.buttons bits.
const LEFT_BUTTON = 1
const RIGHT_BUTTON = 2
const MIDDLE_BUTTON = 4

/** A camera gesture starts after this much pointer motion; below it, a press and release is a click (3.13). */
export const GESTURE_THRESHOLD_PIXELS = 3

/**
 * The camera gesture for the pressed buttons (PointerEvent.buttons: 1 left, 2 right, 4 middle) and modifiers (section 3.13). Fusion: middle pans, Shift + middle orbits. FreeCAD: middle pans, middle with left or with right orbits. Everything else is 'none' (left alone selects or box-selects).
 */
export function gestureFor(
  preset: NavigationPreset,
  buttons: number,
  modifiers: { readonly shift: boolean },
): Gesture {
  if (preset === 'fusion') {
    if (buttons !== MIDDLE_BUTTON) {
      return 'none'
    }
    return modifiers.shift ? 'orbit' : 'pan'
  }
  if (buttons === MIDDLE_BUTTON) {
    return 'pan'
  }
  if (buttons === (MIDDLE_BUTTON | LEFT_BUTTON) || buttons === (MIDDLE_BUTTON | RIGHT_BUTTON)) {
    return 'orbit'
  }
  return 'none'
}

/** A camera change asked for from the keyboard on the focused viewport (3.5). */
export type KeyNavigation =
  /** Orbit about the target as a drag by these CSS pixels would. */
  | { readonly kind: 'orbit'; readonly deltaX: number; readonly deltaY: number }
  /** Pan by these fractions of the viewport height, as a drag in that direction would. */
  | { readonly kind: 'pan'; readonly deltaX: number; readonly deltaY: number }
  /** Zoom by this factor at the viewport center (above 1 zooms out). */
  | { readonly kind: 'zoom'; readonly factor: number }

// 15 degrees at the orbit rate of 0.4 degrees per pixel.
const ORBIT_STEP_PIXELS = 15 / 0.4
const PAN_STEP_FRACTION = 0.1
const ZOOM_STEP = 1.25

const ARROW_DIRECTIONS: Readonly<Record<string, readonly [number, number]>> = {
  ArrowLeft: [-1, 0],
  ArrowRight: [1, 0],
  ArrowUp: [0, -1],
  ArrowDown: [0, 1],
}

/**
 * The camera change for a key on the focused viewport, or null: arrows orbit 15 degrees (Left and Right about world Z, Up and Down about the camera right axis), Shift + arrows pan by 10 percent of the viewport height, `=` or `+` zoom in and `-` zooms out by 1.25. Arrows act like a drag in their direction.
 */
export function keyNavigation(
  key: string,
  modifiers: {
    readonly shift: boolean
    readonly control: boolean
    readonly meta: boolean
    readonly alt: boolean
  },
): KeyNavigation | null {
  if (modifiers.control || modifiers.meta || modifiers.alt) {
    return null
  }
  const direction = ARROW_DIRECTIONS[key]
  if (direction !== undefined) {
    const [x, y] = direction
    return modifiers.shift
      ? { kind: 'pan', deltaX: x * PAN_STEP_FRACTION, deltaY: y * PAN_STEP_FRACTION }
      : { kind: 'orbit', deltaX: x * ORBIT_STEP_PIXELS, deltaY: y * ORBIT_STEP_PIXELS }
  }
  if (key === '=' || key === '+') {
    return { kind: 'zoom', factor: 1 / ZOOM_STEP }
  }
  if (key === '-') {
    return { kind: 'zoom', factor: ZOOM_STEP }
  }
  return null
}
