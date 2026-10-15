import type { WorldAxis } from '../core/labels'
import { formatNumber } from '../expression/format'
import type { PointRef, SketchConstraint, SketchSolution, Units } from '../generated/protocol'
import type { SketchAxisSpans } from '../viewport/sketch-drawables'
import { dimensionGeometry, dimensionLabelText, hasGlyph } from './annotations'
import { constraintReferences, isDimension } from './constraints'
import { defaultLabelPosition } from './dimensions'
import { DRAW_FIELD_NAMES, FIELD_UNITS, type DrawFieldName, type DrawPreview } from './drawing'
import type { Vector2 } from './geometry'
import { issueRings } from './issues'
import { ownerOf, pointPosition, solvedEntity } from './operands'
import { isAxis, ORIGIN_POINT } from './refs'
import {
  axisLabelPosition,
  besideAxis,
  circlePath,
  dimensionPaths,
  drawInputsPlacement,
  entityPoints,
  glyphCenters,
  insetBox,
  insideBox,
  polylinePath,
  projectPolyline,
  screenBox,
  shapePoints,
  type DrawInputsPlacement,
  type Projector,
  type ScreenBox,
} from './screen'

// What the sketch overlay paints each frame (ui.md 3.12, 3.16.2): from a snapshot of the edited
// sketch, the view and the operation in progress, where each overlay element goes and the SVG path
// data it draws, in canvas CSS pixels. Pure, so a paint depends on nothing but its snapshot.

/** Dimension arrowheads are this many CSS pixels long (3.16.2). */
const ARROW_PIXELS = 8
/** Radius of point markers in CSS pixels (times the UI scale). */
const MARKER_PIXELS = 5
/** Inference glyphs sit this many CSS pixels (times the UI scale) right of and below the cursor. */
const INFERENCE_OFFSET_PIXELS = 16
/** Draw inputs sit this many CSS pixels (times the UI scale) off the rubber band's midpoint. */
const DRAW_INPUT_OFFSET_PIXELS = 12
/** Axis labels sit this many CSS pixels (times the UI scale), 1rem, inside the free area (3.16.2). */
const AXIS_LABEL_INSET_PIXELS = 16
/**
 * Sketch axis labels sit this many CSS pixels (times the UI scale), 0.75rem, beside their axis:
 * half a chip and 0.25rem, so the chip clears the dashed line and what touches it (3.16.2).
 */
const AXIS_LABEL_OFFSET_PIXELS = 12
/** Draw inputs and the dimension editor keep this many CSS pixels (times the UI scale), 0.5rem, inside the free area (3.12). */
const TYPING_INSET_PIXELS = 8

/** Where the overlay's elements go this frame, in canvas CSS pixels (3.12 runtime positions). */
export type OverlayPaint = {
  /** Center of each constraint glyph by constraint id; null hides it. */
  readonly glyphs: ReadonlyMap<string, Vector2 | null>
  /** Center of each dimension label by constraint id; null hides it. */
  readonly labels: ReadonlyMap<string, Vector2 | null>
  /** Text of each dimension label by constraint id: its solved value changes during a drag. */
  readonly labelTexts: ReadonlyMap<string, string>
  /** Center of each profile issue ring, in `issueRings` order. */
  readonly issues: readonly (Vector2 | null)[]
  /** SVG path data: dimension lines (stroked) and their arrowheads (filled). */
  readonly dimensionLines: string
  readonly dimensionArrows: string
  /** SVG path data of the rubber band. */
  readonly rubberBand: string
  /** SVG path data stroked in `highlight`: the snap curve and the hovered constraint's curves. */
  readonly highlight: string
  /** SVG path data of point markers in `highlight`: the snap point and constrained points. */
  readonly markers: string
  /** Top left of the inference glyph row and the dimension editor. */
  readonly inference: Vector2 | null
  /** Where the draw inputs go, beside the rubber band and away from the pointer. */
  readonly drawInputs: DrawInputsPlacement | null
  readonly editor: Vector2 | null
  /** Center of the label of the dimension being placed. */
  readonly placing: Vector2 | null
  readonly box: ScreenBox | null
  /** The rubber band's quantities as text, shown in empty draw inputs. */
  readonly measures: Readonly<Partial<Record<DrawFieldName, string>>>
  /** Center of the label of each world axis the sketch's axes lie on; null hides it. */
  readonly axisLabels: ReadonlyMap<WorldAxis, Vector2 | null>
  /** The box the draw inputs and the dimension editor stay inside: the free area, 0.5rem in (3.12). */
  readonly typingArea: ScreenBox
}

