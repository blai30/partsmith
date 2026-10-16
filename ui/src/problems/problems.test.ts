import { describe, expect, it } from 'vitest'
import type { Cause, Diagnostic, Document, FeatureStatus, Repair } from '../generated/protocol'
import type { DocumentState } from '../state/store'
import { stateWithDocument } from '../test/fakes/state'
import { changeNotifications, problemRows } from './problems'

// The Problems panel's rows grouped by root cause (ui.md 3.9) and the notifications a change
// raises for new losses (4.2, 3.22), from statuses alone.

/** A sketch, its extrude, a fillet, a datum plane, a second sketch on it, its extrude and a box. */
const DOCUMENT: Document = {
  format: 'partsmith',
  format_version: 2,
  units: 'mm',
  parameters: [],
  bodies: [
    { id: 'b-1', name: 'Body 1' },
    { id: 'b-2', name: 'Body 2' },
  ],
  features: [
    {
      id: 'f-sk1',
      type: 'sketch',
      plane: { kind: 'origin', plane: 'XY' },
      entities: [],
      constraints: [],
    },
    {
      id: 'f-ext1',
      type: 'extrude',
      body: 'b-1',
      profile: { sketch: 'f-sk1', regions: [] },
      extent: { kind: 'distance', expr: '30 mm' },
      op: 'new',
    },
    { id: 'f-fil1', type: 'fillet', body: 'b-1', edges: [], radius: '2 mm' },
    { id: 'f-dp1', type: 'datum_plane', base: { kind: 'origin', plane: 'XY' }, offset: '30 mm' },
    {
      id: 'f-sk2',
      type: 'sketch',
      plane: { kind: 'datum', feature: 'f-dp1' },
      entities: [],
      constraints: [],
    },
    {
      id: 'f-ext2',
      type: 'extrude',
      body: 'b-1',
      profile: { sketch: 'f-sk2', regions: [] },
      extent: { kind: 'distance', expr: '5 mm' },
      op: 'join',
    },
    { id: 'f-box2', type: 'box', body: 'b-2', size: ['10 mm', '10 mm', '10 mm'], op: 'new' },
  ],
}

function message(
  severity: Diagnostic['severity'],
  code: string,
  fields: Partial<Diagnostic> = {},
): Diagnostic {
  return { severity, code, message: `The ${code} message.`, ...fields }
}

function status(
  id: string,
  state: FeatureStatus['status'],
  messages: readonly Diagnostic[] = [],
  repairs: readonly Repair[] = [],
): FeatureStatus {
  return { id, status: state, messages: [...messages], repairs: [...repairs] }
}

function okStatuses(): Record<string, FeatureStatus> {
  return Object.fromEntries(
    (DOCUMENT.features ?? []).map((feature) => [feature.id, status(feature.id, 'ok')]),
  )
}

/** The ok statuses with these statuses in place of their features'. */
function statuses(...changed: readonly FeatureStatus[]): Record<string, FeatureStatus> {
  return { ...okStatuses(), ...Object.fromEntries(changed.map((entry) => [entry.id, entry])) }
}

function elementRepair(path: string, description: string): Repair {
  return {
    kind: 'element',
    path,
    body: 'b-1',
    reference: { q: 'edge(f-ext1/end-cap|f-ext1/lateral/e-5)', fp: 'fp' },
    resolution: 'missing',
    description,
    candidates: [],
  }
}

function regionRepair(path: string, description: string): Repair {
  return {
    kind: 'region',
    path,
    sketch: 'f-sk1',
    region: { key: 'e-1,e-2,e-3,e-4', fp: 'c=40,25|a=4000' },
    resolution: 'missing',
    description,
    candidates: [],
  }
}

const LINE_DELETED: Cause = {
  text: "This started after you deleted the line from (80, 40) to (70, 50) in 'Sketch 1'.",
  actions: [
    { label: 'Undo', run: { kind: 'undo' } },
    { label: 'Show the line', run: { kind: 'show', feature: 'f-sk1', entity: 'e-5' } },
  ],
}

