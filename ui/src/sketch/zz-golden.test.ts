// Temporary equivalence recorder for the sketch cleanup pass: records the outputs of the sketch
// functions and the sketch controller over seeded random inputs, to compare before and after.

import { afterEach, describe, expect, it, vi } from 'vitest'
import type { SketchConstraint, SketchEntity, SketchSolution } from '../generated/protocol'
import type { SelectionItem } from '../state/store'
import { dimensionGeometry, dimensionLabelText, glyphAnchor, hasGlyph } from './annotations'
import {
  applicableConstraints,
  CONSTRAINT_TYPES,
  constraintFor,
  constraintsFor,
  constraintTitle,
  isDimension,
  pickVerdict,
  type ConstraintType,
} from './constraints'
import { dimensionClick, dimensionPreview, DIMENSION_START } from './dimension-tool'
import {
  defaultLabelPosition,
  dimensionEnds,
  dimensionFor,
  dimensionTypeFor,
  measuredValue,
} from './dimensions'
import {
  drawClick,
  drawFields,
  drawPreview,
  IDLE,
  type DrawingTool,
  type DrawState,
} from './drawing'
import {
  arcEntity,
  circleEntity,
  lineEntity,
  pointEntity,
  sequentialIds,
  solutionFor,
  SKETCH_ID,
} from './fixtures.testing'
import { distancePointToArc, distancePointToSegment, type Vector2 } from './geometry'
import {
  entitySelectionItem,
  hitSelectionItem,
  hitTest,
  originSelectionItem,
  pointSelectionItem,
} from './hit'
import { infer, inferTangent } from './inference'
import { failedSolveMessage, issueRings, outlineProblems, profileFindingMessage } from './issues'
import { curveCircle, lineEnds, operandOf, pointPosition } from './operands'
import { millimetersPerPixelAt, pixelToSketch, sketchToPixel, worldBounds } from './placement'
import { arcThreePointRecipe, rectangleCenterRecipe, rectangleTwoPointRecipe } from './recipes'
import {
  dimensionPaths,
  drawInputsPlacement,
  entitiesInBox,
  entityPoints,
  glyphCenters,
  screenBox,
  shapePoints,
} from './screen'
import { createSketchController, type OverlayPaint, type SketchController } from './session'
import {
  clickAt,
  dragFrom,
  mountView,
  pixelOf,
  pointerAt,
  pressKey,
  settleCommits,
  sketchHarness,
  TOP_CAMERA,
  type TestView,
} from './view.testing'
import { orbit } from '../viewport/camera'

const OUT =
  (globalThis as unknown as { process?: { env: Record<string, string | undefined> } }).process?.env[
    'GOLDEN_OUT'
  ] ?? ''

type FileSystem = {
  mkdirSync(path: string, options: { recursive: boolean }): void
  writeFileSync(path: string, text: string): void
}

function random(seed: number): () => number {
  let state = seed >>> 0
  return () => {
    state = (state + 0x6d2b79f5) >>> 0
    let value = state
    value = Math.imul(value ^ (value >>> 15), value | 1)
    value ^= value + Math.imul(value ^ (value >>> 7), value | 61)
    return ((value ^ (value >>> 14)) >>> 0) / 4294967296
  }
}

function encode(value: unknown): string {
  return JSON.stringify(value, (_key, entry: unknown) => {
    if (typeof entry === 'number') {
      if (Object.is(entry, -0)) {
        return '-0'
      }
      if (!Number.isFinite(entry)) {
        return String(entry)
      }
      return entry
    }
    if (entry instanceof Float32Array) {
      return Array.from(entry)
    }
    if (entry instanceof Map) {
      return Array.from(entry.entries())
    }
    if (entry instanceof Set) {
      return Array.from(entry.values())
    }
    return entry
  })
}

const lines: string[] = []

function record(name: string, value: unknown): void {
  let text: string
  try {
    text = encode(value)
  } catch (error) {
    text = `throws ${String(error)}`
  }
  lines.push(`${name} ${text}`)
}

function attempt(name: string, run: () => unknown): void {
  try {
    record(name, run())
  } catch (error) {
    lines.push(`${name} throws ${error instanceof Error ? error.message : String(error)}`)
  }
}

type Rng = () => number