export const EMPTY_PAINT: OverlayPaint = {
  glyphs: new Map(),
  labels: new Map(),
  labelTexts: new Map(),
  issues: [],
  dimensionLines: '',
  dimensionArrows: '',
  rubberBand: '',
  highlight: '',
  markers: '',
  inference: null,
  drawInputs: null,
  editor: null,
  placing: null,
  box: null,
  measures: {},
  axisLabels: new Map(),
  typingArea: { left: 0, top: 0, right: 0, bottom: 0 },
}

/** A dimension not stored yet, drawn while it is placed or edited, with its label position. */
export type DraftDimension = {
  readonly constraint: SketchConstraint
  /** Where its label is, in sketch coordinates. */
  readonly labelAt: Vector2
}

/** What one paint reads: the edited sketch, the view and the operation in progress. */
export type PaintSnapshot = {
  readonly solution: SketchSolution
  readonly project: Projector
  readonly uiScale: number
  readonly units: Units
  /** Sketch millimeters per CSS pixel at the canvas center, which offsets default labels. */
  readonly pixelSize: number
  /** The chord tolerance of curves, in sketch millimeters. */
  readonly tolerance: number
  /** The sketch's stored constraints, in stored order. */
  readonly constraints: readonly SketchConstraint[]
  readonly showConstraints: boolean
  /** Labels placed this session by constraint id; the others go to their default position. */
  readonly labelPositions: ReadonlyMap<string, Vector2>
  readonly pointer: Vector2 | null
  /** The drawing tool's rubber band, or null. */
  readonly drawn: DrawPreview | null
  /** The dimension the dimension tool places at the pointer, or null. */
  readonly placing: DraftDimension | null
  /** The open dimension editor: where it is in sketch coordinates, and the new dimension it drafts. */
  readonly editor: { readonly at: Vector2; readonly draft: DraftDimension | null } | null
  /** The constraint whose items are highlighted, or null. */
  readonly hovered: string | null
  /** The selection box being dragged, between two canvas pixels, or null. */
  readonly box: { readonly start: Vector2; readonly end: Vector2 } | null
  /** The world axes along the sketch's x and y axes on an origin plane, else null (3.16.2). */
  readonly worldAxes: readonly [WorldAxis, WorldAxis] | null
  /** Where the scene draws the sketch's axes. */
  readonly axisSpans: SketchAxisSpans
  /** The free area (3.2) in canvas pixels: elements anchored outside it are hidden, and the axis labels and typed inputs stay inside it. */
  readonly freeArea: ScreenBox
}

/** The point references and curve ids a constraint names, split into the two. */
function constrainedItems(
  constraint: SketchConstraint,
  solution: SketchSolution,
): { readonly points: readonly PointRef[]; readonly curves: readonly string[] } {
  const points: PointRef[] = []
  const curves: string[] = []
  for (const reference of constraintReferences(constraint)) {
    const owner = ownerOf(reference)
    const entity = solvedEntity(solution, owner)
    if (reference !== owner || reference === ORIGIN_POINT || entity?.type === 'point') {
      points.push(reference)
    } else if (!isAxis(reference)) {
      curves.push(reference)
    }
  }
  return { points, curves }
}

