import { afterEach, describe, expect, it, vi } from 'vitest'
import type { Platform } from '../app/createApp'
import {
  bodyItem,
  cubeDocumentState,
  CYLINDRICAL_FACE,
  documentState,
  elementItem,
  featureItem,
  IMPORT_DOCUMENT,
  originItem,
  readyState,
} from '../layout/fixtures.testing'
import { createFakeViewport, createModelApp, stubMatchMedia } from '../layout/test-app.testing'
import { MESSAGES } from '../messages'
import { DEFAULT_SETTINGS } from '../settings/settings'
import type { ActiveCommand, AppContext, AppState } from '../state/store'
import { LINUX_WEB, MAC_WEB } from '../test/fakes/app'
import { TEST_CATALOG } from '../test/fakes/catalog'
import { CUBE_TOP_FACE } from '../test/fakes/documents'
import { settle } from '../test/fakes/settle'
import { builtinCommands } from './builtin'
import { featureCommands } from './features'
import { createCommandRegistry, type CommandDefinition } from './registry'

afterEach(() => {
  vi.unstubAllGlobals()
})

const WINDOWS_WEB: Platform = { operatingSystem: 'windows', host: 'web' }
const ALL: readonly AppContext[] = ['model', 'sketch', 'print']
const MODEL_PRINT: readonly AppContext[] = ['model', 'print']
const SKETCH: readonly AppContext[] = ['sketch']
const PRINT: readonly AppContext[] = ['print']

function commandOf(id: string, platform: Platform = LINUX_WEB): CommandDefinition {
  const command = builtinCommands(platform).find((candidate) => candidate.id === id)
  if (command === undefined) {
    throw new Error(`No built-in command ${id}`)
  }
  return command
}

// Registry shortcuts of ui.md 3.5 that builtin.ts owns: id, title, shortcuts, contexts.
const SHORTCUT_TABLE: readonly (readonly [string, string, readonly string[], readonly AppContext[]])[] = [
  ['app.palette', 'Command palette', ['Mod+K', 'S'], ALL],
  ['edit.undo', 'Undo', ['Mod+Z'], ALL],
  ['edit.redo', 'Redo', ['Mod+Shift+Z', 'Mod+Y'], ALL],
  ['file.open', 'Open', ['Mod+O'], ALL],
  ['file.save', 'Save', ['Mod+S'], ALL],
  ['file.save_as', 'Save as', ['Mod+Shift+S'], ALL],
  ['file.export', 'Export', ['Mod+Shift+E'], ALL],
  ['edit.delete', 'Delete', ['Delete', 'Backspace'], ALL],
  ['edit.rename', 'Rename', ['F2'], ALL],
  ['view.browser', 'Toggle browser', ['Mod+B'], ALL],
  ['view.fit', 'Fit to view', ['F6', 'Shift+F'], ALL],
  ['view.home', 'Home view', ['Shift+H'], ALL],
  ['view.look_at', 'Look at', ['Shift+L'], ALL],
  ['view.projection', 'Orthographic', ['Shift+P'], ALL],
  ['view.section', 'Section view', ['Shift+X'], ALL],
  ['view.visibility', 'Show or hide selection', ['V'], MODEL_PRINT],
  ['sketch.line', 'Line', ['L'], SKETCH],
  ['sketch.rectangle_2point', '2-point rectangle', ['R'], SKETCH],
  ['sketch.circle', 'Circle', ['C'], SKETCH],
  ['sketch.dimension', 'Dimension', ['D'], SKETCH],
  ['sketch.construction', 'Construction', ['X'], SKETCH],
  ['sketch.constrain_coincident', 'Coincident', ['I'], SKETCH],
  ['sketch.constrain_horizontal', 'Horizontal', ['H'], SKETCH],
  ['sketch.constrain_vertical', 'Vertical', ['V'], SKETCH],
  ['sketch.select_all', 'Select all', ['Mod+A'], SKETCH],
  ['sketch.finish', 'Finish sketch', ['Mod+Enter'], SKETCH],
]

