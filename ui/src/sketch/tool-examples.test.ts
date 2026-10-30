import { describe, expect, it } from 'vitest'

import type { Document, SketchConstraint, SketchEntity } from '../generated/protocol'
import { readProjectFile } from '../test/project-file'
import { constraintFor } from './constraints'
import { drawClick, IDLE, type DrawState, type TypedValue, type TypedValues } from './drawing'
import { pointItem, SKETCH_ID, sequentialIds, solutionFor } from './fixtures.testing'
import type { Vector2 } from './geometry'
import type { DrawingTool } from './tools'

// U-247 (ui.md 6, 3.16.4): the clicks and typed values of features/sketch.md tests 44 to 50,
// replayed through drawClick, commit what each examples/sketch/tool-*.ptsm holds.

/** How far an entity's number may be from the example's: mm, or rad for arc angles. */
const TOLERANCE = 1e-9

/** One click of a replay, with the values typed into the draw inputs before it. */
type Click = { readonly at: Vector2; readonly typed?: TypedValues }

/** Clicks with a tool from `IDLE`, as Esc and a tool change leave it, or the Fix tool on a point. */
type ReplayStep =
  | { readonly tool: DrawingTool; readonly clicks: readonly Click[] }
  | { readonly fixPoint: string }

type ToolExample = {
  /** The features/sketch.md test of the example. */
  readonly test: number
  /** The example's stem under `examples/sketch/`. */
  readonly stem: string
  readonly steps: readonly ReplayStep[]
  /** The inferred constraints that the commit's probe leaves out (3.16.3 step 2), as the tool makes them. */
  readonly leftOut: readonly SketchConstraint[]
}

/** What a replay added to the sketch, in order. */
type Replayed = {
  readonly entities: readonly SketchEntity[]
  readonly constraints: readonly SketchConstraint[]
  /** The ids that the commits' changes name as inferred. */
  readonly inferred: ReadonlySet<string>
}

function typedValue(text: string, value: number): TypedValue {
  return { text, value }
}

const WIDTH_AND_HEIGHT: TypedValues = {
  Width: typedValue('60 mm', 60),
  Height: typedValue('40 mm', 40),
}

const LENGTH_40: TypedValues = { Length: typedValue('40 mm', 40) }

const TOOL_EXAMPLES: readonly ToolExample[] = [
  {
    test: 44,
    stem: 'tool-rectangle',
    steps: [
      {
        tool: 'rectangle_2point',
        clicks: [{ at: [0, 0] }, { at: [50, 30], typed: WIDTH_AND_HEIGHT }],
      },
    ],
    leftOut: [],
  },
  {
    test: 45,
    stem: 'tool-center-rectangle',
    steps: [
      {
        tool: 'rectangle_center',
        clicks: [{ at: [0, 0] }, { at: [50, 30], typed: WIDTH_AND_HEIGHT }],
      },
    ],
    leftOut: [],
  },
  {
    test: 46,
    stem: 'tool-circle-point-snap',
    steps: [
      { tool: 'point', clicks: [{ at: [12, 5] }] },
      { fixPoint: 'e-1' },
      { tool: 'circle', clicks: [{ at: [0, 0] }, { at: [12, 5] }] },
    ],
    leftOut: [],
  },
  {
    test: 47,
    stem: 'tool-arc',
    steps: [
      { tool: 'line', clicks: [{ at: [0, 0] }, { at: [35, 0], typed: LENGTH_40 }] },
      { tool: 'arc_3point', clicks: [{ at: [0, 0] }, { at: [40, 0] }, { at: [20, 20] }] },
    ],
    leftOut: [],
  },
  {
    test: 48,
    stem: 'tool-tangent',
    steps: [
      { tool: 'line', clicks: [{ at: [0, 0] }, { at: [35, 0], typed: LENGTH_40 }] },
      { tool: 'arc_3point', clicks: [{ at: [40, 0] }, { at: [40, 20] }, { at: [50, 10] }] },
      { tool: 'line', clicks: [{ at: [40, 20] }, { at: [0, 20] }, { at: [0, 0] }] },
    ],
    leftOut: [{ id: 'c-11', type: 'vertical', line: 'e-4' }],
  },
  {
    test: 49,
    stem: 'tool-angle',
    steps: [
      {
        tool: 'line',
        clicks: [
          { at: [0, 0] },
          {
            at: [40, 25],
            typed: {
              Length: typedValue('60 mm', 60),
              Angle: typedValue('30 deg', Math.PI / 6),
            },
          },
          { at: [0, 30] },
          { at: [0, 0] },
        ],
      },
    ],
    leftOut: [{ id: 'c-9', type: 'vertical', line: 'e-3' }],
  },
  {
    test: 50,
    stem: 'tool-diameter',
    steps: [
      {
        tool: 'circle',
        clicks: [{ at: [0, 0] }, { at: [10, 5], typed: { Diameter: typedValue('25 mm', 25) } }],
      },
    ],
    leftOut: [],
  },
]

