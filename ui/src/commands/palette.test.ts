import { describe, expect, it } from 'vitest'
import type { RecentFile } from '../files/recent'
import { CUBE_BOUNDS, cubeDocumentState, documentState, readyState } from '../layout/fixtures.testing'
import { createFakeViewport, createModelApp } from '../layout/test-app.testing'
import { MESSAGES } from '../messages'
import type { SketchSession } from '../state/store'
import { PARAMETRIC_DOCUMENT } from '../test/fakes/documents'
import type { PaletteItem } from './fuzzy'
import { paletteItems } from './palette'

const EDITING_SKETCH: SketchSession = {
  sketchId: 'f-sk1',
  tool: null,
  construction: false,
  showConstraints: true,
  selection: [],
  dragging: false,
}

function find(items: readonly PaletteItem[], kind: PaletteItem['kind'], title: string): PaletteItem {
  const item = items.find((candidate) => candidate.kind === kind && candidate.title === title)
  if (item === undefined) {
    throw new Error(`No ${kind} item "${title}" among ${items.map((candidate) => candidate.title).join(', ')}`)
  }
  return item
}

function titles(items: readonly PaletteItem[]): string[] {
  return items.map((item) => item.title)
}

describe('paletteItems', () => {
  it('U-28 in Model context with cube.ptsm contains the feature "Extrude 1"', () => {
    const { app } = createModelApp({ state: readyState(cubeDocumentState()) })
    const item = find(paletteItems(app, []), 'feature', 'Extrude 1')
    expect(item.category).toBeNull()
  })

  it('U-28 in Model context with cube.ptsm contains the body "Cube"', () => {
    const { app } = createModelApp({ state: readyState(cubeDocumentState()) })
    expect(find(paletteItems(app, []), 'body', 'Cube').category).toBeNull()
  })

  it('U-28 contains the setting "Theme: Dark", not checked under the light theme', () => {
    const { app } = createModelApp({ state: readyState(cubeDocumentState()) })
    const item = find(paletteItems(app, []), 'setting', 'Theme: Dark')
    expect(item.category).toBe('Settings')
    expect(app.commands.get(item.id)?.checked?.(app)).toBe(false)
  })

  it('U-28 lists no sketch tools in Model context', () => {
    const { app } = createModelApp({ state: readyState(cubeDocumentState()) })
    const items = paletteItems(app, [])
    expect(items.some((item) => item.kind === 'command' && item.title === 'Line')).toBe(false)
    expect(items.some((item) => item.id === 'sketch.circle')).toBe(false)
  })

  it('U-28 in sketch mode contains "Line"', () => {
    const { app } = createModelApp({
      state: readyState(cubeDocumentState(), { context: 'sketch', sketch: EDITING_SKETCH }),
    })
    expect(find(paletteItems(app, []), 'command', 'Line').id).toBe('sketch.line')
  })

  it('lists no sketch tools while a plane is being picked for a new sketch', () => {
    const { app } = createModelApp({
      state: readyState(cubeDocumentState(), {
        context: 'sketch',
        sketch: { ...EDITING_SKETCH, sketchId: null },
      }),
    })
    expect(titles(paletteItems(app, []))).not.toContain('Line')
  })

  it('gives command items the command id, its category and the reason a disabled command shows', () => {
    const { app } = createModelApp({ state: readyState(cubeDocumentState()) })
    const items = paletteItems(app, [])
    const extrude = find(items, 'command', 'Extrude')
    expect(extrude.id).toBe('feature.extrude')
    expect(extrude.category).toBe('Create')
    expect(extrude.keywords).toContain('pad')
    expect(extrude.disabledReason).toBeNull()
    const undo = find(items, 'command', 'Undo')
    expect(undo.disabledReason).toBe(MESSAGES.nothing_to_undo())
  })

  it('shows the undo label as the detail of Undo', () => {
    const { app } = createModelApp({
      state: readyState(cubeDocumentState({ undoLabel: 'Extrude' })),
    })
    expect(find(paletteItems(app, []), 'command', 'Undo').detail).toBe('Undo Extrude')
  })

  it('lists features by label, including the sketch', () => {
    const { app } = createModelApp({ state: readyState(cubeDocumentState()) })
    const features = paletteItems(app, []).filter((item) => item.kind === 'feature')
    expect(titles(features).toSorted()).toEqual(['Extrude 1', 'Sketch 1'])
  })

  it('lists each parameter as "{name} = {expr}"', () => {
    const { app } = createModelApp({ state: readyState(documentState(PARAMETRIC_DOCUMENT)) })
    expect(find(paletteItems(app, []), 'parameter', 'size = 20 mm').category).toBeNull()
  })

  it('lists recent files as "Open recent: {name}"', () => {
    const { app } = createModelApp({ state: readyState(cubeDocumentState()) })
    const recent: RecentFile = {
      id: 'r-1',
      name: 'bracket.ptsm',
      openedAt: 0,
      handle: null,
      text: '{}',
    }
    expect(find(paletteItems(app, [recent]), 'recent', 'Open recent: bracket.ptsm').category).toBeNull()
  })

  it('running a feature item selects that feature', () => {
    const { app } = createModelApp({ state: readyState(cubeDocumentState()) })
    find(paletteItems(app, []), 'feature', 'Extrude 1').run()
    expect(app.store.getState().selection.map((item) => item.key)).toEqual(['feature:f-ext1'])
  })

  it('running a body item selects the body and frames it', () => {
    const { app } = createModelApp({ state: readyState(cubeDocumentState()) })
    const viewport = createFakeViewport()
    app.attachViewport(viewport)
    find(paletteItems(app, []), 'body', 'Cube').run()
    expect(app.store.getState().selection.map((item) => item.key)).toEqual(['body:b-cube'])
    expect(viewport.fit).toHaveBeenCalledWith(CUBE_BOUNDS)
  })

  it('running a parameter item opens the Parameters panel', () => {
    const { app } = createModelApp({ state: readyState(documentState(PARAMETRIC_DOCUMENT)) })
    find(paletteItems(app, []), 'parameter', 'size = 20 mm').run()
    expect(app.store.getState().panels.side).toBe('parameters')
  })

  it('lists no document items without a document', () => {
    const { app } = createModelApp({ state: readyState(null) })
    const kinds = new Set(paletteItems(app, []).map((item) => item.kind))
    expect(kinds.has('feature')).toBe(false)
    expect(kinds.has('body')).toBe(false)
    expect(kinds.has('parameter')).toBe(false)
  })
})
