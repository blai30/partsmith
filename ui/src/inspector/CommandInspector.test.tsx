import { cleanup, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { installPopoverPolyfill } from '../components/popover-polyfill.testing'
import type { Diagnostic, FeatureStatus } from '../generated/protocol'
import { cubeDocumentState, elementItem, readyState } from '../layout/fixtures.testing'
import { createModelApp, renderWithApp, stubMatchMedia } from '../layout/test-app.testing'
import type { ActiveCommand, AppState, DocumentState } from '../state/store'
import { CUBE_REGION, CUBE_TOP_FRONT_EDGE } from '../test/fakes/documents'
import { settle } from '../test/fakes/settle'
import { startFeatureCommand } from '../app/actions/command'

// The command inspector of the one-interface amendments (ui.md 3.7, 4.2): the inline Reverse, its
// fixes, what "More options" hides, created bodies in their fields and where focus starts.

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

const PROFILE = { sketch: 'f-sk1', regions: [CUBE_REGION.reference] }

function command(fields: Partial<ActiveCommand>): ActiveCommand {
  return {
    featureType: 'extrude',
    featureId: 'f-ext9',
    editing: false,
    title: 'Extrude',
    params: {
      profile: PROFILE,
      extent: { kind: 'distance', expr: '-5 mm' },
      op: 'join',
      body: 'b-cube',
    },
    activeField: null,
    fieldErrors: {},
    applied: true,
    newBody: null,
    helper: null,
    changedPaths: [],
    history: [],
    future: [],
    preview: 'failed',
    ...fields,
  }
}

function withStatus(id: string, messages: readonly Diagnostic[]): DocumentState {
  const document = cubeDocumentState()
  const status: FeatureStatus = {
    id,
    status: messages.some((message) => message.severity === 'error') ? 'error' : 'warning',
    messages: [...messages],
    repairs: [],
  }
  return { ...document, featureStatus: { ...document.featureStatus, [id]: status } }
}

function renderCommand(
  active: ActiveCommand,
  document: DocumentState,
  state: Partial<AppState> = {},
) {
  const test = createModelApp({ state: readyState(document, { command: active, ...state }) })
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

const OUT_OF_RANGE: Diagnostic = {
  severity: 'error',
  code: 'out_of_range',
  message: 'Distance must be more than 0 mm, but it is -5 mm.',
  field: '/extent/expr',
}

describe('the inline Reverse (2.12, 3.7)', () => {
  it('sits at the end of the Distance row as a pressed-state icon button with its help', async () => {
    const { app, user } = renderCommand(
      command({
        params: {
          profile: PROFILE,
          extent: { kind: 'distance', expr: '5 mm' },
          op: 'join',
          body: 'b-cube',
        },
      }),
      cubeDocumentState(),
    )
    const distance = field('extent.expr')
    const reverse = within(distance).getByRole('button', { name: 'Reverse' })
    expect(within(distance).getByTestId('inspector-field')).toHaveAttribute('data-path', 'reverse')
    expect(reverse).toHaveAttribute('aria-pressed', 'false')
    expect(reverse).toHaveAccessibleDescription('Extrudes to the other side of the sketch.')
    await user.click(reverse)
    expect(app.store.getState().command?.params['reverse']).toBe(true)
  })
})

describe('Reverse fixes (4.2, U-107)', () => {
  it('U-107 offers "Reverse and use 5 mm" for a negative distance out of range', async () => {
    const { app, user } = renderCommand(command({}), withStatus('f-ext9', [OUT_OF_RANGE]))
    await user.click(within(inspector()).getByRole('button', { name: 'Reverse and use 5 mm' }))
    expect(app.store.getState().command?.params).toMatchObject({
      reverse: true,
      extent: { kind: 'distance', expr: '5 mm' },
    })
    expect(app.store.getState().command?.changedPaths).toContain('reverse')
  })

  it('U-107 offers no such fix for an expression that is not a plain number', () => {
    renderCommand(
      command({
        params: {
          profile: PROFILE,
          extent: { kind: 'distance', expr: '-wall' },
          op: 'join',
          body: 'b-cube',
        },
      }),
      withStatus('f-ext9', [OUT_OF_RANGE]),
    )
    expect(within(inspector()).queryByRole('button', { name: /^Reverse and use/ })).toBeNull()
  })

  it('U-107 offers no such fix for a field Reverse is not attached to', () => {
    renderCommand(
      command({
        featureType: 'hole',
        featureId: 'f-hole9',
        title: 'Hole',
        params: {
          points: ['e-p1'],
          sketch: 'f-sk1',
          body: 'b-cube',
          diameter: '-3 mm',
          extent: { kind: 'through_all' },
        },
      }),
      withStatus('f-hole9', [{ ...OUT_OF_RANGE, field: '/diameter' }]),
    )
    expect(within(inspector()).queryByRole('button', { name: /^Reverse and use/ })).toBeNull()
  })

  it('U-107 offers "Reverse" for a cut that does nothing', async () => {
    const noEffect: Diagnostic = {
      severity: 'warning',
      code: 'no_effect',
      message: 'Extrude 2 does not change Cube.',
    }
    const { app, user } = renderCommand(
      command({
        params: {
          profile: PROFILE,
          extent: { kind: 'distance', expr: '5 mm' },
          op: 'cut',
          body: 'b-cube',
        },
      }),
      withStatus('f-ext9', [noEffect]),
    )
    const banner = within(inspector()).getByTestId('inspector-error')
    await user.click(within(banner).getByRole('button', { name: 'Reverse' }))
    expect(app.store.getState().command?.params['reverse']).toBe(true)
  })
})

describe('More options shows what it hides (3.7, U-117)', () => {
  it('U-117 names the More fields changed out of sight, and lists every More field in its tooltip', async () => {
    const { user } = renderCommand(
      command({
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
      }),
      cubeDocumentState(),
    )
    const more = within(inspector()).getByTestId('inspector-more')
    expect(more).toHaveTextContent('More options · Tip angle')
    await user.hover(more)
    await waitFor(() => {
      expect(screen.getByRole('tooltip')).toHaveTextContent('Tip angle, Sketch, Body')
    })
  })
})

describe('created bodies in their fields (2.12, U-105)', () => {
  it('U-105 shows a mirror target set to its new body as "Body 1 (new)", with New body first in Choose', async () => {
    const { user } = renderCommand(
      command({
        featureType: 'mirror',
        featureId: 'f-mir9',
        title: 'Mirror',
        params: {
          body: 'b-cube',
          plane: { kind: 'origin', plane: 'XZ' },
          op: 'new',
          target: 'b-new',
        },
        newBody: { id: 'b-new', name: 'Body 1', path: 'target' },
        applied: false,
        preview: 'none',
      }),
      cubeDocumentState(),
    )
    expect(within(field('target')).getByTestId('selection-field')).toHaveTextContent('Body 1 (new)')
    await user.click(within(field('target')).getByRole('button', { name: 'Choose Target body' }))
    const menu = screen.getByRole('menu', { name: 'Choose Target body' })
    expect(
      within(menu)
        .getAllByRole('menuitem')
        .map((item) => item.textContent),
    ).toEqual(['New body'])
  })
})

describe('where focus starts (3.7 step 5, U-104)', () => {
  it('U-104 a Fillet started with a preselected edge focuses Radius with its text selected', async () => {
    const test = createModelApp({
      state: readyState(cubeDocumentState(), { selection: [elementItem(CUBE_TOP_FRONT_EDGE)] }),
    })
    renderWithApp(test.app)
    await startFeatureCommand(test.app, 'fillet')
    await settle()
    const radius = within(field('radius')).getByRole('combobox', { name: 'Radius' })
    expect(radius).toHaveFocus()
    expect(test.app.store.getState().command?.activeField).toBe('edges')
  })

  it('focuses a required empty selection field that waits for a pick', async () => {
    const test = createModelApp({ state: readyState(cubeDocumentState()) })
    renderWithApp(test.app)
    await startFeatureCommand(test.app, 'fillet')
    await settle()
    expect(within(field('edges')).getByTestId('selection-field')).toHaveFocus()
    expect(within(field('edges')).getByTestId('selection-field')).toHaveTextContent(
      'Select edges or a face',
    )
  })
})
