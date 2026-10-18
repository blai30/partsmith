import { cleanup, screen, within } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { installPopoverPolyfill } from '../components/popover-polyfill.testing'
import type { Diagnostic, Document, FeatureStatus, FixRef } from '../generated/protocol'
import { documentState, readyState } from '../layout/fixtures.testing'
import { createModelApp, renderWithApp, stubMatchMedia } from '../layout/test-app.testing'
import type { ActiveCommand, DocumentState } from '../state/store'
import { CUBE_FEATURES_DOCUMENT, CUBE_TOP_FRONT_EDGE, featuresOf } from '../test/fakes/documents'
import { CUBE_BOUNDS, meshSummary } from '../test/fakes/state'
import { COLOR_CLASSES } from '../theme/roles'

// Limits under fields (ui.md 3.7, U-139): while a fillet, chamfer or shell command's preview
// status has a "too large" message with a suggestion, the field the message is about shows
// "Largest that works: {suggestion}" under its error, in warningText, with the warningMark border
// instead of the errorMark one; the message's actions stay in the banner.

beforeEach(installPopoverPolyfill)

beforeEach(() => {
  stubMatchMedia()
})

afterEach(() => {
  cleanup()
  vi.unstubAllGlobals()
})

const FILLET_EDGES = featuresOf(CUBE_FEATURES_DOCUMENT)[2]?.edges as readonly unknown[]

/** A command editing a feature of cube-features.ptsm with these params, its preview failed. */
function editing(featureType: string, featureId: string, title: string, params: object) {
  const command: ActiveCommand = {
    featureType,
    featureId,
    editing: true,
    title,
    params: { body: 'b-cube', ...params },
    activeField: null,
    fieldErrors: {},
    applied: true,
    newBody: null,
    helper: null,
    changedPaths: [],
    history: [],
    future: [],
    preview: 'failed',
  }
  return command
}

function editingFillet(radius: string): ActiveCommand {
  return editing('fillet', 'f-fil1', 'Fillet 1', { edges: FILLET_EDGES, radius })
}

/** The preview status of `feature`: an error with these messages. */
function failing(feature: string, messages: readonly Diagnostic[]): FeatureStatus {
  return { id: feature, status: 'error', messages: [...messages], repairs: [] }
}

/** cube-features.ptsm focused on `status`'s feature in a transaction, as during its command. */
function previewing(
  status: FeatureStatus,
  recipe: Document = CUBE_FEATURES_DOCUMENT,
): DocumentState {
  const document = documentState(recipe, {
    name: 'cube-features',
    meshes: { 'b-cube': meshSummary(1, CUBE_BOUNDS) },
    focus: status.id,
    transactionOpen: true,
  })
  return { ...document, featureStatus: { ...document.featureStatus, [status.id]: status } }
}

function renderCommand(command: ActiveCommand, document: DocumentState): void {
  const test = createModelApp({ state: readyState(document, { command }) })
  renderWithApp(test.app)
}

function field(path: string): HTMLElement {
  const found = within(screen.getByTestId('inspector'))
    .getAllByTestId('inspector-field')
    .find((element) => element.getAttribute('data-path') === path)
  if (found === undefined) {
    throw new Error(`No inspector field ${path}`)
  }
  return found
}

/** The expression input of a field, a combobox while it offers parameter names (2.13). */
function input(path: string, label: string): HTMLElement {
  return within(field(path)).getByRole('combobox', { name: label })
}

function hasRoleClasses(element: Element, classes: string): boolean {
  const names = classes.split(' ')
  return [element, ...element.querySelectorAll('*')].some((candidate) =>
    names.every((name) => candidate.classList.contains(name)),
  )
}

/** Whether the element has warningMark's border classes and none of errorMark's. */
function hasWarningBorder(element: HTMLElement): boolean {
  const has = (name: string) => element.classList.contains(name)
  return (
    COLOR_CLASSES.warningMark.border.split(' ').every(has) &&
    !COLOR_CLASSES.errorMark.border.split(' ').some(has)
  )
}

function fixOf(feature: string, code: string, path: string): FixRef {
  return { feature, code, field: path, cause: false, index: 0, revision: 1 }
}

/** NC05's message on Radius: the largest radius that works is about 2.3 mm. */
const FILLET_TOO_LARGE: Diagnostic = {
  severity: 'error',
  code: 'fillet_too_large',
  message:
    "Fillet radius 3 mm is too large for the selected edges: the largest that works is about 2.3 mm. The top edge of the cut made by 'Box 2' lies next to a wall of the cut made by 'Box 2', which is only 2.4 mm across.",
  field: '/radius',
  suggestion: '2.3 mm',
  actions: [
    {
      label: 'Use 2.3 mm',
      run: { kind: 'apply', fix: fixOf('f-fil1', 'fillet_too_large', '/radius') },
    },
  ],
}

