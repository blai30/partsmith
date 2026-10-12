import { clsx } from 'clsx/lite'
import { House } from 'lucide-react'
import { useRef, useState, type PointerEvent, type Ref } from 'react'
import { COLOR_CLASSES } from '../theme/roles'
import type { Quaternion } from './math'
import { GESTURE_THRESHOLD_PIXELS } from './navigation'
import {
  CUBE_FACES,
  FACE_PLACEMENT_CLASSES,
  faceCells,
  faceName,
  VIEW_CUBE_PARTS,
  viewCubeRotation,
  type CubeFace,
  type FaceCell,
  type ViewCubePart,
} from './view-cube'

export type ViewCubeProps = {
  /** The rotor, which the viewport controller turns every frame with `orientElement(rotor, viewCubeRotorMatrix(camera.rotation))`. */
  readonly rotorRef: Ref<HTMLDivElement>
  /** A face, edge or corner was chosen: animate to this rotation, keeping target, distance and projection (3.14). */
  readonly onChooseView: (rotation: Quaternion) => void
  /** The Home view button: the iso view fitted to the visible bounds. */
  readonly onHome: () => void
  /** A drag on the cube: orbit about the target by these CSS pixel deltas, as navigation does. */
  readonly onOrbit: (deltaX: number, deltaY: number) => void
}

// Bit of PointerEvent.buttons for the primary button.
const PRIMARY_BUTTON = 1

type Press = {
  readonly pointerId: number
  readonly startX: number
  readonly startY: number
  lastX: number
  lastY: number
  dragging: boolean
}

const FACES_WITH_CELLS: readonly {
  readonly face: CubeFace
  readonly cells: readonly FaceCell[]
}[] = CUBE_FACES.map((face) => ({ face, cells: faceCells(face) }))

const PARTS_BY_ID: ReadonlyMap<string, ViewCubePart> = new Map(
  VIEW_CUBE_PARTS.map((cubePart) => [cubePart.id, cubePart]),
)

function partAt(target: EventTarget | null): ViewCubePart | null {
  if (!(target instanceof Element)) {
    return null
  }
  const id = target.closest<HTMLElement>('[data-view-cube-part]')?.dataset.viewCubePart
  return id === undefined ? null : (PARTS_BY_ID.get(id) ?? null)
}

/**
 * The view cube (3.14): 6 faces, 12 edges and 8 corners as buttons in a CSS 3D cube whose rotor follows the camera, a drag that orbits, and the Home view button. Each edge band and corner square also shows on the part's other faces, where it acts the same and highlights with it. The cube is not in the tab order, since the viewport is one tab stop (3.24). Only the face views and the iso view are also commands (`view.standard.*`); the keyboard reaches the other views by orbiting with the arrow keys in 15 degree steps (3.5), exactly for the edge views and the top corners, but not for the bottom corners.
 */
