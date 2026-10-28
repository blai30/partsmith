import { act, cleanup, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { showPickHint } from '../app/actions/notifications'
import { installPopoverPolyfill } from '../components/popover-polyfill.testing'
import { pressEnterAsResultShows } from '../expression/enter-timing.testing'
import type {
  BoundingBox,
  Diagnostic,
  Dimension,
  FeatureStatus,
  FixRef,
} from '../generated/protocol'
import {
  activeCommandWith,
  CYLINDRICAL_FACE,
  cubeDocumentState,
  documentState,
  readyState,
  rectSketchDocumentState,
} from '../layout/fixtures.testing'
import { createModelApp, renderWithApp, stubMatchMedia } from '../layout/test-app.testing'
import { MESSAGES } from '../messages'
import { sketchSession } from '../sketch/fixtures.testing'
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
  RECT_REGION,
  RECT_SKETCH_SOLUTION,
  TWO_BODIES_DOCUMENT,
} from '../test/fakes/documents'
import {
  bodyItem,
  elementItem,
  featureItem,
  originItem,
  parameterItem,
  regionItem,
  sketchPointItem,
} from '../test/fakes/selection'
import { deferred } from '../test/fakes/deferred'
import { stubResizeObserver } from '../test/fakes/resize-observer'
import { settle } from '../test/fakes/settle'
import { CUBE_BOUNDS, meshSummary } from '../test/fakes/state'
import { lostEdgeRepair } from '../test/fakes/repairs'
import { createFakeViewport } from '../test/fakes/viewport'
import { hasTabularFigures, isCode } from '../test/type-roles'
import { COLOR_CLASSES } from '../theme/roles'
import { Inspector } from './Inspector'
import { FILLET_EDGES, hasRoleClasses, inspectorField } from './inspector.testing'
import type { Rectangle } from './placement'

beforeEach(installPopoverPolyfill)

beforeEach(() => {
  stubMatchMedia()
})

afterEach(() => {
  cleanup()
  vi.useRealTimers()
  vi.unstubAllGlobals()
})

const EDIT_FILLET = activeCommandWith({
  featureType: 'fillet',
  featureId: 'f-fil1',
  editing: true,
  title: 'Fillet 1',
  params: { body: 'b-cube', edges: FILLET_EDGES, radius: '2 mm' },
  applied: true,
  preview: 'shown',
})

const NEW_EXTRUDE = activeCommandWith({
  featureType: 'extrude',
  featureId: 'f-0123456789ab',
  title: 'Extrude',
  params: { extent: { kind: 'distance', expr: '10 mm' }, op: 'new' },
  activeField: 'profile',
})

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

function fieldPaths(): (string | null)[] {
  return within(inspector())
    .getAllByTestId('inspector-field')
    .map((element) => element.getAttribute('data-path'))
}

function status(
  messages: readonly Diagnostic[],
  state: FeatureStatus['status'] = 'error',
): FeatureStatus {
  return { id: 'f-fil1', status: state, messages: [...messages], repairs: [] }
}

const SMALLER_FIX: FixRef = {
  feature: 'f-fil1',
  code: 'fillet_too_large',
  field: '/radius',
  cause: false,
  index: 0,
  revision: 1,
}

