import { describe, expect, it } from 'vitest'
import type { Diagnostic, FeatureStatus } from '../generated/protocol'
import { DEFAULT_SETTINGS } from '../settings/settings'
import {
  CUBE_DOCUMENT,
  CUBE_FEATURES_DOCUMENT,
  emptyDocument,
  withFeatureChanges,
} from '../test/fakes/documents'
import { checkResult, finding, printReport, stateWithDocument } from '../test/fakes/state'
import {
  editingSession,
  selectActiveFeatureIds,
  selectEditedSketch,
  selectEmptyState,
  selectFeatureLabel,
  selectHealthBadge,
  selectOriginVisible,
  selectProblemCount,
  selectProblems,
  selectUnsaved,
} from './selectors'
import type { ActiveCommand, AppState, DocumentState, PrintState, SketchSession } from './store'

describe('selectEmptyState (3.2)', () => {
  const COMMAND: ActiveCommand = {
    featureType: 'box',
    featureId: 'f-box1',
    editing: false,
    title: 'Box',
    params: {},
    activeField: null,
    fieldErrors: {},
    applied: false,
    newBody: null,
    helper: null,
    changedPaths: [],
    history: [],
    future: [],
    preview: 'none',
  }
  const PICKING_PLANE: SketchSession = {
    sketchId: null,
    reattaching: null,
    tool: null,
    construction: false,
    showConstraints: true,
    selection: [],
    dragging: false,
    cameraBefore: null,
  }

  it('U-95 is true for a ready core with a recipe without features, no command and no sketch session', () => {
    expect(selectEmptyState(stateWithDocument(emptyDocument()))).toBe(true)
  })

  it('U-95 is false with one feature', () => {
    const recipe = { ...emptyDocument(), features: CUBE_DOCUMENT.features?.slice(0, 1) ?? [] }
    expect(selectEmptyState(stateWithDocument(recipe))).toBe(false)
  })

  it('U-95 is false with an active command', () => {
    expect(selectEmptyState(stateWithDocument(emptyDocument(), { command: COMMAND }))).toBe(false)
  })

  it('U-95 is false during plane picking', () => {
    const state = stateWithDocument(emptyDocument(), { context: 'sketch', sketch: PICKING_PLANE })
    expect(selectEmptyState(state)).toBe(false)
  })

  it('U-95 is false while the core is loading', () => {
    const state = stateWithDocument(emptyDocument(), {
      core: {
        status: 'loading',
        engineVersion: null,
        fatal: null,
        pendingMutations: 0,
        stillWorking: false,
      },
    })
    expect(selectEmptyState(state)).toBe(false)
  })

  it('is false without a document', () => {
    expect(selectEmptyState({ ...stateWithDocument(emptyDocument()), document: null })).toBe(false)
  })
})

function withVisible(state: AppState, sessionVisible: Record<string, boolean>): AppState {
  return { ...state, view: { ...state.view, sessionVisible } }
}

describe('selectOriginVisible (3.10)', () => {
  it('U-96 with no features shows the axes and the origin point, not the planes', () => {
    const state = stateWithDocument(emptyDocument())
    expect(selectOriginVisible(state, 'origin:X')).toBe(true)
    expect(selectOriginVisible(state, 'origin:Y')).toBe(true)
    expect(selectOriginVisible(state, 'origin:Z')).toBe(true)
    expect(selectOriginVisible(state, 'origin:O')).toBe(true)
    expect(selectOriginVisible(state, 'origin:XY')).toBe(false)
  })

  it('U-96 with one feature hides origin:X', () => {
    expect(selectOriginVisible(stateWithDocument(CUBE_DOCUMENT), 'origin:X')).toBe(false)
  })

  it('U-96 an entry {origin:X: true} shows it with features', () => {
    const state = withVisible(stateWithDocument(CUBE_DOCUMENT), { 'origin:X': true })
    expect(selectOriginVisible(state, 'origin:X')).toBe(true)
  })

  it('U-96 an entry {origin:Z: false} hides it with none', () => {
    const state = withVisible(stateWithDocument(emptyDocument()), { 'origin:Z': false })
    expect(selectOriginVisible(state, 'origin:Z')).toBe(false)
  })
})