// Commands without a default shortcut (3.5): id, title, contexts.
const UNBOUND_TABLE: readonly (readonly [string, string, readonly AppContext[]])[] = [
  ['context.model', 'Model', ALL],
  ['context.sketch', 'Sketch', ALL],
  ['context.print', 'Print', ALL],
  ['edit.edit_feature', 'Edit feature', MODEL_PRINT],
  ['edit.suppress', 'Suppress', MODEL_PRINT],
  ['edit.unsuppress', 'Unsuppress', MODEL_PRINT],
  ['edit.clear_selection', 'Clear selection', ALL],
  ['view.parameters', 'Parameters', ALL],
  ['view.problems', 'Problems', ALL],
  ['view.health', 'Print health', ALL],
  ['view.timeline', 'Toggle timeline', ALL],
  ['view.standard.top', 'View: Top', ALL],
  ['view.standard.bottom', 'View: Bottom', ALL],
  ['view.standard.front', 'View: Front', ALL],
  ['view.standard.back', 'View: Back', ALL],
  ['view.standard.left', 'View: Left', ALL],
  ['view.standard.right', 'View: Right', ALL],
  ['view.standard.iso', 'View: Home', ALL],
  ['view.filter.body', 'Select bodies', MODEL_PRINT],
  ['view.filter.face', 'Select faces', MODEL_PRINT],
  ['view.filter.edge', 'Select edges', MODEL_PRINT],
  ['view.filter.vertex', 'Select vertices', MODEL_PRINT],
  ['view.lock', 'Lock selection', MODEL_PRINT],
  ['view.unlock', 'Unlock selection', MODEL_PRINT],
  ['sketch.rectangle_center', 'Center rectangle', SKETCH],
  ['sketch.arc_3point', 'Arc', SKETCH],
  ['sketch.point', 'Point', SKETCH],
  ['sketch.constrain_point_on_object', 'Point on object', SKETCH],
  ['sketch.constrain_parallel', 'Parallel', SKETCH],
  ['sketch.constrain_perpendicular', 'Perpendicular', SKETCH],
  ['sketch.constrain_tangent', 'Tangent', SKETCH],
  ['sketch.constrain_collinear', 'Collinear', SKETCH],
  ['sketch.constrain_concentric', 'Concentric', SKETCH],
  ['sketch.constrain_equal', 'Equal', SKETCH],
  ['sketch.constrain_symmetric', 'Symmetric', SKETCH],
  ['sketch.constrain_midpoint', 'Midpoint', SKETCH],
  ['sketch.constrain_fix', 'Fix', SKETCH],
  ['sketch.show_constraints', 'Show constraints', SKETCH],
  ['print.printer', 'Choose printer', PRINT],
  ['print.overhangs', 'Overhangs', PRINT],
  ['print.checks', 'Checks', PRINT],
  ['print.check_now', 'Check print', ALL],
  ['print.export_3mf', 'Export 3MF', ALL],
  ['print.export_stl', 'Export STL', ALL],
  ['print.export_obj', 'Export OBJ', ALL],
  ['print.export_step', 'Export STEP', ALL],
  ['document.units.mm', 'Units: mm', ALL],
  ['document.units.cm', 'Units: cm', ALL],
  ['document.units.m', 'Units: m', ALL],
  ['document.units.in', 'Units: in', ALL],
  ['settings.theme.system', 'Theme: System', ALL],
  ['settings.theme.light', 'Theme: Light', ALL],
  ['settings.theme.dark', 'Theme: Dark', ALL],
  ['settings.theme.toggle', 'Toggle dark theme', ALL],
  ['settings.navigation.fusion', 'Navigation: Fusion', ALL],
  ['settings.navigation.freecad', 'Navigation: FreeCAD', ALL],
  ['settings.scale.100', 'UI scale: 100%', ALL],
  ['settings.scale.125', 'UI scale: 125%', ALL],
  ['settings.scale.150', 'UI scale: 150%', ALL],
  ['settings.scale.200', 'UI scale: 200%', ALL],
  ['settings.scale.300', 'UI scale: 300%', ALL],
]