const TOO_LARGE: Diagnostic = {
  severity: 'error',
  code: 'fillet_too_large',
  message:
    'Fillet radius 15 mm is too large for the selected edges. The largest radius that works is about 2.3 mm.',
  field: '/radius',
  suggestion: '2.3 mm',
  actions: [{ label: 'Use 2.3 mm', run: { kind: 'apply', fix: SMALLER_FIX } }],
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
    const edges = within(inspectorField('edges')).getByTestId('selection-field')
    expect(edges).toHaveAttribute('role', 'button')
    expect(edges).toHaveAttribute('data-count', '4')
    expect(edges).toHaveAttribute('data-active', 'false')
    expect(edges).toHaveTextContent('4 edges')
    await waitFor(() => {
      expect(within(inspectorField('radius')).getByTestId('expression-result')).toHaveTextContent(
        '= 2 mm',
      )
    })
    const radius = within(inspectorField('radius')).getByRole('combobox', { name: 'Radius' })
    expect(radius).toHaveClass('tabular-nums')
    expect(radius).not.toHaveClass('font-mono')
  })

  it("U-205 sets U-31's Distance input in tabular figures, not code", () => {
    const command: ActiveCommand = {
      ...NEW_EXTRUDE,
      params: { extent: { kind: 'distance', expr: '30 mm' }, op: 'new' },
    }
    renderInspector(rectSketchDocumentState(), { command })
    const distance = within(inspectorField('extent.expr')).getByRole('combobox', {
      name: 'Distance',
    })
    expect(hasTabularFigures(distance)).toBe(true)
    expect(isCode(distance)).toBe(false)
  })

  it('makes a selection field active on click and empties it with its clear button', async () => {
    const { app, user } = renderInspector(filletDocument(), { command: EDIT_FILLET })
    const edges = within(inspectorField('edges')).getByTestId('selection-field')
    await user.click(edges)
    expect(app.store.getState().command?.activeField).toBe('edges')
    expect(edges).toHaveAttribute('data-active', 'true')
    expect(hasRoleClasses(edges, COLOR_CLASSES.accent.border)).toBe(true)
    await user.click(within(inspectorField('edges')).getByRole('button', { name: 'Clear Edges' }))
    expect(app.store.getState().command?.params.edges ?? []).toEqual([])
  })

  it('shows "Select regions" in an empty required selection field, dashed in the accent', () => {
    renderInspector(rectSketchDocumentState(), { command: NEW_EXTRUDE })
    const profile = within(inspectorField('profile')).getByTestId('selection-field')
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
    const profile = within(inspectorField('profile')).getByTestId('selection-field')
    const shown = within(inspectorField('profile')).getByText(hint)
    expect(hasRoleClasses(shown, COLOR_CLASSES.textMuted.foreground)).toBe(true)
    // The field's help (2.12) describes the box too, after the hint.
    const help = 'The closed sketch shapes to extrude. Click inside a shape to add it.'
    expect(profile).toHaveAccessibleDescription(`${hint} ${help}`)
    expect(within(inspectorField('profile')).getByTestId('field-error')).toHaveTextContent('')
    act(() => {
      vi.advanceTimersByTime(3_000)
    })
    expect(within(inspectorField('profile')).queryByText(hint)).toBeNull()
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
    const edges = within(inspectorField('edges')).getByTestId('selection-field')
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
    const extent = within(inspectorField('extent.kind')).getByRole('radiogroup')
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
      const extent = within(inspectorField('extent.kind'))
      expect(extent.queryByRole('radiogroup')).toBeNull()
      const select = extent.getByRole('combobox', { name: 'Extent' })
      expect(
        within(select)
          .getAllByRole('option')
          .map((option) => option.textContent),
      ).toEqual(['Distance', 'Symmetric', 'Two sides', 'Through all'])
      // Its own chevron follows the UI scale, where the browser's arrow would not (3.7).
      expect(select).toHaveClass('appearance-none', 'pr-8')
      const chevron = select.nextElementSibling
      expect(chevron?.tagName.toLowerCase()).toBe('svg')
      expect(chevron).toHaveAttribute('aria-hidden', 'true')
      expect(chevron).toHaveClass('pointer-events-none', 'absolute', 'right-2', 'size-4')
      await user.selectOptions(select, 'Through all')
      expect(app.store.getState().command?.params.extent).toMatchObject({ kind: 'through_all' })
    } finally {
      scrollWidth.mockRestore()
      clientWidth.mockRestore()
    }
  })

  it('writes a typed expression into the command params', async () => {
    const { app, user } = renderInspector(filletDocument(), { command: EDIT_FILLET })
    const radius = within(inspectorField('radius')).getByRole('combobox', { name: 'Radius' })
    await user.clear(radius)
    await user.type(radius, '3 mm')
    await waitFor(() => {
      expect(app.store.getState().command?.params.radius).toBe('3 mm')
    })
  })

  it('previews a retyped value only once its text is evaluated (3.7)', async () => {
    const { core, user } = renderInspector(filletDocument(), { command: EDIT_FILLET })
    const field = inspectorField('radius')
    await waitFor(() => {
      expect(within(field).getByTestId('expression-result')).toHaveTextContent('= 2 mm')
    })
    const evaluation = deferred<{ value: number; dimension: Dimension }>()
    core.stub('evaluate', () => evaluation.promise)
    const previewOfThree = [
      1,
      null,
      expect.arrayContaining([
        expect.objectContaining({
          type: 'update_feature',
          feature: expect.objectContaining({ radius: '3' }),
        }),
      ]),
    ]
    const radius = within(field).getByRole('combobox', { name: 'Radius' })
    await user.type(radius, '3', { initialSelectionStart: 0, initialSelectionEnd: 4 })
    await settle()
    expect(radius).toHaveValue('3')
    expect(core.callsTo('applyEdits')).not.toContainEqual(previewOfThree)
    await act(async () => {
      evaluation.resolve({ value: 3, dimension: { length: 0, angle: 0 } })
      await settle()
    })
    expect(core.callsTo('applyEdits')).toContainEqual(previewOfThree)
  })

  it('previews nothing of a text still being evaluated when its optional field is emptied (3.7)', async () => {
    const hole = activeCommandWith({
      featureType: 'hole',
      featureId: 'f-hole9',
      title: 'Hole',
      params: {
        points: ['e-p1'],
        sketch: 'f-sk1',
        body: 'b-cube',
        diameter: '3.2 mm',
        hole_type: { kind: 'simple' },
        extent: { kind: 'through_all' },
        tip_angle: '118 deg',
      },
      applied: true,
      preview: 'shown',
    })
    const { core, user } = renderInspector(cubeDocumentState(), { command: hole })
    await user.click(within(inspector()).getByTestId('inspector-more'))
    const field = inspectorField('tip_angle')
    await waitFor(() => {
      expect(within(field).getByTestId('expression-result')).toHaveTextContent('= 118°')
    })
    core.stub('evaluate', () => deferred<{ value: number; dimension: Dimension }>().promise)
    const tipAngle = within(field).getByRole('combobox', { name: 'Tip angle' })
    await user.type(tipAngle, '9', { initialSelectionStart: 0, initialSelectionEnd: 7 })
    await user.clear(tipAngle)
    await settle()
    const previews = core.callsTo('applyEdits')
    expect(previews).not.toContainEqual([
      1,
      null,
      expect.arrayContaining([
        expect.objectContaining({ feature: expect.objectContaining({ tip_angle: '9' }) }),
      ]),
    ])
    expect(previews.at(-1)).toEqual([
      1,
      null,
      [
        expect.objectContaining({
          type: 'update_feature',
          feature: expect.not.objectContaining({ tip_angle: expect.anything() }),
        }),
      ],
    ])
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
    const radius = within(inspectorField('radius')).getByRole('combobox', { name: 'Radius' })
    await waitFor(() => {
      expect(within(inspectorField('radius')).getByTestId('expression-result')).toHaveTextContent(
        '= 2 mm',
      )
    })
    await user.click(radius)
    await user.keyboard('{Enter}')
    await waitFor(() => {
      expect(core.callsTo('commitTransaction')).toEqual([[1]])
    })
  })

  it('E-13 confirms with Enter pressed as the result shows, before the field reports it (3.5)', async () => {
    const command: ActiveCommand = {
      ...NEW_EXTRUDE,
      params: {
        profile: { sketch: 'f-sk1', regions: [RECT_REGION.reference] },
        extent: { kind: 'distance', expr: '10 mm' },
        op: 'new',
      },
      activeField: null,
    }
    const { app, core, user } = renderInspector(rectSketchDocumentState(), { command })
    const field = inspectorField('extent.expr')
    const result = within(field).getByTestId('expression-result')
    await waitFor(() => {
      expect(result).toHaveTextContent('= 10 mm')
    })
    const evaluation = deferred<{ value: number; dimension: Dimension }>()
    core.stub('evaluate', () => evaluation.promise)
    const distance = within(field).getByRole('combobox', { name: 'Distance' })
    await user.clear(distance)
    await user.paste('10')
    await settle()
    await pressEnterAsResultShows(distance, result, 'mm', () => {
      evaluation.resolve({ value: 10, dimension: { length: 0, angle: 0 } })
    })
    await waitFor(() => {
      expect(core.callsTo('commitTransaction')).toEqual([[1]])
    })
    expect(app.store.getState().command).toBeNull()
  })

  it('fills a body field from its Choose menu of bodies with geometry', async () => {
    const { app, user } = renderInspector(filletDocument(), {
      command: { ...EDIT_FILLET, params: { radius: '2 mm' } },
    })
    await user.click(within(inspector()).getByTestId('inspector-more'))
    await user.click(within(inspectorField('body')).getByRole('button', { name: /Choose/ }))
    await user.click(screen.getByRole('menuitem', { name: /Cube/ }))
    expect(app.store.getState().command?.params.body).toBe('b-cube')
  })
})