describe('limits under fields (3.7, U-139)', () => {
  it('U-139 shows "Largest that works: 2.3 mm" under the radius field error, in warningText', () => {
    renderCommand(editingFillet('3 mm'), previewing(failing('f-fil1', [FILLET_TOO_LARGE])))
    const radius = field('radius')
    const limit = within(radius).getByTestId('field-limit')
    expect(limit).toHaveTextContent('Largest that works: 2.3 mm')
    expect(hasRoleClasses(limit, COLOR_CLASSES.warningText.foreground)).toBe(true)
    const error = within(radius).getByTestId('field-error')
    expect(error).toHaveTextContent(FILLET_TOO_LARGE.message)
    expect(error.compareDocumentPosition(limit) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy()
  })

  it('U-139 gives the field the warningMark border instead of errorMark', () => {
    renderCommand(editingFillet('3 mm'), previewing(failing('f-fil1', [FILLET_TOO_LARGE])))
    expect(hasWarningBorder(input('radius', 'Radius'))).toBe(true)
  })

  it("U-139 keeps the message's actions in the banner, not under the field", () => {
    renderCommand(editingFillet('3 mm'), previewing(failing('f-fil1', [FILLET_TOO_LARGE])))
    const banner = screen.getByTestId('inspector-error')
    expect(within(banner).getByRole('button', { name: 'Use 2.3 mm' })).toBeInTheDocument()
    expect(within(field('radius')).queryByRole('button', { name: 'Use 2.3 mm' })).toBeNull()
  })

  it('U-139 shows no limit for a message without a suggestion, and keeps the errorMark border', () => {
    const { suggestion: _suggestion, ...withoutSuggestion } = FILLET_TOO_LARGE
    renderCommand(editingFillet('3 mm'), previewing(failing('f-fil1', [withoutSuggestion])))
    expect(within(field('radius')).queryByTestId('field-limit')).toBeNull()
    const radius = input('radius', 'Radius')
    for (const name of COLOR_CLASSES.errorMark.border.split(' ')) {
      expect(radius).toHaveClass(name)
    }
  })

  it('U-139 shows no limit in an inch document for a number without a unit, whose suggestion keeps its size', () => {
    const inches = { ...CUBE_FEATURES_DOCUMENT, units: 'in' } as const
    const message: Diagnostic = { ...FILLET_TOO_LARGE, suggestion: '2 mm' }
    renderCommand(editingFillet('2'), previewing(failing('f-fil1', [message]), inches))
    expect(within(field('radius')).queryByTestId('field-limit')).toBeNull()
  })

  it('U-139 shows the limit in an inch document for a value with its unit', () => {
    const inches = { ...CUBE_FEATURES_DOCUMENT, units: 'in' } as const
    const message: Diagnostic = { ...FILLET_TOO_LARGE, suggestion: '1.9 in' }
    renderCommand(editingFillet('2 in'), previewing(failing('f-fil1', [message]), inches))
    expect(within(field('radius')).getByTestId('field-limit')).toHaveTextContent(
      'Largest that works: 1.9 in',
    )
  })

  it('U-139 shows the limit of shell_too_thick under Thickness', () => {
    const tooThick: Diagnostic = {
      severity: 'error',
      code: 'shell_too_thick',
      message:
        "Shell thickness 30 mm is too thick for body 'Cube': the largest that works is about 9.5 mm.",
      field: '/thickness',
      suggestion: '9.5 mm',
      actions: [
        {
          label: 'Use 9.5 mm',
          run: { kind: 'apply', fix: fixOf('f-sh1', 'shell_too_thick', '/thickness') },
        },
      ],
    }
    const shell = featuresOf(CUBE_FEATURES_DOCUMENT)[3]
    const command = editing('shell', 'f-sh1', 'Shell 1', {
      remove_faces: shell?.['remove_faces'],
      thickness: '30 mm',
    })
    renderCommand(command, previewing(failing('f-sh1', [tooThick])))
    expect(within(field('thickness')).getByTestId('field-limit')).toHaveTextContent(
      'Largest that works: 9.5 mm',
    )
    expect(hasWarningBorder(input('thickness', 'Thickness'))).toBe(true)
  })

  it('U-139 shows the limit of chamfer_too_large under Distance', () => {
    const tooLarge: Diagnostic = {
      severity: 'error',
      code: 'chamfer_too_large',
      message:
        'Chamfer distance 15 mm is too large for the selected edges: the largest that works is about 9.9 mm.',
      field: '/distance/expr',
      suggestion: '9.9 mm',
    }
    const command: ActiveCommand = {
      ...editing('chamfer', 'f-cha9', 'Chamfer', {
        edges: [CUBE_TOP_FRONT_EDGE.reference],
        distance: { kind: 'equal', expr: '15 mm' },
      }),
      editing: false,
    }
    renderCommand(command, previewing(failing('f-cha9', [tooLarge])))
    expect(within(field('distance.expr')).getByTestId('field-limit')).toHaveTextContent(
      'Largest that works: 9.9 mm',
    )
    expect(hasWarningBorder(input('distance.expr', 'Distance'))).toBe(true)
  })

  it('U-139 shows no limit for another message with a suggestion, such as an out of range value', () => {
    const outOfRange: Diagnostic = {
      severity: 'error',
      code: 'out_of_range',
      message: 'Radius must be more than 0 mm, but it is -1 mm.',
      field: '/radius',
      suggestion: '1 mm',
    }
    renderCommand(editingFillet('-1 mm'), previewing(failing('f-fil1', [outOfRange])))
    expect(within(field('radius')).queryByTestId('field-limit')).toBeNull()
  })
})