const EXTRUDE_COMMAND: ActiveCommand = {
  featureType: 'extrude',
  featureId: 'f-new1',
  editing: false,
  title: 'Extrude',
  params: { extent: { kind: 'distance', expr: '10 mm' }, op: 'join', body: 'b-cube' },
  activeField: 'profile',
  fieldErrors: {},
  applied: false,
  newBody: null,
  preview: 'none',
}

function cubeApp(overrides: Partial<AppState> = {}) {
  return createModelApp({ state: readyState(cubeDocumentState(), overrides) })
}

describe('the built-in commands with the test catalog', () => {
  const platforms: readonly Platform[] = [
    MAC_WEB,
    WINDOWS_WEB,
    LINUX_WEB,
    { operatingSystem: 'mac', host: 'desktop' },
    { operatingSystem: 'windows', host: 'desktop' },
    { operatingSystem: 'linux', host: 'desktop' },
  ]

  it.each(platforms)('U-22 have no shortcut conflicts on $operatingSystem $host', (platform) => {
    const registry = createCommandRegistry()
    for (const command of [...builtinCommands(platform), ...featureCommands(TEST_CATALOG)]) {
      registry.register(command)
    }
    expect(registry.conflicts(platform)).toEqual([])
  })

  it('have unique ids, so they register together', () => {
    const registry = createCommandRegistry()
    expect(() => {
      for (const command of [...builtinCommands(LINUX_WEB), ...featureCommands(TEST_CATALOG)]) {
        registry.register(command)
      }
    }).not.toThrow()
  })
})

describe('the shortcut table of 3.5', () => {
  it.each(SHORTCUT_TABLE)('%s is "%s" with %j in %j', (id, title, shortcuts, contexts) => {
    const command = commandOf(id)
    expect(command.title).toBe(title)
    expect(command.shortcuts).toEqual(shortcuts)
    expect([...command.contexts].toSorted()).toEqual([...contexts].toSorted())
  })

  it('binds New to Mod+N on the desktop only, where browsers do not reserve it', () => {
    expect(commandOf('file.new', { operatingSystem: 'linux', host: 'desktop' }).shortcuts).toEqual([
      'Mod+N',
    ])
    expect(commandOf('file.new', LINUX_WEB).shortcuts).toEqual([])
    expect(commandOf('file.new', LINUX_WEB).title).toBe('New')
  })

  it.each(UNBOUND_TABLE)('%s is "%s" in %j with no shortcut', (id, title, contexts) => {
    const command = commandOf(id)
    expect(command.title).toBe(title)
    expect(command.shortcuts).toEqual([])
    expect([...command.contexts].toSorted()).toEqual([...contexts].toSorted())
  })

  it('puts the theme, navigation and scale commands in the Settings category', () => {
    for (const [id] of UNBOUND_TABLE.filter(([candidate]) => candidate.startsWith('settings.'))) {
      expect(commandOf(id).category).toBe('Settings')
    }
  })
})

