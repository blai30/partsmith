import { describe, expect, it } from 'vitest'
import { DEFAULT_SETTINGS } from '../settings/settings'
import { CUBE_DOCUMENT, CUBE_FEATURES_DOCUMENT, withFeatureChanges } from '../test/fakes/documents'
import { checkResult, finding, printReport, stateWithDocument } from '../test/fakes/state'
import { selectActiveFeatureIds, selectFeatureLabel, selectHealthBadge } from './selectors'
import type { AppState, PrintState } from './store'

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
