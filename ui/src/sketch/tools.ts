import { isConstraintType, type ConstraintType } from './constraints'
import type { DrawingTool } from './drawing'
import type { SketchToolId } from './session'

// Kinds of sketch tools (ui.md 3.16.3).

/** The drawing tools, which place new geometry and show a crosshair. */
export const DRAWING_TOOLS: ReadonlySet<SketchToolId> = new Set<DrawingTool>([
  'line',
  'rectangle_2point',
  'rectangle_center',
  'circle',
  'arc_3point',
  'point',
])

/** Whether the tool draws new geometry (as opposed to dimensioning or constraining). */
export function isDrawingTool(tool: SketchToolId | null): tool is DrawingTool {
  return tool !== null && DRAWING_TOOLS.has(tool)
}

const CONSTRAINT_PREFIX = 'constrain_'

/** The constraint of a constraint tool, or null for the other tools. */
export function constraintTool(tool: SketchToolId | null): ConstraintType | null {
  if (tool === null || !tool.startsWith(CONSTRAINT_PREFIX)) {
    return null
  }
  const type = tool.slice(CONSTRAINT_PREFIX.length)
  return isConstraintType(type) ? type : null
}