const BEVEL_EDGE = "the top edge of 'Extrude 1' along a line of 'Sketch 1'"

/** U-132's 'Fillet 1' after the edit: a warning that its first edge is missing. */
const FILLET_LOST = status(
  'f-fil1',
  'warning',
  [message('warning', 'reference_missing', { field: '/edges/0', cause: LINE_DELETED })],
  [elementRepair('/edges/0', BEVEL_EDGE)],
)

function stateWith(
  featureStatus: Record<string, FeatureStatus>,
  diagnostics: readonly Diagnostic[] = [],
) {
  const changes: Partial<DocumentState> = { featureStatus, diagnostics: [...diagnostics] }
  return stateWithDocument(DOCUMENT, {}, changes)
}

describe('problemRows (3.9)', () => {
  const DATUM_LOST = status('f-dp1', 'error', [
    message('error', 'reference_not_found', { field: '/base/face' }),
  ])
  const UPSTREAM = message('error', 'upstream_failed', { root: 'f-dp1' })

  it('U-129 nests features whose first message has a root under that root, in timeline order', () => {
    const rows = problemRows(
      stateWith(
        statuses(
          DATUM_LOST,
          status('f-sk2', 'error', [UPSTREAM]),
          status('f-ext2', 'error', [UPSTREAM]),
          status('f-box2', 'warning', [message('warning', 'unknown_params')]),
        ),
      ),
    )
    expect(rows.rows).toEqual([
      {
        feature: 'f-dp1',
        messages: DATUM_LOST.messages,
        nested: [
          { feature: 'f-sk2', root: 'f-dp1', messages: [UPSTREAM] },
          { feature: 'f-ext2', root: 'f-dp1', messages: [UPSTREAM] },
        ],
      },
      { feature: 'f-box2', messages: [message('warning', 'unknown_params')], nested: [] },
    ])
  })

  it('U-129 lists info notices as notes, not as problems', () => {
    const note = message('info', 'reference_followed', { field: '/edges/0' })
    const rows = problemRows(stateWith(statuses(status('f-fil1', 'ok', [note]))))
    expect(rows.rows).toEqual([])
    expect(rows.notes).toEqual([{ feature: 'f-fil1', message: note }])
  })

  it("keeps a warning feature's info notice out of its row and in the notes", () => {
    const warning = message('warning', 'reference_guessed', { field: '/edges/0' })
    const note = message('info', 'reference_followed', { field: '/edges/1' })
    const rows = problemRows(stateWith(statuses(status('f-fil1', 'warning', [warning, note]))))
    expect(rows.rows).toEqual([{ feature: 'f-fil1', messages: [warning], nested: [] }])
    expect(rows.notes).toEqual([{ feature: 'f-fil1', message: note }])
  })

  it('lets a root whose status is ok head its group', () => {
    const noEffect = message('warning', 'no_effect', { root: 'f-sk2' })
    const rows = problemRows(stateWith(statuses(status('f-ext2', 'warning', [noEffect]))))
    expect(rows.rows).toEqual([
      {
        feature: 'f-sk2',
        messages: [],
        nested: [{ feature: 'f-ext2', root: 'f-sk2', messages: [noEffect] }],
      },
    ])
  })

  it('lists a feature at the top level when its root is not in the document', () => {
    const orphan = message('error', 'upstream_failed', { root: 'f-gone' })
    const rows = problemRows(stateWith(statuses(status('f-ext2', 'error', [orphan]))))
    expect(rows.rows).toEqual([{ feature: 'f-ext2', messages: [orphan], nested: [] }])
  })

  it('follows the features with one row per document diagnostic', () => {
    const unused = message('warning', 'unused_parameter')
    const profile = message('warning', 'unknown_printer_profile')
    const rows = problemRows(stateWith(statuses(DATUM_LOST), [unused, profile]))
    expect(rows.rows.map((row) => row.feature)).toEqual(['f-dp1', null, null])
    expect(rows.rows.slice(1)).toEqual([
      { feature: null, messages: [unused], nested: [] },
      { feature: null, messages: [profile], nested: [] },
    ])
  })
})