function coordinate(rng: Rng): number {
  const roll = rng()
  if (roll < 0.15) {
    return Math.round(rng() * 40 - 20)
  }
  if (roll < 0.2) {
    return 0
  }
  return rng() * 80 - 40
}

function point(rng: Rng): [number, number] {
  return [coordinate(rng), coordinate(rng)]
}

function entities(rng: Rng): SketchEntity[] {
  const count = 1 + Math.floor(rng() * 6)
  const result: SketchEntity[] = []
  for (let i = 0; i < count; i += 1) {
    const id = `e-${String(i + 1)}`
    const roll = rng()
    if (roll < 0.4) {
      const start = point(rng)
      const end: Vector2 =
        rng() < 0.2
          ? [start[0], coordinate(rng)]
          : rng() < 0.2
            ? [coordinate(rng), start[1]]
            : point(rng)
      result.push(lineEntity(id, start, end, rng() < 0.1))
    } else if (roll < 0.6) {
      result.push(circleEntity(id, point(rng), 1 + rng() * 25))
    } else if (roll < 0.8) {
      result.push(arcEntity(id, point(rng), 1 + rng() * 25, rng() * 7 - 0.5, rng() * 7 - 0.5))
    } else {
      result.push(pointEntity(id, point(rng), rng() < 0.2))
    }
  }
  return result
}

function itemsOf(solution: SketchSolution): SelectionItem[] {
  return [
    ...solution.entities.map((entity) => entitySelectionItem(SKETCH_ID, entity)),
    ...solution.points.map((solved) => pointSelectionItem(SKETCH_ID, solved.point)),
    originSelectionItem(SKETCH_ID, 'point'),
    originSelectionItem(SKETCH_ID, 'x_axis'),
    originSelectionItem(SKETCH_ID, 'y_axis'),
  ]
}

function pick<Value>(rng: Rng, values: readonly Value[]): Value | undefined {
  return values[Math.floor(rng() * values.length)]
}

function selectionOf(rng: Rng, items: readonly SelectionItem[]): SelectionItem[] {
  const count = Math.floor(rng() * 4)
  const result: SelectionItem[] = []
  for (let i = 0; i < count; i += 1) {
    const item = pick(rng, items)
    if (item !== undefined) {
      result.push(item)
    }
  }
  return result
}

const DIMENSION_TYPES = [
  'distance',
  'horizontal_distance',
  'vertical_distance',
  'radius',
  'diameter',
  'angle',
] as const

const TOOLS: readonly DrawingTool[] = [
  'line',
  'rectangle_2point',
  'rectangle_center',
  'circle',
  'arc_3point',
  'point',
]

const project = (point: Vector2): Vector2 | null =>
  point[0] > 1e5 ? null : [512 + point[0] * 8, 512 - point[1] * 8]

