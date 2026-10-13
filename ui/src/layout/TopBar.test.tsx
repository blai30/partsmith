import { cleanup, screen, waitFor, within } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { installPopoverPolyfill } from '../components/popover-polyfill.testing'
import { MESSAGES } from '../messages'
import { DEFAULT_SETTINGS, SETTINGS_KEY } from '../settings/settings'
import { INITIAL_STATE, type AppState, type SketchSession } from '../state/store'
import { catalogOf, TEST_CATALOG_ENTRIES } from '../test/fakes/catalog'
import { CUBE_TOP_FACE, RECT_REGION } from '../test/fakes/documents'
import { settle } from '../test/fakes/settle'
import { checkResult, finding, printReport } from '../test/fakes/state'
import {
  cubeDocumentState,
  documentState,
  featureItem,
  rectSketchDocumentState,
  regionItem,
  THREE_BOXES_DOCUMENT,
} from './fixtures.testing'
import { renderModelApp, stubMatchMedia } from './test-app.testing'

beforeEach(installPopoverPolyfill)

beforeEach(() => {
  stubMatchMedia()
})

afterEach(() => {
  cleanup()
  document.documentElement.classList.remove('dark')
  vi.unstubAllGlobals()
  vi.restoreAllMocks()
})

const EDITING_SKETCH: SketchSession = {
  sketchId: 'f-sk1',
  tool: null,
  construction: false,
  showConstraints: true,
  selection: [],
  dragging: false,
  cameraBefore: null,
}

function renderApp(state: Partial<AppState> = {}, document = cubeDocumentState()) {
  const test = renderModelApp({ document, state })
  return { ...test, topBar: screen.getByTestId('top-bar') }
}

type AddedBody = {
  readonly type: 'add_body'
  readonly index: number
  readonly body: { readonly id: string; readonly name: string }
}

type AddedFeature = {
  readonly type: 'add_feature'
  readonly index: number
  readonly feature: Readonly<Record<string, unknown>> & { readonly id: string }
}

/** A Storage kept in a map, for checking what the app saves. */
class MemoryStorage {
  private readonly items = new Map<string, string>()
  get length(): number {
    return this.items.size
  }
  clear(): void {
    this.items.clear()
  }
  getItem(key: string): string | null {
    return this.items.get(key) ?? null
  }
  key(index: number): string | null {
    return [...this.items.keys()][index] ?? null
  }
  removeItem(key: string): void {
    this.items.delete(key)
  }
  setItem(key: string, value: string): void {
    this.items.set(key, value)
  }
}

