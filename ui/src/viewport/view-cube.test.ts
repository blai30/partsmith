import { describe, expect, it } from 'vitest'
import { classifyView, DEFAULT_CAMERA, forward, standardViewRotation, up } from './camera'
import type { Vector3 } from './math'
import {
  CUBE_FACES,
  FACE_PLACEMENT_CLASSES,
  faceCells,
  partForFaces,
  VIEW_CUBE_PARTS,
  viewCubeRotation,
  viewCubeRotorMatrix,
  type CubeFace,
  type ViewCubePart,
} from './view-cube'

type Matrix3 = readonly (readonly [number, number, number])[]

const DEGREE = Math.PI / 180

function multiply(first: Matrix3, second: Matrix3): Matrix3 {
  return [0, 1, 2].map((row) => {
    const values = [0, 1, 2].map((column) =>
      [0, 1, 2].reduce((sum, i) => sum + (first[row]?.[i] ?? 0) * (second[i]?.[column] ?? 0), 0),
    )
    return [values[0] ?? 0, values[1] ?? 0, values[2] ?? 0] as const
  })
}

function apply(matrix: Matrix3, vector: Vector3): Vector3 {
  const row = (index: number): number =>
    (matrix[index]?.[0] ?? 0) * vector[0] +
    (matrix[index]?.[1] ?? 0) * vector[1] +
    (matrix[index]?.[2] ?? 0) * vector[2]
  return [row(0), row(1), row(2)]
}

// CSS rotateX and rotateY in CSS axes (x right, y down, z toward the viewer).
function rotateX(angle: number): Matrix3 {
  const cosine = Math.cos(angle)
  const sine = Math.sin(angle)
  return [
    [1, 0, 0],
    [0, cosine, -sine],
    [0, sine, cosine],
  ]
}

function rotateY(angle: number): Matrix3 {
  const cosine = Math.cos(angle)
  const sine = Math.sin(angle)
  return [
    [cosine, 0, sine],
    [0, 1, 0],
    [-sine, 0, cosine],
  ]
}

const IDENTITY: Matrix3 = [
  [1, 0, 0],
  [0, 1, 0],
  [0, 0, 1],
]

// What Tailwind 4.3.3 emits for these classes: `transform: rotateX() rotateY()` and `translate` in the parent's frame, in rem.
function placement(classes: string): { readonly rotation: Matrix3; readonly translation: Vector3 } {
  let rotationX = IDENTITY
  let rotationY = IDENTITY
  const translation: [number, number, number] = [0, 0, 0]
  for (const name of classes.split(' ')) {
    const match = /^(-?)(rotate|translate)-([xyz])-(\d+)$/.exec(name)
    if (match === null) {
      throw new Error(`Unexpected placement class ${name}.`)
    }
    const [, sign, kind, axis, amount] = match
    const value = (sign === '-' ? -1 : 1) * Number(amount)
    const axisIndex = ['x', 'y', 'z'].indexOf(axis ?? '')
    if (kind === 'translate') {
      translation[axisIndex] = value * 0.25
    } else if (axis === 'x') {
      rotationX = rotateX(value * DEGREE)
    } else {
      rotationY = rotateY(value * DEGREE)
    }
  }
  return { rotation: multiply(rotationX, rotationY), translation }
}

function rotorRotation(face: CubeFace): Matrix3 {
  const matrix = viewCubeRotorMatrix(standardViewRotation(face))
  const at = (row: number, column: number): number => matrix[column * 4 + row] ?? Number.NaN
  return [0, 1, 2].map((row) => [at(row, 0), at(row, 1), at(row, 2)] as const)
}

function closeTo(values: readonly number[]): unknown[] {
  return values.map((value) => expect.closeTo(value, 6))
}

function partNamed(label: string): ViewCubePart {
  const found = VIEW_CUBE_PARTS.find((cubePart) => cubePart.label === label)
  if (found === undefined) {
    throw new Error(`No part named ${label}.`)
  }
  return found
}

function countOf(kind: ViewCubePart['kind']): number {
  return VIEW_CUBE_PARTS.filter((cubePart) => cubePart.kind === kind).length
}

