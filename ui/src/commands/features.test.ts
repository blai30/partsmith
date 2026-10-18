import { ArrowUpFromLine, Layers2, PencilRuler, Shapes } from 'lucide-react'
import { describe, expect, it } from 'vitest'
import type { CatalogEntry, Document, ProfileFinding, SketchEntity } from '../generated/protocol'
import { screen, within } from '@testing-library/react'
import { editFeature } from '../app/actions/command'
import { installPopoverPolyfill } from '../components/popover-polyfill.testing'
import {
  CATALOG_TYPES,
  cubeDocumentState,
  documentState,
  readyState,
  rectSketchDocumentState,
} from '../layout/fixtures.testing'
import { createModelApp, renderWithApp, stubMatchMedia } from '../layout/test-app.testing'
import { MESSAGES } from '../messages'
import {
  lineEntity,
  pointEntity,
  sketchSession,
  solutionFor,
  solvedSketch,
} from '../sketch/fixtures.testing'
import type { DocumentState } from '../state/store'
import { catalogOf, TEST_CATALOG, TEST_CATALOG_ENTRIES } from '../test/fakes/catalog'
import {
  CUBE_FEATURES_DOCUMENT,
  emptyDocument,
  featuresOf,
  RECT_REGION,
  RECT_SKETCH_DOCUMENT,
  RECT_SKETCH_SOLUTION,
  TWO_BODIES_DOCUMENT,
  withFeatureChanges,
  type FeatureRecord,
} from '../test/fakes/documents'
import { actionLabels, onlyNotification, runAction } from '../test/fakes/test-app'
import { createFakeViewport, expectBoundsContain, lastFramedBounds } from '../test/fakes/viewport'
import { settle } from '../test/fakes/settle'
import { CUBE_BOUNDS, meshSummary, PEG_BOUNDS } from '../test/fakes/state'
import { FEATURE_DESCRIPTIONS } from './descriptions'
import { featureCommands } from './features'
import { COMMAND_KEYWORDS } from './keywords'
import type { CommandDefinition, Keyword } from './registry'

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

const fusion = (word: string): Keyword => ({ word, app: 'Fusion' })
const freecad = (word: string): Keyword => ({ word, app: 'FreeCAD' })

// ui.md 2.10, the feature rows of COMMAND_KEYWORDS.
const KEYWORDS: readonly (readonly [string, readonly Keyword[]])[] = [
  ['sketch', ['drawing', 'profile', 'new sketch', fusion('Create Sketch'), 'sketcher']],
  [
    'extrude',
    [freecad('Pad'), freecad('Pocket'), fusion('Press Pull'), 'extrusion', 'pull', 'push'],
  ],
  ['revolve', [freecad('Revolution'), freecad('Groove'), 'lathe', 'spin']],
  ['hole', ['drill', 'counterbore', 'countersink', 'screw hole']],
  ['fillet', ['round', 'radius', 'rounded edge']],
  ['chamfer', ['bevel']],
  ['shell', ['hollow', freecad('Thickness')]],
  [
    'boolean',
    [fusion('Combine'), 'union', 'subtract', 'intersect', 'join', 'cut', 'merge', 'group'],
  ],
  ['mirror', [freecad('Mirrored'), 'symmetry']],
  ['linear_pattern', [fusion('Rectangular Pattern'), freecad('LinearPattern'), 'array']],
  ['circular_pattern', [freecad('PolarPattern'), 'polar pattern', 'array']],
  ['datum_plane', [fusion('Offset Plane'), 'construction plane', 'work plane']],
  ['move_body', [fusion('Move/Copy'), freecad('Placement'), 'translate', 'rotate', 'transform']],
  ['split_body', ['slice', 'cut body']],
  ['box', [freecad('Additive Box'), 'cube', 'block', 'primitive', 'shape']],
  ['cylinder', [freecad('Additive Cylinder'), 'rod', 'pin', 'primitive', 'shape']],
  ['sphere', [freecad('Additive Sphere'), 'ball', 'primitive', 'shape']],
  ['cone', [freecad('Additive Cone'), 'taper', 'primitive', 'shape']],
  ['torus', [freecad('Additive Torus'), 'ring', 'donut', 'primitive', 'shape']],
]

