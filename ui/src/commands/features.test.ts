import { ArrowUpFromLine, Layers2, PencilRuler, Shapes } from 'lucide-react'
import { describe, expect, it } from 'vitest'
import type { CatalogEntry } from '../generated/protocol'
import {
  CATALOG_TYPES,
  catalogOf,
  cubeDocumentState,
  readyState,
  TEST_CATALOG,
  TEST_CATALOG_ENTRIES,
} from '../layout/fixtures.testing'
import { createTestApp, settle } from '../layout/test-app.testing'
import { FEATURE_KEYWORDS, featureCommands } from './features'
import type { CommandDefinition } from './registry'

function commandsById(): ReadonlyMap<string, CommandDefinition> {
  return new Map(featureCommands(TEST_CATALOG).map((command) => [command.id, command]))
}

function commandFor(type: string): CommandDefinition {
  const command = commandsById().get(`feature.${type}`)
  if (command === undefined) {
    throw new Error(`No command for ${type}`)
  }
  return command
}

// ui.md 2.10, FEATURE_KEYWORDS.
const KEYWORDS: readonly (readonly [string, readonly string[]])[] = [
  ['extrude', ['pad', 'pocket', 'extrusion']],
  ['revolve', ['revolution', 'groove', 'lathe']],
  ['hole', ['drill', 'counterbore', 'countersink']],
  ['fillet', ['round', 'radius']],
  ['chamfer', ['bevel']],
  ['shell', ['hollow', 'thickness']],
  ['boolean', ['combine', 'union', 'subtract', 'intersect', 'join', 'cut']],
  ['mirror', ['mirrored', 'symmetry']],
  ['linear_pattern', ['array', 'rectangular pattern']],
  ['circular_pattern', ['polar pattern', 'array']],
  ['datum_plane', ['construction plane', 'offset plane', 'work plane']],
  ['move_body', ['placement', 'translate', 'rotate']],
  ['split_body', ['slice', 'cut body']],
  ['box', ['cube', 'block']],
  ['cylinder', ['rod', 'pin']],
  ['sphere', ['ball']],
  ['cone', ['taper']],
  ['torus', ['ring', 'donut']],
  ['sketch', ['drawing', 'profile']],
]

describe('featureCommands', () => {
  it('U-29 makes one feature.<type> command per catalog entry, except import', () => {
    const ids = featureCommands(TEST_CATALOG).map((command) => command.id)
    const expected = CATALOG_TYPES.filter((type) => type !== 'import').map((type) => `feature.${type}`)
    expect(ids.toSorted()).toEqual(expected.toSorted())
  })

  it('U-29 titles each command with its manifest label', () => {
    for (const entry of TEST_CATALOG_ENTRIES.filter((candidate) => candidate.type !== 'import')) {
      expect(commandFor(entry.type).title).toBe(entry.manifest.label)
    }
  })

  it('U-29 gives extrude the shortcut E', () => {
    expect(commandFor('extrude').shortcuts).toEqual(['E'])
  })

  it('U-29 gives extrude keywords that include pad', () => {
    expect(commandFor('extrude').keywords).toContain('pad')
  })

  it('U-29 falls back to the Shapes icon for an unknown icon name', () => {
    const unknown: CatalogEntry = {
      type: 'gear',
      manifest: { label: 'Gear', category: 'create', icon: 'gear-wheel', version: 1, fields: [] },
      params_schema: { type: 'object', properties: {} },
    }
    const [command] = featureCommands(catalogOf([unknown]))
    expect(command?.id).toBe('feature.gear')
    expect(command?.icon).toBe(Shapes)
  })

  it('takes the icon from FEATURE_ICONS by the manifest icon name', () => {
    expect(commandFor('extrude').icon).toBe(ArrowUpFromLine)
    expect(commandFor('sketch').icon).toBe(PencilRuler)
    expect(commandFor('datum_plane').icon).toBe(Layers2)
  })

  it('maps manifest categories to command categories, with sketch under Create', () => {
    expect(commandFor('extrude').category).toBe('Create')
    expect(commandFor('box').category).toBe('Create')
    expect(commandFor('fillet').category).toBe('Modify')
    expect(commandFor('mirror').category).toBe('Pattern')
    expect(commandFor('linear_pattern').category).toBe('Pattern')
    expect(commandFor('datum_plane').category).toBe('Construct')
    expect(commandFor('sketch').category).toBe('Create')
  })

  it('lists every feature command in the Model, Sketch and Print contexts', () => {
    for (const command of featureCommands(TEST_CATALOG)) {
      expect([...command.contexts].toSorted()).toEqual(['model', 'print', 'sketch'])
    }
  })

  it('binds exactly Extrude E, Hole H, Fillet F and Move body M (3.5)', () => {
    const bound = featureCommands(TEST_CATALOG)
      .filter((command) => command.shortcuts.length > 0)
      .map((command) => [command.id, command.shortcuts])
    expect(Object.fromEntries(bound)).toEqual({
      'feature.extrude': ['E'],
      'feature.hole': ['H'],
      'feature.fillet': ['F'],
      'feature.move_body': ['M'],
    })
  })

  it.each(KEYWORDS)('gives %s the keywords of 2.10', (type, keywords) => {
    expect(commandFor(type).keywords).toEqual(keywords)
    expect(FEATURE_KEYWORDS[type]).toEqual(keywords)
  })
})

describe('running feature commands', () => {
  it('starts the inspector command with a transaction named after the manifest label', async () => {
    const { app, core } = createTestApp({ state: readyState(cubeDocumentState()) })
    await app.commands.run(app, 'feature.fillet', 'palette')
    await settle()
    expect(core.beginTransaction).toHaveBeenCalledWith(1, 'Fillet')
    expect(app.store.getState().command?.featureType).toBe('fillet')
  })

  it('switches from Print to Model before starting the command (3.3)', async () => {
    const { app, core } = createTestApp({
      state: readyState(cubeDocumentState(), { context: 'print' }),
    })
    await app.commands.run(app, 'feature.fillet', 'toolbar')
    await settle()
    expect(app.store.getState().context).toBe('model')
    expect(core.beginTransaction).toHaveBeenCalledWith(1, 'Fillet')
  })

  it('runs the sketch type as a new sketch, not an inspector command', async () => {
    const { app, core } = createTestApp({ state: readyState(cubeDocumentState()) })
    await app.commands.run(app, 'feature.sketch', 'toolbar')
    await settle()
    expect(core.beginTransaction).not.toHaveBeenCalled()
    expect(app.store.getState().command).toBeNull()
    expect(app.store.getState().context).toBe('sketch')
    expect(app.store.getState().sketch?.sketchId).toBeNull()
  })
})