/** The constraint glyphs' centers, hidden outside the free area, or none while constraints are hidden. */
function glyphLayer(snapshot: PaintSnapshot): OverlayPaint['glyphs'] {
  if (!snapshot.showConstraints) {
    return new Map<string, Vector2 | null>()
  }
  const centers = glyphCenters(
    snapshot.constraints.filter(hasGlyph),
    snapshot.solution,
    snapshot.project,
    snapshot.uiScale,
  )
  return new Map(
    [...centers].map(([id, center]) => [id, insideBox(center, snapshot.freeArea)] as const),
  )
}

/**
 * The stored dimensions' labels and texts, and the lines and arrowheads of every dimension drawn:
 * the stored ones, then the one being placed, then the one the editor drafts.
 */
export function dimensionLayer(
  snapshot: PaintSnapshot,
): Pick<OverlayPaint, 'labels' | 'labelTexts' | 'dimensionLines' | 'dimensionArrows'> {
  const { solution, project } = snapshot
  const labels = new Map<string, Vector2 | null>()
  const labelTexts = new Map<string, string>()
  const values = new Map(solution.constraints.map((entry) => [entry.id, entry.value ?? null]))
  const lines: string[] = []
  const arrows: string[] = []
  function addDimension(constraint: SketchConstraint, labelAt: Vector2): void {
    const paths = dimensionPaths(
      dimensionGeometry(constraint, solution, labelAt),
      project,
      snapshot.tolerance,
      ARROW_PIXELS,
    )
    lines.push(paths.lines)
    arrows.push(paths.arrows)
  }
  for (const constraint of snapshot.constraints) {
    if (!isDimension(constraint)) {
      continue
    }
    const labelAt =
      snapshot.labelPositions.get(constraint.id) ??
      defaultLabelPosition(constraint, solution, snapshot.pixelSize)
    labels.set(constraint.id, insideBox(project(labelAt), snapshot.freeArea))
    labelTexts.set(
      constraint.id,
      dimensionLabelText(constraint, values.get(constraint.id) ?? null, snapshot.units),
    )
    addDimension(constraint, labelAt)
  }
  for (const draft of [snapshot.placing, snapshot.editor?.draft ?? null]) {
    if (draft !== null) {
      addDimension(draft.constraint, draft.labelAt)
    }
  }
  return {
    labels,
    labelTexts,
    dimensionLines: lines.filter((path) => path !== '').join(' '),
    dimensionArrows: arrows.filter((path) => path !== '').join(' '),
  }
}

/** The rubber band, and in `highlight` its snap and the hovered constraint's curves and points. */
export function highlightLayer(
  snapshot: PaintSnapshot,
): Pick<OverlayPaint, 'rubberBand' | 'highlight' | 'markers'> {
  const { solution, project, tolerance, drawn } = snapshot
  const highlight: Vector2[][] = []
  const markers: Vector2[] = []
  const rubberBand =
    drawn === null
      ? ''
      : polylinePath(
          drawn.shapes.flatMap((shape) => projectPolyline(shapePoints(shape, tolerance), project)),
        )
  if (drawn?.snap !== null && drawn?.snap !== undefined) {
    const marker = project(drawn.snap.at)
    if (marker !== null) {
      markers.push(marker)
    }
    const snapped = drawn.snap.kind === 'curve' ? solvedEntity(solution, drawn.snap.entity) : null
    if (snapped !== null && snapped !== undefined) {
      highlight.push(...projectPolyline(entityPoints(snapped, tolerance), project))
    }
  }
  const hovered =
    snapshot.hovered === null
      ? undefined
      : snapshot.constraints.find((constraint) => constraint.id === snapshot.hovered)
  if (hovered !== undefined) {
    const items = constrainedItems(hovered, solution)
    for (const curve of items.curves) {
      const entity = solvedEntity(solution, curve)
      if (entity !== undefined) {
        highlight.push(...projectPolyline(entityPoints(entity, tolerance), project))
      }
    }
    for (const point of items.points) {
      const at = pointPosition(solution, point)
      const pixel = at === null ? null : project(at)
      if (pixel !== null) {
        markers.push(pixel)
      }
    }
  }
  return {
    rubberBand,
    highlight: polylinePath(highlight),
    markers: markers
      .map((marker) => circlePath(marker, MARKER_PIXELS * snapshot.uiScale))
      .join(' '),
  }
}