describe('selectFeatureLabel', () => {
  it('labels features with the catalog (features.md 3.12)', () => {
    const state = stateWithDocument(CUBE_DOCUMENT)
    expect(selectFeatureLabel(state, 'f-sk1')).toBe('Sketch 1')
    expect(selectFeatureLabel(state, 'f-ext1')).toBe('Extrude 1')
  })

  it('shows a record name', () => {
    const state = stateWithDocument(withFeatureChanges(CUBE_DOCUMENT, 'f-ext1', { name: 'Base' }))
    expect(selectFeatureLabel(state, 'f-ext1')).toBe('Base')
  })
})

describe('selectActiveFeatureIds', () => {
  it('lists every feature in timeline order when nothing is rolled back', () => {
    expect(selectActiveFeatureIds(stateWithDocument(CUBE_FEATURES_DOCUMENT))).toEqual([
      'f-sk1',
      'f-ext1',
      'f-fil1',
      'f-sh1',
    ])
  })

  it('stops before the rollback feature', () => {
    const state = stateWithDocument({ ...CUBE_FEATURES_DOCUMENT, rollback: 'f-fil1' })
    expect(selectActiveFeatureIds(state)).toEqual(['f-sk1', 'f-ext1'])
  })

  it('is empty without a document', () => {
    expect(selectActiveFeatureIds({ ...stateWithDocument(CUBE_DOCUMENT), document: null })).toEqual(
      [],
    )
  })
})

describe('editingSession and selectEditedSketch', () => {
  const SESSION: SketchSession = {
    sketchId: 'f-sk1',
    reattaching: null,
    tool: null,
    construction: false,
    showConstraints: true,
    selection: [],
    dragging: false,
    cameraBefore: null,
  }

  it('give the edited sketch in sketch mode', () => {
    const state = stateWithDocument(CUBE_DOCUMENT, { context: 'sketch', sketch: SESSION })
    expect(editingSession(state)?.sketchId).toBe('f-sk1')
    expect(selectEditedSketch(state)).toBe('f-sk1')
  })

  it('while a plane is picked: no editing session and a null sketch', () => {
    const state = stateWithDocument(CUBE_DOCUMENT, {
      context: 'sketch',
      sketch: { ...SESSION, sketchId: null },
    })
    expect(editingSession(state)).toBeNull()
    expect(selectEditedSketch(state)).toBeNull()
  })

  it('outside sketch mode: no editing session and an undefined sketch', () => {
    const state = stateWithDocument(CUBE_DOCUMENT, { sketch: SESSION })
    expect(editingSession(state)).toBeNull()
    expect(selectEditedSketch(state)).toBeUndefined()
  })
})

describe('selectUnsaved (3.19)', () => {
  it('is true when the core reports the document dirty or a recovered copy is not saved', () => {
    expect(selectUnsaved(stateWithDocument(CUBE_DOCUMENT, {}, { dirty: true }))).toBe(true)
    expect(selectUnsaved(stateWithDocument(CUBE_DOCUMENT, {}, { localUnsaved: true }))).toBe(true)
  })

  it('is false for a saved document and without a document', () => {
    expect(selectUnsaved(stateWithDocument(CUBE_DOCUMENT))).toBe(false)
    expect(selectUnsaved({ ...stateWithDocument(CUBE_DOCUMENT), document: null })).toBe(false)
  })
})

function withPrint(print: Partial<PrintState>, hiddenChecks: readonly string[] = []): AppState {
  const state = stateWithDocument(CUBE_DOCUMENT, {}, { revision: 4 })
  return {
    ...state,
    print: { ...state.print, ...print },
    settings: { ...DEFAULT_SETTINGS, hiddenChecks },
  }
}