function partOfFace(face: CubeFace): ViewCubePart {
  const found = partForFaces([face])
  if (found === null) {
    throw new Error(`No part for the ${face} face.`)
  }
  return found
}

function orientationOf(cubePart: ViewCubePart): { forward: Vector3; up: Vector3 } {
  const camera = { ...DEFAULT_CAMERA, rotation: viewCubeRotation(cubePart) }
  return { forward: forward(camera), up: up(camera) }
}

describe('view cube parts', () => {
  it('has 6 faces, 12 edges and 8 corners with distinct names', () => {
    expect([countOf('face'), countOf('edge'), countOf('corner')]).toEqual([6, 12, 8])
    expect(new Set(VIEW_CUBE_PARTS.map((cubePart) => cubePart.label)).size).toBe(26)
    expect(new Set(VIEW_CUBE_PARTS.map((cubePart) => cubePart.id)).size).toBe(26)
  })

  it('names faces "{Face} view"', () => {
    expect(
      VIEW_CUBE_PARTS.filter((cubePart) => cubePart.kind === 'face').map(({ label }) => label),
    ).toEqual(['Front view', 'Back view', 'Left view', 'Right view', 'Top view', 'Bottom view'])
  })

  it('names edges with their faces in the order Front, Back, Left, Right, Top, Bottom', () => {
    expect(
      VIEW_CUBE_PARTS.filter((cubePart) => cubePart.kind === 'edge').map(({ label }) => label),
    ).toEqual([
      'Front left edge view',
      'Front right edge view',
      'Front top edge view',
      'Front bottom edge view',
      'Back left edge view',
      'Back right edge view',
      'Back top edge view',
      'Back bottom edge view',
      'Left top edge view',
      'Left bottom edge view',
      'Right top edge view',
      'Right bottom edge view',
    ])
  })

  it('names corners "{Front or Back} {Left or Right} {Top or Bottom} corner view"', () => {
    expect(
      VIEW_CUBE_PARTS.filter((cubePart) => cubePart.kind === 'corner').map(({ label }) => label),
    ).toEqual([
      'Front left top corner view',
      'Front left bottom corner view',
      'Front right top corner view',
      'Front right bottom corner view',
      'Back left top corner view',
      'Back left bottom corner view',
      'Back right top corner view',
      'Back right bottom corner view',
    ])
  })

  it('finds a part by its faces in any order and none for opposite faces', () => {
    expect(partForFaces(['top', 'right', 'front'])?.label).toBe('Front right top corner view')
    expect(partForFaces(['front', 'back'])).toBeNull()
  })
})

describe('viewCubeRotation', () => {
  it.each(CUBE_FACES)('the %s face sets its standard view', (face) => {
    const rotation = viewCubeRotation(partOfFace(face))
    expect(classifyView({ ...DEFAULT_CAMERA, rotation })).toBe(face)
  })

  it('the front right top corner is the iso view', () => {
    const rotation = viewCubeRotation(partNamed('Front right top corner view'))
    expect(classifyView({ ...DEFAULT_CAMERA, rotation })).toBe('iso')
  })

  it('an edge looks from the normalized sum of its face directions with up the projection of +Z', () => {
    const { forward: edgeForward, up: edgeUp } = orientationOf(partNamed('Front top edge view'))
    expect(edgeForward).toEqual(closeTo([0, Math.SQRT1_2, -Math.SQRT1_2]))
    expect(edgeUp).toEqual(closeTo([0, Math.SQRT1_2, Math.SQRT1_2]))
  })

  it('a side edge keeps +Z up', () => {
    const { forward: edgeForward, up: edgeUp } = orientationOf(partNamed('Back left edge view'))
    expect(edgeForward).toEqual(closeTo([Math.SQRT1_2, -Math.SQRT1_2, 0]))
    expect(edgeUp).toEqual(closeTo([0, 0, 1]))
  })

  it('a bottom corner looks up from below with up the projection of +Z', () => {
    const { forward: cornerForward, up: cornerUp } = orientationOf(
      partNamed('Back left bottom corner view'),
    )
    const third = 1 / Math.sqrt(3)
    expect(cornerForward).toEqual(closeTo([third, -third, third]))
    expect(cornerUp[2]).toBeGreaterThan(0)
  })
})

