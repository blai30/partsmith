import { act, cleanup, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { showPickHint } from '../app/actions/notifications'
import { installPopoverPolyfill } from '../components/popover-polyfill.testing'
import type { Diagnostic, FeatureStatus } from '../generated/protocol'
import {
  bodyItem,
  CYLINDRICAL_FACE,
  cubeDocumentState,
  documentState,
  elementItem,
  featureItem,
  meshSummary,
  CUBE_BOUNDS,
  originItem,
  parameterItem,
  readyState,
  rectSketchDocumentState,
  regionItem,
  sketchPointItem,
} from '../layout/fixtures.testing'
import { createModelApp, renderWithApp, stubMatchMedia } from '../layout/test-app.testing'
import { MESSAGES } from '../messages'
import {
  createAppStore,
  type ActiveCommand,
  type AppState,
  type DocumentState,
} from '../state/store'
import {
  CUBE_FEATURES_DOCUMENT,
  CUBE_FRONT_FACE,
  CUBE_TOP_FACE,
  CUBE_TOP_FRONT_EDGE,
  featuresOf,
  RECT_REGION,
  RECT_SKETCH_SOLUTION,
  TWO_BODIES_DOCUMENT,
} from '../test/fakes/documents'
import { settle } from '../test/fakes/settle'
import { COLOR_CLASSES, FONT_CLASSES } from '../theme/roles'

let uninstallPopovers: () => void

beforeEach(() => {
  uninstallPopovers = installPopoverPolyfill()
  stubMatchMedia()
})

afterEach(() => {
  cleanup()
  uninstallPopovers()
  vi.useRealTimers()
  vi.unstubAllGlobals()
})

const FILLET_EDGES = featuresOf(CUBE_FEATURES_DOCUMENT)[2]?.edges as readonly unknown[]

const EDIT_FILLET: ActiveCommand = {
  featureType: 'fillet',
  featureId: 'f-fil1',
  editing: true,
  title: 'Fillet 1',
  params: { body: 'b-cube', edges: FILLET_EDGES, radius: '2 mm' },
  activeField: null,
  fieldErrors: {},
  applied: true,
  newBody: null,
  helper: null,
  changedPaths: [],
  history: [],
  future: [],
  preview: 'shown',
}

const NEW_EXTRUDE: ActiveCommand = {
  featureType: 'extrude',
  featureId: 'f-0123456789ab',
  editing: false,
  title: 'Extrude',
  params: { extent: { kind: 'distance', expr: '10 mm' }, op: 'new' },
  activeField: 'profile',
  fieldErrors: {},
  applied: false,
  newBody: null,
  helper: null,
  changedPaths: [],
  history: [],
  future: [],
  preview: 'none',
}

function filletDocument(filletStatus: FeatureStatus | null = null): DocumentState {
  const document = documentState(CUBE_FEATURES_DOCUMENT, {
    name: 'cube-features',
    meshes: { 'b-cube': meshSummary(1, CUBE_BOUNDS) },
    focus: 'f-fil1',
    transactionOpen: true,
  })
  return filletStatus === null
    ? document
    : { ...document, featureStatus: { ...document.featureStatus, [filletStatus.id]: filletStatus } }
}

function renderInspector(document: DocumentState, state: Partial<AppState> = {}) {
  const test = createModelApp({ state: readyState(document, state) })
  renderWithApp(test.app)
  return { ...test, user: userEvent.setup() }
}

function inspector(): HTMLElement {
  return screen.getByTestId('inspector')
}

function field(path: string): HTMLElement {
  const found = within(inspector())
    .getAllByTestId('inspector-field')
    .find((element) => element.getAttribute('data-path') === path)
  if (found === undefined) {
    throw new Error(`No inspector field ${path}`)
  }
  return found
}

function fieldPaths(): (string | null)[] {
  return within(inspector())
    .getAllByTestId('inspector-field')
    .map((element) => element.getAttribute('data-path'))
}

function hasRoleClasses(element: HTMLElement, classes: string): boolean {
  const names = classes.split(' ')
  return [element, ...element.querySelectorAll('*')].some((candidate) =>
    names.every((name) => candidate.classList.contains(name)),
  )
}

function status(
  messages: readonly Diagnostic[],
  state: FeatureStatus['status'] = 'error',
): FeatureStatus {
  return { id: 'f-fil1', status: state, messages: [...messages], repairs: [] }
}

const TOO_LARGE: Diagnostic = {
  severity: 'error',
  code: 'fillet_too_large',
  message:
    'Fillet radius 15 mm is too large for the selected edges. The largest radius that works is about 2.3 mm.',
  field: '/radius',
  suggestion: '2.3 mm',
}

describe('the command inspector (3.7)', () => {
  it('is a non-modal dialog titled with the feature label, mirroring the preview state', () => {
    renderInspector(filletDocument(), { command: EDIT_FILLET })
    const panel = inspector()
    expect(panel).toHaveAttribute('role', 'dialog')
    expect(panel).toHaveAttribute('aria-modal', 'false')
    expect(panel).toHaveAttribute('data-mode', 'command')
    expect(panel).toHaveAttribute('data-preview', 'shown')
    expect(panel).toHaveAccessibleName('Fillet 1')
  })

  it('titles a new feature with the manifest label', () => {
    renderInspector(rectSketchDocumentState(), { command: NEW_EXTRUDE })
    expect(inspector()).toHaveAccessibleName('Extrude')
    expect(inspector()).toHaveAttribute('data-preview', 'none')
  })

  it('shows the essential fields, and the others behind "More options"', async () => {
    const { app, user } = renderInspector(filletDocument(), { command: EDIT_FILLET })
    expect(fieldPaths()).toEqual(['edges', 'radius'])
    const more = within(inspector()).getByTestId('inspector-more')
    expect(more).toHaveTextContent('More options')
    expect(more).toHaveAttribute('aria-expanded', 'false')
    await user.click(more)
    expect(more).toHaveTextContent('Fewer options')
    expect(more).toHaveAttribute('aria-expanded', 'true')
    expect(fieldPaths()).toEqual(['edges', 'radius', 'body'])
    expect(app.store.getState().settings.moreExpanded.fillet).toBe(true)
  })

  it('shows a selection field with its count, an expression field with its result', async () => {
    renderInspector(filletDocument(), { command: EDIT_FILLET })
    const edges = within(field('edges')).getByTestId('selection-field')
    expect(edges).toHaveAttribute('role', 'button')
    expect(edges).toHaveAttribute('data-count', '4')
    expect(edges).toHaveAttribute('data-active', 'false')
    expect(edges).toHaveTextContent('4 edges')
    await waitFor(() => {
      expect(within(field('radius')).getByTestId('expression-result')).toHaveTextContent('= 2 mm')
    })
    expect(within(field('radius')).getByRole('combobox', { name: 'Radius' })).toHaveClass(
      ...FONT_CLASSES.values.split(' '),
    )
  })

  it('makes a selection field active on click and empties it with its clear button', async () => {
    const { app, user } = renderInspector(filletDocument(), { command: EDIT_FILLET })
    const edges = within(field('edges')).getByTestId('selection-field')
    await user.click(edges)
    expect(app.store.getState().command?.activeField).toBe('edges')
    expect(edges).toHaveAttribute('data-active', 'true')
    expect(hasRoleClasses(edges, COLOR_CLASSES.accent.border)).toBe(true)
    await user.click(within(field('edges')).getByRole('button', { name: 'Clear Edges' }))
    expect(app.store.getState().command?.params.edges ?? []).toEqual([])
  })

  it('shows "Select regions" in an empty required selection field, dashed in the accent', () => {
    renderInspector(rectSketchDocumentState(), { command: NEW_EXTRUDE })
    const profile = within(field('profile')).getByTestId('selection-field')
    expect(profile).toHaveTextContent('Select regions')
    expect(profile).toHaveAttribute('data-count', '0')
    expect(profile).toHaveAttribute('data-active', 'true')
    expect(hasRoleClasses(profile, 'border-dashed')).toBe(true)
  })

  it('shows the hint about a refused pick under its field for 3 s (3.7)', () => {
    vi.useFakeTimers()
    const { app } = renderInspector(rectSketchDocumentState(), { command: NEW_EXTRUDE })
    const hint = MESSAGES.pick_kind('regions')
    act(() => {
      showPickHint(app, hint, 'profile')
    })
    const profile = within(field('profile')).getByTestId('selection-field')
    const shown = within(field('profile')).getByText(hint)
    expect(hasRoleClasses(shown, COLOR_CLASSES.textMuted.foreground)).toBe(true)
    // The field's help (2.12) describes the box too, after the hint.
    const help = 'The closed sketch shapes to extrude. Click inside a shape to add it.'
    expect(profile).toHaveAccessibleDescription(`${hint} ${help}`)
    expect(within(field('profile')).getByTestId('field-error')).toHaveTextContent('')
    act(() => {
      vi.advanceTimersByTime(3_000)
    })
    expect(within(field('profile')).queryByText(hint)).toBeNull()
    expect(profile).toHaveAccessibleDescription(help)
  })

  it('shows the sketch hint about a refused plane under no field', () => {
    const { app } = renderInspector(rectSketchDocumentState(), { command: NEW_EXTRUDE })
    act(() => {
      showPickHint(app, MESSAGES.pick_plane(), null)
    })
    expect(within(inspector()).queryByText(MESSAGES.pick_plane())).toBeNull()
  })

  it('hovering a selection box marks its field hovered, so the viewport highlights its items (3.7)', async () => {
    const { app, user } = renderInspector(filletDocument(), { command: EDIT_FILLET })
    const edges = within(field('edges')).getByTestId('selection-field')
    await user.hover(edges)
    expect(app.store.getState().command?.hoveredField).toBe('edges')
    await user.unhover(edges)
    expect(app.store.getState().command?.hoveredField).toBeNull()
  })

  it('disables OK while incomplete, with the fill_fields reason', () => {
    renderInspector(rectSketchDocumentState(), { command: NEW_EXTRUDE })
    const ok = within(inspector()).getByTestId('inspector-ok')
    expect(ok).toHaveAttribute('aria-disabled', 'true')
    expect(ok).toHaveAccessibleDescription(MESSAGES.fill_fields())
  })

  it('names the keys of Cancel and of a complete OK in their tooltips: Esc and Enter (3.7)', () => {
    renderInspector(filletDocument(), { command: EDIT_FILLET })
    // The header's close button is named Cancel too; the footer's comes last.
    const cancel = within(inspector()).getAllByRole('button', { name: 'Cancel' }).at(-1)
    expect(cancel).toHaveAccessibleDescription(expect.stringContaining('Esc'))
    const ok = within(inspector()).getByTestId('inspector-ok')
    expect(ok).not.toHaveAttribute('aria-disabled')
    expect(ok).toHaveAccessibleDescription(expect.stringContaining('OK'))
    expect(ok).toHaveAccessibleDescription(expect.stringContaining('Enter'))
  })

  it('shows a choice of up to 4 options as a segmented radio group', async () => {
    const { app, user } = renderInspector(rectSketchDocumentState(), { command: NEW_EXTRUDE })
    const extent = within(field('extent.kind')).getByRole('radiogroup')
    expect(within(extent).getAllByRole('radio')).toHaveLength(4)
    expect(within(extent).getByRole('radio', { name: 'Distance' })).toBeChecked()
    await user.click(within(extent).getByRole('radio', { name: 'Symmetric' }))
    expect(app.store.getState().command?.params.extent).toEqual({
      kind: 'symmetric',
      expr: '10 mm',
    })
  })

  it('shows a segmented choice whose segments overflow the field as a select, so no option is clipped', async () => {
    // A row is as wide as its field but its segments need more room than that.
    const scrollWidth = vi
      .spyOn(HTMLElement.prototype, 'scrollWidth', 'get')
      .mockImplementation(function measured(this: HTMLElement) {
        return this.querySelector('[role=radiogroup]') === null ? 0 : 380
      })
    const clientWidth = vi
      .spyOn(HTMLElement.prototype, 'clientWidth', 'get')
      .mockImplementation(function measured(this: HTMLElement) {
        return this.querySelector('[role=radiogroup]') === null ? 0 : 286
      })
    try {
      const { app, user } = renderInspector(rectSketchDocumentState(), { command: NEW_EXTRUDE })
      const extent = within(field('extent.kind'))
      expect(extent.queryByRole('radiogroup')).toBeNull()
      const select = extent.getByRole('combobox', { name: 'Extent' })
      expect(
        within(select)
          .getAllByRole('option')
          .map((option) => option.textContent),
      ).toEqual(['Distance', 'Symmetric', 'Two sides', 'Through all'])
      await user.selectOptions(select, 'Through all')
      expect(app.store.getState().command?.params.extent).toMatchObject({ kind: 'through_all' })
    } finally {
      scrollWidth.mockRestore()
      clientWidth.mockRestore()
    }
  })

  it('writes a typed expression into the command params', async () => {
    const { app, user } = renderInspector(filletDocument(), { command: EDIT_FILLET })
    const radius = within(field('radius')).getByRole('combobox', { name: 'Radius' })
    await user.clear(radius)
    await user.type(radius, '3 mm')
    await waitFor(() => {
      expect(app.store.getState().command?.params.radius).toBe('3 mm')
    })
  })

  it('U-66 OK commits the transaction', async () => {
    const { core, user } = renderInspector(filletDocument(), { command: EDIT_FILLET })
    await user.click(within(inspector()).getByTestId('inspector-ok'))
    await waitFor(() => {
      expect(core.callsTo('commitTransaction')).toEqual([[1]])
    })
    expect(core.callsTo('cancelTransaction')).toEqual([])
  })

  it('U-66 Cancel cancels the transaction and closes the inspector', async () => {
    const { app, core, user } = renderInspector(filletDocument(), { command: EDIT_FILLET })
    const [cancel] = within(inspector()).getAllByRole('button', { name: 'Cancel' })
    await user.click(cancel ?? inspector())
    await waitFor(() => {
      expect(core.callsTo('cancelTransaction')).toEqual([[1]])
    })
    expect(core.callsTo('commitTransaction')).toEqual([])
    expect(app.store.getState().command).toBeNull()
    expect(screen.queryByTestId('inspector')).toBeNull()
  })

  it('confirms with Enter in an expression field when complete', async () => {
    const { core, user } = renderInspector(filletDocument(), { command: EDIT_FILLET })
    const radius = within(field('radius')).getByRole('combobox', { name: 'Radius' })
    await waitFor(() => {
      expect(within(field('radius')).getByTestId('expression-result')).toHaveTextContent('= 2 mm')
    })
    await user.click(radius)
    await user.keyboard('{Enter}')
    await waitFor(() => {
      expect(core.callsTo('commitTransaction')).toEqual([[1]])
    })
  })

  it('fills a body field from its Choose menu of bodies with geometry', async () => {
    const { app, user } = renderInspector(filletDocument(), {
      command: { ...EDIT_FILLET, params: { radius: '2 mm' } },
    })
    await user.click(within(inspector()).getByTestId('inspector-more'))
    await user.click(within(field('body')).getByRole('button', { name: /Choose/ }))
    await user.click(screen.getByRole('menuitem', { name: /Cube/ }))
    expect(app.store.getState().command?.params.body).toBe('b-cube')
  })
})

describe('help on hover (2.10, 3.7)', () => {
  const OPERATION_HELP =
    'New body makes a separate part. Join adds to the body, Cut removes from it, and Intersect keeps only the overlap.'

  it('marks a label with help with cursor-help and describes its control with the help', () => {
    renderInspector(rectSketchDocumentState(), { command: NEW_EXTRUDE })
    const operation = field('op')
    expect(within(operation).getByText('Operation')).toHaveClass('cursor-help')
    expect(within(operation).getByRole('radiogroup')).toHaveAccessibleDescription(OPERATION_HELP)
    expect(within(field('profile')).getByTestId('selection-field')).toHaveAccessibleDescription(
      'The closed sketch shapes to extrude. Click inside a shape to add it.',
    )
  })

  it('shows the help in a tooltip when the pointer rests on the label', async () => {
    const { user } = renderInspector(rectSketchDocumentState(), { command: NEW_EXTRUDE })
    await user.hover(within(field('op')).getByText('Operation'))
    await waitFor(() => {
      expect(screen.getByRole('tooltip')).toHaveTextContent('Intersect keeps only the overlap')
    })
  })

  it('gives a field without help no tooltip and no cue', () => {
    renderInspector(rectSketchDocumentState(), { command: NEW_EXTRUDE })
    const distance = field('extent.expr')
    expect(within(distance).getByText('Distance')).not.toHaveClass('cursor-help')
    expect(within(distance).getByRole('combobox')).not.toHaveAccessibleDescription(/Extrude/)
  })

  it('describes expression fields with their help', () => {
    renderInspector(filletDocument(), {
      command: {
        ...EDIT_FILLET,
        featureType: 'shell',
        featureId: 'f-sh1',
        title: 'Shell 1',
        params: { body: 'b-cube', remove_faces: [], thickness: '2 mm' },
      },
    })
    expect(within(field('thickness')).getByRole('combobox')).toHaveAccessibleDescription(
      /a multiple of the nozzle width prints best/,
    )
  })

  it('gives the inspector title the command description as its tooltip', () => {
    renderInspector(rectSketchDocumentState(), { command: NEW_EXTRUDE })
    const title = within(inspector()).getByRole('heading', { name: 'Extrude' })
    expect(title).toHaveAccessibleDescription(
      'Pulls a closed sketch shape straight out into a solid, or cuts it into one.',
    )
  })
})

describe('the regions a profile field can choose (3.7)', () => {
  it('leaves out a sketch that is hidden by default because a feature uses it', () => {
    renderInspector(cubeDocumentState(), { command: NEW_EXTRUDE })
    expect(within(field('profile')).queryByRole('button', { name: /Choose/ })).toBeNull()
  })

  it('lists the regions of a sketch shown this session', async () => {
    const { user } = renderInspector(cubeDocumentState(), {
      command: NEW_EXTRUDE,
      view: { ...createAppStore().getState().view, sessionVisible: { 'feature:f-sk1': true } },
    })
    await user.click(within(field('profile')).getByRole('button', { name: /Choose/ }))
    expect(screen.getByRole('menuitem', { name: /^Region 1 \(/ })).toBeInTheDocument()
  })
})

describe('feature status in the inspector (3.7, 4.2)', () => {
  it('U-87 shows an error message with a field under that field', async () => {
    renderInspector(filletDocument(status([TOO_LARGE])), {
      command: { ...EDIT_FILLET, preview: 'failed' },
    })
    await waitFor(() => {
      expect(within(field('radius')).getByTestId('field-error')).toHaveTextContent(
        TOO_LARGE.message,
      )
    })
  })

  it('U-87 its action "Use 2.3 mm" sets radius to 2.3 mm', async () => {
    const { app, user } = renderInspector(filletDocument(status([TOO_LARGE])), {
      command: { ...EDIT_FILLET, preview: 'failed' },
    })
    const [use] = within(inspector()).getAllByRole('button', { name: 'Use 2.3 mm' })
    await user.click(use ?? inspector())
    expect(app.store.getState().command?.params.radius).toBe('2.3 mm')
  })

  it('U-87 maps a message with field /edges/1 to the edges field', () => {
    const lost: Diagnostic = {
      severity: 'warning',
      code: 'reference_ambiguous',
      message: 'One of the edges selected for Edges now matches 2 edges, so it was skipped.',
      field: '/edges/1',
    }
    renderInspector(filletDocument(status([lost], 'warning')), { command: EDIT_FILLET })
    expect(within(field('edges')).getByTestId('field-error')).toHaveTextContent(lost.message)
  })

  it('shows errors in an alert banner on the error background', () => {
    renderInspector(filletDocument(status([TOO_LARGE])), {
      command: { ...EDIT_FILLET, preview: 'failed' },
    })
    const banner = within(inspector()).getByTestId('inspector-error')
    expect(banner).toHaveTextContent(TOO_LARGE.message)
    expect(
      banner.closest('[role="alert"]') ?? banner.querySelector('[role="alert"]'),
    ).not.toBeNull()
    expect(hasRoleClasses(banner, COLOR_CLASSES.errorSubtle.background)).toBe(true)
    expect(inspector()).toHaveAttribute('data-preview', 'failed')
  })

  it('shows warnings without an error on the warning background', () => {
    const warning: Diagnostic = {
      severity: 'warning',
      code: 'no_effect',
      message: "This feature does not change body 'Cube'.",
    }
    renderInspector(filletDocument(status([warning], 'warning')), { command: EDIT_FILLET })
    const banner = within(inspector()).getByTestId('inspector-error')
    expect(banner).toHaveTextContent(warning.message)
    expect(hasRoleClasses(banner, COLOR_CLASSES.warningSubtle.background)).toBe(true)
  })

  it('shows no banner for an ok feature', () => {
    renderInspector(filletDocument(), { command: EDIT_FILLET })
    expect(within(inspector()).queryByTestId('inspector-error')).toBeNull()
  })
})

describe('the selection inspector (3.8)', () => {
  it('shows nothing with an empty selection', () => {
    renderInspector(cubeDocumentState())
    expect(screen.queryByTestId('inspector')).toBeNull()
  })

  it('shows one body with its size, center and visibility', () => {
    renderInspector(cubeDocumentState(), { selection: [bodyItem('b-cube', 'Cube')] })
    const panel = inspector()
    expect(panel).toHaveAttribute('data-mode', 'selection')
    expect(panel).toHaveAccessibleName('Selection')
    expect(within(panel).getByTestId('selection-title')).toHaveTextContent('Cube')
    expect(within(panel).getByTestId('body-size')).toHaveTextContent('20 × 20 × 20 mm')
    expect(within(panel).getByTestId('body-center')).toHaveTextContent('0, 0, 10 mm')
    expect(within(panel).getByTestId('body-size')).toHaveClass(...FONT_CLASSES.values.split(' '))
    expect(within(panel).getByRole('switch', { name: 'Visible' })).toBeChecked()
  })

  it('shows "Empty" for a body without geometry', () => {
    renderInspector(cubeDocumentState({ meshes: {} }), { selection: [bodyItem('b-cube', 'Cube')] })
    expect(within(inspector()).getByTestId('body-size')).toHaveTextContent('Empty')
  })

  it('commits a material with Enter as "Change material"', async () => {
    const { core, user } = renderInspector(cubeDocumentState(), {
      selection: [bodyItem('b-cube', 'Cube')],
    })
    const material = within(inspector()).getByRole('combobox', { name: 'Material' })
    await user.type(material, 'PETG{Enter}')
    await settle()
    expect(core.callsTo('applyEdits')).toEqual([
      [
        1,
        'Change material',
        [{ type: 'update_body', body: { id: 'b-cube', name: 'Cube', material: 'PETG' } }],
      ],
    ])
  })

  it('shows one feature with its type, Edit and Suppress', async () => {
    const { core, user } = renderInspector(cubeDocumentState(), {
      selection: [featureItem('f-ext1', 'Extrude 1')],
    })
    const panel = inspector()
    expect(within(panel).getByTestId('selection-title')).toHaveTextContent('Extrude 1')
    expect(panel).toHaveTextContent('Extrude')
    expect(within(panel).getByRole('button', { name: 'Suppress' })).toBeInTheDocument()
    await user.click(within(panel).getByRole('button', { name: 'Edit' }))
    await waitFor(() => {
      expect(core.callsTo('beginTransaction')).toEqual([[1, 'Edit Extrude 1']])
    })
  })

  it('U-110 offers "Re-pick Edges" for a feature with a lost edge, which edits it with Edges active', async () => {
    const lost: FeatureStatus = {
      ...status([TOO_LARGE]),
      repairs: [
        {
          kind: 'element',
          path: '/edges/0',
          body: 'b-cube',
          reference: { q: 'edge(top,front)', fp: 'fp-edge' },
          resolution: 'missing',
          centroid: [0, 0, 20],
        },
      ],
    }
    const document = documentState(CUBE_FEATURES_DOCUMENT, {
      meshes: { 'b-cube': meshSummary(1, CUBE_BOUNDS) },
      featureStatus: { ...documentState(CUBE_FEATURES_DOCUMENT).featureStatus, 'f-fil1': lost },
    })
    const { app, user } = renderInspector(document, {
      selection: [featureItem('f-fil1', 'Fillet 1')],
    })
    const panel = inspector()
    expect(within(panel).queryByRole('button', { name: 'Edit' })).toBeNull()
    await user.click(within(panel).getByRole('button', { name: 'Re-pick Edges' }))
    await waitFor(() => {
      expect(app.store.getState().command?.activeField).toBe('edges')
    })
  })

  it('offers "Use {suggestion}" for a feature message at rest, labeled "Change {label}"', async () => {
    const document = documentState(CUBE_FEATURES_DOCUMENT, {
      meshes: { 'b-cube': meshSummary(1, CUBE_BOUNDS) },
    })
    const failed = {
      ...document,
      featureStatus: { ...document.featureStatus, 'f-fil1': status([TOO_LARGE]) },
    }
    const { core, user } = renderInspector(failed, {
      selection: [featureItem('f-fil1', 'Fillet 1')],
    })
    expect(inspector()).toHaveTextContent(TOO_LARGE.message)
    await user.click(within(inspector()).getByRole('button', { name: 'Use 2.3 mm' }))
    await waitFor(() => {
      expect(core.callsTo('applyEdits')).toEqual([
        [
          1,
          'Change Fillet 1',
          [
            {
              type: 'update_feature',
              feature: { ...featuresOf(CUBE_FEATURES_DOCUMENT)[2], radius: '2.3 mm' },
            },
          ],
        ],
      ])
    })
  })

  it('shows one flat face with its surface and area, and suggests Sketch, Hole and Shell', () => {
    renderInspector(cubeDocumentState(), { selection: [elementItem(CUBE_TOP_FACE)] })
    const panel = inspector()
    expect(within(panel).getByTestId('selection-title')).toHaveTextContent('1 face')
    expect(panel).toHaveTextContent('Flat face')
    expect(panel).toHaveTextContent('400 mm²')
    for (const name of ['Sketch', 'Hole', 'Shell']) {
      expect(
        within(panel).getByRole('button', { name: new RegExp(`^${name}`) }),
      ).toBeInTheDocument()
    }
  })

  it('counts several faces', () => {
    renderInspector(cubeDocumentState(), {
      selection: [elementItem(CUBE_TOP_FACE), elementItem(CUBE_FRONT_FACE)],
    })
    expect(within(inspector()).getByTestId('selection-title')).toHaveTextContent('2 faces')
  })

  it('shows one edge with its curve and length and suggests Fillet and Chamfer', () => {
    renderInspector(cubeDocumentState(), { selection: [elementItem(CUBE_TOP_FRONT_EDGE)] })
    const panel = inspector()
    expect(within(panel).getByTestId('selection-title')).toHaveTextContent('1 edge')
    expect(panel).toHaveTextContent('Straight edge')
    expect(panel).toHaveTextContent('20 mm')
    expect(within(panel).getByRole('button', { name: /^Fillet/ })).toBeInTheDocument()
    expect(within(panel).getByRole('button', { name: /^Chamfer/ })).toBeInTheDocument()
  })

  it('runs a suggested command with the selection preselected', async () => {
    const { app, core, user } = renderInspector(cubeDocumentState(), {
      selection: [elementItem(CUBE_TOP_FRONT_EDGE)],
    })
    await user.click(within(inspector()).getByRole('button', { name: /^Fillet/ }))
    await waitFor(() => {
      expect(core.callsTo('beginTransaction')).toEqual([[1, 'Fillet']])
    })
    expect(app.store.getState().command?.params.edges).toEqual([CUBE_TOP_FRONT_EDGE.reference])
    expect(app.store.getState().command?.params.body).toBe('b-cube')
  })

  it('shows regions with Extrude and Revolve', () => {
    renderInspector(rectSketchDocumentState(), { selection: [regionItem('f-sk1', RECT_REGION)] })
    const panel = inspector()
    expect(within(panel).getByTestId('selection-title')).toHaveTextContent('1 region')
    expect(within(panel).getByRole('button', { name: /^Extrude/ })).toBeInTheDocument()
    expect(within(panel).getByRole('button', { name: /^Revolve/ })).toBeInTheDocument()
  })

  it('shows an origin plane as "Top plane (XY)" with Sketch', () => {
    renderInspector(cubeDocumentState(), { selection: [originItem('XY')] })
    const panel = inspector()
    expect(within(panel).getByTestId('selection-title')).toHaveTextContent('Top plane (XY)')
    expect(within(panel).getByRole('button', { name: /^Sketch/ })).toBeInTheDocument()
  })

  it('shows "{n} items" for a mixed selection', () => {
    renderInspector(cubeDocumentState(), {
      selection: [bodyItem('b-cube', 'Cube'), parameterItem('p-size', 'size')],
    })
    expect(within(inspector()).getByTestId('selection-title')).toHaveTextContent('2 items')
  })

  it('offers Construction for a point entity selected by its point reference in a sketch', () => {
    const solution = {
      ...RECT_SKETCH_SOLUTION,
      entities: [
        ...RECT_SKETCH_SOLUTION.entities,
        {
          id: 'e-p1',
          type: 'point' as const,
          position: [10, 5] as [number, number],
          constrained: 'under_constrained' as const,
        },
      ],
      points: [...RECT_SKETCH_SOLUTION.points, { point: 'e-p1', at: [10, 5] as [number, number] }],
    }
    renderInspector(
      rectSketchDocumentState({ sketches: { 'f-sk1': { solution, display: null } } }),
      {
        context: 'sketch',
        sketch: {
          sketchId: 'f-sk1',
          tool: null,
          construction: false,
          showConstraints: true,
          selection: [sketchPointItem('f-sk1', 'e-p1')],
          dragging: false,
          cameraBefore: null,
        },
      },
    )
    expect(within(inspector()).getByRole('switch', { name: 'Construction' })).not.toBeChecked()
  })

  it('E-88 suggests Move body (with M), Mirror and Split body for one body, run with it preselected', async () => {
    const { app, user } = renderInspector(cubeDocumentState(), {
      selection: [bodyItem('b-cube', 'Cube')],
    })
    const panel = inspector()
    const move = within(panel).getByRole('button', { name: 'Move body' })
    expect(move).toHaveTextContent('M')
    expect(move).toHaveAttribute('aria-keyshortcuts', 'M')
    expect(within(move).getByText('M')).toHaveAttribute('aria-hidden', 'true')
    expect(within(panel).getByRole('button', { name: 'Mirror' })).toBeInTheDocument()
    expect(within(panel).getByRole('button', { name: 'Split body' })).toBeInTheDocument()
    await user.click(move)
    await waitFor(() => {
      expect(app.store.getState().command?.featureType).toBe('move_body')
    })
    expect(app.store.getState().command?.params['body']).toBe('b-cube')
  })

  it('E-92 measures the volume of one body with no tool', async () => {
    const test = createModelApp({
      state: readyState(cubeDocumentState(), { selection: [bodyItem('b-cube', 'Cube')] }),
    })
    test.core.stub('measure', () => Promise.resolve({ volume: 8000 }))
    renderWithApp(test.app)
    const row = await waitFor(() => {
      const found = within(inspector())
        .getAllByTestId('measure-row')
        .find((candidate) => candidate.getAttribute('data-measure') === 'volume')
      expect(found).toHaveTextContent('8000 mm³')
      return found
    })
    expect(row).toHaveTextContent('Volume')
  })

  it('E-89 names the feature that made a face, edits it, and selects its body', async () => {
    const { app, core, user } = renderInspector(cubeDocumentState(), {
      selection: [elementItem(CUBE_FRONT_FACE)],
    })
    const panel = inspector()
    expect(panel).toHaveTextContent('Made by Extrude 1')
    await user.click(within(panel).getByRole('button', { name: 'Select Cube' }))
    expect(app.store.getState().selection.map((item) => item.key)).toEqual(['body:b-cube'])
    cleanup()
    const again = renderInspector(cubeDocumentState(), {
      selection: [elementItem(CUBE_FRONT_FACE)],
    })
    await again.user.click(within(inspector()).getByRole('button', { name: 'Edit Extrude 1' }))
    await waitFor(() => {
      expect(again.core.callsTo('beginTransaction')).toEqual([[1, 'Edit Extrude 1']])
    })
    expect(core.callsTo('beginTransaction')).toEqual([])
  })

  it('lights up the timeline item of the feature while the "Made by" row is hovered', async () => {
    const { app, user } = renderInspector(cubeDocumentState(), {
      selection: [elementItem(CUBE_TOP_FRONT_EDGE)],
    })
    const row = within(inspector()).getByTestId('made-by')
    await user.hover(row)
    expect(app.store.getState().hover).toEqual({ key: 'feature:f-ext1', kind: 'feature' })
    const item = screen
      .getAllByTestId('timeline-item')
      .find((candidate) => candidate.getAttribute('data-feature-id') === 'f-ext1')
    expect(item?.querySelector('button')).toHaveClass(...COLOR_CLASSES.hover.active.split(' '))
    await user.unhover(row)
    expect(app.store.getState().hover).toBeNull()
  })

  it('E-90 shows two bodies as "2 bodies" with Boolean', () => {
    const twoBodies = documentState(TWO_BODIES_DOCUMENT, {
      meshes: { 'b-cube': meshSummary(1, CUBE_BOUNDS), 'b-peg': meshSummary(2, CUBE_BOUNDS) },
    })
    renderInspector(twoBodies, {
      selection: [bodyItem('b-cube', 'Cube'), bodyItem('b-peg', 'Peg')],
    })
    const panel = inspector()
    expect(within(panel).getByTestId('selection-title')).toHaveTextContent('2 bodies')
    expect(within(panel).getByRole('button', { name: 'Boolean' })).toBeInTheDocument()
  })

  it('shows a body and a plane as "2 items" with Mirror and Split body', () => {
    renderInspector(cubeDocumentState(), {
      selection: [bodyItem('b-cube', 'Cube'), originItem('XZ')],
    })
    const panel = inspector()
    expect(within(panel).getByTestId('selection-title')).toHaveTextContent('2 items')
    expect(within(panel).getByRole('button', { name: 'Mirror' })).toBeInTheDocument()
    expect(within(panel).getByRole('button', { name: 'Split body' })).toBeInTheDocument()
  })

  it('shows sketch points in Model context as "1 point" with Hole', () => {
    renderInspector(cubeDocumentState(), { selection: [sketchPointItem('f-sk1', 'e-1.start')] })
    const panel = inspector()
    expect(within(panel).getByTestId('selection-title')).toHaveTextContent('1 point')
    expect(within(panel).getByRole('button', { name: 'Hole' })).toBeInTheDocument()
  })

  it('shows an origin axis by name with no suggestions', () => {
    renderInspector(cubeDocumentState(), { selection: [originItem('X')] })
    const panel = inspector()
    expect(within(panel).getByTestId('selection-title')).toHaveTextContent('X axis')
    expect(within(panel).queryAllByRole('button', { name: /^(Sketch|Hole|Mirror)/ })).toEqual([])
  })

  it('shows the diameter of a cylindrical face', () => {
    renderInspector(cubeDocumentState(), { selection: [elementItem(CYLINDRICAL_FACE, 'Peg')] })
    const row = within(inspector()).getByTestId('measure-row')
    expect(row).toHaveAttribute('data-measure', 'diameter')
    expect(row).toHaveTextContent('Diameter')
    expect(row).toHaveTextContent('8 mm')
  })

  it('places itself at a runtime position only', () => {
    renderInspector(cubeDocumentState(), { selection: [bodyItem('b-cube', 'Cube')] })
    const panel = inspector()
    const positioned = panel.closest('[data-positioned]') ?? panel
    expect(positioned).toHaveAttribute('data-positioned')
    expect(positioned.getAttribute('style')).toMatch(/^translate: [-\d.]+px [-\d.]+px;?$/)
  })
})