// ui.md 2.10, FEATURE_DESCRIPTIONS.
const DESCRIPTIONS: readonly (readonly [string, string])[] = [
  [
    'sketch',
    'Draws a 2D shape on a plane or a flat face, to extrude, revolve or place holes from.',
  ],
  ['extrude', 'Pulls a closed sketch shape straight out into a solid, or cuts it into one.'],
  ['revolve', 'Spins a closed sketch shape around an axis into a round solid, or cuts with it.'],
  ['hole', 'Drills round holes where you click on a flat face or at sketch points.'],
  ['box', 'Adds a box of the given length, width and height.'],
  ['cylinder', 'Adds a cylinder of the given radius and height.'],
  ['sphere', 'Adds a ball of the given radius.'],
  ['cone', 'Adds a cone, or a tapered cylinder, from two radii and a height.'],
  ['torus', 'Adds a ring from the radius of its center line and the radius of its tube.'],
  ['fillet', 'Rounds the selected edges.'],
  ['chamfer', 'Bevels the selected edges with a flat cut.'],
  ['shell', 'Hollows out a body, leaving walls of the given thickness and the picked faces open.'],
  ['boolean', 'Joins, cuts or intersects one body with other bodies.'],
  ['move_body', 'Moves and rotates a whole body.'],
  ['split_body', 'Cuts a body in two along a plane, for example to fit a large part on the bed.'],
  ['mirror', 'Mirrors a body across a plane, joined to it or as a new body.'],
  ['linear_pattern', 'Repeats a body in a row, or in a grid with a second direction.'],
  ['circular_pattern', 'Repeats a body around an axis.'],
  ['datum_plane', 'Adds a plane offset from another plane, to sketch on or mirror across.'],
]

