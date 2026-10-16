import { describe, expect, it } from 'vitest'
import type { Cause, Diagnostic, Document, Edit, FixRef, Repair } from '../../generated/protocol'
import { MESSAGES } from '../../messages'
import { lineEntity } from '../../sketch/fixtures.testing'
import { actionLabels, runAction, startWithDocument, type FakeApp } from '../../test/fakes/app'
import type { FakeFeatureStatus } from '../../test/fakes/core'
import {
  CUBE_FEATURES_DOCUMENT,
  CUBE_SKETCH_SOLUTION,
  CUBE_TOP_FRONT_EDGE,
} from '../../test/fakes/documents'
import { lostEdgeRepair } from '../../test/fakes/repairs'
import { settle } from '../../test/fakes/settle'
import { createFakeViewport } from '../../test/fakes/viewport'
import { confirmCommand, editFeature, setCommandField } from './command'
import { applyEdits, undo } from './document'
import { commitSketchChange, enterSketch, finishSketch } from './sketch'
import { deleteFeatures } from './timeline'

// Notifications for new losses after a user change (ui.md 4.2, 3.22): one per root, with the
// cause line, "Show what '{label}' lost", the cause's first revert and Undo, never a candidate.

/** cube-features.ptsm with a second sketch, 'Sketch 2', at the end. */
const DOCUMENT: Document = {
  ...CUBE_FEATURES_DOCUMENT,
  features: [
    ...(CUBE_FEATURES_DOCUMENT.features ?? []),
    {
      id: 'f-sk2',
      type: 'sketch',
      plane: { kind: 'origin', plane: 'XY' },
      entities: [],
      constraints: [],
    },
  ],
}

const BEVEL_EDGE = "the top edge of 'Extrude 1' along a line of 'Sketch 1'"

const LINE_DELETED: Cause = {
  text: "This started after you deleted the line from (80, 40) to (70, 50) in 'Sketch 1'.",
  actions: [
    { label: 'Undo', run: { kind: 'undo' } },
    { label: 'Show the line', run: { kind: 'show', feature: 'f-sk1', entity: 'e-5' } },
  ],
}

const COUNT_BACK: FixRef = {
  feature: 'f-fil1',
  code: 'reference_missing',
  field: '/edges/0',
  cause: true,
  index: 0,
  revision: 2,
}

const COUNT_LOWERED: Cause = {
  text: "This started after you changed Count of 'Linear pattern 1' from 3 to 2.",
  actions: [
    { label: "Set Count of 'Linear pattern 1' back to 3", run: { kind: 'apply', fix: COUNT_BACK } },
    { label: 'Undo', run: { kind: 'undo' } },
  ],
}

/** The ghost of the lost edge: (0, -10, 20) to (10, -10, 20), buffer 0 of each recompute frame. */
const GHOST_POINTS = [0, -10, 20, 10, -10, 20]

function lostEdge(path: string, description = BEVEL_EDGE): Repair {
  return lostEdgeRepair({
    path,
    description,
    ghost: { positions: 0, kind: 'lines', frame: 'world' },
    candidates: [
      {
        reference: CUBE_TOP_FRONT_EDGE.reference,
        description: "the right top edge of 'Extrude 1'",
        relation: "on the top of 'Extrude 1', about 21 mm from where it was",
        meaning: true,
        distance: 21,
        score: 0.46,
      },
      {
        reference: { q: 'edge(f-ext1/end-cap|f-ext1/side-2)', fp: 'fp-back' },
        description: "the back top edge of 'Extrude 1'",
        relation: "on the top of 'Extrude 1', about 35 mm from where it was",
        meaning: true,
        distance: 35,
        score: 0.38,
      },
    ],
  })
}

function missing(field: string, cause: Cause | null = LINE_DELETED): Diagnostic {
  return {
    severity: 'warning',
    code: 'reference_missing',
    message: `The edge selected for Edges at ${field} is gone, so it was skipped.`,
    field,
    ...(cause === null ? {} : { cause }),
  }
}

/** U-132's 'Fillet 1' after the edit. */
function filletLost(cause: Cause = LINE_DELETED): FakeFeatureStatus {
  return {
    status: 'warning',
    messages: [missing('/edges/0', cause)],
    repairs: [lostEdge('/edges/0')],
  }
}

const RADIUS_3: readonly Edit[] = [
  {
    type: 'update_feature',
    feature: { id: 'f-fil1', type: 'fillet', body: 'b-cube', edges: [], radius: '3 mm' },
  },
]

