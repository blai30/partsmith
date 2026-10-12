import { describe, expect, it } from 'vitest'
import type { FeatureStatus, PrintReport } from '../generated/protocol'
import { documentState, meshSummary, readyState, CUBE_BOUNDS } from '../layout/fixtures.testing'
import { createAppStore, INITIAL_STATE, type AppState } from '../state/store'
import { CUBE_FEATURES_DOCUMENT, CUBE_TOP_FACE } from '../test/fakes/documents'
import { exportFailedText, exportStatus, type ExportStatusLine } from './status'

// What an export is about to write (ui.md 3.18).

function failing(id: string): FeatureStatus {
  return {
    id,
    status: 'error',
    messages: [{ severity: 'error', code: 'fillet_too_large', message: 'Too large.' }],
    repairs: [],
  }
}

/** cube-features.ptsm with the given features in error. */
function stateWith(failed: readonly string[], overrides: Partial<AppState> = {}): AppState {
  const document = documentState(CUBE_FEATURES_DOCUMENT, {
    name: 'cube',
    revision: 4,
    meshes: { 'b-cube': meshSummary(1, CUBE_BOUNDS) },
  })
  const featureStatus = { ...document.featureStatus }
  for (const id of failed) {
    featureStatus[id] = failing(id)
  }
  return createAppStore(readyState({ ...document, featureStatus }, overrides)).getState()
}

const OK_REPORT: PrintReport = {
  profile: 'generic-fdm',
  orientation: { kind: 'as_modeled' },
  summary: { health: 'ok', errors: 0, warnings: 0, infos: 0, label: 'Ready to print' },
  bodies: [],
}

function texts(lines: readonly ExportStatusLine[]): readonly string[] {
  return lines.map((line) => line.text)
}

describe('exportStatus (U-119)', () => {
  it('U-119 for 3MF with f-fil1 in error and no report names the failure and "Print check: Not checked"', () => {
    const lines = exportStatus(stateWith(['f-fil1']), '3mf')
    expect(texts(lines)).toEqual([
      'Fillet 1 failed, so it is not in the export.',
      'Print check: Not checked',
    ])
    expect(lines.map((line) => line.kind)).toEqual(['failed', 'print_check'])
    expect(lines[1]).toMatchObject({ checkOffered: true })
  })

  it('U-119 counts two features in error', () => {
    expect(texts(exportStatus(stateWith(['f-fil1', 'f-sh1']), '3mf'))[0]).toBe(
      '2 features failed, so they are not in the export.',
    )
  })

  it('U-119 with no errors and a current ok report gives the one line "Print check: Ready to print"', () => {
    const state = stateWith([], {
      print: { ...INITIAL_STATE.print, report: OK_REPORT, reportRevision: 4 },
    })
    const lines = exportStatus(state, 'stl')
    expect(texts(lines)).toEqual(['Print check: Ready to print'])
    expect(lines[0]).toMatchObject({ checkOffered: false })
  })

  it('U-119 for OBJ with no errors gives no line', () => {
    expect(exportStatus(stateWith([]), 'obj')).toEqual([])
    expect(texts(exportStatus(stateWith(['f-fil1']), 'step'))).toEqual([
      'Fillet 1 failed, so it is not in the export.',
    ])
  })

  it('U-119 with a face-down orientation set, 3MF adds the orientation_set line', () => {
    const state = stateWith([], {
      print: {
        ...INITIAL_STATE.print,
        orientation: { kind: 'face_down', body: 'b-cube', face: CUBE_TOP_FACE.reference },
      },
    })
    const lines = exportStatus(state, '3mf')
    expect(lines.at(-1)).toEqual({
      kind: 'orientation',
      text: 'Printing with the picked face of Cube on the bed.',
    })
    expect(exportStatus(state, 'obj')).toEqual([])
  })

  it('marks an out-of-date report and offers to check again', () => {
    const state = stateWith([], {
      print: { ...INITIAL_STATE.print, report: OK_REPORT, reportRevision: 3 },
    })
    const [line] = exportStatus(state, '3mf')
    expect(line).toMatchObject({
      text: 'Print check: Ready to print (out of date)',
      checkOffered: true,
    })
  })

  it('leaves out failed features after the rollback marker', () => {
    const state = stateWith(['f-sh1'])
    const rolledBack = {
      ...state,
      document:
        state.document === null
          ? null
          : {
              ...state.document,
              recipe: { ...state.document.recipe, rollback: 'f-sh1' },
            },
    }
    expect(exportFailedText(rolledBack)).toBeNull()
  })
})