describe('featureCommands', () => {
  it('U-29 makes one feature.<type> command per catalog entry, except import', () => {
    const ids = featureCommands(TEST_CATALOG).map((command) => command.id)
    const expected = CATALOG_TYPES.filter((type) => type !== 'import').map(
      (type) => `feature.${type}`,
    )
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

  it('U-29 gives extrude keywords that include Pad, tagged FreeCAD', () => {
    expect(commandFor('extrude').keywords).toContainEqual({ word: 'Pad', app: 'FreeCAD' })
  })

  it('U-29 gives extrude the description of FEATURE_DESCRIPTIONS', () => {
    expect(commandFor('extrude').description).toBe(FEATURE_DESCRIPTIONS['feature.extrude'])
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

  it('lists every feature command in the Model, Sketch and Print contexts (2.10, 3.3)', () => {
    for (const command of featureCommands(TEST_CATALOG)) {
      expect([...command.contexts].toSorted()).toEqual(['model', 'print', 'sketch'])
    }
  })

  it('makes H, F and M work in Model and Print only, and E everywhere (3.5)', () => {
    // Sketch keeps H for Horizontal, so U-22 finds no conflict.
    const shortcutContexts = Object.fromEntries(
      featureCommands(TEST_CATALOG)
        .filter((command) => command.shortcuts.length > 0)
        .map((command) => [command.id, [...(command.shortcutContexts ?? [])].toSorted()]),
    )
    expect(shortcutContexts).toEqual({
      'feature.extrude': ['model', 'print', 'sketch'],
      'feature.hole': ['model', 'print'],
      'feature.fillet': ['model', 'print'],
      'feature.move_body': ['model', 'print'],
    })
  })

  it('gives commands without a shortcut no shortcut contexts', () => {
    expect(commandFor('chamfer').shortcutContexts).toBeUndefined()
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
    expect(COMMAND_KEYWORDS[`feature.${type}`]).toEqual(keywords)
  })

  it.each(DESCRIPTIONS)('gives %s the description of 2.10', (type, description) => {
    expect(commandFor(type).description).toBe(description)
  })

  it('gives a type without a table entry no keywords and no description', () => {
    const unknown: CatalogEntry = {
      type: 'gear',
      manifest: { label: 'Gear', category: 'create', icon: 'gear', version: 1, fields: [] },
      params_schema: { type: 'object', properties: {} },
    }
    const [command] = featureCommands(catalogOf([unknown]))
    expect(command?.keywords).toEqual([])
    expect(command?.description).toBeUndefined()
  })
})

describe('running feature commands', () => {
  it('starts the inspector command with a transaction named after the manifest label', async () => {
    const { app, core } = createModelApp({ state: readyState(cubeDocumentState()) })
    await app.commands.run(app, 'feature.fillet', 'palette')
    await settle()
    expect(core.callsTo('beginTransaction')).toEqual([[1, 'Fillet']])
    expect(app.store.getState().command?.featureType).toBe('fillet')
  })

  it('switches from Print to Model before starting the command (3.3)', async () => {
    const { app, core } = createModelApp({
      state: readyState(cubeDocumentState(), { context: 'print' }),
    })
    await app.commands.run(app, 'feature.fillet', 'toolbar')
    await settle()
    expect(app.store.getState().context).toBe('model')
    expect(core.callsTo('beginTransaction')).toEqual([[1, 'Fillet']])
  })

  it('runs the sketch type as a new sketch, not an inspector command', async () => {
    const { app, core } = createModelApp({ state: readyState(cubeDocumentState()) })
    await app.commands.run(app, 'feature.sketch', 'toolbar')
    await settle()
    expect(core.callsTo('beginTransaction')).toEqual([])
    expect(app.store.getState().command).toBeNull()
    expect(app.store.getState().context).toBe('sketch')
    expect(app.store.getState().sketch?.sketchId).toBeNull()
  })

  it('finishes the sketch, switches to Model and preselects its only region for Extrude (3.3)', async () => {
    const { app, core } = createModelApp({
      state: readyState(rectSketchDocumentState(), {
        context: 'sketch',
        sketch: sketchSession(),
      }),
    })
    await app.commands.run(app, 'feature.extrude', 'palette')
    await settle()
    expect(app.store.getState().context).toBe('model')
    expect(app.store.getState().sketch).toBeNull()
    expect(core.callsTo('beginTransaction')).toEqual([[1, 'Extrude']])
    expect(app.store.getState().command?.params.profile).toEqual({
      sketch: 'f-sk1',
      regions: [RECT_REGION.reference],
    })
  })
})

/** The availability of the command of a feature type in a document. */
function availabilityIn(document: DocumentState, type: string) {
  const { app } = createModelApp({ state: readyState(document) })
  return app.commands.get(`feature.${type}`)?.availability(app)
}

describe('availability reasons (2.10, U-98)', () => {
  const NEW_DOCUMENT = documentState(emptyDocument())

  it('U-98 disables extrude and fillet in a new document with what is missing', () => {
    expect(availabilityIn(NEW_DOCUMENT, 'extrude')).toEqual({
      state: 'disabled',
      reason: 'Draw a closed shape in a sketch first.',
      code: 'needs_sketch_region',
    })
    expect(availabilityIn(NEW_DOCUMENT, 'fillet')).toEqual({
      state: 'disabled',
      reason: 'Make a solid first, for example with Extrude or Box.',
      code: 'needs_solid',
    })
    expect(availabilityIn(NEW_DOCUMENT, 'box')).toEqual({ state: 'enabled' })
    expect(availabilityIn(NEW_DOCUMENT, 'sketch')).toEqual({ state: 'enabled' })
  })

  it('U-98 running a disabled feature command explains it with the message id as its code (2.8, 3.22)', async () => {
    const { app } = createModelApp({ state: readyState(NEW_DOCUMENT) })
    await app.commands.run(app, 'feature.fillet', 'shortcut')
    expect(app.store.getState().notifications).toMatchObject([
      { severity: 'info', message: MESSAGES.needs_solid(), code: 'needs_solid' },
    ])
  })

  it('U-98 renders the Pattern group aria-disabled with its first command’s reason', () => {
    installPopoverPolyfill()
    stubMatchMedia()
    const { app } = createModelApp({ state: readyState(NEW_DOCUMENT) })
    renderWithApp(app)
    const toolbar = screen.getByRole('toolbar', { name: 'Model tools' })
    const pattern = within(toolbar).getByRole('button', { name: 'Pattern' })
    expect(pattern).toHaveAttribute('aria-disabled', 'true')
    expect(pattern).toHaveAccessibleDescription(expect.stringContaining(MESSAGES.needs_solid()))
  })

  it('U-98 enables extrude and revolve for a sketch with a region, and keeps fillet disabled', () => {
    const sketch = rectSketchDocumentState()
    expect(availabilityIn(sketch, 'extrude')).toEqual({ state: 'enabled' })
    expect(availabilityIn(sketch, 'revolve')).toEqual({ state: 'enabled' })
    expect(availabilityIn(sketch, 'fillet')).toEqual({
      state: 'disabled',
      reason: MESSAGES.needs_solid(),
      code: 'needs_solid',
    })
  })

  it('U-98 enables fillet on the cube and asks Boolean for a second body', () => {
    expect(availabilityIn(cubeDocumentState(), 'fillet')).toEqual({ state: 'enabled' })
    expect(availabilityIn(cubeDocumentState(), 'boolean')).toEqual({
      state: 'disabled',
      reason:
        'Boolean needs two bodies with geometry. Make a second body first, for example with Box.',
      code: 'needs_two_bodies',
    })
    const twoBodies = documentState(TWO_BODIES_DOCUMENT, {
      meshes: { 'b-cube': meshSummary(1, CUBE_BOUNDS), 'b-peg': meshSummary(2, PEG_BOUNDS) },
    })
    expect(availabilityIn(twoBodies, 'boolean')).toEqual({ state: 'enabled' })
  })

  it('U-98 disables a new fillet without geometry, while editing one still starts', async () => {
    const suppressed = documentState(
      withFeatureChanges(CUBE_FEATURES_DOCUMENT, 'f-ext1', { suppressed: true }),
    )
    expect(availabilityIn(suppressed, 'fillet')).toEqual({
      state: 'disabled',
      reason: MESSAGES.needs_solid(),
      code: 'needs_solid',
    })
    const { app, core } = createModelApp({ state: readyState(suppressed) })
    await editFeature(app, 'f-fil1')
    expect(core.callsTo('beginTransaction')).toEqual([[1, 'Edit Fillet 1']])
  })

  it('counts the sketch being edited, and leaves out suppressed and rolled-back sketches', () => {
    const suppressedSketch = rectSketchDocumentState({
      recipe: withFeatureChanges(rectSketchDocumentState().recipe, 'f-sk1', { suppressed: true }),
    })
    expect(availabilityIn(suppressedSketch, 'extrude')?.state).toBe('disabled')
    const { app } = createModelApp({
      state: readyState(suppressedSketch, {
        context: 'sketch',
        sketch: sketchSession(),
      }),
    })
    expect(app.commands.get('feature.extrude')?.availability(app)).toEqual({ state: 'enabled' })
    const rolledBack = rectSketchDocumentState({
      recipe: { ...rectSketchDocumentState().recipe, rollback: 'f-sk1' },
    })
    expect(availabilityIn(rolledBack, 'extrude')?.state).toBe('disabled')
  })
})

// Newcomer error corpus NC01 (ui.md 2.10, U-140): BLOCK's outline with a 0.3 mm gap at the origin,
// whose one open_endpoint finding stands for two open ends.
const NC01_LINES = [
  lineEntity('e-1', [0, 0], [80, 0]),
  lineEntity('e-2', [80, 0], [80, 50]),
  lineEntity('e-3', [80, 50], [0, 50]),
  lineEntity('e-4', [0, 50], [0, 0.3]),
]
const NC01_GAP: ProfileFinding = {
  type: 'open_endpoint',
  entity: 'e-4',
  point: [0, 0.3],
  connect: { first: 'e-4.end', second: 'e-1.start', gap: 0.3 },
}

/** An unpaired open end of `entity` at `point`. */
function openEnd(entity: string, point: [number, number]): ProfileFinding {
  return { type: 'open_endpoint', entity, point }
}

/** A sketch record on XY with these entities. */
function sketchOf(id: string, entities: readonly SketchEntity[]): FeatureRecord {
  return {
    id,
    type: 'sketch',
    plane: { kind: 'origin', plane: 'XY' },
    entities: [...entities],
    constraints: [],
  }
}

/** A recipe of these sketches and nothing else. */
function sketchesRecipe(sketches: readonly FeatureRecord[]): Document {
  return { ...emptyDocument(), features: [...sketches] }
}

/** A document whose sketches have no region and these findings, by sketch id. */
function openSketches(
  sketches: readonly (readonly [
    id: string,
    entities: readonly SketchEntity[],
    findings: readonly ProfileFinding[],
  ])[],
): DocumentState {
  return documentState(sketchesRecipe(sketches.map(([id, entities]) => sketchOf(id, entities))), {
    sketches: Object.fromEntries(
      sketches.map(([id, entities, findings]) => [
        id,
        solvedSketch(solutionFor(entities, { feature: id, profile_findings: [...findings] })),
      ]),
    ),
  })
}

const NC01_STATE = openSketches([['f-sk1', NC01_LINES, [NC01_GAP]]])
const NC01_REASON = "'Sketch 1' has no closed shape: 2 ends are not connected."

describe('an open outline names its sketch (2.10, U-140)', () => {
  it('U-140 disables extrude and revolve with needs_closed_shape and the action "Show \'Sketch 1\'"', () => {
    for (const type of ['extrude', 'revolve']) {
      expect(availabilityIn(NC01_STATE, type)).toMatchObject({
        state: 'disabled',
        reason: NC01_REASON,
        code: 'needs_closed_shape',
        action: { label: "Show 'Sketch 1'" },
      })
    }
  })

  it('U-140 running it shows the reason as an info notification with the Show action', async () => {
    const { app } = createModelApp({ state: readyState(NC01_STATE) })
    await app.commands.run(app, 'feature.extrude', 'toolbar')
    const notification = onlyNotification(app)
    expect(notification).toMatchObject({
      severity: 'info',
      message: NC01_REASON,
      code: 'needs_closed_shape',
    })
    expect(actionLabels(notification)).toEqual(["Show 'Sketch 1'"])
    expect(app.store.getState().command).toBeNull()
  })

  it('U-140 "Show \'Sketch 1\'" enters the sketch and frames both open ends', async () => {
    const { app } = createModelApp({ state: readyState(NC01_STATE) })
    const viewport = createFakeViewport()
    app.attachViewport(viewport)
    await app.commands.run(app, 'feature.extrude', 'toolbar')
    runAction(onlyNotification(app), "Show 'Sketch 1'")
    await settle()
    expect(app.store.getState().context).toBe('sketch')
    expect(app.store.getState().sketch?.sketchId).toBe('f-sk1')
    // The two ends, grown to a fifth of the 80 by 50 sketch about their center (3.16.8).
    const { bounds } = lastFramedBounds(viewport)
    expectBoundsContain(bounds, [
      [0, 0.3, 0],
      [0, 0, 0],
    ])
    expect(bounds.min[0]).toBeCloseTo(-8)
    expect(bounds.max[0]).toBeCloseTo(8)
    expect(bounds.min[1]).toBeCloseTo(-4.85)
    expect(bounds.max[1]).toBeCloseTo(5.15)
  })

  it("U-140 the toolbar's Extrude is aria-disabled with the reason as its description (4.2)", () => {
    installPopoverPolyfill()
    stubMatchMedia()
    const { app } = createModelApp({ state: readyState(NC01_STATE) })
    renderWithApp(app)
    const extrude = within(screen.getByRole('toolbar', { name: 'Model tools' })).getByRole(
      'button',
      { name: 'Extrude' },
    )
    expect(extrude).toHaveAttribute('aria-disabled', 'true')
    expect(extrude).toHaveAccessibleDescription(expect.stringContaining(NC01_REASON))
  })

  it('U-140 names the sketch with the fewest open ends', () => {
    const state = openSketches([
      [
        'f-sk1',
        [lineEntity('e-1', [0, 0], [10, 0]), lineEntity('e-2', [0, 20], [10, 20])],
        [
          openEnd('e-1', [0, 0]),
          openEnd('e-1', [10, 0]),
          openEnd('e-2', [0, 20]),
          openEnd('e-2', [10, 20]),
        ],
      ],
      ['f-sk2', NC01_LINES, [NC01_GAP]],
    ])
    expect(availabilityIn(state, 'extrude')).toMatchObject({
      reason: "'Sketch 2' has no closed shape: 2 ends are not connected.",
      action: { label: "Show 'Sketch 2'" },
    })
  })

  it('U-140 names the later sketch among sketches with as many open ends', () => {
    const state = openSketches([
      ['f-sk1', NC01_LINES, [NC01_GAP]],
      ['f-sk2', NC01_LINES, [NC01_GAP]],
    ])
    expect(availabilityIn(state, 'extrude')).toMatchObject({
      reason: "'Sketch 2' has no closed shape: 2 ends are not connected.",
    })
  })

  it('U-140 counts one open end in the singular', () => {
    const state = openSketches([
      ['f-sk1', [lineEntity('e-1', [0, 0], [10, 0])], [openEnd('e-1', [10, 0])]],
    ])
    expect(availabilityIn(state, 'extrude')).toMatchObject({
      reason: "'Sketch 1' has no closed shape: 1 end is not connected.",
      code: 'needs_closed_shape',
    })
  })

  it('U-140 a sketch with no region and no open end (only points) asks for a closed shape', () => {
    const state = openSketches([['f-sk1', [pointEntity('e-p1', [5, 5])], []]])
    expect(availabilityIn(state, 'extrude')).toEqual({
      state: 'disabled',
      reason: 'Draw a closed shape in a sketch first.',
      code: 'needs_sketch_region',
    })
  })

  it('U-140 leaves out a suppressed sketch with open ends', () => {
    const suppressed = {
      ...NC01_STATE,
      recipe: withFeatureChanges(NC01_STATE.recipe, 'f-sk1', { suppressed: true }),
    }
    expect(availabilityIn(suppressed, 'extrude')).toMatchObject({ code: 'needs_sketch_region' })
  })

  it('U-140 enables extrude once any active sketch has a region', () => {
    const [rectangle] = featuresOf(RECT_SKETCH_DOCUMENT)
    if (rectangle === undefined) {
      throw new Error('rect-sketch.ptsm has no sketch.')
    }
    const closed: DocumentState = {
      ...NC01_STATE,
      recipe: sketchesRecipe([...featuresOf(NC01_STATE.recipe), { ...rectangle, id: 'f-sk2' }]),
      sketches: {
        ...NC01_STATE.sketches,
        'f-sk2': solvedSketch({ ...RECT_SKETCH_SOLUTION, feature: 'f-sk2' }),
      },
    }
    expect(availabilityIn(closed, 'extrude')).toEqual({ state: 'enabled' })
  })
})