async function withStatuses(): Promise<{
  readonly fake: FakeApp
  readonly statuses: Record<string, FakeFeatureStatus>
}> {
  const statuses: Record<string, FakeFeatureStatus> = {}
  const fake = await startWithDocument('cube-features.ptsm', DOCUMENT, {
    core: { statuses, ghosts: [GHOST_POINTS], solutions: { 'f-sk1': CUBE_SKETCH_SOLUTION } },
  })
  return { fake, statuses }
}

function notifications(fake: FakeApp) {
  return fake.app.store.getState().notifications
}

describe('a notification for every new loss (4.2, U-132)', () => {
  it('U-132 a new lost edge raises exactly one warning naming the feature and the element', async () => {
    const { fake, statuses } = await withStatuses()
    statuses['f-fil1'] = filletLost()
    await applyEdits(fake.app, 'Delete', RADIUS_3)
    await settle()
    expect(notifications(fake)).toHaveLength(1)
    expect(notifications(fake)[0]).toMatchObject({
      severity: 'warning',
      code: 'feature_lost',
      message: MESSAGES.feature_lost('Fillet 1', BEVEL_EDGE),
      cause: LINE_DELETED.text,
      features: ['f-fil1'],
    })
  })

  it(`U-132 its actions are "Show what 'Fillet 1' lost" and Undo, never a candidate`, async () => {
    const { fake, statuses } = await withStatuses()
    statuses['f-fil1'] = filletLost()
    await applyEdits(fake.app, 'Delete', RADIUS_3)
    await settle()
    const [notification] = notifications(fake)
    expect(notification === undefined ? [] : actionLabels(notification)).toEqual([
      MESSAGES.show_lost('Fillet 1'),
      'Undo',
    ])
  })

  it("U-132 the cause's first apply action comes between them", async () => {
    const { fake, statuses } = await withStatuses()
    statuses['f-fil1'] = filletLost(COUNT_LOWERED)
    await applyEdits(fake.app, 'Change Linear pattern 1', RADIUS_3)
    await settle()
    const [notification] = notifications(fake)
    expect(notification === undefined ? [] : actionLabels(notification)).toEqual([
      "Show what 'Fillet 1' lost",
      "Set Count of 'Linear pattern 1' back to 3",
      'Undo',
    ])
  })

  it("U-132 the cause's revert runs apply_fix with its FixRef", async () => {
    const { fake, statuses } = await withStatuses()
    statuses['f-fil1'] = filletLost(COUNT_LOWERED)
    await applyEdits(fake.app, 'Change Linear pattern 1', RADIUS_3)
    await settle()
    const [notification] = notifications(fake)
    if (notification === undefined) {
      throw new Error('No notification was raised.')
    }
    runAction(notification, "Set Count of 'Linear pattern 1' back to 3")
    await settle()
    expect(fake.transport.commandsOfType('apply_fix').map((command) => command.fix)).toEqual([
      COUNT_BACK,
    ])
  })

  it(`U-132 "Show what 'Fillet 1' lost" edits it with Edges active and frames the ghost`, async () => {
    const { fake, statuses } = await withStatuses()
    const viewport = createFakeViewport()
    fake.app.attachViewport(viewport)
    statuses['f-fil1'] = filletLost()
    await applyEdits(fake.app, 'Delete', RADIUS_3)
    await settle()
    const [notification] = notifications(fake)
    if (notification === undefined) {
      throw new Error('No notification was raised.')
    }
    runAction(notification, "Show what 'Fillet 1' lost")
    await settle()
    expect(fake.app.store.getState().command).toMatchObject({
      featureId: 'f-fil1',
      editing: true,
      activeField: 'edges',
    })
    const [bounds] = viewport.zoomToBounds.mock.calls.at(-1) ?? []
    for (const point of [
      [0, -10, 20],
      [10, -10, 20],
    ] as const) {
      for (const axis of [0, 1, 2] as const) {
        expect(bounds?.min[axis]).toBeLessThanOrEqual(point[axis])
        expect(bounds?.max[axis]).toBeGreaterThanOrEqual(point[axis])
      }
    }
  })
})

