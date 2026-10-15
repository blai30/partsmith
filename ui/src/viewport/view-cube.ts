import type { WorldAxis } from '../core/labels'
import { rotationLookingAlong, standardViewRotation } from './camera'
import { add, rotateVector, type Quaternion, type Vector3 } from './math'

export type CubeFace = 'front' | 'back' | 'left' | 'right' | 'top' | 'bottom'

/** Faces in naming order (3.14): edge and corner names list their faces in this order. */
export const CUBE_FACES: readonly CubeFace[] = ['front', 'back', 'left', 'right', 'top', 'bottom']

/** A clickable part of the view cube: a face, an edge between two faces or a corner of three. */
export type ViewCubePart = {
  /** The part's faces joined by '-', in naming order, such as 'front-top'. */
  readonly id: string
  readonly kind: 'face' | 'edge' | 'corner'
  /** Its faces in naming order; the first is the face that holds the part's button. */
  readonly faces: readonly CubeFace[]
  /** The accessible name, such as "Front view", "Front top edge view" or "Front right top corner view". */
  readonly label: string
}

// Direction from the target toward the eye when looking at each face (2.11): the front view looks from -Y.
const FACE_DIRECTIONS: Readonly<Record<CubeFace, Vector3>> = {
  front: [0, -1, 0],
  back: [0, 1, 0],
  left: [-1, 0, 0],
  right: [1, 0, 0],
  top: [0, 0, 1],
  bottom: [0, 0, -1],
}

function capitalized(text: string): string {
  return text.charAt(0).toUpperCase() + text.slice(1)
}

function part(faces: readonly CubeFace[]): ViewCubePart {
  const kind = faces.length === 1 ? 'face' : faces.length === 2 ? 'edge' : 'corner'
  const words = capitalized(faces.join(' '))
  return {
    id: faces.join('-'),
    kind,
    faces,
    label: kind === 'face' ? `${words} view` : `${words} ${kind} view`,
  }
}

function opposite(first: CubeFace, second: CubeFace): boolean {
  const [x, y, z] = add(FACE_DIRECTIONS[first], FACE_DIRECTIONS[second])
  return x === 0 && y === 0 && z === 0
}

function buildParts(): readonly ViewCubePart[] {
  const faces = CUBE_FACES.map((face) => part([face]))
  const edges: ViewCubePart[] = []
  CUBE_FACES.forEach((first, index) => {
    for (const second of CUBE_FACES.slice(index + 1)) {
      if (!opposite(first, second)) {
        edges.push(part([first, second]))
      }
    }
  })
  const corners: ViewCubePart[] = []
  for (const depth of ['front', 'back'] as const) {
    for (const side of ['left', 'right'] as const) {
      for (const height of ['top', 'bottom'] as const) {
        corners.push(part([depth, side, height]))
      }
    }
  }
  return [...faces, ...edges, ...corners]
}

/** The 6 faces, 12 edges and 8 corners of the view cube (3.14). */
export const VIEW_CUBE_PARTS: readonly ViewCubePart[] = buildParts()

const PARTS_BY_FACES = new Map(
  VIEW_CUBE_PARTS.map((cubePart) => [[...cubePart.faces].toSorted().join(' '), cubePart]),
)

const PARTS_BY_ID: ReadonlyMap<string, ViewCubePart> = new Map(
  VIEW_CUBE_PARTS.map((cubePart) => [cubePart.id, cubePart]),
)

/** The part with this id (`ViewCubePart.id`), or null. */
export function partById(id: string): ViewCubePart | null {
  return PARTS_BY_ID.get(id) ?? null
}

/** The part made of exactly these faces, in any order, or null when they do not meet. */
export function partForFaces(faces: readonly CubeFace[]): ViewCubePart | null {
  return PARTS_BY_FACES.get([...faces].toSorted().join(' ')) ?? null
}

/**
 * The camera rotation a part sets (3.14): a face sets its standard view; an edge or corner looks from the normalized sum of its face directions with up the projection of +Z onto the view plane.
 */
export function viewCubeRotation(cubePart: ViewCubePart): Quaternion {
  const [first] = cubePart.faces
  if (cubePart.kind === 'face' && first !== undefined) {
    return standardViewRotation(first)
  }
  const position = cubePart.faces.reduce<Vector3>(
    (sum, face) => add(sum, FACE_DIRECTIONS[face]),
    [0, 0, 0],
  )
  return rotationLookingAlong([-position[0], -position[1], -position[2]], [0, 0, 1])
}

/**
 * The CSS `matrix3d` (column-major 4 × 4) that turns the cube's rotor to match a camera rotation. The rotor's untransformed frame is the top view in CSS axes (x right, y down, z toward the viewer): world (x, y, z) sits at rotor (x, -y, z), so the matrix is the world-to-camera rotation conjugated by that flip of y.
 */