describe.skipIf(OUT === '')('golden', () => {
  it('records the pure functions', { timeout: 120_000 }, () => {
    const rng = random(12345)
    for (let round = 0; round < 400; round += 1) {
      const stored = entities(rng)
      const base = solutionFor(stored)
      const solution: SketchSolution = {
        ...base,
        profile_findings:
          rng() < 0.3
            ? [
                { type: 'open_endpoint', entity: 'e-1', point: point(rng) },
                {
                  type: 'overlap',
                  entities: ['e-1', 'e-2'],
                  start: point(rng),
                  end: point(rng),
                },
                { type: 'self_intersection', entities: ['e-1', 'e-2'], point: point(rng) },
                { type: 'zero_length', entity: 'e-2' },
              ]
            : [],
      }
      const items = itemsOf(solution)
      const tag = `r${String(round)}`
      record(
        `${tag} items`,
        items.map((item) => [item.key, operandOf(item)]),
      )
      for (let probe = 0; probe < 6; probe += 1) {
        const cursor = point(rng)
        const scale = 0.01 + rng() * 0.5
        attempt(`${tag} hit${String(probe)}`, () => [
          hitTest(solution, cursor, {
            millimetersPerPixel: scale,
            uiScale: 1,
            includeOrigin: true,
          }),
          hitTest(solution, cursor, {
            millimetersPerPixel: scale,
            uiScale: 2,
            includeOrigin: false,
          }),
        ])
        const hit = hitTest(solution, cursor, {
          millimetersPerPixel: scale * 10,
          uiScale: 1,
          includeOrigin: true,
        })
        attempt(`${tag} hititem${String(probe)}`, () =>
          hit === null ? null : hitSelectionItem(SKETCH_ID, hit, solution),
        )
        const anchorPoint = pick(rng, solution.points)
        attempt(`${tag} infer${String(probe)}`, () => [
          infer({
            cursor,
            millimetersPerPixel: scale * 20,
            solution,
            anchor: null,
            tool: 'circle',
            suppressed: false,
          }),
          infer({
            cursor,
            millimetersPerPixel: scale * 20,
            solution,
            anchor:
              anchorPoint === undefined ? null : { at: anchorPoint.at, point: anchorPoint.point },
            tool: 'line',
            suppressed: false,
          }),
          infer({
            cursor,
            millimetersPerPixel: scale,
            solution,
            anchor: { at: point(rng), point: null },
            tool: 'line',
            suppressed: rng() < 0.2,
          }),
          anchorPoint === undefined
            ? null
            : inferTangent(solution, anchorPoint.point, point(rng), rng() < 0.5),
        ])
        for (const tool of TOOLS) {
          const first = { at: point(rng), snap: null }
          const states: DrawState[] = [
            IDLE,
            {
              stage: 'line',
              start: first,
              previousEnd: rng() < 0.5 ? null : (anchorPoint?.point ?? null),
              chainStart: anchorPoint === undefined ? [] : [anchorPoint.point],
            },
            { stage: 'second', first },
            { stage: 'third', first, second: { at: point(rng), snap: null } },
          ]
          for (const [index, state] of states.entries()) {
            const typedRoll = rng()
            const input = {
              cursor,
              millimetersPerPixel: scale,
              uiScale: 1 + Math.floor(rng() * 2),
              suppressed: rng() < 0.1,
              solution,
              typed:
                typedRoll < 0.6
                  ? {}
                  : typedRoll < 0.7
                    ? { Length: { text: '12', value: 12 }, Angle: { text: '30', value: 0.5 } }
                    : typedRoll < 0.8
                      ? { Width: { text: '10', value: 10 }, Height: { text: '7', value: 7 } }
                      : typedRoll < 0.9
                        ? { Diameter: { text: '9', value: 9 }, Radius: { text: '30', value: 30 } }
                        : {
                            Angle: { text: '90', value: Math.PI / 2 },
                            Radius: { text: '1', value: 1 },
                          },
            }
            attempt(`${tag} draw ${tool} ${String(probe)} ${String(index)}`, () => [
              drawFields(tool, state),
              drawPreview(tool, state, input),
              drawClick(tool, state, input, sequentialIds(), rng() < 0.5),
            ])
          }
        }
        attempt(`${tag} recipes ${String(probe)}`, () => [
          arcThreePointRecipe({
            start: cursor,
            end: point(rng),
            through: point(rng),
            construction: false,
            ids: sequentialIds(),
          }),
          rectangleTwoPointRecipe({
            corner: cursor,
            opposite: point(rng),
            construction: true,
            ids: sequentialIds(),
          }),
          rectangleCenterRecipe({
            center: cursor,
            corner: point(rng),
            construction: false,
            ids: sequentialIds(),
          }),
        ])
        const selection = selectionOf(rng, items)
        const labelAt = point(rng)
        attempt(`${tag} dimtype ${String(probe)}`, () =>
          dimensionTypeFor(selection, labelAt, solution),
        )
        for (const type of DIMENSION_TYPES) {
          attempt(`${tag} measured ${type} ${String(probe)}`, () =>
            measuredValue(type, selection, labelAt, solution),
          )
          attempt(`${tag} dimfor ${type} ${String(probe)}`, () => [
            dimensionFor(type, selection, '5', rng() < 0.5, labelAt, sequentialIds(), solution),
            dimensionFor(type, selection, 'a', true, labelAt, sequentialIds()),
          ])
        }
        attempt(`${tag} constraints ${String(probe)}`, () => [
          applicableConstraints(selection, solution),
          CONSTRAINT_TYPES.map((type: ConstraintType) => [
            constraintsFor(type, selection, solution, sequentialIds()),
            constraintFor(type, selection, solution, sequentialIds()),
            pickVerdict(type, selection, solution),
          ]),
        ])
        let picking = DIMENSION_START
        for (let click = 0; click < 4; click += 1) {
          const item = rng() < 0.2 ? null : (pick(rng, items) ?? null)
          const at = point(rng)
          const result = dimensionClick(picking, item, at, solution)
          record(`${tag} dimclick ${String(probe)} ${String(click)}`, [
            result,
            dimensionPreview(picking, at, solution),
          ])
          picking = result.picking
        }
        attempt(`${tag} screen ${String(probe)}`, () => [
          entitiesInBox(
            solution,
            project,
            screenBox([rng() * 1024, rng() * 1024], [rng() * 1024, rng() * 1024]),
            rng() < 0.5 ? 'window' : 'crossing',
            0.05,
          ),
          drawInputsPlacement(cursor, rng() < 0.2 ? null : point(rng), 12),
          solution.entities.map((entity) => entityPoints(entity, 0.1 + rng())),
          shapePoints(
            { kind: 'arc', center: cursor, radius: 5, start: rng() * 6, end: rng() * 9 },
            0.02,
          ),
        ])
      }
      // Constraints and dimensions over the solution.
      const constraints: SketchConstraint[] = []
      for (let i = 0; i < 8; i += 1) {
        const selection = selectionOf(rng, items)
        const type = pick(rng, CONSTRAINT_TYPES)
        if (type !== undefined) {
          constraints.push(
            ...constraintsFor(type, selection, solution, { next: () => `g-${String(i)}` }),
          )
        }
        const dimensionType = pick(rng, DIMENSION_TYPES)
        if (dimensionType !== undefined) {
          try {
            const built = dimensionFor(
              dimensionType,
              selection,
              rng() < 0.5 ? '12.5' : 'width * 2',
              rng() < 0.7,
              point(rng),
              { next: () => `d-${String(i)}` },
              solution,
            )
            constraints.push(built)
          } catch {
            // Not dimensionable.
          }
        }
      }
      const valued: SketchSolution = {
        ...solution,
        constraints: constraints.map((constraint) => ({
          id: constraint.id,
          state: 'satisfied',
          ...(isDimension(constraint) && rng() < 0.7 ? { value: rng() * 40 } : {}),
        })),
      }
      for (const constraint of constraints) {
        const labelAt = point(rng)
        attempt(`${tag} constraint ${constraint.id}`, () => [
          constraint,
          hasGlyph(constraint),
          glyphAnchor(constraint, valued),
          constraintTitle(constraint, valued, 'mm'),
          constraintTitle(constraint, null, 'in'),
          isDimension(constraint)
            ? [
                dimensionLabelText(constraint, rng() < 0.5 ? null : rng() * 30, 'mm'),
                dimensionLabelText(constraint, rng() * 3, 'in'),
              ]
            : null,
          dimensionGeometry(constraint, valued, labelAt),
          dimensionPaths(dimensionGeometry(constraint, valued, labelAt), project, 0.05, 8),
          defaultLabelPosition(constraint, valued, 0.1 + rng()),
          defaultLabelPosition(constraint, solution, 0.1),
          dimensionEnds(constraint, valued),
        ])
      }
      attempt(`${tag} glyphs`, () =>
        glyphCenters(constraints.filter(hasGlyph), valued, project, 1 + Math.floor(rng() * 2)),
      )
      attempt(`${tag} issues`, () => [
        issueRings(solution),
        outlineProblems(solution),
        solution.profile_findings.map(profileFindingMessage),
        failedSolveMessage({ ...solution, conflicting: rng() < 0.5 ? ['c-1'] : ['c-1', 'c-2'] }),
      ])
      attempt(`${tag} operands`, () => [
        solution.entities.map((entity) => curveCircle(solution, entity.id)),
        solution.entities.map((entity) => lineEnds(solution, entity.id)),
        solution.points.map((solved) => pointPosition(solution, solved.point)),
        distancePointToSegment(point(rng), point(rng), point(rng)),
        distancePointToArc(point(rng), point(rng), 3, rng() * 6, rng() * 9),
      ])
    }
    const camera = { ...orbit(TOP_CAMERA, 60, 40, [0, 0, 0]), projection: 'perspective' as const }
    const rngPlacement = random(99)
    for (let i = 0; i < 200; i += 1) {
      const pixel: Vector2 = [rngPlacement() * 1024, rngPlacement() * 1024]
      const placement = solutionFor([]).placement
      attempt(`placement ${String(i)}`, () => [
        pixelToSketch(camera, { width: 1024, height: 768 }, placement, pixel),
        sketchToPixel(camera, { width: 1024, height: 768 }, placement, pixel),
        millimetersPerPixelAt(camera, { width: 1024, height: 768 }, placement, pixel),
        worldBounds(placement, [pixel, [pixel[1], -pixel[0]]], rngPlacement()),
      ])
    }
    expect(lines.length).toBeGreaterThan(0)
  })

  let view: TestView | null = null
  let controller: SketchController | null = null

  afterEach(() => {
    controller?.dispose()
    controller = null
    view?.remove()
    view = null
  })

  it('records the controller', { timeout: 120_000 }, async () => {
    let counter = 0
    vi.spyOn(crypto, 'getRandomValues').mockImplementation(
      <T extends ArrayBufferView | null>(array: T): T => {
        if (array instanceof Uint8Array) {
          for (let i = 0; i < array.length; i += 1) {
            counter += 1
            array[i] = (counter * 37) % 256
          }
        }
        return array
      },
    )
    const stored: SketchEntity[] = [
      lineEntity('e-1', [0, 0], [20, 0]),
      lineEntity('e-2', [20, 0], [20, 15]),
      circleEntity('e-3', [-15, 10], 6),
      arcEntity('e-4', [10, -15], 8, 0.2, 2.5),
      pointEntity('e-5', [-10, -10]),
      lineEntity('e-6', [-30, 20], [-5, 28]),
    ]
    const constraints: SketchConstraint[] = [
      { id: 'c-1', type: 'horizontal', line: 'e-1' },
      { id: 'c-2', type: 'distance', line: 'e-1', expr: '20' },
      { id: 'c-3', type: 'vertical', line: 'e-2' },
      { id: 'c-4', type: 'radius', curve: 'e-4', expr: '8' },
      { id: 'c-5', type: 'diameter', curve: 'e-3', expr: 'd * 2', driving: false },
      { id: 'c-6', type: 'angle', lines: ['e-1', 'e-6'], expr: '15' },
      { id: 'c-7', type: 'horizontal_distance', points: ['e-5', 'e-1.end'], expr: '30' },
      { id: 'c-8', type: 'vertical_distance', points: ['e-5', 'e-6.start'], expr: '30' },
      { id: 'c-9', type: 'distance', point: 'e-5', line: 'e-6', expr: '5' },
      { id: 'c-10', type: 'tangent', curves: ['e-6', 'e-3'] },
      { id: 'c-11', type: 'fix', target: 'e-5' },
      { id: 'c-12', type: 'parallel', lines: ['e-1', 'e-6'] },
      { id: 'c-13', type: 'equal', curves: ['e-3', 'e-4'] },
      { id: 'c-14', type: 'coincident', points: ['e-1.end', 'e-2.start'] },
    ]
    const base = solutionFor(stored)
    const solution: SketchSolution = {
      ...base,
      constraints: constraints.map((constraint, index) => ({
        id: constraint.id,
        state: index % 5 === 3 ? 'redundant' : 'satisfied',
        ...(isDimension(constraint) && index !== 6 ? { value: 3 + index } : {}),
      })),
      profile_findings: [{ type: 'open_endpoint', entity: 'e-6', point: [-5, 28] }],
    }
    const harness = sketchHarness({ entities: stored, constraints, solution })
    view = mountView()
    const paints: OverlayPaint[] = []
    const created = createSketchController({ app: harness.app, canvas: view.canvas })
    controller = created
    created.setPainter((paint) => {
      paints.push(paint)
    })
    const canvas = view.canvas
    let step = 0
    const snapshot = (name: string): void => {
      created.repaint()
      record(`ctl ${String(step)} ${name}`, [created.getModel(), paints.at(-1) ?? null])
      step += 1
    }
    const setTool = (tool: string | null): void => {
      harness.app.store.setState((state) =>
        state.sketch === null
          ? {}
          : { sketch: { ...state.sketch, tool: tool as never, selection: [] } },
      )
    }
    const rng = random(777)
    const tools = [
      'line',
      'rectangle_2point',
      'rectangle_center',
      'circle',
      'arc_3point',
      'point',
      'dimension',
      'constrain_coincident',
      'constrain_horizontal',
      'constrain_tangent',
      'constrain_fix',
      null,
    ]
    snapshot('start')
    for (const tool of tools) {
      setTool(tool)
      snapshot(`tool ${String(tool)}`)
      for (let i = 0; i < 12; i += 1) {
        const at = point(rng)
        const roll = rng()
        if (roll < 0.45) {
          pointerAt(canvas, 'pointermove', at, { ctrlKey: rng() < 0.1 })
          snapshot(`move ${String(tool)}`)
        } else if (roll < 0.8) {
          clickAt(canvas, at, { shiftKey: rng() < 0.1 })
          await settleCommits()
          snapshot(`click ${String(tool)}`)
        } else if (roll < 0.85) {
          pressKey(String(Math.floor(rng() * 9) + 1))
          await settleCommits()
          snapshot(`type ${String(tool)}`)
        } else if (roll < 0.9) {
          pressKey('Enter')
          await settleCommits()
          snapshot(`enter ${String(tool)}`)
        } else if (roll < 0.95) {
          pressKey('Escape')
          snapshot(`escape ${String(tool)}`)
        } else {
          dragFrom(canvas, at, point(rng))
          await settleCommits()
          snapshot(`drag ${String(tool)}`)
        }
      }
      const model = created.getModel()
      if (model.editor !== null) {
        if (model.editor.round !== null) {
          created.setEditorRound(model.editor.round === 'radius' ? 'diameter' : 'radius')
          snapshot('round')
        }
        created.setEditorText('7')
        snapshot('editor text')
        await created.commitEditor()
        await settleCommits()
        snapshot('editor commit')
        created.closeEditor()
      }
      if (model.drawInputs.length > 0) {
        const [first] = model.drawInputs
        if (first !== undefined) {
          created.setDrawText(first.name, '6')
          await settleCommits()
          snapshot('draw text')
          await created.commitDrawInputs()
          await settleCommits()
          snapshot('draw commit')
        }
      }
    }
    setTool(null)
    for (const id of ['c-1', 'c-2', 'c-6', 'c-9', 'c-10', 'c-13', 'c-14']) {
      created.hoverConstraint(id)
      snapshot(`hover ${id}`)
      created.selectConstraint(id, rng() < 0.3)
      snapshot(`select ${id}`)
    }
    created.hoverConstraint(null)
    for (const id of ['c-2', 'c-5', 'c-6', 'c-7', 'c-8', 'c-9']) {
      created.editDimension(id)
      snapshot(`edit ${id}`)
      created.setEditorReference(true)
      snapshot(`reference ${id}`)
      created.closeEditor()
    }
    // Box selection both ways and a point drag.
    dragFrom(canvas, [-40, -40], [40, 40])
    snapshot('window')
    dragFrom(canvas, [40, 40], [-40, -40])
    snapshot('crossing')
    pointerAt(canvas, 'pointerdown', [-30, 20])
    pointerAt(canvas, 'pointermove', [-28, 22], { buttons: 1 })
    snapshot('drag move')
    pointerAt(canvas, 'pointerup', [-28, 22])
    await settleCommits()
    snapshot('drag end')
    pointerAt(canvas, 'pointerdown', [33, -33])
    pointerAt(canvas, 'pointermove', [45, -20], { buttons: 1 })
    snapshot('box move')
    pointerAt(canvas, 'pointermove', [25, -45], { buttons: 1 })
    snapshot('box move back')
    pressKey('Escape')
    snapshot('box escape')
    record('pixel', pixelOf([1, 2]))
    record('applied', harness.applied)
    record('probes', harness.probes)
    record('state', harness.app.store.getState().sketch)
    if (OUT !== '') {
      const fileSystemModule = 'node:fs'
      const { mkdirSync, writeFileSync } = (await import(
        /* @vite-ignore */ fileSystemModule
      )) as FileSystem
      mkdirSync(OUT.slice(0, OUT.lastIndexOf('/')), { recursive: true })
      writeFileSync(OUT, `${lines.join('\n')}\n`)
    }
  })
})