describe('availability', () => {
  it('U-89 disables every export command with finish_command_first while a feature command is active', () => {
    const { app } = cubeApp({ command: EXTRUDE_COMMAND })
    for (const id of [
      'file.export',
      'print.export_3mf',
      'print.export_stl',
      'print.export_obj',
      'print.export_step',
    ]) {
      expect(app.commands.get(id)?.availability(app)).toEqual({
        state: 'disabled',
        reason: MESSAGES.finish_command_first('Extrude'),
      })
    }
  })

  it('U-89 disables Edit feature with import_not_editable for an import feature', () => {
    const { app } = createModelApp({
      state: readyState(documentState(IMPORT_DOCUMENT), {
        selection: [featureItem('f-imp1', 'Import 1')],
      }),
    })
    expect(app.commands.get('edit.edit_feature')?.availability(app)).toEqual({
      state: 'disabled',
      reason: MESSAGES.import_not_editable(),
    })
  })

  it('enables Edit feature with one feature selected', () => {
    const { app } = cubeApp({ selection: [featureItem('f-ext1', 'Extrude 1')] })
    expect(app.commands.get('edit.edit_feature')?.availability(app)).toEqual({ state: 'enabled' })
  })

  it('disables Undo with "Nothing to undo." and enables it with the undo label as detail', () => {
    const { app } = cubeApp()
    const undo = app.commands.get('edit.undo')
    expect(undo?.availability(app)).toEqual({ state: 'disabled', reason: MESSAGES.nothing_to_undo() })
    const document = app.store.getState().document
    app.store.setState({ document: document && { ...document, undoLabel: 'Extrude' } })
    expect(undo?.availability(app)).toEqual({ state: 'enabled' })
    expect(undo?.detail?.(app)).toBe('Undo Extrude')
  })

  it('disables Redo with "Nothing to redo." and shows the redo label as detail', () => {
    const { app } = cubeApp()
    const redo = app.commands.get('edit.redo')
    expect(redo?.availability(app)).toEqual({ state: 'disabled', reason: MESSAGES.nothing_to_redo() })
    const document = app.store.getState().document
    app.store.setState({ document: document && { ...document, redoLabel: 'Fillet' } })
    expect(redo?.availability(app)).toEqual({ state: 'enabled' })
    expect(redo?.detail?.(app)).toBe('Redo Fillet')
  })

  it('disables Redo with "Finish or cancel {title} first." during a feature command (3.20)', () => {
    const { app } = cubeApp({ command: EXTRUDE_COMMAND })
    const document = app.store.getState().document
    app.store.setState({ document: document && { ...document, redoLabel: 'Fillet' } })
    expect(app.commands.get('edit.redo')?.availability(app)).toEqual({
      state: 'disabled',
      reason: MESSAGES.finish_command_first('Extrude'),
    })
  })

  it('disables Look at without a flat face or plane selected', () => {
    const { app } = cubeApp()
    expect(app.commands.get('view.look_at')?.availability(app)).toEqual({
      state: 'disabled',
      reason: MESSAGES.look_at_disabled(),
    })
    app.store.setState({ selection: [elementItem(CYLINDRICAL_FACE, 'Peg')] })
    expect(app.commands.get('view.look_at')?.availability(app).state).toBe('disabled')
  })

  it('enables Look at with a flat face or an origin plane selected', () => {
    const { app } = cubeApp({ selection: [elementItem(CUBE_TOP_FACE)] })
    expect(app.commands.get('view.look_at')?.availability(app)).toEqual({ state: 'enabled' })
    app.store.setState({ selection: [originItem('XZ')] })
    expect(app.commands.get('view.look_at')?.availability(app)).toEqual({ state: 'enabled' })
  })

  it('disables core-dependent commands with core_not_running after a core failure (4.4)', () => {
    const { app } = cubeApp({
      core: { status: 'failed', engineVersion: null, fatal: 'HTTP 404', pendingMutations: 0 },
    })
    for (const id of ['file.save', 'file.open', 'feature.extrude', 'print.check_now']) {
      expect(app.commands.get(id)?.availability(app)).toEqual({
        state: 'disabled',
        reason: MESSAGES.core_not_running(),
      })
    }
    expect(app.commands.get('app.palette')?.availability(app)).toEqual({ state: 'enabled' })
  })
})