/** Replays `steps` with one id source, construction off, 0.1 mm per pixel, UI scale 1 and no suppression. */
function replay(steps: readonly ReplayStep[]): Replayed {
  const ids = sequentialIds()
  const entities: SketchEntity[] = []
  const constraints: SketchConstraint[] = []
  const inferred = new Set<string>()
  for (const step of steps) {
    if ('fixPoint' in step) {
      const fix = constraintFor('fix', [pointItem(step.fixPoint)], solutionFor(entities), ids)
      if (fix === null) {
        throw new Error(`The Fix tool makes no constraint on ${step.fixPoint}.`)
      }
      constraints.push(fix)
      continue
    }
    let state: DrawState = IDLE
    for (const click of step.clicks) {
      // Each example comes back from the solver as stored, so the entities as drawn are the solution.
      const input = {
        cursor: click.at,
        millimetersPerPixel: 0.1,
        uiScale: 1,
        suppressed: false,
        solution: solutionFor(entities),
        typed: click.typed ?? {},
      }
      const { state: next, commit } = drawClick(step.tool, state, input, ids, false)
      state = next
      if (commit !== null) {
        entities.push(...(commit.change.add?.entities ?? []))
        constraints.push(...(commit.change.add?.constraints ?? []))
        for (const id of commit.change.inferred) {
          inferred.add(id)
        }
      }
    }
  }
  return { entities, constraints, inferred }
}

/** The entities and constraints of `f-sk1` in `examples/sketch/<stem>.ptsm`. */
async function exampleSketch(
  stem: string,
): Promise<{ entities: SketchEntity[]; constraints: SketchConstraint[] }> {
  const document: Document = JSON.parse(await readProjectFile(`/../examples/sketch/${stem}.ptsm`))
  const sketch = document.features?.find((feature) => feature.id === SKETCH_ID)
  if (sketch === undefined) {
    throw new Error(`examples/sketch/${stem}.ptsm has no ${SKETCH_ID}.`)
  }
  return {
    entities: sketch['entities'] as SketchEntity[],
    constraints: sketch['constraints'] as SketchConstraint[],
  }
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
}

/** Where `actual` differs from `expected`: a number by more than `TOLERANCE`, anything else at all. */
function differences(actual: unknown, expected: unknown, where: string): string[] {
  if (typeof actual === 'number' && typeof expected === 'number') {
    return Math.abs(actual - expected) <= TOLERANCE
      ? []
      : [`${where} is ${String(actual)}, expected ${String(expected)}`]
  }
  if (Array.isArray(actual) && Array.isArray(expected)) {
    if (actual.length !== expected.length) {
      return [`${where} has ${String(actual.length)} items, expected ${String(expected.length)}`]
    }
    return actual.flatMap((item: unknown, i) =>
      differences(item, expected[i], `${where}/${String(i)}`),
    )
  }
  if (isRecord(actual) && isRecord(expected)) {
    const keys = [...new Set([...Object.keys(actual), ...Object.keys(expected)])].toSorted()
    return keys.flatMap((key) => {
      if (!(key in actual)) {
        return [`${where}/${key} is missing`]
      }
      if (!(key in expected)) {
        return [`${where}/${key} is unexpected`]
      }
      return differences(actual[key], expected[key], `${where}/${key}`)
    })
  }
  return Object.is(actual, expected)
    ? []
    : [`${where} is ${JSON.stringify(actual)}, expected ${JSON.stringify(expected)}`]
}

describe('U-247 the drawing tools commit what their examples hold (3.16.4, features/sketch.md tests 44 to 50)', () => {
  it.each(TOOL_EXAMPLES)(
    'U-247 ($test) $stem: the entities are those of the example within 1e-9',
    async ({ stem, steps }) => {
      const example = await exampleSketch(stem)

      const replayed = replay(steps)

      expect(differences(replayed.entities, example.entities, 'entities')).toEqual([])
    },
  )

  it.each(TOOL_EXAMPLES)(
    'U-247 ($test) $stem: the constraints are those of the example, without the ones the probe leaves out',
    async ({ stem, steps, leftOut }) => {
      const example = await exampleSketch(stem)
      const leftOutIds = new Set(leftOut.map((constraint) => constraint.id))

      const replayed = replay(steps)

      expect(
        replayed.constraints.filter((constraint) => !leftOutIds.has(constraint.id)),
      ).toStrictEqual(example.constraints)
    },
  )

  it.each(TOOL_EXAMPLES.filter((example) => example.leftOut.length > 0))(
    'U-247 ($test) $stem: the constraints the probe leaves out are inferred ones the replay adds',
    ({ steps, leftOut }) => {
      const replayed = replay(steps)

      for (const constraint of leftOut) {
        expect(replayed.constraints).toContainEqual(constraint)
        expect(replayed.inferred.has(constraint.id)).toBe(true)
      }
    },
  )

  it('U-247 (46) the Fix tool adds c-1 on the point that the point tool committed', () => {
    const replayed = replay([{ tool: 'point', clicks: [{ at: [12, 5] }] }, { fixPoint: 'e-1' }])

    expect(replayed.constraints).toStrictEqual([{ id: 'c-1', type: 'fix', target: 'e-1' }])
  })
})

describe('the comparison of a replay with its example', () => {
  it('takes a number within 1e-9 of the example, as 29.999999999999996 for 30', () => {
    expect(
      differences([51.96152422706632, 29.999999999999996], [51.96152422706632, 30], 'end'),
    ).toEqual([])
  })

  it('reports a number further than 1e-9 from the example', () => {
    expect(differences({ radius: 13.000001 }, { radius: 13 }, 'e-2')).toEqual([
      'e-2/radius is 13.000001, expected 13',
    ])
  })

  it('reports a member the example does not have, such as a construction flag', () => {
    expect(differences({ id: 'e-1', construction: true }, { id: 'e-1' }, 'entities/0')).toEqual([
      'entities/0/construction is unexpected',
    ])
  })

  it('reports a different count of entities', () => {
    expect(differences([1, 2], [1], 'entities')).toEqual(['entities has 2 items, expected 1'])
  })

  it('compares anything but a number exactly', () => {
    expect(differences({ type: 'arc' }, { type: 'circle' }, 'e-2')).toEqual([
      'e-2/type is "arc", expected "circle"',
    ])
  })
})