describe('selectHealthBadge (U-81)', () => {
  it('is none without a report', () => {
    expect(selectHealthBadge(withPrint({}))).toMatchObject({ state: 'none' })
  })

  it('is running while an analysis is in flight', () => {
    expect(selectHealthBadge(withPrint({ running: true }))).toMatchObject({ state: 'running' })
  })

  it('is ok when the report has no errors or warnings, info findings included', () => {
    const report = printReport({
      'b-cube': [checkResult('bed_fit', []), checkResult('islands', [finding('islands', 'info')])],
    })
    expect(selectHealthBadge(withPrint({ report, reportRevision: 4 }))).toMatchObject({
      state: 'ok',
      errors: 0,
      warnings: 0,
      stale: false,
    })
  })

  it('is warning with the warning count when there are warnings and no errors', () => {
    const report = printReport({
      'b-cube': [
        checkResult('overhangs', [
          finding('overhangs', 'warning'),
          finding('overhangs', 'warning'),
        ]),
        checkResult('thin_walls', [finding('thin_walls', 'warning')]),
      ],
    })
    expect(selectHealthBadge(withPrint({ report, reportRevision: 4 }))).toMatchObject({
      state: 'warning',
      errors: 0,
      warnings: 3,
    })
  })

  it('is error with both counts when there are errors, summed over bodies', () => {
    const report = printReport({
      'b-cube': [checkResult('bed_fit', [finding('bed_fit', 'error')])],
      'b-peg': [checkResult('overhangs', [finding('overhangs', 'warning')])],
    })
    expect(selectHealthBadge(withPrint({ report, reportRevision: 4 }))).toMatchObject({
      state: 'error',
      errors: 1,
      warnings: 1,
    })
  })

  it("adds each check's omitted findings to the count of its listed findings' severity", () => {
    const report = printReport({
      'b-cube': [checkResult('overhangs', [finding('overhangs', 'warning')], 4)],
    })
    expect(selectHealthBadge(withPrint({ report, reportRevision: 4 }))).toMatchObject({
      state: 'warning',
      warnings: 5,
    })
  })

  it('U-81 leaves hidden checks out of the counts', () => {
    const report = printReport({
      'b-cube': [
        checkResult('overhangs', [finding('overhangs', 'warning')], 2),
        checkResult('bed_fit', [finding('bed_fit', 'error')]),
      ],
    })
    expect(selectHealthBadge(withPrint({ report, reportRevision: 4 }, ['bed_fit']))).toMatchObject({
      state: 'warning',
      errors: 0,
      warnings: 3,
    })
    expect(
      selectHealthBadge(withPrint({ report, reportRevision: 4 }, ['bed_fit', 'overhangs'])),
    ).toMatchObject({ state: 'ok', errors: 0, warnings: 0 })
  })

  it('U-81 always counts mesh validity, even when stored settings list it as hidden', () => {
    const report = printReport({
      'b-cube': [checkResult('mesh_validity', [finding('mesh_validity', 'error')])],
    })
    expect(
      selectHealthBadge(withPrint({ report, reportRevision: 4 }, ['mesh_validity'])),
    ).toMatchObject({ state: 'error', errors: 1 })
  })

  it('U-81 a report for an older revision is stale and keeps its state', () => {
    const report = printReport({
      'b-cube': [checkResult('overhangs', [finding('overhangs', 'warning')])],
    })
    expect(selectHealthBadge(withPrint({ report, reportRevision: 3 }))).toMatchObject({
      state: 'warning',
      stale: true,
    })
    expect(selectHealthBadge(withPrint({ report, reportRevision: 4 }))).toMatchObject({
      stale: false,
    })
  })
})

/** The state with the document changed as a mesh or rebuild-time update changes it. */
function remeshed(state: AppState): AppState {
  const document = state.document
  if (document === null) {
    return state
  }
  return {
    ...state,
    document: {
      ...document,
      revision: document.revision + 1,
      lastRebuildMilliseconds: 42,
      meshes: {},
    },
  }
}