/** Where the HTML chrome goes: inference glyphs, draw inputs, the editor, the placed label and the box. */
export function chromePositions(
  snapshot: PaintSnapshot,
): Pick<OverlayPaint, 'inference' | 'drawInputs' | 'editor' | 'placing' | 'box' | 'measures'> {
  const { pointer, drawn, project, uiScale } = snapshot
  const inputsAt = drawn?.inputsAt === null || drawn === null ? null : project(drawn.inputsAt)
  const measures: Partial<Record<DrawFieldName, string>> = {}
  for (const name of DRAW_FIELD_NAMES) {
    const value = drawn?.measures[name]
    if (value !== undefined) {
      measures[name] = formatNumber(value, FIELD_UNITS[name], snapshot.units)
    }
  }
  return {
    inference:
      pointer === null || drawn === null || drawn.glyphs.length === 0
        ? null
        : [
            pointer[0] + INFERENCE_OFFSET_PIXELS * uiScale,
            pointer[1] + INFERENCE_OFFSET_PIXELS * uiScale,
          ],
    drawInputs:
      inputsAt === null
        ? null
        : drawInputsPlacement(inputsAt, pointer, DRAW_INPUT_OFFSET_PIXELS * uiScale),
    editor: snapshot.editor === null ? null : project(snapshot.editor.at),
    placing: snapshot.placing === null ? null : insideBox(pointer, snapshot.freeArea),
    box: snapshot.box === null ? null : screenBox(snapshot.box.start, snapshot.box.end),
    measures,
  }
}

/**
 * The labels of a sketch's axes on an origin plane (3.16.2): each world axis's letter where the
 * axis's positive half leaves the free area, 1rem inside its edge, moved 0.75rem off the axis to
 * its negative side, or hidden while that half does not cross it; none on a datum plane or a face.
 */
export function axisLabelLayer(snapshot: PaintSnapshot): OverlayPaint['axisLabels'] {
  const labels = new Map<WorldAxis, Vector2 | null>()
  if (snapshot.worldAxes === null) {
    return labels
  }
  const area = insetBox(snapshot.freeArea, AXIS_LABEL_INSET_PIXELS * snapshot.uiScale)
  const { center, reach } = snapshot.axisSpans
  const [alongX, alongY] = snapshot.worldAxes
  const directions: readonly (readonly [WorldAxis, Vector2, number])[] = [
    [alongX, [1, 0], center[0]],
    [alongY, [0, 1], center[1]],
  ]
  const offset = AXIS_LABEL_OFFSET_PIXELS * snapshot.uiScale
  for (const [axis, direction, middle] of directions) {
    const onAxis = axisLabelPosition(
      snapshot.project,
      direction,
      [middle - reach, middle + reach],
      area,
    )
    labels.set(
      axis,
      onAxis === null ? null : besideAxis(snapshot.project, direction, onAxis, area, offset),
    )
  }
  return labels
}

/** Everything the overlay paints from a snapshot. */
export function overlayPaint(snapshot: PaintSnapshot): OverlayPaint {
  const { labels, labelTexts, dimensionLines, dimensionArrows } = dimensionLayer(snapshot)
  return {
    glyphs: glyphLayer(snapshot),
    labels,
    labelTexts,
    issues: issueRings(snapshot.solution).map((ring) =>
      insideBox(snapshot.project(ring.at), snapshot.freeArea),
    ),
    dimensionLines,
    dimensionArrows,
    ...highlightLayer(snapshot),
    ...chromePositions(snapshot),
    axisLabels: axisLabelLayer(snapshot),
    typingArea: insetBox(snapshot.freeArea, TYPING_INSET_PIXELS * snapshot.uiScale),
  }
}
