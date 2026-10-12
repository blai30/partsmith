import { act, cleanup, fireEvent, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { installPopoverPolyfill } from '../components/popover-polyfill.testing'
import type { ActiveCommand, AppState, SketchSession } from '../state/store'
import { RECT_REGION } from '../test/fakes/documents'
import { settle } from '../test/fakes/settle'
import {
  bodyItem,
  cubeDocumentState,
  readyState,
  rectSketchDocumentState,
  sketchLineItem,
} from './fixtures.testing'
import { commandComplete, escapeStep } from './ladders'
import { createModelApp, deferred, renderWithApp, stubMatchMedia } from './test-app.testing'

let uninstallPopovers: () => void

beforeEach(() => {
  uninstallPopovers = installPopoverPolyfill()
  stubMatchMedia()
})

afterEach(() => {
  cleanup()
  uninstallPopovers()
  vi.unstubAllGlobals()
})

/** A new extrude whose required profile is set, so it can be confirmed. */
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
  preview: 'shown',
}

const INCOMPLETE_EXTRUDE: ActiveCommand = {
  ...COMPLETE_EXTRUDE,
  params: { extent: { kind: 'distance', expr: '10 mm' }, op: 'new' },
  activeField: 'profile',
  applied: false,
  preview: 'none',
}

const EDITING_SKETCH: SketchSession = {
  sketchId: 'f-sk1',
  tool: null,
  construction: false,
  showConstraints: true,
  selection: [],
  dragging: false,
}

function renderApp(state: Partial<AppState>) {
  const test = createModelApp({ state: readyState(rectSketchDocumentState(), state) })
  renderWithApp(test.app)
  return { ...test, user: userEvent.setup() }
}

function viewportSurface(): HTMLElement {
  return within(screen.getByTestId('viewport')).getByRole('application')
}

describe('commandComplete', () => {
  it('is false without a command and true once every required field is set', () => {
    const { app } = createModelApp({ state: readyState(rectSketchDocumentState()) })
    expect(commandComplete(app.store.getState())).toBe(false)
    app.store.setState({ command: INCOMPLETE_EXTRUDE })
    expect(commandComplete(app.store.getState())).toBe(false)
    app.store.setState({ command: COMPLETE_EXTRUDE })
    expect(commandComplete(app.store.getState())).toBe(true)
  })

  it('is false while a field has an error', () => {
    const { app } = createModelApp({
      state: readyState(rectSketchDocumentState(), {
        command: { ...COMPLETE_EXTRUDE, fieldErrors: { 'extent.expr': 'Distance needs a value.' } },
      }),
    })
    expect(commandComplete(app.store.getState())).toBe(false)
  })
})

describe('the Enter ladder (3.5)', () => {
  it('confirms a complete command with Enter on the focused viewport', async () => {
    const { core } = renderApp({ command: COMPLETE_EXTRUDE })
    viewportSurface().focus()
    fireEvent.keyDown(viewportSurface(), { key: 'Enter' })
    await waitFor(() => {
      expect(core.callsTo('commitTransaction')).toEqual([[1]])
    })
  })

  it('does nothing while the command is incomplete', async () => {
    const { core } = renderApp({ command: INCOMPLETE_EXTRUDE })
    fireEvent.keyDown(viewportSurface(), { key: 'Enter' })
    await settle()
    expect(core.callsTo('commitTransaction')).toEqual([])
  })

  it('leaves Mod+Enter to the shortcuts', async () => {
    const { core } = renderApp({ command: COMPLETE_EXTRUDE })
    fireEvent.keyDown(viewportSurface(), { key: 'Enter', ctrlKey: true })
    await settle()
    expect(core.callsTo('commitTransaction')).toEqual([])
  })

  it('confirms with Enter pressed in an inspector field before its text is evaluated', async () => {
    const result = deferred<{ value: number; dimension: { length: number; angle: number } }>()
    const { core, user } = renderApp({ command: COMPLETE_EXTRUDE })
    await settle()
    core.stub('evaluate', () => result.promise)
    const distance = within(screen.getByTestId('inspector')).getByRole('textbox', {
      name: 'Distance',
    })
    await user.clear(distance)
    await user.type(distance, '25{Enter}')
    await settle()
    expect(core.callsTo('commitTransaction')).toEqual([])
    await act(async () => {
      result.resolve({ value: 25, dimension: { length: 0, angle: 0 } })
      await settle()
    })
    await waitFor(() => {
      expect(core.callsTo('commitTransaction')).toEqual([[1]])
    })
  })

  it('lets a focused button in the inspector take its own Enter', async () => {
    const { core } = renderApp({ command: COMPLETE_EXTRUDE })
    const cancel = within(screen.getByTestId('inspector')).getAllByRole('button', {
      name: 'Cancel',
    })[0]
    fireEvent.keyDown(cancel ?? document.body, { key: 'Enter' })
    await settle()
    expect(core.callsTo('commitTransaction')).toEqual([])
  })
})

describe('escapeStep (3.5)', () => {
  it('clears the in-sketch selection when no tool is active', () => {
    const { app } = createModelApp({
      state: readyState(rectSketchDocumentState(), {
        context: 'sketch',
        sketch: { ...EDITING_SKETCH, selection: [sketchLineItem('f-sk1', 'e-1')] },
      }),
    })
    expect(escapeStep(app)).toBe(true)
    expect(app.store.getState().sketch?.selection).toEqual([])
    expect(escapeStep(app)).toBe(false)
    expect(app.store.getState().context).toBe('sketch')
  })

  it('clears the app selection, and then has nothing left to do', () => {
    const { app } = createModelApp({
      state: readyState(cubeDocumentState(), { selection: [bodyItem('b-cube', 'Cube')] }),
    })
    expect(escapeStep(app)).toBe(true)
    expect(app.store.getState().selection).toEqual([])
    expect(escapeStep(app)).toBe(false)
  })
})
