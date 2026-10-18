import { cleanup, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { installPopoverPolyfill } from '../components/popover-polyfill.testing'
import type { Diagnostic, FeatureStatus, FixAction, FixRef } from '../generated/protocol'
import {
  activeCommandWith,
  cubeDocumentState,
  documentState,
  readyState,
} from '../layout/fixtures.testing'
import { createModelApp, renderWithApp, stubMatchMedia } from '../layout/test-app.testing'
import type { ActiveCommand, AppState, DocumentState } from '../state/store'
import { CUBE_REGION, CUBE_TOP_FRONT_EDGE, emptyDocument } from '../test/fakes/documents'
import { elementItem } from '../test/fakes/selection'
import { settle } from '../test/fakes/settle'
import { startFeatureCommand } from '../app/actions/command'
import { COLOR_CLASSES } from '../theme/roles'
import { inspectorField } from './inspector.testing'

// The command inspector of the one-interface amendments (ui.md 3.7, 4.2): the inline Reverse, its
// fixes, what "More options" hides, created bodies in their fields and where focus starts.

beforeEach(installPopoverPolyfill)

beforeEach(() => {
  stubMatchMedia()
})

afterEach(() => {
  cleanup()
  vi.unstubAllGlobals()
  vi.restoreAllMocks()
})

const PROFILE = { sketch: 'f-sk1', regions: [CUBE_REGION.reference] }

function command(fields: Partial<ActiveCommand>): ActiveCommand {
  return activeCommandWith({
    featureType: 'extrude',
    featureId: 'f-ext9',
    title: 'Extrude',
    params: {
      profile: PROFILE,
      extent: { kind: 'distance', expr: '-5 mm' },
      op: 'join',
      body: 'b-cube',
    },
    applied: true,
    preview: 'failed',
    ...fields,
  })
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
    const distance = inspectorField('extent.expr')
    const reverse = within(distance).getByRole('button', { name: 'Reverse' })
    expect(within(distance).getByTestId('inspector-field')).toHaveAttribute('data-path', 'reverse')
    expect(reverse).toHaveAttribute('aria-pressed', 'false')
    expect(reverse).toHaveAccessibleDescription('Extrudes to the other side of the sketch.')
    await user.click(reverse)
    expect(app.store.getState().command?.params['reverse']).toBe(true)
  })
})

/** The FixRef of a "Turn on Reverse" action of the extrude command's message `code`. */
function reverseFix(code: string): FixRef {
  return { feature: 'f-ext9', code, cause: false, index: 0, revision: 1 }
}

function turnOnReverse(code: string): FixAction {
  return { label: 'Turn on Reverse', run: { kind: 'apply', fix: reverseFix(code) } }
}

/** The banner's buttons named like a Reverse fix. */
function reverseButtons(): string[] {
  return within(within(inspector()).getByTestId('inspector-error'))
    .queryAllByRole('button')
    .map((button) => button.textContent ?? '')
    .filter((name) => /Reverse/.test(name))
}