describe('running built-in commands', () => {
  it('U-66 Undo during a feature command cancels it and calls no undo', async () => {
    const { app, core } = cubeApp({ command: EXTRUDE_COMMAND })
    const document = app.store.getState().document
    app.store.setState({ document: document && { ...document, undoLabel: 'Fillet' } })
    await app.commands.run(app, 'edit.undo', 'shortcut')
    await settle()
    expect(core.callsTo('cancelTransaction')).toEqual([[1]])
    expect(core.callsTo('undo')).toEqual([])
    expect(app.store.getState().command).toBeNull()
  })

  it('Undo calls the core undo through the mutation queue', async () => {
    const { app, core } = cubeApp()
    const document = app.store.getState().document
    app.store.setState({ document: document && { ...document, undoLabel: 'Extrude' } })
    await app.commands.run(app, 'edit.undo', 'shortcut')
    await settle()
    expect(core.callsTo('undo')).toEqual([[1]])
  })

  it('switches context with context.print and context.model', async () => {
    const { app } = cubeApp()
    await app.commands.run(app, 'context.print', 'palette')
    expect(app.store.getState().context).toBe('print')
    await app.commands.run(app, 'context.model', 'palette')
    expect(app.store.getState().context).toBe('model')
  })

  it('toggles the browser and the timeline', async () => {
    const { app } = cubeApp()
    const browserOpen = app.store.getState().panels.browserOpen
    await app.commands.run(app, 'view.browser', 'shortcut')
    expect(app.store.getState().panels.browserOpen).toBe(!browserOpen)
    await app.commands.run(app, 'view.timeline', 'palette')
    expect(app.store.getState().panels.timelineCollapsed).toBe(true)
  })

  it('opens the Parameters, Problems and Print health side panels', async () => {
    const { app } = cubeApp()
    await app.commands.run(app, 'view.parameters', 'palette')
    expect(app.store.getState().panels.side).toBe('parameters')
    await app.commands.run(app, 'view.problems', 'palette')
    expect(app.store.getState().panels.side).toBe('problems')
    await app.commands.run(app, 'view.health', 'palette')
    expect(app.store.getState().panels.side).toBe('health')
  })

  it('drives the mounted viewport: fit, standard views, home and projection', async () => {
    const { app } = cubeApp()
    const viewport = createFakeViewport()
    app.attachViewport(viewport)
    await app.commands.run(app, 'view.fit', 'shortcut')
    expect(viewport.fit).toHaveBeenCalledTimes(1)
    await app.commands.run(app, 'view.standard.front', 'palette')
    expect(viewport.setStandardView).toHaveBeenLastCalledWith('front')
    await app.commands.run(app, 'view.standard.iso', 'palette')
    expect(viewport.setStandardView).toHaveBeenLastCalledWith('iso')
    await app.commands.run(app, 'view.projection', 'shortcut')
    expect(viewport.toggleProjection).toHaveBeenCalledTimes(1)
  })

  it('Home view sets the iso view and fits the visible bounds', async () => {
    const { app } = cubeApp()
    const viewport = createFakeViewport()
    app.attachViewport(viewport)
    await app.commands.run(app, 'view.home', 'shortcut')
    expect(viewport.setStandardView).toHaveBeenCalledWith('iso')
    expect(viewport.fit).toHaveBeenCalled()
  })

  it('shows the projection toggle as checked in orthographic', () => {
    const { app } = cubeApp()
    const projection = app.commands.get('view.projection')
    expect(projection?.checked?.(app)).toBe(false)
    app.store.setState({ view: { ...app.store.getState().view, projection: 'orthographic' } })
    expect(projection?.checked?.(app)).toBe(true)
  })

  it('toggles a selection filter kind and shows it as checked', async () => {
    const { app } = cubeApp()
    expect(app.commands.get('view.filter.edge')?.checked?.(app)).toBe(true)
    await app.commands.run(app, 'view.filter.edge', 'palette')
    expect(app.store.getState().view.filter.edge).toBe(false)
    expect(app.commands.get('view.filter.edge')?.checked?.(app)).toBe(false)
    expect(app.commands.get('view.filter.body')?.checked?.(app)).toBe(false)
  })

  it('clears the selection', async () => {
    const { app } = cubeApp({ selection: [bodyItem('b-cube', 'Cube')] })
    await app.commands.run(app, 'edit.clear_selection', 'palette')
    expect(app.store.getState().selection).toEqual([])
  })

  it('locks and unlocks the selected bodies for the session', async () => {
    const { app, core } = cubeApp({ selection: [bodyItem('b-cube', 'Cube')] })
    await app.commands.run(app, 'view.lock', 'palette')
    expect(app.store.getState().view.locked).toEqual(['b-cube'])
    await app.commands.run(app, 'view.unlock', 'palette')
    expect(app.store.getState().view.locked).toEqual([])
    expect(core.callsTo('applyEdits')).toEqual([])
  })

  it('suppresses the selected feature in one edit labeled "Suppress {label}"', async () => {
    const { app, core } = cubeApp({ selection: [featureItem('f-ext1', 'Extrude 1')] })
    await app.commands.run(app, 'edit.suppress', 'palette')
    await settle()
    expect(core.callsTo('applyEdits')).toHaveLength(1)
    const [documentHandle, label, edits] = core.callsTo('applyEdits')[0] ?? []
    expect(documentHandle).toBe(1)
    expect(label).toBe('Suppress Extrude 1')
    expect(edits).toEqual([
      {
        type: 'update_feature',
        feature: expect.objectContaining({ id: 'f-ext1', type: 'extrude', suppressed: true }),
      },
    ])
  })

  it('changes units with "Change units to {unit}" and the units_changed notification', async () => {
    const { app, core } = cubeApp()
    await app.commands.run(app, 'document.units.in', 'palette')
    await settle()
    expect(core.callsTo('applyEdits')).toEqual([
      [1, 'Change units to in', [{ type: 'set_units', units: 'in' }]],
    ])
    expect(app.store.getState().notifications.map((notification) => notification.message)).toContain(
      MESSAGES.units_changed('in'),
    )
  })

  it('chooses a theme, a navigation preset and a UI scale, each shown as checked', async () => {
    const { app } = cubeApp()
    await app.commands.run(app, 'settings.theme.dark', 'palette')
    expect(app.store.getState().settings.theme).toBe('dark')
    expect(app.commands.get('settings.theme.dark')?.checked?.(app)).toBe(true)
    expect(app.commands.get('settings.theme.light')?.checked?.(app)).toBe(false)
    await app.commands.run(app, 'settings.navigation.freecad', 'palette')
    expect(app.store.getState().settings.navigation).toBe('freecad')
    expect(app.commands.get('settings.navigation.freecad')?.checked?.(app)).toBe(true)
    await app.commands.run(app, 'settings.scale.150', 'palette')
    expect(app.store.getState().settings.uiScale).toBe(1.5)
    expect(app.commands.get('settings.scale.150')?.checked?.(app)).toBe(true)
  })

  it('checks "Toggle dark theme" while the resolved theme is dark, following the OS for System', () => {
    stubMatchMedia({ dark: true })
    const { app } = cubeApp({ settings: { ...DEFAULT_SETTINGS, theme: 'system' } })
    const toggle = app.commands.get('settings.theme.toggle')
    expect(toggle?.checked?.(app)).toBe(true)
    app.store.setState({ settings: { ...DEFAULT_SETTINGS, theme: 'light' } })
    expect(toggle?.checked?.(app)).toBe(false)
  })

  it('"Toggle dark theme" switches to the opposite of the resolved theme', async () => {
    stubMatchMedia({ dark: false })
    const { app } = cubeApp({ settings: { ...DEFAULT_SETTINGS, theme: 'system' } })
    await app.commands.run(app, 'settings.theme.toggle', 'palette')
    expect(app.store.getState().settings.theme).toBe('dark')
    await app.commands.run(app, 'settings.theme.toggle', 'palette')
    expect(app.store.getState().settings.theme).toBe('light')
  })
})