describe('help on hover (2.10, 3.7)', () => {
  const OPERATION_HELP =
    'New body makes a separate part. Join adds to the body, Cut removes from it, and Intersect keeps only the overlap.'

  it('marks a label with help with cursor-help and describes its control with the help', () => {
    renderInspector(rectSketchDocumentState(), { command: NEW_EXTRUDE })
    const operation = inspectorField('op')
    expect(within(operation).getByText('Operation')).toHaveClass('cursor-help')
    expect(within(operation).getByRole('radiogroup')).toHaveAccessibleDescription(OPERATION_HELP)
    expect(
      within(inspectorField('profile')).getByTestId('selection-field'),
    ).toHaveAccessibleDescription(
      'The closed sketch shapes to extrude. Click inside a shape to add it.',
    )
  })

  it('shows the help in a tooltip when the pointer rests on the label', async () => {
    const { user } = renderInspector(rectSketchDocumentState(), { command: NEW_EXTRUDE })
    await user.hover(within(inspectorField('op')).getByText('Operation'))
    await waitFor(() => {
      expect(screen.getByRole('tooltip')).toHaveTextContent('Intersect keeps only the overlap')
    })
  })

  it('gives a field without help no tooltip and no cue', () => {
    renderInspector(rectSketchDocumentState(), { command: NEW_EXTRUDE })
    const distance = inspectorField('extent.expr')
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
    expect(within(inspectorField('thickness')).getByRole('combobox')).toHaveAccessibleDescription(
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
    expect(within(inspectorField('profile')).queryByRole('button', { name: /Choose/ })).toBeNull()
  })

  it('lists the regions of a sketch shown this session', async () => {
    const { user } = renderInspector(cubeDocumentState(), {
      command: NEW_EXTRUDE,
      view: { ...createAppStore().getState().view, sessionVisible: { 'feature:f-sk1': true } },
    })
    await user.click(within(inspectorField('profile')).getByRole('button', { name: /Choose/ }))
    expect(screen.getByRole('menuitem', { name: /^Region 1 \(/ })).toBeInTheDocument()
  })
})

describe('feature status in the inspector (3.7, 4.2)', () => {
  it('U-87 shows an error message with a field under that field', async () => {
    renderInspector(filletDocument(status([TOO_LARGE])), {
      command: { ...EDIT_FILLET, preview: 'failed' },
    })
    await waitFor(() => {
      expect(within(inspectorField('radius')).getByTestId('field-error')).toHaveTextContent(
        TOO_LARGE.message,
      )
    })
  })

  it('U-87 its action "Use 2.3 mm" calls applyFix with its FixRef', async () => {
    const { core, user } = renderInspector(filletDocument(status([TOO_LARGE])), {
      command: { ...EDIT_FILLET, preview: 'failed' },
    })
    const [use] = within(inspector()).getAllByRole('button', { name: 'Use 2.3 mm' })
    await user.click(use ?? inspector())
    await waitFor(() => {
      expect(core.callsTo('applyFix')).toEqual([[1, SMALLER_FIX]])
    })
    expect(core.callsTo('applyEdits')).toEqual([])
  })

  it('U-87 offers no "Use" of its own for a suggestion without an action (open issue 26)', () => {
    renderInspector(filletDocument(status([{ ...TOO_LARGE, actions: [] }])), {
      command: { ...EDIT_FILLET, preview: 'failed' },
    })
    expect(within(inspector()).queryByRole('button', { name: /^Use / })).toBeNull()
  })

  it('U-211 draws the fix "Use 2.3 mm" on the banner as a banner button', () => {
    renderInspector(filletDocument(status([TOO_LARGE])), {
      command: { ...EDIT_FILLET, preview: 'failed' },
    })
    const fix = within(screen.getByTestId('inspector-error')).getByRole('button', {
      name: 'Use 2.3 mm',
    })
    expect(fix).toHaveClass(COLOR_CLASSES.tonal.banner)
    expect(fix).toHaveClass(COLOR_CLASSES.hover.banner)
    expect(fix).not.toHaveClass(COLOR_CLASSES.tonal.background)
  })

  it('shows an info notice alone on a neutral banner, with its actions and no field error (3.7)', () => {
    const followed: Diagnostic = {
      severity: 'info',
      code: 'reference_followed',
      message:
        "The right top edge of 'Extrude 1' selected for Edges now follows the edge that replaced it.",
      field: '/edges/0',
      actions: [
        {
          label: 'Keep this match',
          run: {
            kind: 'apply',
            fix: {
              feature: 'f-fil1',
              code: 'reference_followed',
              field: '/edges/0',
              cause: false,
              index: 0,
              revision: 1,
            },
          },
        },
      ],
    }
    renderInspector(filletDocument(status([followed], 'ok')), { command: EDIT_FILLET })
    const banner = within(inspector()).getByTestId('inspector-error')
    expect(banner).toHaveAttribute('data-severity', 'info')
    expect(banner).toHaveTextContent(followed.message)
    expect(within(banner).getByRole('button', { name: 'Keep this match' })).toBeInTheDocument()
    // The field's error is a live region that stays mounted (see the pick hint test above): empty.
    expect(within(inspectorField('edges')).getByTestId('field-error')).toHaveTextContent('')
  })

  it('U-87 maps a message with field /edges/1 to the edges field', () => {
    const lost: Diagnostic = {
      severity: 'warning',
      code: 'reference_ambiguous',
      message: 'One of the edges selected for Edges now matches 2 edges, so it was skipped.',
      field: '/edges/1',
    }
    renderInspector(filletDocument(status([lost], 'warning')), { command: EDIT_FILLET })
    expect(within(inspectorField('edges')).getByTestId('field-error')).toHaveTextContent(
      lost.message,
    )
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
    expect(within(panel).getByTestId('body-size')).toHaveAttribute('data-value', '20 × 20 × 20 mm')
    expect(within(panel).getByTestId('body-center')).toHaveAttribute('data-value', '0, 0, 10 mm')
    expect(within(panel).getByRole('switch', { name: 'Visible' })).toBeChecked()
  })

  describe('coordinate readouts (3.8, U-182)', () => {
    const BOX_BOUNDS: BoundingBox = { min: [0, 0, 0], max: [40, 20, 10] }
    const AXIS_FOREGROUNDS = [
      COLOR_CLASSES.axisX.foreground,
      COLOR_CLASSES.axisY.foreground,
      COLOR_CLASSES.axisZ.foreground,
    ]

    function renderBox(): HTMLElement {
      renderInspector(cubeDocumentState({ meshes: { 'b-cube': meshSummary(1, BOX_BOUNDS) } }), {
        selection: [bodyItem('b-cube', 'Cube')],
      })
      return inspector()
    }

    it('U-182 reads the size and center after their axis letters, keeping the plain text', () => {
      const panel = renderBox()
      const size = within(panel).getByTestId('body-size')
      const center = within(panel).getByTestId('body-center')
      expect(size.textContent).toBe('X 40 Y 20 Z 10 mm')
      expect(size).toHaveAttribute('data-value', '40 × 20 × 10 mm')
      expect(center.textContent).toBe('X 20 Y 10 Z 5 mm')
      expect(center).toHaveAttribute('data-value', '20, 10, 5 mm')
    })

    it.each(['body-size', 'body-center'])(
      'U-182 puts %s on a line of its own under its label, in three cells and the unit',
      (testId) => {
        const readout = within(renderBox()).getByTestId(testId)
        expect(readout).toHaveClass(
          'flex',
          'w-full',
          'flex-wrap',
          'items-baseline',
          'gap-x-2',
          'gap-y-1',
        )
        const label = readout.previousElementSibling
        expect(label).toHaveTextContent(testId === 'body-size' ? /^Size$/ : /^Center$/)
        expect(label).toHaveClass('text-xs', COLOR_CLASSES.textMuted.foreground)
        expect(readout.parentElement).toHaveClass('flex-col')
        const cells = [...readout.children].slice(0, 3)
        for (const [index, cell] of cells.entries()) {
          expect(cell).toHaveClass('flex', 'flex-1', 'basis-0', 'items-baseline', 'gap-1')
          const letter = within(cell as HTMLElement).getByTestId('axis-letter')
          expect(letter).toHaveTextContent(['X', 'Y', 'Z'][index] ?? '')
          expect(letter).not.toHaveAttribute('aria-hidden')
          expect(letter).toHaveClass('font-semibold', AXIS_FOREGROUNDS[index] ?? '')
          expect(cell.lastElementChild).toHaveClass('tabular-nums')
          expect(cell.lastElementChild).not.toHaveClass('font-mono')
        }
        expect(readout.lastElementChild).toHaveTextContent(/^mm$/)
        expect(readout.lastElementChild).toHaveClass('shrink-0')
      },
    )

    it('U-182 shows "Empty" without letters and no center for a body without geometry', () => {
      renderInspector(cubeDocumentState({ meshes: {} }), {
        selection: [bodyItem('b-cube', 'Cube')],
      })
      const size = within(inspector()).getByTestId('body-size')
      expect(size).toHaveTextContent(/^Empty$/)
      expect(size).toHaveAttribute('data-value', 'Empty')
      expect(within(inspector()).queryByTestId('axis-letter')).toBeNull()
      expect(within(inspector()).queryByTestId('body-center')).toBeNull()
    })

    it('U-182 reads a vertex’s position after its axis letters', () => {
      const vertex = {
        name: 'vertex(f-ext1/end-cap|f-ext1/side-0|f-ext1/side-1)',
        kind: 'vertex' as const,
        body: 'b-cube',
        feature: 'f-ext1',
        reference: CUBE_TOP_FACE.reference,
        centroid: [10, -10, 20] as [number, number, number],
        adjacent: [],
      }
      renderInspector(cubeDocumentState(), { selection: [elementItem(vertex)] })
      const position = within(inspector()).getByTestId('vertex-position')
      expect(position.textContent).toBe('X 10 Y -10 Z 20 mm')
      expect(position).toHaveAttribute('data-value', '10, -10, 20 mm')
      expect(position.previousElementSibling).toHaveTextContent(/^Position$/)
    })
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

  it(`U-110 offers "Show what 'Fillet 1' lost" and "Edit" for a feature with a lost edge, and no "Re-pick"`, async () => {
    const lost: FeatureStatus = { ...status([TOO_LARGE]), repairs: [lostEdgeRepair()] }
    const document = documentState(CUBE_FEATURES_DOCUMENT, {
      meshes: { 'b-cube': meshSummary(1, CUBE_BOUNDS) },
      featureStatus: { ...documentState(CUBE_FEATURES_DOCUMENT).featureStatus, 'f-fil1': lost },
    })
    const { app, user } = renderInspector(document, {
      selection: [featureItem('f-fil1', 'Fillet 1')],
    })
    const panel = inspector()
    expect(within(panel).getByRole('button', { name: 'Edit' })).toBeInTheDocument()
    expect(within(panel).queryByRole('button', { name: /^Re-pick/ })).toBeNull()
    await user.click(within(panel).getByRole('button', { name: "Show what 'Fillet 1' lost" }))
    await waitFor(() => {
      expect(app.store.getState().command?.activeField).toBe('edges')
    })
  })

  it("runs a feature message's apply action at rest through applyFix (4.2)", async () => {
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
      expect(core.callsTo('applyFix')).toEqual([[1, SMALLER_FIX]])
    })
    expect(core.callsTo('beginTransaction')).toEqual([])
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
        sketch: sketchSession({ selection: [sketchPointItem('f-sk1', 'e-p1')] }),
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
    expect(item?.querySelector('button')).toHaveClass(COLOR_CLASSES.hover.active)
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

/** The inspector's runtime translate (3.12), read from its style attribute. */
function translate(): string {
  return /^translate: ([^;]+);?$/.exec(inspector().getAttribute('style') ?? '')?.[1] ?? ''
}

describe('placement in the free-area layer (3.7, U-187)', () => {
  // The free-area layer of a 1280 by 720 window with the browser open, in page pixels.
  const LAYER: Rectangle = { x: 280, y: 64, width: 992, height: 548 }
  let boxes = new Map<string, Rectangle>()

  /** A rectangle given in the layer's coordinates, in page pixels. */
  function inLayer(x: number, y: number, width: number, height: number): Rectangle {
    return { x: LAYER.x + x, y: LAYER.y + y, width, height }
  }

  beforeEach(() => {
    boxes = new Map([
      ['layer', LAYER],
      ['viewport-controls', inLayer(16, 16, 42, 150)],
      // An empty slot holds only a hidden live region.
      ['top-center-slot', inLayer(176, 16, 640, 0)],
    ])
    // Elements are found by their test id, or by their accessible name when they have none.
    vi.spyOn(Element.prototype, 'getBoundingClientRect').mockImplementation(function box(
      this: Element,
    ) {
      const testId = this instanceof HTMLElement ? this.dataset['testid'] : undefined
      const key = testId ?? this.getAttribute('aria-label') ?? ''
      return DOMRect.fromRect(boxes.get(key) ?? { x: 0, y: 0, width: 0, height: 0 })
    })
    vi.spyOn(HTMLElement.prototype, 'offsetWidth', 'get').mockImplementation(function width(
      this: HTMLElement,
    ) {
      return this.dataset['testid'] === 'inspector' ? 320 : 0
    })
    vi.spyOn(HTMLElement.prototype, 'offsetHeight', 'get').mockImplementation(function height(
      this: HTMLElement,
    ) {
      return this.dataset['testid'] === 'inspector' ? 300 : 0
    })
  })

  afterEach(() => {
    vi.restoreAllMocks()
  })

  /**
   * Renders the inspector in a free-area layer beside the viewport controls, the view cube and the
   * top-center slot, with a viewport whose free area lies at (280, 64) in the canvas and whose
   * items project to `items` in canvas pixels.
   */
  function renderInLayer(
    state: Partial<AppState>,
    document: DocumentState = cubeDocumentState(),
    items: Rectangle = { x: 600, y: 200, width: 40, height: 40 },
  ) {
    const test = createModelApp({ state: readyState(document, state) })
    const viewport = createFakeViewport()
    viewport.viewWindow.mockReturnValue({ width: 1280, height: 720, free: LAYER })
    viewport.screenRectangle.mockImplementation((selected) =>
      selected.length === 0 ? null : items,
    )
    test.app.attachViewport(viewport)
    renderWithApp(
      test.app,
      <div data-testid="layer">
        <div data-testid="viewport-controls" />
        <fieldset data-testid="view-cube">
          <button type="button" aria-label="Home view" />
        </fieldset>
        <div data-testid="top-center-slot" />
        <Inspector />
      </div>,
    )
    return test
  }

  it('U-187 opens right of the viewport controls with nothing preselected', () => {
    renderInLayer({ command: NEW_EXTRUDE }, rectSketchDocumentState())
    expect(translate()).toBe('74px 16px')
  })

  it('U-187 opens beside its items, moved from canvas to free-area coordinates', () => {
    renderInLayer({ selection: [bodyItem('b-cube', 'Cube')] })
    expect(translate()).toBe('376px 136px')
  })

  it('U-187 opens under the top-center slot content where it fits', () => {
    boxes.set('top-center-slot', inLayer(176, 16, 640, 80))
    renderInLayer({ command: NEW_EXTRUDE }, rectSketchDocumentState())
    expect(translate()).toBe('74px 112px')
  })

  it('U-187 opens left of its items where right of them would reach the view cube', () => {
    // The cube at top-11 right-9, its Home view button 10rem from the free area's right edge.
    boxes.set('view-cube', inLayer(860, 44, 96, 96))
    boxes.set('Home view', inLayer(832, 16, 24, 24))
    // Items at (520, 136) in the free area: right of them the card would end at 912, past 832.
    renderInLayer({ selection: [bodyItem('b-cube', 'Cube')] }, cubeDocumentState(), {
      x: 800,
      y: 200,
      width: 40,
      height: 40,
    })
    expect(translate()).toBe('184px 136px')
  })

  it('U-187 opens at the free area corner where right of the controls is too narrow', () => {
    boxes.set('layer', { ...LAYER, width: 384 })
    renderInLayer({ command: NEW_EXTRUDE }, rectSketchDocumentState())
    expect(translate()).toBe('16px 16px')
  })

  it('clamps a dragged position kept from an earlier inspector into the free area', () => {
    const { app } = renderInLayer({})
    act(() => {
      app.store.setState((state) => ({
        panels: { ...state.panels, inspectorPosition: { x: 900, y: 400 } },
        selection: [bodyItem('b-cube', 'Cube')],
      }))
    })
    // 992 - 320 and 548 - 300: the whole card stays in the free area.
    expect(translate()).toBe('672px 248px')
    expect(app.store.getState().panels.inspectorPosition).toEqual({ x: 900, y: 400 })
  })

  it('moves an open card back into a free area that shrinks under it', () => {
    const observers = stubResizeObserver()
    const { app } = renderInLayer({})
    act(() => {
      app.store.setState((state) => ({
        panels: { ...state.panels, inspectorPosition: { x: 672, y: 16 } },
        selection: [bodyItem('b-cube', 'Cube')],
      }))
    })
    expect(translate()).toBe('672px 16px')
    // A docked side panel opens: the free area is 624 pixels wide.
    boxes.set('layer', { ...LAYER, width: 624 })
    act(() => {
      observers.fire()
    })
    expect(translate()).toBe('304px 16px')
  })
})