describe('which losses are new (4.2, U-133)', () => {
  it('U-133 a loss that was there before the edit raises nothing', async () => {
    const statuses: Record<string, FakeFeatureStatus> = { 'f-fil1': filletLost() }
    const fake = await startWithDocument('cube-features.ptsm', DOCUMENT, { core: { statuses } })
    await applyEdits(fake.app, 'Change Fillet 1', RADIUS_3)
    await settle()
    expect(notifications(fake)).toEqual([])
  })

  it('U-133 losses during a sketch session raise one notification when it ends, once', async () => {
    const { fake, statuses } = await withStatuses()
    await enterSketch(fake.app, 'f-sk1')
    await settle()
    statuses['f-fil1'] = filletLost()
    await commitSketchChange(fake.app, 'Line', {
      add: { entities: [lineEntity('e-9', [0, 0], [5, 5])], constraints: [] },
      requested: [],
      inferred: [],
    })
    await settle()
    statuses['f-fil1'] = {
      status: 'warning',
      messages: [missing('/edges/0'), missing('/edges/1', null)],
      repairs: [lostEdge('/edges/0'), lostEdge('/edges/1', "the front top edge of 'Extrude 1'")],
    }
    await commitSketchChange(fake.app, 'Line', {
      add: { entities: [lineEntity('e-10', [5, 5], [9, 0])], constraints: [] },
      requested: [],
      inferred: [],
    })
    await settle()
    expect(notifications(fake)).toEqual([])
    await finishSketch(fake.app)
    await settle()
    expect(notifications(fake)).toHaveLength(1)
    expect(notifications(fake)[0]).toMatchObject({
      code: 'feature_lost',
      message: MESSAGES.feature_lost('Fillet 1', BEVEL_EDGE),
      problems: ['f-fil1 reference_missing /edges/0', 'f-fil1 reference_missing /edges/1'],
    })
  })

  it('U-133 an undo that removes the losses dismisses the notification', async () => {
    const { fake, statuses } = await withStatuses()
    statuses['f-fil1'] = filletLost()
    await applyEdits(fake.app, 'Delete', RADIUS_3)
    await settle()
    expect(notifications(fake)).toHaveLength(1)
    delete statuses['f-fil1']
    await undo(fake.app)
    await settle()
    expect(notifications(fake)).toEqual([])
  })

  it('U-133 a feature command raises its loss after OK, compared with before it began, and nothing during previews', async () => {
    const { fake, statuses } = await withStatuses()
    await editFeature(fake.app, 'f-fil1')
    await settle()
    statuses['f-fil1'] = filletLost()
    setCommandField(fake.app, 'radius', '3 mm')
    await settle()
    expect(fake.transport.commandsOfType('apply_edits').length).toBeGreaterThan(0)
    expect(notifications(fake)).toEqual([])
    await confirmCommand(fake.app)
    await settle()
    expect(notifications(fake)).toMatchObject([
      { code: 'feature_lost', message: MESSAGES.feature_lost('Fillet 1', BEVEL_EDGE) },
    ])
  })
})

describe('what notifies (4.2, U-134)', () => {
  it.each([
    'reference_guessed',
    'reference_widened',
    'region_resized',
    'region_remapped',
    'sketch_turned_over',
  ])(
    `U-134 %s raises "'Fillet 1' needs a check after this change." with "Check 'Fillet 1'" and Undo`,
    async (code) => {
      const { fake, statuses } = await withStatuses()
      statuses['f-fil1'] = {
        status: 'warning',
        messages: [
          { severity: 'warning', code, message: `The ${code} message.`, field: '/edges/0' },
        ],
        repairs: [lostEdge('/edges/0')],
      }
      await applyEdits(fake.app, 'Change Fillet 1', RADIUS_3)
      await settle()
      expect(notifications(fake)).toHaveLength(1)
      const [notification] = notifications(fake)
      expect(notification).toMatchObject({
        severity: 'warning',
        code: 'feature_check',
        message: MESSAGES.feature_check('Fillet 1'),
      })
      expect(notification === undefined ? [] : actionLabels(notification)).toEqual([
        MESSAGES.check_feature('Fillet 1'),
        'Undo',
      ])
    },
  )

  it(`U-134 a face fallback of 'Sketch 2' raises "'Sketch 2' lost the top of 'Extrude 1'."`, async () => {
    const { fake, statuses } = await withStatuses()
    statuses['f-sk2'] = {
      status: 'warning',
      messages: [
        {
          severity: 'warning',
          code: 'plane_fallback',
          message: "'Sketch 2' lost the face it was on.",
          field: '/plane/face',
        },
      ],
      repairs: [
        lostEdgeRepair({
          path: '/plane/face',
          description: "the top of 'Extrude 1'",
          candidates: [],
        }),
      ],
    }
    await applyEdits(fake.app, 'Change Fillet 1', RADIUS_3)
    await settle()
    expect(notifications(fake)).toHaveLength(1)
    const [notification] = notifications(fake)
    expect(notification?.message).toBe("'Sketch 2' lost the top of 'Extrude 1'.")
    expect(notification === undefined ? [] : actionLabels(notification)).toEqual([
      "Show what 'Sketch 2' lost",
      'Undo',
    ])
  })

  it.each(['reference_followed', 'region_split'])(
    'U-134 a %s notice raises nothing',
    async (code) => {
      const { fake, statuses } = await withStatuses()
      statuses['f-fil1'] = {
        status: 'ok',
        messages: [{ severity: 'info', code, message: `The ${code} notice.`, field: '/edges/0' }],
      }
      await applyEdits(fake.app, 'Change Fillet 1', RADIUS_3)
      await settle()
      expect(notifications(fake)).toEqual([])
    },
  )
})