describe('the top bar (3.2)', () => {
  it('toggles the browser with "Toggle browser", which reflects it in aria-pressed', async () => {
    const { app, user, topBar } = renderApp()
    const toggle = within(topBar).getByRole('button', { name: 'Toggle browser' })
    expect(toggle).toHaveAttribute('aria-pressed', 'true')
    await user.click(toggle)
    expect(app.store.getState().panels.browserOpen).toBe(false)
    expect(toggle).toHaveAttribute('aria-pressed', 'false')
    expect(screen.queryByTestId('browser')).toBeNull()
  })

  it('shows the document name with data-dirty and an empty save state for a clean document', () => {
    renderApp()
    expect(screen.getByTestId('document-name')).toHaveTextContent('cube')
    expect(screen.getByTestId('document-name')).toHaveAttribute('data-dirty', 'false')
    expect(screen.getByTestId('save-state').textContent).toBe('')
  })

  it('shows "Edited" while the core reports the document dirty', () => {
    renderApp({}, cubeDocumentState({ dirty: true }))
    expect(screen.getByTestId('document-name')).toHaveAttribute('data-dirty', 'true')
    expect(screen.getByTestId('save-state')).toHaveTextContent('Edited')
  })

  it('shows "Edited" for a recovered document the core thinks is clean', () => {
    renderApp({}, cubeDocumentState({ localUnsaved: true }))
    expect(screen.getByTestId('document-name')).toHaveAttribute('data-dirty', 'true')
    expect(screen.getByTestId('save-state')).toHaveTextContent('Edited')
  })

  it('opens the Document menu with its entries in order', async () => {
    const { user } = renderApp()
    await user.click(screen.getByTestId('document-name'))
    const menu = screen.getByTestId('document-menu')
    expect(menu).toHaveAttribute('role', 'menu')
    const labels = within(menu)
      .getAllByRole('menuitem')
      .map((item) => item.textContent?.trim() ?? '')
    const expected = [
      'New',
      'Open...',
      'Open recent',
      'Save',
      'Save as...',
      'Export...',
      'Theme',
      'Navigation',
      'UI scale',
    ]
    expect(expected.map((label) => labels.findIndex((text) => text.startsWith(label)))).toEqual(
      expected.map((_, index) => index),
    )
  })

  it('switches context with the Context radio group', async () => {
    const { app, user, topBar } = renderApp()
    const context = within(topBar).getByRole('radiogroup', { name: 'Context' })
    expect(within(context).getByRole('radio', { name: 'Model' })).toBeChecked()
    await user.click(within(context).getByRole('radio', { name: 'Print' }))
    expect(app.store.getState().context).toBe('print')
    expect(screen.getByTestId('app')).toHaveAttribute('data-context', 'print')
  })

  it('starts a new sketch with the Sketch radio outside sketch mode (3.3)', async () => {
    const { app, user, topBar } = renderApp()
    await user.click(within(topBar).getByRole('radio', { name: 'Sketch' }))
    await settle()
    expect(app.store.getState().context).toBe('sketch')
    expect(app.store.getState().sketch?.sketchId).toBeNull()
  })

  it('U-113 describes the Sketch radio as "Edit Sketch 1" with the sketch row selected, and choosing it edits that sketch (3.3)', async () => {
    const { app, user, topBar } = renderApp({ selection: [featureItem('f-sk1', 'Sketch 1')] })
    const radio = within(topBar).getByRole('radio', { name: 'Sketch' })
    expect(radio).toHaveAccessibleDescription('Edit Sketch 1')
    await user.hover(radio)
    await waitFor(() => {
      expect(screen.getByRole('tooltip')).toHaveTextContent('Edit Sketch 1')
    })
    await user.click(radio)
    await settle()
    expect(app.store.getState().context).toBe('sketch')
    expect(app.store.getState().sketch?.sketchId).toBe('f-sk1')
  })

  it('describes the Sketch radio as "New sketch" outside sketch mode and not at all inside it, keeping its focus (3.3)', async () => {
    const { app, user, topBar } = renderApp()
    const radio = within(topBar).getByRole('radio', { name: 'Sketch' })
    expect(radio).toHaveAccessibleDescription('New sketch')
    expect(within(topBar).getByRole('radio', { name: 'Model' })).not.toHaveAccessibleDescription()
    radio.focus()
    await user.keyboard(' ')
    await settle()
    expect(app.store.getState().context).toBe('sketch')
    expect(radio).toHaveFocus()
    expect(radio).not.toHaveAccessibleDescription()
  })

  it('cancels an active feature command when switching from Model to Print, keeping the selection', async () => {
    const { app, core, user, topBar } = renderApp({
      command: {
        featureType: 'fillet',
        featureId: 'f-fil9',
        editing: false,
        title: 'Fillet',
        params: {},
        activeField: 'edges',
        fieldErrors: {},
        applied: false,
        newBody: null,
        helper: null,
        changedPaths: [],
        history: [],
        future: [],
        preview: 'none',
      },
      selection: [regionItem('f-sk1', RECT_REGION)],
    })
    await user.click(within(topBar).getByRole('radio', { name: 'Print' }))
    await settle()
    expect(core.callsTo('cancelTransaction')).toEqual([[1]])
    expect(app.store.getState().command).toBeNull()
    expect(app.store.getState().context).toBe('print')
    expect(app.store.getState().selection).toHaveLength(1)
  })

  it('finishes the sketch when choosing Model in sketch mode', async () => {
    const { app, user, topBar } = renderApp({ context: 'sketch', sketch: EDITING_SKETCH })
    await user.click(within(topBar).getByRole('radio', { name: 'Model' }))
    await settle()
    expect(app.store.getState().context).toBe('model')
    expect(app.store.getState().sketch).toBeNull()
  })

  it('names the context toolbar after the context', () => {
    renderApp()
    expect(screen.getByRole('toolbar', { name: 'Model tools' })).toBeInTheDocument()
    cleanup()
    renderApp({ context: 'print' })
    expect(screen.getByRole('toolbar', { name: 'Print tools' })).toBeInTheDocument()
    cleanup()
    renderApp({ context: 'sketch', sketch: EDITING_SKETCH })
    expect(screen.getByRole('toolbar', { name: 'Sketch tools' })).toBeInTheDocument()
  })

  it('shows the Model tools in toolbar order, with Shapes and Pattern groups', () => {
    renderApp()
    const toolbar = screen.getByRole('toolbar', { name: 'Model tools' })
    const names = [
      'Sketch',
      'Extrude',
      'Revolve',
      'Hole',
      'Fillet',
      'Chamfer',
      'Shell',
      'Shapes',
      'Pattern',
      'Mirror',
      'Boolean',
    ]
    const buttons = names.map((name) => within(toolbar).getByRole('button', { name }))
    expect(within(toolbar).getAllByRole('button')).toHaveLength(names.length)
    for (let i = 1; i < buttons.length; i++) {
      const order = buttons[i - 1]?.compareDocumentPosition(buttons[i] ?? toolbar) ?? 0
      expect(order & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy()
    }
  })

  it('leaves out toolbar entries whose command is not registered', () => {
    const catalog = catalogOf(TEST_CATALOG_ENTRIES.filter((entry) => entry.type !== 'revolve'))
    renderApp({ catalog })
    const toolbar = screen.getByRole('toolbar', { name: 'Model tools' })
    expect(within(toolbar).queryByRole('button', { name: 'Revolve' })).toBeNull()
    expect(within(toolbar).getByRole('button', { name: 'Extrude' })).toBeInTheDocument()
  })

  it('opens a group button’s menu with its commands', async () => {
    const { user } = renderApp()
    await user.click(screen.getByRole('button', { name: 'Shapes' }))
    const menu = screen.getByRole('menu')
    expect(
      within(menu)
        .getAllByRole('menuitem')
        .map((item) => item.textContent?.trim()),
    ).toEqual([
      expect.stringContaining('Box'),
      expect.stringContaining('Cylinder'),
      expect.stringContaining('Sphere'),
      expect.stringContaining('Cone'),
      expect.stringContaining('Torus'),
    ])
  })

  it('leaves out a group button whose commands are all hidden, as while picking a sketch plane', () => {
    renderApp({ context: 'sketch', sketch: { ...EDITING_SKETCH, sketchId: null } })
    const toolbar = screen.getByRole('toolbar', { name: 'Sketch tools' })
    expect(within(toolbar).queryByRole('button', { name: 'Rectangle' })).toBeNull()
    expect(within(toolbar).queryByRole('button', { name: 'Constrain' })).toBeNull()
    cleanup()
    renderApp({ context: 'sketch', sketch: EDITING_SKETCH })
    const editing = screen.getByRole('toolbar', { name: 'Sketch tools' })
    expect(within(editing).getByRole('button', { name: 'Rectangle' })).toBeInTheDocument()
    expect(within(editing).getByRole('button', { name: 'Constrain' })).toBeInTheDocument()
  })

  it('marks disabled toolbar commands with aria-disabled', () => {
    renderApp({
      core: { status: 'failed', engineVersion: null, fatal: 'HTTP 404', pendingMutations: 0 },
    })
    expect(screen.getByRole('button', { name: 'Extrude' })).toHaveAttribute('aria-disabled', 'true')
  })

  it('describes a toolbar command on the second line of its tooltip (2.10, 3.24)', async () => {
    const { topBar, user } = renderApp()
    const shell = within(topBar).getByRole('button', { name: 'Shell' })
    expect(shell).toHaveAccessibleDescription(/Hollows out a body/)
    await user.hover(shell)
    await waitFor(() => {
      expect(screen.getByRole('tooltip')).toHaveTextContent(
        'Hollows out a body, leaving walls of the given thickness and the picked faces open.',
      )
    })
  })

  it('presses the active sketch tool', () => {
    renderApp({ context: 'sketch', sketch: { ...EDITING_SKETCH, tool: 'line' } })
    const toolbar = screen.getByRole('toolbar', { name: 'Sketch tools' })
    expect(within(toolbar).getByRole('button', { name: 'Line' })).toHaveAttribute(
      'aria-pressed',
      'true',
    )
    expect(within(toolbar).getByRole('button', { name: 'Circle' })).toHaveAttribute(
      'aria-pressed',
      'false',
    )
  })

  it('shows the formatted Mod+K hint on the Search button and opens the palette', async () => {
    const { app, user } = renderApp()
    const button = screen.getByTestId('palette-button')
    expect(button).toHaveTextContent('Search')
    expect(button).toHaveTextContent('Ctrl+K')
    await user.click(button)
    expect(app.store.getState().panels.paletteOpen).toBe(true)
    expect(screen.getByTestId('palette')).toBeInTheDocument()
  })

  it('opens the export popover with the Export button', async () => {
    const { user, topBar } = renderApp()
    await user.click(within(topBar).getByRole('button', { name: 'Export' }))
    expect(screen.getByTestId('export-popover')).toBeVisible()
  })

  it('U-94 the theme toggle switches to dark, adds the dark class and stores the choice', async () => {
    const storage = new MemoryStorage()
    vi.stubGlobal('localStorage', storage)
    stubMatchMedia({ dark: false })
    const { app, user } = renderApp({ settings: { ...DEFAULT_SETTINGS, theme: 'system' } })
    const toggle = screen.getByTestId('theme-toggle')
    expect(toggle).toHaveAccessibleName('Dark theme')
    expect(toggle).toHaveAttribute('aria-pressed', 'false')
    await user.click(toggle)
    expect(app.store.getState().settings.theme).toBe('dark')
    expect(document.documentElement).toHaveClass('dark')
    expect(storage.getItem(SETTINGS_KEY)).toContain('"theme":"dark"')
    expect(toggle).toHaveAttribute('aria-pressed', 'true')
  })

  it('U-94 clicking the theme toggle again sets light and removes the dark class', async () => {
    const storage = new MemoryStorage()
    vi.stubGlobal('localStorage', storage)
    stubMatchMedia({ dark: false })
    const { app, user } = renderApp({ settings: { ...DEFAULT_SETTINGS, theme: 'system' } })
    const toggle = screen.getByTestId('theme-toggle')
    await user.click(toggle)
    await user.click(toggle)
    expect(app.store.getState().settings.theme).toBe('light')
    expect(document.documentElement).not.toHaveClass('dark')
    expect(storage.getItem(SETTINGS_KEY)).toContain('"theme":"light"')
  })

  it('applies the dark class for a dark OS while the theme follows the system', () => {
    stubMatchMedia({ dark: true })
    renderApp({ settings: { ...DEFAULT_SETTINGS, theme: 'system' } })
    expect(document.documentElement).toHaveClass('dark')
    expect(screen.getByTestId('theme-toggle')).toHaveAttribute('aria-pressed', 'true')
  })
})

describe('feature commands from the toolbar (3.7)', () => {
  it('U-64 Extrude with a preselected region and no bodies begins "Extrude", adds Body 1 and the feature, then focuses it', async () => {
    const { core, user } = renderApp(
      { selection: [regionItem('f-sk1', RECT_REGION)] },
      rectSketchDocumentState(),
    )
    // The selection inspector suggests Extrude too (3.8); this is the toolbar's.
    const toolbar = screen.getByRole('toolbar', { name: 'Model tools' })
    await user.click(within(toolbar).getByRole('button', { name: 'Extrude' }))
    await waitFor(() => {
      expect(core.callsTo('setFocus')).toHaveLength(1)
    })
    expect(core.callsTo('beginTransaction')).toEqual([[1, 'Extrude']])
    const applied = core.callsTo('applyEdits')
    expect(applied).toHaveLength(1)
    const [documentHandle, label, edits] = applied[0] ?? []
    expect(documentHandle).toBe(1)
    expect(label).toBeNull()
    const [addBody, addFeature, ...rest] = edits as readonly [AddedBody, AddedFeature, ...unknown[]]
    expect(rest).toEqual([])
    expect(addBody).toEqual({
      type: 'add_body',
      index: 0,
      body: { id: expect.stringMatching(/^b-[0-9a-f]{12}$/), name: 'Body 1' },
    })
    const bodyId = addBody.body.id
    expect(addFeature).toEqual({
      type: 'add_feature',
      index: 1,
      feature: expect.objectContaining({
        id: expect.stringMatching(/^f-[0-9a-f]{12}$/),
        type: 'extrude',
        body: bodyId,
        profile: { sketch: 'f-sk1', regions: [RECT_REGION.reference] },
        extent: { kind: 'distance', expr: '10 mm' },
        op: 'new',
      }),
    })
    expect(core.callsTo('setFocus')).toEqual([[1, addFeature.feature.id]])
    expect(Object.keys(addFeature.feature)).not.toContain('name')
  })

  it('U-65 with the marker before the second of 3 features, the first preview inserts at index 1 with no set_rollback', async () => {
    const { core, user } = renderApp(
      {},
      documentState({ ...THREE_BOXES_DOCUMENT, rollback: 'f-box2' }),
    )
    await user.click(screen.getByRole('button', { name: 'Shapes' }))
    await user.click(screen.getByRole('menuitem', { name: /Box/ }))
    await waitFor(() => {
      expect(core.callsTo('applyEdits')).toHaveLength(1)
    })
    const [, , edits] = core.callsTo('applyEdits')[0] ?? []
    const addFeature = (edits as readonly Record<string, unknown>[]).find(
      (edit) => edit.type === 'add_feature',
    )
    expect(addFeature?.index).toBe(1)
    for (const [, , batch] of core.callsTo('applyEdits')) {
      expect(
        (batch as readonly Record<string, unknown>[]).some((edit) => edit.type === 'set_rollback'),
      ).toBe(false)
    }
  })

  it('U-89 disables the Export button during a feature command', async () => {
    const { user, topBar } = renderApp({
      command: {
        featureType: 'fillet',
        featureId: 'f-fil9',
        editing: false,
        title: 'Fillet',
        params: {},
        activeField: 'edges',
        fieldErrors: {},
        applied: false,
        newBody: null,
        helper: null,
        changedPaths: [],
        history: [],
        future: [],
        preview: 'none',
      },
    })
    const exportButton = within(topBar).getByRole('button', { name: 'Export' })
    expect(exportButton).toHaveAttribute('aria-disabled', 'true')
    await user.click(exportButton)
    expect(screen.queryByTestId('export-popover')).toBeNull()
  })
})

describe('the Print toolbar (3.3, 3.17)', () => {
  it('E-47 lists Printer, Overhang map, Checks, Print check, Lay on face, Export 3MF and Export', () => {
    renderApp({ context: 'print' })
    const toolbar = screen.getByRole('toolbar', { name: 'Print tools' })
    const names = within(toolbar)
      .getAllByRole('button')
      .map((button) => button.getAttribute('aria-label') ?? button.textContent)
    expect(names).toEqual([
      'Printer: Generic FDM 0.4 mm (default)',
      'Overhang map',
      'Checks',
      'Print check',
      'Lay on face',
      'Export 3MF',
      'Export',
    ])
    expect(within(toolbar).getByRole('button', { name: 'Overhang map' })).toHaveAttribute(
      'aria-pressed',
      'true',
    )
  })

  it('Lay on face starts the face pick with the print hint, and Esc ends it (3.17)', async () => {
    const { app, user } = renderApp({ context: 'print' })
    const toolbar = screen.getByRole('toolbar', { name: 'Print tools' })
    const layOnFace = within(toolbar).getByRole('button', { name: 'Lay on face' })
    expect(layOnFace).toHaveAttribute('aria-pressed', 'false')
    // The live region is there before the pick, so its prompt is announced (3.24).
    const hint = screen.getByTestId('print-hint')
    expect(hint).toHaveTextContent('')
    expect(hint).toHaveClass('sr-only')
    await user.click(layOnFace)
    expect(app.store.getState().print.pickingFace).toBe(true)
    expect(screen.getByTestId('print-hint')).toBe(hint)
    expect(hint).toHaveTextContent('Pick the face that should touch the bed.')
    expect(hint).toHaveRole('status')
    expect(hint).not.toHaveClass('sr-only')
    await user.keyboard('{Escape}')
    expect(app.store.getState().print.pickingFace).toBe(false)
    expect(hint).toHaveTextContent('')
    expect(hint).toHaveClass('sr-only')
  })

  it('Lay on face is pressed while an orientation is set, and a click prints as modeled again', async () => {
    const orientation = {
      kind: 'face_down',
      body: 'b-cube',
      face: CUBE_TOP_FACE.reference,
    } as const
    const { app, user } = renderApp({
      context: 'print',
      print: { ...INITIAL_STATE.print, orientation },
    })
    const toolbar = screen.getByRole('toolbar', { name: 'Print tools' })
    const layOnFace = within(toolbar).getByRole('button', { name: 'Lay on face' })
    expect(layOnFace).toHaveAttribute('aria-pressed', 'true')
    await user.click(layOnFace)
    expect(app.store.getState().print).toMatchObject({ orientation: null, pickingFace: false })
    expect(layOnFace).toHaveAttribute('aria-pressed', 'false')
  })

  it('names the printer in use, the default one when the document has none', () => {
    renderApp({ context: 'print' })
    const toolbar = screen.getByRole('toolbar', { name: 'Print tools' })
    expect(
      within(toolbar).getByRole('button', { name: 'Printer: Generic FDM 0.4 mm (default)' }),
    ).toBeInTheDocument()
  })

  it('disables the printer button with finish_command_first during a feature command', async () => {
    const { app, user } = renderApp({
      context: 'print',
      command: {
        featureType: 'fillet',
        featureId: 'f-fil9',
        editing: false,
        title: 'Fillet',
        params: {},
        activeField: 'edges',
        fieldErrors: {},
        applied: false,
        newBody: null,
        helper: null,
        changedPaths: [],
        history: [],
        future: [],
        preview: 'none',
      },
    })
    const toolbar = screen.getByRole('toolbar', { name: 'Print tools' })
    const printer = within(toolbar).getByRole('button', { name: /^Printer: / })
    expect(printer).toHaveAttribute('aria-disabled', 'true')
    await user.click(printer)
    expect(screen.queryByTestId('printer-menu')).toBeNull()
    expect(app.store.getState().notifications.map((notification) => notification.message)).toEqual([
      MESSAGES.finish_command_first('Fillet'),
    ])
  })

  it('shows the shown finding count on Print check and opens the print check panel', async () => {
    const report = printReport({
      'b-cube': [
        checkResult('overhangs', [finding('overhangs', 'warning')]),
        checkResult('bed_fit', [finding('bed_fit', 'error')]),
      ],
    })
    const { app, user } = renderApp({
      context: 'print',
      print: { ...INITIAL_STATE.print, report, reportRevision: cubeDocumentState().revision },
    })
    const toolbar = screen.getByRole('toolbar', { name: 'Print tools' })
    const button = within(toolbar).getByRole('button', { name: 'Print check' })
    expect(button).toHaveTextContent('2')
    await user.click(button)
    expect(app.store.getState().panels.side).toBe('health')
    expect(screen.getByTestId('health-list')).toBeInTheDocument()
  })
})
