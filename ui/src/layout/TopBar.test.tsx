import { cleanup, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { installPopoverPolyfill } from '../components/popover-polyfill.testing'
import { MESSAGES } from '../messages'
import { DEFAULT_SETTINGS, SETTINGS_KEY } from '../settings/settings'
import type { AppState, SketchSession } from '../state/store'
import { catalogOf, TEST_CATALOG_ENTRIES } from '../test/fakes/catalog'
import { RECT_REGION } from '../test/fakes/documents'
import { settle } from '../test/fakes/settle'
import {
  cubeDocumentState,
  documentState,
  readyState,
  rectSketchDocumentState,
  regionItem,
  THREE_BOXES_DOCUMENT,
} from './fixtures.testing'
import { createModelApp, renderWithApp, stubMatchMedia } from './test-app.testing'

let uninstallPopovers: () => void

beforeEach(() => {
  uninstallPopovers = installPopoverPolyfill()
  stubMatchMedia()
})

afterEach(() => {
  cleanup()
  uninstallPopovers()
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
}

function renderApp(state: Partial<AppState> = {}, document = cubeDocumentState()) {
  const test = createModelApp({ state: readyState(document, state) })
  renderWithApp(test.app)
  return { ...test, user: userEvent.setup(), topBar: screen.getByTestId('top-bar') }
}

/** A Storage kept in a map, for checking what the app saves. */
class MemoryStorage implements Storage {
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
      .getAllByRole(/^menuitem/)
      .map((item) => item.textContent?.trim() ?? '')
    const expected = ['New', 'Open...', 'Open recent', 'Save', 'Save as...', 'Export...', 'Theme', 'Navigation', 'UI scale']
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

  it('shows the Model tools in toolbar order, with Primitive and Pattern groups', () => {
    renderApp()
    const toolbar = screen.getByRole('toolbar', { name: 'Model tools' })
    expect(within(toolbar).getAllByRole('button').map((button) => button.getAttribute('aria-label') ?? button.textContent)).toEqual([
      'Sketch',
      'Extrude',
      'Revolve',
      'Hole',
      'Fillet',
      'Chamfer',
      'Shell',
      'Primitive',
      'Pattern',
      'Mirror',
      'Boolean',
    ])
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
    await user.click(screen.getByRole('button', { name: 'Primitive' }))
    const menu = screen.getByRole('menu')
    expect(within(menu).getAllByRole('menuitem').map((item) => item.textContent?.trim())).toEqual([
      expect.stringContaining('Box'),
      expect.stringContaining('Cylinder'),
      expect.stringContaining('Sphere'),
      expect.stringContaining('Cone'),
      expect.stringContaining('Torus'),
    ])
  })

  it('marks disabled toolbar commands with aria-disabled', () => {
    renderApp({ core: { status: 'failed', engineVersion: null, fatal: 'HTTP 404', pendingMutations: 0 } })
    expect(screen.getByRole('button', { name: 'Extrude' })).toHaveAttribute('aria-disabled', 'true')
  })

  it('presses the active sketch tool', () => {
    renderApp({ context: 'sketch', sketch: { ...EDITING_SKETCH, tool: 'line' } })
    const toolbar = screen.getByRole('toolbar', { name: 'Sketch tools' })
    expect(within(toolbar).getByRole('button', { name: 'Line' })).toHaveAttribute('aria-pressed', 'true')
    expect(within(toolbar).getByRole('button', { name: 'Circle' })).toHaveAttribute('aria-pressed', 'false')
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
    await user.click(screen.getByRole('button', { name: 'Extrude' }))
    await waitFor(() => {
      expect(core.callsTo('setFocus')).toHaveLength(1)
    })
    expect(core.callsTo('beginTransaction')).toEqual([[1, 'Extrude']])
    const applied = core.callsTo('applyEdits')
    expect(applied).toHaveLength(1)
    const [documentHandle, label, edits] = applied[0] ?? []
    expect(documentHandle).toBe(1)
    expect(label).toBeNull()
    const [addBody, addFeature, ...rest] = edits as readonly Record<string, unknown>[]
    expect(rest).toEqual([])
    expect(addBody).toEqual({
      type: 'add_body',
      index: 0,
      body: { id: expect.stringMatching(/^b-[0-9a-f]{12}$/), name: 'Body 1' },
    })
    const bodyId = (addBody?.body as { id: string }).id
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
    const featureId = (addFeature?.feature as { id: string }).id
    expect(core.callsTo('setFocus')).toEqual([[1, featureId]])
    expect(Object.keys(addFeature?.feature as object)).not.toContain('name')
  })

  it('U-65 with the marker before the second of 3 features, the first preview inserts at index 1 with no set_rollback', async () => {
    const { core, user } = renderApp(
      {},
      documentState({ ...THREE_BOXES_DOCUMENT, rollback: 'f-box2' }),
    )
    await user.click(screen.getByRole('button', { name: 'Primitive' }))
    await user.click(screen.getByRole('menuitem', { name: /Box/ }))
    await waitFor(() => {
      expect(core.callsTo('applyEdits')).toHaveLength(1)
    })
    const [, , edits] = core.callsTo('applyEdits')[0] ?? []
    const addFeature = (edits as readonly Record<string, unknown>[]).find((edit) => edit.type === 'add_feature')
    expect(addFeature?.index).toBe(1)
    for (const [, , batch] of core.callsTo('applyEdits')) {
      expect((batch as readonly Record<string, unknown>[]).some((edit) => edit.type === 'set_rollback')).toBe(false)
    }
  })

  it('shows finish_command_first when Export is chosen during a feature command (U-89)', async () => {
    const { app, user, topBar } = renderApp({
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
        preview: 'none',
      },
    })
    const exportButton = within(topBar).getByRole('button', { name: 'Export' })
    expect(exportButton).toHaveAttribute('aria-disabled', 'true')
    await user.click(exportButton)
    expect(screen.queryByTestId('export-popover')).toBeNull()
    expect(app.store.getState().notifications.map((notification) => notification.message)).toContain(
      MESSAGES.finish_command_first('Fillet'),
    )
  })
})
