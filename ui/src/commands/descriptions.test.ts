import { describe, expect, it } from 'vitest'
import { LINUX_WEB } from '../test/fakes/app'
import { TEST_CATALOG } from '../test/fakes/catalog'
import { builtinCommands } from './builtin'
import { featureCommands } from './features'
import type { CommandDefinition } from './registry'

// U-111: the commands that must say what they do (2.10).
const SKETCH_TOOLS: ReadonlySet<string> = new Set([
  'sketch.line',
  'sketch.rectangle_2point',
  'sketch.rectangle_center',
  'sketch.circle',
  'sketch.arc_3point',
  'sketch.point',
  'sketch.construction',
  'sketch.dimension',
  'sketch.finish',
])

function described(): readonly CommandDefinition[] {
  return [...builtinCommands(LINUX_WEB), ...featureCommands(TEST_CATALOG)].filter(
    (command) =>
      command.id.startsWith('feature.') ||
      command.id.startsWith('sketch.constrain_') ||
      SKETCH_TOOLS.has(command.id),
  )
}

describe('command descriptions (2.10)', () => {
  it('cover every feature command, constraint, sketch tool and Finish sketch', () => {
    const ids = described().map((command) => command.id)
    expect(ids).toContain('feature.extrude')
    expect(ids).toContain('sketch.constrain_fix')
    expect(ids).toContain('sketch.finish')
    expect(ids.length).toBe(9 + 13 + featureCommands(TEST_CATALOG).length)
  })

  it.each(described().map((command) => [command.id, command.description ?? ''] as const))(
    'U-111 %s has a description of 1 to 120 characters that ends with a period',
    (_id, description) => {
      expect(description.length).toBeGreaterThanOrEqual(1)
      expect(description.length).toBeLessThanOrEqual(120)
      expect(description.endsWith('.')).toBe(true)
    },
  )

  it.each(described().map((command) => [command.id, command.description ?? ''] as const))(
    'U-111 %s uses the words of the interface, never schema or code terms',
    (_id, description) => {
      for (const word of ['omitted', 'default', 'params', 'None', 'serde']) {
        expect(description).not.toContain(word)
      }
    },
  )
})