describe('one root, one notification (4.2, U-144)', () => {
  const REGION = "the region of 'Sketch 1' around (40, 25)"
  const BACK_LINE_DELETED: Cause = {
    text: "This started after you deleted the line from (80, 50) to (0, 50) in 'Sketch 1'.",
    actions: [{ label: 'Undo', run: { kind: 'undo' } }],
  }
  const EXTRUDE_LOST: FakeFeatureStatus = {
    status: 'error',
    messages: [
      {
        severity: 'error',
        code: 'profile_not_found',
        message:
          "The region of 'Sketch 1' selected for Profile is gone because part of its outline was deleted.",
        field: '/profile',
        cause: BACK_LINE_DELETED,
      },
    ],
    repairs: [
      {
        kind: 'region',
        path: '/profile/regions/0',
        sketch: 'f-sk1',
        region: { key: 'e-1,e-2,e-3,e-4', fp: 'c=40,25|a=4000' },
        resolution: 'missing',
        description: REGION,
        candidates: [],
      },
    ],
  }
  const FILLET_EMPTY: FakeFeatureStatus = {
    status: 'error',
    messages: [
      {
        severity: 'error',
        code: 'body_empty',
        message:
          "Body 'Cube' is empty here because 'Extrude 1', which makes it, failed. Fix 'Extrude 1' first.",
        field: '/body',
        root: 'f-ext1',
      },
    ],
  }

  it('U-144 the root and the feature that failed because of it raise one error notification', async () => {
    const { fake, statuses } = await withStatuses()
    statuses['f-ext1'] = EXTRUDE_LOST
    statuses['f-fil1'] = FILLET_EMPTY
    await applyEdits(fake.app, 'Delete', RADIUS_3)
    await settle()
    expect(notifications(fake)).toHaveLength(1)
    expect(notifications(fake)[0]).toMatchObject({
      severity: 'error',
      code: 'features_lost',
      message:
        "'Extrude 1' lost the region of 'Sketch 1' around (40, 25), so 1 feature after it failed too.",
      features: ['f-ext1', 'f-fil1'],
      cause: BACK_LINE_DELETED.text,
    })
  })

  it('U-144 a feature newly in error outside the group raises its own failure notification', async () => {
    const { fake, statuses } = await withStatuses()
    statuses['f-ext1'] = EXTRUDE_LOST
    statuses['f-fil1'] = FILLET_EMPTY
    statuses['f-sh1'] = {
      status: 'error',
      messages: [
        { severity: 'error', code: 'shell_failed', message: 'The shell could not be built.' },
      ],
    }
    await applyEdits(fake.app, 'Delete', RADIUS_3)
    await settle()
    expect(notifications(fake).map((notification) => notification.message)).toEqual([
      "'Extrude 1' lost the region of 'Sketch 1' around (40, 25), so 1 feature after it failed too.",
      MESSAGES.feature_failed('Shell 1', 'The shell could not be built.'),
    ])
  })

  it('U-144 a delete that raises a loss notification shows no "Deleted {label}." notification', async () => {
    const { fake, statuses } = await withStatuses()
    statuses['f-ext1'] = EXTRUDE_LOST
    statuses['f-fil1'] = FILLET_EMPTY
    await deleteFeatures(fake.app, ['f-sk2'])
    await settle()
    expect(notifications(fake).map((notification) => notification.code)).toEqual(['features_lost'])
  })

  it('U-144 an undo that makes them all ok dismisses both', async () => {
    const { fake, statuses } = await withStatuses()
    statuses['f-ext1'] = EXTRUDE_LOST
    statuses['f-fil1'] = FILLET_EMPTY
    statuses['f-sh1'] = {
      status: 'error',
      messages: [
        { severity: 'error', code: 'shell_failed', message: 'The shell could not be built.' },
      ],
    }
    await applyEdits(fake.app, 'Delete', RADIUS_3)
    await settle()
    expect(notifications(fake)).toHaveLength(2)
    for (const id of ['f-ext1', 'f-fil1', 'f-sh1']) {
      delete statuses[id]
    }
    await undo(fake.app)
    await settle()
    expect(notifications(fake)).toEqual([])
  })
})
