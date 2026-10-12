export type NavigationPreset = 'fusion' | 'freecad'
export type Gesture = 'orbit' | 'pan' | 'none'

/** One row of the navigation table of 3.13: what the user does and the gesture that does it. */
export type NavigationRow = { readonly action: string; readonly gesture: string }

// The rows both presets share, in table order around the preset-specific ones (3.13).
const SELECT: NavigationRow = { action: 'Select', gesture: 'Left click' }
const PAN: NavigationRow = { action: 'Pan', gesture: 'Middle drag' }
const ORBIT_WITHOUT_MIDDLE: NavigationRow = {
  action: 'Orbit without a middle button (also a two-finger press and drag on a macOS trackpad)',
  gesture: 'Right drag',
}
const PAN_WITHOUT_MIDDLE: NavigationRow = {
  action: 'Pan without a middle button',
  gesture: 'Shift + right drag',
}
const ZOOM: NavigationRow = { action: 'Zoom', gesture: 'Wheel at the cursor' }
const PINCH: NavigationRow = { action: 'Zoom with a trackpad pinch', gesture: 'At the cursor' }
const VIEW_CUBE: NavigationRow = {
  action: 'Orbit with the view cube',
  gesture: 'Left drag on the cube',
}
const BOX_SELECTION: NavigationRow = {
  action: 'Box selection',
  gesture: 'Left drag on empty space, sketch mode only',
}

/**
 * The navigation presets of M1 in menu order (3.13). `rows` are the preset's column of the 3.13
 * table, in table order, which the keyboard and mouse panel renders (3.5).
 */
export const NAVIGATION_PRESETS: readonly {
  readonly id: NavigationPreset
  readonly label: string
  /** Whether a middle-button double-click fits the view. */
  readonly middleDoubleClickFits: boolean
  readonly rows: readonly NavigationRow[]
}[] = [
  {
    id: 'fusion',
    label: 'Fusion',
    middleDoubleClickFits: true,
    rows: [
      SELECT,
      PAN,
      { action: 'Orbit', gesture: 'Shift + middle drag' },
      ORBIT_WITHOUT_MIDDLE,
      PAN_WITHOUT_MIDDLE,
      ZOOM,
      PINCH,
      { action: 'Fit to view', gesture: 'Middle double-click' },
      VIEW_CUBE,
      BOX_SELECTION,
    ],
  },
  {
    id: 'freecad',
    label: 'FreeCAD',
    middleDoubleClickFits: false,
    rows: [
      SELECT,
      PAN,
      {
        action: 'Orbit',
        gesture: 'Middle + left drag, or middle + right drag (press middle first)',
      },
      ORBIT_WITHOUT_MIDDLE,
      PAN_WITHOUT_MIDDLE,
      ZOOM,
      PINCH,
      { action: 'Fit to view', gesture: 'None' },
      VIEW_CUBE,
      BOX_SELECTION,
    ],
  },
]

// PointerEvent.buttons bits.
const LEFT_BUTTON = 1
const RIGHT_BUTTON = 2
const MIDDLE_BUTTON = 4

/** A camera gesture starts after this much pointer motion; below it, a press and release is a click (3.13). */
export const GESTURE_THRESHOLD_PIXELS = 3

/**
 * The camera gesture for the pressed buttons (PointerEvent.buttons: 1 left, 2 right, 4 middle) and
 * modifiers (section 3.13). In both presets the right button alone orbits and pans with Shift, so a
 * mouse without a middle button and a trackpad work with either. Fusion: middle pans, Shift +
 * middle orbits. FreeCAD: middle pans, middle with left or with right orbits. Everything else is
 * 'none' (left alone selects or box-selects).
 */
export function gestureFor(
  preset: NavigationPreset,
  buttons: number,
  modifiers: { readonly shift: boolean },
): Gesture {
  if (buttons === RIGHT_BUTTON) {
    return modifiers.shift ? 'pan' : 'orbit'
  }
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

const LEGEND_ORBIT: Readonly<Record<NavigationPreset, string>> = {
  fusion: 'Shift+middle-drag',
  freecad: 'middle+left-drag',
}

/** One-line legend of the preset's camera gestures, shown in the empty state (2.11, 3.2). */
export function navigationLegend(preset: NavigationPreset): string {
  return `Orbit: ${LEGEND_ORBIT[preset]}, right-drag or drag the view cube. Pan: middle-drag or Shift+right-drag. Zoom: wheel or pinch.`
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
