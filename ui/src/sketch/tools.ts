import { isConstraintType, type ConstraintType } from './constraints'
import { DRAWING_TOOLS, type DrawingTool } from './drawing'
import type { SketchToolId } from './session'

// Kinds of sketch tools (ui.md 3.16.3).

const DRAWING_TOOL_SET: ReadonlySet<SketchToolId> = new Set<SketchToolId>(DRAWING_TOOLS)

/** Whether the tool draws new geometry (as opposed to dimensioning or constraining). */
export function isDrawingTool(tool: SketchToolId | null): tool is DrawingTool {
  return tool !== null && DRAWING_TOOL_SET.has(tool)
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