describe('viewCubeRotorMatrix', () => {
  it('is the identity in the top view', () => {
    expect(Array.from(viewCubeRotorMatrix(standardViewRotation('top')))).toEqual(
      closeTo([1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1]),
    )
  })

  it('turns the front face toward the viewer and the top face up in the front view', () => {
    const rotor = rotorRotation('front')
    // The front face sits at rotor +y and the top face at rotor +z.
    expect(apply(rotor, [0, 1, 0])).toEqual(closeTo([0, 0, 1]))
    expect(apply(rotor, [0, 0, 1])).toEqual(closeTo([0, -1, 0]))
  })

  it('is a rotation for any camera', () => {
    const matrix = viewCubeRotorMatrix(DEFAULT_CAMERA.rotation)
    const column = (index: number): Vector3 => [
      matrix[index * 4] ?? 0,
      matrix[index * 4 + 1] ?? 0,
      matrix[index * 4 + 2] ?? 0,
    ]
    const [x, y, z] = [column(0), column(1), column(2)]
    const crossXY: Vector3 = [
      x[1] * y[2] - x[2] * y[1],
      x[2] * y[0] - x[0] * y[2],
      x[0] * y[1] - x[1] * y[0],
    ]
    expect(crossXY).toEqual(closeTo(z))
  })
})

describe('FACE_PLACEMENT_CLASSES', () => {
  it.each(CUBE_FACES)(
    'the %s face reads frontal and upright from its standard view, 3rem toward the viewer',
    (face) => {
      const rotor = rotorRotation(face)
      const { rotation, translation } = placement(FACE_PLACEMENT_CLASSES[face])
      const seen = multiply(rotor, rotation)
      expect(seen.flat()).toEqual(closeTo(IDENTITY.flat()))
      expect(apply(rotor, translation)).toEqual(closeTo([0, 0, 3]))
    },
  )

  it('moves faces out only along the 3D utilities 3.14 allows: translate-z-12 and translate-x or -y of the same 3rem', () => {
    for (const face of CUBE_FACES) {
      const translations = FACE_PLACEMENT_CLASSES[face]
        .split(' ')
        .filter((name) => name.includes('translate'))
      expect(translations).toHaveLength(1)
      expect(translations[0]).toMatch(/^-?translate-[xyz]-12$/)
    }
  })
})

describe('faceCells', () => {
  it('reads the front face as seen from the front view', () => {
    expect(faceCells('front').map((cell) => cell.part.label)).toEqual([
      'Front left top corner view',
      'Front top edge view',
      'Front right top corner view',
      'Front left edge view',
      'Front view',
      'Front right edge view',
      'Front left bottom corner view',
      'Front bottom edge view',
      'Front right bottom corner view',
    ])
  })

  it('reads the top face with back at the top, as seen from the top view', () => {
    const labels = faceCells('top').map((cell) => cell.part.label)
    expect(labels[1]).toBe('Back top edge view')
    expect(labels[3]).toBe('Left top edge view')
    expect(labels[7]).toBe('Front top edge view')
  })

  it('gives each cell its shape', () => {
    expect(faceCells('right').map((cell) => cell.shape)).toEqual([
      'corner',
      'horizontal',
      'corner',
      'vertical',
      'center',
      'vertical',
      'corner',
      'horizontal',
      'corner',
    ])
  })

  it('shows every edge on 2 faces, every corner on 3, and each part as a button exactly once', () => {
    const cells = CUBE_FACES.flatMap((face) => faceCells(face))
    for (const cubePart of VIEW_CUBE_PARTS) {
      const ofPart = cells.filter((cell) => cell.part === cubePart)
      expect(ofPart).toHaveLength(cubePart.faces.length)
      expect(ofPart.filter((cell) => cell.primary)).toHaveLength(1)
    }
  })

  it('puts the button of each part on its first face', () => {
    const buttons = faceCells('front').filter((cell) => cell.primary)
    expect(buttons).toHaveLength(9)
    const back = faceCells('top').filter((cell) => cell.primary)
    expect(back.map((cell) => cell.part.label)).toEqual(['Top view'])
  })
})