describe('changeNotifications (4.2, 3.22)', () => {
  it('U-132 a new lost reference is one warning notice naming the feature, the element and the cause', () => {
    expect(changeNotifications(okStatuses(), statuses(FILLET_LOST), DOCUMENT)).toEqual([
      {
        kind: 'lost',
        root: 'f-fil1',
        description: BEVEL_EDGE,
        failed: 0,
        severity: 'warning',
        cause: LINE_DELETED,
        features: ['f-fil1'],
        problems: ['f-fil1 reference_missing /edges/0'],
      },
    ])
  })

  it('U-133 a loss whose key was in the statuses before raises nothing', () => {
    expect(changeNotifications(statuses(FILLET_LOST), statuses(FILLET_LOST), DOCUMENT)).toEqual([])
  })

  it('U-133 the same code at another field is a new loss', () => {
    const second = status(
      'f-fil1',
      'warning',
      [...FILLET_LOST.messages, message('warning', 'reference_missing', { field: '/edges/1' })],
      [...FILLET_LOST.repairs, elementRepair('/edges/1', "the front top edge of 'Extrude 1'")],
    )
    expect(changeNotifications(statuses(FILLET_LOST), statuses(second), DOCUMENT)).toMatchObject([
      {
        kind: 'lost',
        root: 'f-fil1',
        description: "the front top edge of 'Extrude 1'",
        problems: ['f-fil1 reference_missing /edges/1'],
      },
    ])
  })

  it.each([
    'reference_guessed',
    'reference_widened',
    'region_resized',
    'region_remapped',
    'sketch_turned_over',
  ])('U-134 %s asks for a check: nothing is lost', (code) => {
    const checked = status(
      'f-fil1',
      'warning',
      [message('warning', code, { field: '/edges/0' })],
      [elementRepair('/edges/0', BEVEL_EDGE)],
    )
    expect(changeNotifications(okStatuses(), statuses(checked), DOCUMENT)).toEqual([
      {
        kind: 'check',
        root: 'f-fil1',
        cause: null,
        features: ['f-fil1'],
        problems: [`f-fil1 ${code} /edges/0`],
      },
    ])
  })

  it("U-134 a sketch face fallback with an element repair is a loss of the sketch's face", () => {
    const fallback = status(
      'f-sk2',
      'warning',
      [message('warning', 'plane_fallback', { field: '/plane/face' })],
      [elementRepair('/plane/face', "the top of 'Extrude 1'")],
    )
    expect(changeNotifications(okStatuses(), statuses(fallback), DOCUMENT)).toMatchObject([
      { kind: 'lost', root: 'f-sk2', description: "the top of 'Extrude 1'", severity: 'warning' },
    ])
  })

  it.each(['reference_followed', 'region_split'])('U-134 a %s notice raises nothing', (code) => {
    const notice = status('f-fil1', 'ok', [message('info', code, { field: '/edges/0' })])
    expect(changeNotifications(okStatuses(), statuses(notice), DOCUMENT)).toEqual([])
  })

  it('U-134 a no_effect warning under a sketch that lost its face before raises nothing', () => {
    const fallback = status(
      'f-sk2',
      'warning',
      [message('warning', 'plane_fallback', { field: '/plane/face' })],
      [elementRepair('/plane/face', "the top of 'Extrude 1'")],
    )
    const noEffect = status('f-ext2', 'warning', [
      message('warning', 'no_effect', { root: 'f-sk2' }),
    ])
    expect(changeNotifications(statuses(fallback), statuses(fallback, noEffect), DOCUMENT)).toEqual(
      [],
    )
  })

  it('a loss without repair data asks for a check', () => {
    const lost = status('f-fil1', 'warning', [
      message('warning', 'reference_missing', { field: '/edges/0' }),
    ])
    expect(changeNotifications(okStatuses(), statuses(lost), DOCUMENT)).toMatchObject([
      { kind: 'check', root: 'f-fil1' },
    ])
  })

  it("describes the loss by the repair at the message's field", () => {
    const lost = status(
      'f-fil1',
      'warning',
      [message('warning', 'reference_missing', { field: '/edges/1' })],
      [elementRepair('/edges/0', 'the first edge'), elementRepair('/edges/1', 'the second edge')],
    )
    expect(changeNotifications(okStatuses(), statuses(lost), DOCUMENT)).toMatchObject([
      { kind: 'lost', description: 'the second edge' },
    ])
  })

  it('describes the loss by the first repair when none is at its field', () => {
    const lost = status(
      'f-fil1',
      'error',
      [message('error', 'references_not_found', { field: '/edges' })],
      [elementRepair('/edges/0', 'the first edge'), elementRepair('/edges/1', 'the second edge')],
    )
    expect(changeNotifications(okStatuses(), statuses(lost), DOCUMENT)).toMatchObject([
      { kind: 'lost', description: 'the first edge', severity: 'error' },
    ])
  })

  describe('one root, one notice (U-144, newcomer corpus NC11)', () => {
    const REGION = "the region of 'Sketch 1' around (40, 25)"
    const BACK_LINE_DELETED: Cause = {
      text: "This started after you deleted the line from (80, 50) to (0, 50) in 'Sketch 1'.",
      actions: [{ label: 'Undo', run: { kind: 'undo' } }],
    }
    const EXTRUDE_LOST = status(
      'f-ext1',
      'error',
      [message('error', 'profile_not_found', { field: '/profile', cause: BACK_LINE_DELETED })],
      [regionRepair('/profile/regions/0', REGION)],
    )
    const FILLET_EMPTY = status('f-fil1', 'error', [
      message('error', 'body_empty', { field: '/body', root: 'f-ext1' }),
    ])

    it('U-144 a root and a feature that failed because of it give one error notice', () => {
      expect(
        changeNotifications(okStatuses(), statuses(EXTRUDE_LOST, FILLET_EMPTY), DOCUMENT),
      ).toEqual([
        {
          kind: 'lost',
          root: 'f-ext1',
          description: REGION,
          failed: 1,
          severity: 'error',
          cause: BACK_LINE_DELETED,
          features: ['f-ext1', 'f-fil1'],
          problems: ['f-ext1 profile_not_found /profile'],
        },
      ])
    })

    it('U-144 a feature newly in error outside the group gets one failed notice, last', () => {
      const box = status('f-box2', 'error', [message('error', 'kernel')])
      expect(
        changeNotifications(okStatuses(), statuses(EXTRUDE_LOST, FILLET_EMPTY, box), DOCUMENT),
      ).toEqual([
        expect.objectContaining({ kind: 'lost', root: 'f-ext1', features: ['f-ext1', 'f-fil1'] }),
        { kind: 'failed', features: ['f-box2'] },
      ])
    })

    it('a feature that fails because of a root without a new loss is a failure of its own', () => {
      expect(
        changeNotifications(statuses(EXTRUDE_LOST), statuses(EXTRUDE_LOST, FILLET_EMPTY), DOCUMENT),
      ).toEqual([{ kind: 'failed', features: ['f-fil1'] }])
    })
  })

  it('orders the notices by the timeline position of their roots', () => {
    const fallback = status(
      'f-sk2',
      'warning',
      [message('warning', 'plane_fallback', { field: '/plane/face' })],
      [elementRepair('/plane/face', "the top of 'Extrude 1'")],
    )
    const roots = changeNotifications(okStatuses(), statuses(fallback, FILLET_LOST), DOCUMENT).map(
      (notice) => (notice.kind === 'failed' ? null : notice.root),
    )
    expect(roots).toEqual(['f-fil1', 'f-sk2'])
  })
})