export function ViewCube({ rotorRef, onChooseView, onHome, onOrbit }: ViewCubeProps) {
  const [hovered, setHovered] = useState<string | null>(null)
  const press = useRef<Press | null>(null)
  // Set while the click that ends a drag is dispatched, so the drag does not also choose a view.
  const afterDrag = useRef(false)

  function choose(cubePart: ViewCubePart): void {
    if (!afterDrag.current) {
      onChooseView(viewCubeRotation(cubePart))
    }
  }

  function handlePointerDown(event: PointerEvent<HTMLFieldSetElement>): void {
    if (event.button !== 0 || press.current !== null) {
      return
    }
    press.current = {
      pointerId: event.pointerId,
      startX: event.clientX,
      startY: event.clientY,
      lastX: event.clientX,
      lastY: event.clientY,
      dragging: false,
    }
  }

  function handlePointerMove(event: PointerEvent<HTMLFieldSetElement>): void {
    const current = press.current
    if (current === null || current.pointerId !== event.pointerId) {
      return
    }
    // The button was released where the cube did not see it, so this is a hover, not a drag.
    if ((event.buttons & PRIMARY_BUTTON) === 0) {
      press.current = null
      return
    }
    if (!current.dragging) {
      const moved = Math.hypot(event.clientX - current.startX, event.clientY - current.startY)
      if (moved <= GESTURE_THRESHOLD_PIXELS) {
        return
      }
      current.dragging = true
      event.currentTarget.setPointerCapture?.(event.pointerId)
      setHovered(null)
    }
    onOrbit(event.clientX - current.lastX, event.clientY - current.lastY)
    current.lastX = event.clientX
    current.lastY = event.clientY
  }

  function endPress(event: PointerEvent<HTMLFieldSetElement>): void {
    const current = press.current
    if (current === null || current.pointerId !== event.pointerId) {
      return
    }
    press.current = null
    if (!current.dragging) {
      return
    }
    if (event.currentTarget.hasPointerCapture?.(event.pointerId) === true) {
      event.currentTarget.releasePointerCapture(event.pointerId)
    }
    afterDrag.current = true
    setTimeout(() => {
      afterDrag.current = false
    }, 0)
  }

  // A press that has not become a drag has no pointer capture, so it ends when the pointer leaves the cube, where its release would go unseen.
  function cancelPress(event: PointerEvent<HTMLFieldSetElement>): void {
    const current = press.current
    if (current !== null && current.pointerId === event.pointerId && !current.dragging) {
      press.current = null
    }
  }

  function handlePointerOver(event: PointerEvent<HTMLFieldSetElement>): void {
    if (press.current?.dragging !== true) {
      setHovered(partAt(event.target)?.id ?? null)
    }
  }

  return (
    <fieldset
      data-testid="view-cube"
      aria-label="View cube"
      className="absolute top-4 right-4 size-24 touch-none select-none"
      onPointerDown={handlePointerDown}
      onPointerMove={handlePointerMove}
      onPointerUp={endPress}
      onPointerCancel={endPress}
      onLostPointerCapture={endPress}
      onPointerOver={handlePointerOver}
      onPointerLeave={(event) => {
        cancelPress(event)
        setHovered(null)
      }}
    >
      <div className="absolute inset-0 perspective-distant">
        <div ref={rotorRef} className="absolute inset-0 transform-3d">
          {FACES_WITH_CELLS.map(({ face, cells }) => (
            <div
              key={face}
              className={clsx(
                'absolute inset-0 grid grid-cols-8 grid-rows-8 border backface-hidden',
                COLOR_CLASSES.border.border,
                FACE_PLACEMENT_CLASSES[face],
              )}
            >
              {cells.map(({ part: cubePart, shape, primary }) => {
                const className = clsx(
                  'cursor-pointer transition-colors duration-150 motion-reduce:transition-none',
                  shape === 'corner' && 'col-span-2 row-span-2',
                  shape === 'horizontal' && 'col-span-4 row-span-2',
                  shape === 'vertical' && 'col-span-2 row-span-4',
                  shape === 'center' && 'col-span-4 row-span-4',
                  hovered === cubePart.id
                    ? COLOR_CLASSES.hover.active
                    : COLOR_CLASSES.surfaceRaised.background,
                )
                return primary ? (
                  <button
                    key={cubePart.id}
                    type="button"
                    tabIndex={-1}
                    aria-label={cubePart.label}
                    data-view-cube-part={cubePart.id}
                    className={clsx(className, COLOR_CLASSES.focusRing.outline)}
                    onClick={() => choose(cubePart)}
                  />
                ) : (
                  <div
                    key={cubePart.id}
                    aria-hidden="true"
                    data-view-cube-part={cubePart.id}
                    className={className}
                    onClick={() => choose(cubePart)}
                  />
                )
              })}
              <span
                aria-hidden="true"
                className={clsx(
                  'pointer-events-none absolute inset-0 flex items-center justify-center text-xs font-medium whitespace-nowrap',
                  COLOR_CLASSES.textMuted.foreground,
                )}
              >
                {faceName(face)}
              </span>
            </div>
          ))}
        </div>
      </div>
      <button
        type="button"
        tabIndex={-1}
        aria-label="Home view"
        className={clsx(
          'absolute top-0 left-0 z-10 flex size-6 cursor-pointer items-center justify-center rounded-sm border transition-colors duration-150 motion-reduce:transition-none',
          COLOR_CLASSES.surfaceRaised.background,
          COLOR_CLASSES.border.border,
          COLOR_CLASSES.textMuted.foreground,
          COLOR_CLASSES.hover.background,
          COLOR_CLASSES.focusRing.outline,
        )}
        onClick={() => {
          if (!afterDrag.current) {
            onHome()
          }
        }}
      >
        <House aria-hidden="true" className="size-4" strokeWidth={1.75} />
      </button>
    </fieldset>
  )
}