describe('selectProblems', () => {
  const FILLET_ERROR: FeatureStatus = {
    id: 'f-fil1',
    status: 'error',
    messages: [
      { severity: 'error', code: 'fillet_too_large', message: 'The fillet is too large.' },
    ],
    repairs: [],
  }
  const SHELL_WARNING: FeatureStatus = {
    id: 'f-sh1',
    status: 'warning',
    messages: [{ severity: 'warning', code: 'thin_wall', message: 'A wall is thin.' }],
    repairs: [],
  }
  const UNUSED: Diagnostic = {
    severity: 'warning',
    code: 'unused_parameter',
    message: 'The parameter width is not used.',
  }

  function withProblems(changes: Partial<DocumentState> = {}): AppState {
    return stateWithDocument(
      CUBE_FEATURES_DOCUMENT,
      {},
      {
        featureStatus: {
          'f-ext1': { id: 'f-ext1', status: 'ok', messages: [], repairs: [] },
          'f-sh1': SHELL_WARNING,
          'f-fil1': FILLET_ERROR,
        },
        diagnostics: [UNUSED],
        ...changes,
      },
    )
  }

  it('lists failing and warning features in timeline order, then the document diagnostics (3.9)', () => {
    expect(selectProblems(withProblems())).toEqual({
      rows: [
        { feature: 'f-fil1', messages: FILLET_ERROR.messages, nested: [] },
        { feature: 'f-sh1', messages: SHELL_WARNING.messages, nested: [] },
        { feature: null, messages: [UNUSED], nested: [] },
      ],
      notes: [],
    })
  })

  it('gives the same list back when only meshes or the rebuild time changed', () => {
    const state = withProblems()
    const problems = selectProblems(state)
    expect(selectProblems(remeshed(state))).toBe(problems)
  })

  it('gives a new list when the statuses change', () => {
    const state = withProblems()
    const problems = selectProblems(state)
    const fixed = withProblems({ featureStatus: {}, diagnostics: [] })
    expect(selectProblems(fixed)).not.toBe(problems)
    expect(selectProblems(fixed)).toEqual({ rows: [], notes: [] })
  })

  it('is empty without a document', () => {
    expect(selectProblems({ ...withProblems(), document: null })).toEqual({ rows: [], notes: [] })
  })
})

/** cube-features.ptsm with only these statuses and diagnostics. */
function withStatuses(statuses: readonly FeatureStatus[], diagnostics: readonly Diagnostic[] = []) {
  return stateWithDocument(
    CUBE_FEATURES_DOCUMENT,
    {},
    {
      featureStatus: Object.fromEntries(statuses.map((status) => [status.id, status])),
      diagnostics: [...diagnostics],
    },
  )
}

describe('selectProblemCount (3.9)', () => {
  const ROOT_ERROR: FeatureStatus = {
    id: 'f-ext1',
    status: 'error',
    messages: [{ severity: 'error', code: 'profile_not_found', message: 'The region is gone.' }],
    repairs: [],
  }
  const FAILED_AFTER_ROOT: FeatureStatus = {
    id: 'f-fil1',
    status: 'error',
    messages: [
      { severity: 'error', code: 'body_empty', message: 'The body is empty.', root: 'f-ext1' },
    ],
    repairs: [],
  }
  const NOTE: FeatureStatus = {
    id: 'f-sh1',
    status: 'ok',
    messages: [{ severity: 'info', code: 'reference_followed', message: 'It follows the face.' }],
    repairs: [],
  }

  it('counts one problem per root, not its nested features', () => {
    expect(selectProblemCount(withStatuses([ROOT_ERROR, FAILED_AFTER_ROOT]))).toBe(1)
  })

  it('counts each document diagnostic', () => {
    const unused: Diagnostic = { severity: 'warning', code: 'unused_parameter', message: 'Unused.' }
    expect(selectProblemCount(withStatuses([ROOT_ERROR], [unused, unused]))).toBe(3)
  })

  it('does not count notes', () => {
    expect(selectProblemCount(withStatuses([NOTE]))).toBe(0)
  })
})