export function viewCubeRotorMatrix(rotation: Quaternion): Float32Array {
  const rightAxis = rotateVector(rotation, [1, 0, 0])
  const upAxis = rotateVector(rotation, [0, 1, 0])
  const backAxis = rotateVector(rotation, [0, 0, 1])
  return new Float32Array([
    rightAxis[0],
    -upAxis[0],
    backAxis[0],
    0,
    -rightAxis[1],
    upAxis[1],
    -backAxis[1],
    0,
    rightAxis[2],
    -upAxis[2],
    backAxis[2],
    0,
    0,
    0,
    0,
    1,
  ])
}

/**
 * Placement of each face in the rotor with Tailwind 3D utilities (3.14), for a cube as large as the view cube's `size-24` box (half-size 3rem): the rotation turns the face so it reads upright from its standard view, and the translation, in the rotor's frame, moves it out along its direction.
 */
export const FACE_PLACEMENT_CLASSES: Readonly<Record<CubeFace, string>> = {
  front: '-rotate-x-90 translate-y-12',
  back: '-rotate-x-90 rotate-y-180 -translate-y-12',
  left: '-rotate-x-90 -rotate-y-90 -translate-x-12',
  right: '-rotate-x-90 rotate-y-90 translate-x-12',
  top: 'translate-z-12',
  bottom: 'rotate-x-180 -translate-z-12',
}

/**
 * The world axes each face's horizontal and vertical sides run along, as the face reads upright from
 * its standard view (3.14): the sides are drawn in those axes' colors, so every edge of the cube
 * shows its axis.
 */
export const FACE_SIDE_AXES: Readonly<
  Record<CubeFace, { readonly horizontal: WorldAxis; readonly vertical: WorldAxis }>
> = {
  front: { horizontal: 'X', vertical: 'Z' },
  back: { horizontal: 'X', vertical: 'Z' },
  left: { horizontal: 'Y', vertical: 'Z' },
  right: { horizontal: 'Y', vertical: 'Z' },
  top: { horizontal: 'X', vertical: 'Y' },
  bottom: { horizontal: 'X', vertical: 'Y' },
}

/** The shape of a cell of a face's 3 × 3 grid: corner squares, edge bands along the sides and the center. */
export type FaceCellShape = 'corner' | 'horizontal' | 'vertical' | 'center'

export type FaceCell = {
  readonly part: ViewCubePart
  readonly shape: FaceCellShape
  /** Whether this cell is the part's button: the cell on the part's first face. */
  readonly primary: boolean
}

function faceAlong(direction: Vector3): CubeFace {
  const rounded = direction.map(Math.round)
  const face = CUBE_FACES.find((candidate) =>
    FACE_DIRECTIONS[candidate].every((component, index) => component === rounded[index]),
  )
  if (face === undefined) {
    throw new RangeError(`No cube face looks along (${direction.join(', ')}).`)
  }
  return face
}

function cellShape(column: number, row: number): FaceCellShape {
  if (column !== 1 && row !== 1) {
    return 'corner'
  }
  if (column === 1 && row === 1) {
    return 'center'
  }
  return row === 1 ? 'vertical' : 'horizontal'
}

/**
 * The 9 cells of a face, row by row as the face reads from its standard view: corners, edge bands and the face itself in the center. Every edge appears on its 2 faces and every corner on its 3; only the cell on a part's first face is its button.
 */
export function faceCells(face: CubeFace): readonly FaceCell[] {
  const rotation = standardViewRotation(face)
  const rightFace = faceAlong(rotateVector(rotation, [1, 0, 0]))
  const leftFace = faceAlong(rotateVector(rotation, [-1, 0, 0]))
  const upFace = faceAlong(rotateVector(rotation, [0, 1, 0]))
  const downFace = faceAlong(rotateVector(rotation, [0, -1, 0]))
  const cells: FaceCell[] = []
  for (const row of [0, 1, 2]) {
    for (const column of [0, 1, 2]) {
      const faces: CubeFace[] = [face]
      if (column !== 1) {
        faces.push(column === 0 ? leftFace : rightFace)
      }
      if (row !== 1) {
        faces.push(row === 0 ? upFace : downFace)
      }
      const cubePart = partForFaces(faces)
      if (cubePart === null) {
        throw new RangeError(`The faces ${faces.join(', ')} do not meet.`)
      }
      cells.push({
        part: cubePart,
        shape: cellShape(column, row),
        primary: cubePart.faces[0] === face,
      })
    }
  }
  return cells
}

/** The visible label of a face. */
export function faceName(face: CubeFace): string {
  return capitalized(face)
}