describe('Reverse fixes (4.2, U-107)', () => {
  it('U-107 offers "Reverse and use 5 mm" for a negative distance out of range', async () => {
    const { app, user } = renderCommand(command({}), withStatus('f-ext9', [OUT_OF_RANGE]))
    await user.click(within(inspector()).getByRole('button', { name: 'Reverse and use 5 mm' }))
    expect(app.store.getState().command?.params).toMatchObject({
      reverse: true,
      extent: { kind: 'distance', expr: '5 mm' },
    })
    expect(app.store.getState().command?.changedPaths).toContain('reverse')
    expect(within(inspectorField('extent.expr')).getByRole('combobox')).toHaveFocus()
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

  const CUT = command({
    params: {
      profile: PROFILE,
      extent: { kind: 'distance', expr: '5 mm' },
      op: 'cut',
      body: 'b-cube',
    },
  })

  const NO_EFFECT: Diagnostic = {
    severity: 'warning',
    code: 'no_effect',
    message: "This cut does not touch body 'Cube', so nothing was removed.",
  }

  const THROUGH_ALL_MISSES: Diagnostic = {
    severity: 'error',
    code: 'through_all_misses',
    message: "This cut through everything does not reach body 'Cube'.",
  }

  it('U-141 offers no Reverse of its own for a cut that does nothing', () => {
    renderCommand(CUT, withStatus('f-ext9', [NO_EFFECT]))
    const banner = within(inspector()).getByTestId('inspector-error')
    // U-188: a status banner inside the card.
    expect(banner).toHaveClass('rounded-lg')
    expect(reverseButtons()).toEqual([])
  })

  it('U-141 offers no Reverse of its own for a through-all cut that misses', () => {
    renderCommand(CUT, withStatus('f-ext9', [THROUGH_ALL_MISSES]))
    expect(reverseButtons()).toEqual([])
  })

  it(`U-141 shows exactly the core's "Turn on Reverse" when the message offers it`, () => {
    renderCommand(
      CUT,
      withStatus('f-ext9', [{ ...NO_EFFECT, actions: [turnOnReverse('no_effect')] }]),
    )
    expect(reverseButtons()).toEqual(['Turn on Reverse'])
  })

  it('U-141 shows exactly "Turn on Reverse" for a through-all miss that offers it', () => {
    renderCommand(
      CUT,
      withStatus('f-ext9', [
        { ...THROUGH_ALL_MISSES, actions: [turnOnReverse('through_all_misses')] },
      ]),
    )
    expect(reverseButtons()).toEqual(['Turn on Reverse'])
  })

  it('U-107 "Turn on Reverse" calls applyFix with its FixRef', async () => {
    const { core, user } = renderCommand(
      CUT,
      withStatus('f-ext9', [{ ...NO_EFFECT, actions: [turnOnReverse('no_effect')] }]),
    )
    await user.click(within(inspector()).getByRole('button', { name: 'Turn on Reverse' }))
    await settle()
    expect(core.callsTo('applyFix')).toEqual([[1, reverseFix('no_effect')]])
  })
})

describe('a status message about a field takes the user to it (4.2)', () => {
  it('moves focus to the checked option of a segmented choice such as Operation', async () => {
    const message: Diagnostic = {
      severity: 'warning',
      code: 'no_effect',
      message: 'Extrude 2 does not change Cube.',
      field: '/op',
    }
    const { user } = renderCommand(
      command({
        params: {
          profile: PROFILE,
          extent: { kind: 'distance', expr: '5 mm' },
          op: 'join',
          body: 'b-cube',
        },
      }),
      withStatus('f-ext9', [message]),
    )
    const banner = within(inspector()).getByTestId('inspector-error')
    await user.click(within(banner).getByRole('button', { name: message.message }))
    expect(within(inspectorField('op')).getByRole('radio', { checked: true })).toHaveFocus()
  })

  it('moves focus to a switch such as Keep tools', async () => {
    const message: Diagnostic = {
      severity: 'warning',
      code: 'no_effect',
      message: 'Boolean 1 does not change Cube.',
      field: '/keep_tools',
    }
    const { user } = renderCommand(
      command({
        featureType: 'boolean',
        featureId: 'f-bool9',
        title: 'Boolean',
        params: { target: 'b-cube', tools: ['b-peg'], op: 'join', keep_tools: true },
      }),
      withStatus('f-bool9', [message]),
    )
    const banner = within(inspector()).getByTestId('inspector-error')
    await user.click(within(banner).getByRole('button', { name: message.message }))
    expect(within(inspectorField('keep_tools')).getByRole('switch')).toHaveFocus()
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

/** The axis letter in the inspector field at a path. */
function letterOf(path: string): HTMLElement {
  return within(inspectorField(path)).getByTestId('axis-letter')
}

describe('axis letters in fields (3.7, U-181)', () => {
  const AXIS_FOREGROUNDS = {
    X: COLOR_CLASSES.axisX.foreground,
    Y: COLOR_CLASSES.axisY.foreground,
    Z: COLOR_CLASSES.axisZ.foreground,
  } as const

  it('U-181 draws X, Y and Z inside the Move body fields in the world axes’ colors', () => {
    renderCommand(
      command({
        featureType: 'move_body',
        featureId: 'f-mov9',
        title: 'Move body',
        params: { body: 'b-cube', translation: ['0', '0', '0'] },
      }),
      cubeDocumentState(),
    )
    for (const [index, axis] of (['X', 'Y', 'Z'] as const).entries()) {
      const path = `translation[${String(index)}]`
      const input = within(inspectorField(path)).getByRole('combobox', { name: `Move ${axis}` })
      expect(input).toHaveClass('pl-6')
      const letter = letterOf(path)
      expect(letter).toHaveTextContent(axis)
      expect(letter).toHaveAttribute('data-axis', axis)
      expect(letter).toHaveAttribute('data-world', 'true')
      expect(letter).toHaveAttribute('aria-hidden', 'true')
      expect(letter).toHaveClass(
        'pointer-events-none',
        'absolute',
        'inset-y-0',
        'left-0',
        'w-6',
        'text-xs',
        'font-semibold',
        AXIS_FOREGROUNDS[axis],
      )
    }
  })

  it('U-181 mutes the Position letters of a box on the Front plane, keeping the field names', async () => {
    const { user } = renderCommand(
      command({
        featureType: 'box',
        featureId: 'f-box9',
        title: 'Box',
        params: {
          size: ['20 mm', '20 mm', '20 mm'],
          op: 'new',
          plane: { kind: 'origin', plane: 'XZ' },
          position: ['0', '0', '0'],
          body: 'b-new',
        },
        newBody: { id: 'b-new', name: 'Body 1', path: 'body' },
      }),
      cubeDocumentState(),
    )
    await user.click(within(inspector()).getByTestId('inspector-more'))
    const input = within(inspectorField('position[0]')).getByRole('combobox', {
      name: 'Position X',
    })
    expect(input).toHaveClass('pl-6')
    for (const path of ['position[0]', 'position[1]', 'position[2]']) {
      const letter = letterOf(path)
      expect(letter).toHaveAttribute('data-world', 'false')
      expect(letter).toHaveClass(COLOR_CLASSES.textMuted.foreground)
      expect(letter).not.toHaveClass(AXIS_FOREGROUNDS.X)
    }
  })

  it('U-181 gives a field without an axis no letter and no letter padding', () => {
    renderCommand(command({}), cubeDocumentState())
    const distance = inspectorField('extent.expr')
    expect(within(distance).queryByTestId('axis-letter')).toBeNull()
    expect(within(distance).getByRole('combobox', { name: 'Distance' })).not.toHaveClass('pl-6')
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
    expect(within(inspectorField('target')).getByTestId('selection-field')).toHaveTextContent(
      'Body 1 (new)',
    )
    await user.click(
      within(inspectorField('target')).getByRole('button', { name: 'Choose Target body' }),
    )
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
    const radius = within(inspectorField('radius')).getByRole('combobox', { name: 'Radius' })
    expect(radius).toHaveFocus()
    expect(test.app.store.getState().command?.activeField).toBe('edges')
  })

  it('U-104 Box in a new document opens with Length focused and selected and no active field, since it makes its own body', async () => {
    const test = createModelApp({ state: readyState(documentState(emptyDocument())) })
    renderWithApp(test.app)
    await startFeatureCommand(test.app, 'box')
    await settle()
    expect(test.app.store.getState().command?.activeField).toBeNull()
    const length = within(inspectorField('size[0]')).getByRole('combobox', { name: 'Length' })
    expect(length).toHaveFocus()
    expect(length).toHaveProperty('selectionStart', 0)
    expect(length).toHaveProperty('selectionEnd', length.getAttribute('value')?.length ?? 0)
  })

  it('brings the whole focused field into view, its label too, not the input alone', async () => {
    const focus = vi.spyOn(HTMLElement.prototype, 'focus')
    const scroll = vi.spyOn(Element.prototype, 'scrollIntoView')
    const test = createModelApp({
      state: readyState(cubeDocumentState(), { selection: [elementItem(CUBE_TOP_FRONT_EDGE)] }),
    })
    renderWithApp(test.app)
    await startFeatureCommand(test.app, 'fillet')
    await settle()
    const radius = within(inspectorField('radius')).getByRole('combobox', { name: 'Radius' })
    expect(radius).toHaveFocus()
    const focused = focus.mock.contexts.lastIndexOf(radius)
    expect(focus.mock.calls[focused]).toEqual([{ preventScroll: true }])
    const scrolled = scroll.mock.contexts.lastIndexOf(inspectorField('radius'))
    expect(scroll.mock.calls[scrolled]).toEqual([{ block: 'nearest' }])
  })

  it('focuses a required empty selection field that waits for a pick', async () => {
    const test = createModelApp({ state: readyState(cubeDocumentState()) })
    renderWithApp(test.app)
    await startFeatureCommand(test.app, 'fillet')
    await settle()
    expect(within(inspectorField('edges')).getByTestId('selection-field')).toHaveFocus()
    expect(within(inspectorField('edges')).getByTestId('selection-field')).toHaveTextContent(
      'Select edges or a face',
    )
  })
})
