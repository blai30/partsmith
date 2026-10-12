import { describe, expect, it } from 'vitest'
import {
  bodyItem,
  cubeDocumentState,
  elementItem,
  readyState,
  rectSketchDocumentState,
  sketchLineItem,
} from '../layout/fixtures.testing'
import { createModelApp } from '../layout/test-app.testing'
import { MESSAGES } from '../messages'
import { INITIAL_STATE, type ActiveCommand, type AppState } from '../state/store'
import { CUBE_TOP_FRONT_EDGE, RECT_REGION } from '../test/fakes/documents'
import { viewportMenuItems, type ViewportMenuItem } from './menu'

// The viewport context menu (ui.md 3.13).

function appWith(document = cubeDocumentState(), overrides: Partial<AppState> = {}) {
  return createModelApp({ state: readyState(document, overrides) }).app
}

function labels(items: readonly ViewportMenuItem[]): readonly string[] {
  return items.flatMap((item) => (item.kind === 'command' ? [item.label] : []))
}

const COMPLETE_EXTRUDE: ActiveCommand = {
  featureType: 'extrude',
  featureId: 'f-new1',
  editing: false,
  title: 'Extrude',
  params: {
    profile: { sketch: 'f-sk1', regions: [RECT_REGION.reference] },
    extent: { kind: 'distance', expr: '10 mm' },
    op: 'new',
  },
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

describe('viewportMenuItems (U-122)', () => {
  it('U-122 with one edge selected after a Fillet run: Repeat Fillet, Fillet, Chamfer, Edit Extrude 1, Fit to view', () => {
    const app = appWith(cubeDocumentState(), {
      selection: [elementItem(CUBE_TOP_FRONT_EDGE)],
      lastFeatureType: 'fillet',
    })
    const items = viewportMenuItems(app)
    expect(labels(items)).toEqual([
      'Repeat Fillet',
      'Fillet',
      'Chamfer',
      'Edit Extrude 1',
      'Fit to view',
    ])
    // Fit to view stands apart after a separator.
    expect(items.at(-2)).toEqual({ kind: 'separator' })
    expect(items.find((item) => item.kind === 'command' && item.label === 'Fillet')).toMatchObject({
      command: 'feature.fillet',
      shortcut: 'F',
    })
  })

  it('U-122 with the body "Cube" selected it includes "Hide"', () => {
    const app = appWith(cubeDocumentState(), { selection: [bodyItem('b-cube', 'Cube')] })
    expect(labels(viewportMenuItems(app))).toContain('Hide')
    expect(labels(viewportMenuItems(app))).not.toContain('Repeat')
  })

  it('names the toggle "Show" for a hidden body', () => {
    const base = cubeDocumentState()
    const hidden = {
      ...base,
      recipe: { ...base.recipe, bodies: [{ id: 'b-cube', name: 'Cube', visible: false }] },
    }
    const app = appWith(hidden, { selection: [bodyItem('b-cube', 'Cube')] })
    expect(labels(viewportMenuItems(app))).toContain('Show')
  })

  it('U-122 in sketch mode with one line selected: Horizontal, Vertical, Fix, Construction, Delete, Finish sketch', () => {
    const app = appWith(rectSketchDocumentState(), {
      context: 'sketch',
      sketch: {
        sketchId: 'f-sk1',
        tool: null,
        construction: false,
        showConstraints: true,
        selection: [sketchLineItem('f-sk1', 'e-1')],
        dragging: false,
        cameraBefore: null,
      },
    })
    const items = viewportMenuItems(app)
    expect(labels(items)).toEqual([
      'Horizontal',
      'Vertical',
      'Fix',
      'Construction',
      'Delete',
      'Finish sketch',
    ])
    expect(items.at(-2)).toEqual({ kind: 'separator' })
  })

  it('U-122 with a complete command active: OK and Cancel', () => {
    const app = appWith(rectSketchDocumentState(), { command: COMPLETE_EXTRUDE })
    expect(labels(viewportMenuItems(app))).toEqual(['OK', 'Cancel'])
  })

  it('offers only Cancel while the command is incomplete', () => {
    const app = appWith(rectSketchDocumentState(), {
      command: { ...COMPLETE_EXTRUDE, params: { op: 'new' }, activeField: 'profile' },
    })
    expect(labels(viewportMenuItems(app))).toEqual(['Cancel'])
  })

  it('U-122 edit.repeat before any feature command is disabled with "No command to repeat yet."', () => {
    const app = appWith(cubeDocumentState(), { selection: [elementItem(CUBE_TOP_FRONT_EDGE)] })
    expect(app.store.getState().lastFeatureType).toBe(INITIAL_STATE.lastFeatureType)
    expect(app.commands.get('edit.repeat')?.availability(app)).toEqual({
      state: 'disabled',
      reason: MESSAGES.nothing_to_repeat(),
      code: 'nothing_to_repeat',
    })
    expect(labels(viewportMenuItems(app))[0]).toBe('Fillet')
  })

  it('offers Fit to view alone with nothing selected', () => {
    expect(labels(viewportMenuItems(appWith()))).toEqual(['Fit to view'])
  })
})
