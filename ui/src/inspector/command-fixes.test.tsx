import { act, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import {
  cancelCommand,
  editFeature,
  setCommandField,
  startFeatureCommand,
  undoCommandChange,
} from '../app/actions/command'
import { runFixAction } from '../app/actions/fixes'
import { installPopoverPolyfill } from '../components/popover-polyfill.testing'
import type { Diagnostic, Document, FixRef } from '../generated/protocol'
import {
  renderWithApp,
  stubAnimationFrames,
  stubElementSize,
  stubMatchMedia,
  type FrameQueue,
} from '../layout/test-app.testing'
import { MESSAGES } from '../messages'
import { activeCommand, currentDocument, startWithDocument, type FakeApp } from '../test/fakes/app'
import type { FakeFeatureStatus, FakeFix } from '../test/fakes/core'
import { deferred } from '../test/fakes/deferred'
import {
  CUBE_EXTRUDE,
  CUBE_FEATURES_DOCUMENT,
  CUBE_SKETCH_SOLUTION,
  featuresOf,
  PARAMETRIC_DOCUMENT,
} from '../test/fakes/documents'
import {
  R16_DOCUMENT,
  R16_EDGES,
  R16_FILLET,
  R16_FRONT_CUT_EDGE,
  R16_GHOST_POINTS,
  R16_STATUS,
  R16_STORED_EDGE,
  R16_USE_FRONT,
  r16Fix,
} from '../test/fakes/repairs'
import { settle } from '../test/fakes/settle'

// Fixes inside a feature command (ui.md 3.7, 4.2; U-128, U-87, U-107 and U-145): an apply action
// of the banner joins the transaction through apply_fix, waits while the preview is pending, and
// the command takes the record the fix wrote as one history step.

// The WebGL2 backend cannot run in jsdom; the viewport gets a FakeBackend instead.
vi.mock('../viewport/webgl2/backend', async () => {
  const fakes = await import('../test/fakes/backend')
  return { createWebGl2Backend: () => fakes.createFakeBackend() }
})

let restoreSize: () => void
let frames: FrameQueue

beforeEach(installPopoverPolyfill)

beforeEach(() => {
  restoreSize = stubElementSize(HTMLCanvasElement.prototype, { width: 800, height: 600 })
  stubMatchMedia({ reducedMotion: true })
  frames = stubAnimationFrames()
})

afterEach(() => {
  restoreSize()
  vi.unstubAllGlobals()
  vi.restoreAllMocks()
})

function flush(): void {
  act(() => {
    frames.flush()
  })
}

async function settleAll(): Promise<void> {
  await act(async () => {
    await settle()
  })
  flush()
}

function inspector(): HTMLElement {
  return screen.getByTestId('inspector')
}

function banner(): HTMLElement {
  return within(inspector()).getByTestId('inspector-error')
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

/** The fix A0 of `R16-STATUS`: 'Fillet 1' takes C1. */
const USE_FRONT_FIX: FakeFix = {
  feature: 'f-fil1',
  code: 'references_not_found',
  field: '/edges',
  index: 0,
  label: R16_USE_FRONT.label,
  edits: [{ type: 'update_feature', feature: { ...R16_FILLET, edges: [R16_FRONT_CUT_EDGE] } }],
}

const R16_FAKE_STATUS: FakeFeatureStatus = {
  status: R16_STATUS.status,
  messages: R16_STATUS.messages,
  repairs: R16_STATUS.repairs,
}

/** The app and the statuses its fake core reports, which a test changes before an edit. */
type EditedApp = { readonly fake: FakeApp; readonly statuses: Record<string, FakeFeatureStatus> }

/** The R16 document open in the real app over the fake core, rendered, with 'Fillet 1' edited. */
async function editingR16(): Promise<EditedApp> {
  const statuses: Record<string, FakeFeatureStatus> = { 'f-fil1': R16_FAKE_STATUS }
  const fake = await startWithDocument('rabbet.ptsm', R16_DOCUMENT, {
    core: {
      statuses,
      ghosts: [R16_GHOST_POINTS],
      elements: { 'b-1': R16_EDGES },
      fixes: [USE_FRONT_FIX],
    },
  })
  renderWithApp(fake.app)
  await act(async () => {
    await editFeature(fake.app, 'f-fil1')
    await settle()
  })
  flush()
  return { fake, statuses }
}

describe('a candidate in a command (3.7, U-128)', () => {
  it("U-128 clicking button 1 calls applyFix with A0's FixRef", async () => {
    const { fake } = await editingR16()
    const user = userEvent.setup()
    const [first] = within(banner()).getAllByTestId('repair-candidate-button')
    await user.click(first ?? banner())
    await settleAll()
    expect(fake.transport.commandsOfType('apply_fix').map((sent) => sent.fix)).toEqual([r16Fix(0)])
  })

  it("U-128 clicking marker 1 calls applyFix with A0's FixRef", async () => {
    const { fake } = await editingR16()
    const user = userEvent.setup()
    const marker = screen
      .getAllByTestId('repair-candidate')
      .find((element) => element.getAttribute('data-number') === '1')
    await user.click(marker ?? banner())
    await settleAll()
    expect(fake.transport.commandsOfType('apply_fix').map((sent) => sent.fix)).toEqual([r16Fix(0)])
  })

  it('U-128 while the fix runs the preview is pending and the candidates wait', async () => {
    const { fake } = await editingR16()
    const user = userEvent.setup()
    const answer = deferred<void>()
    fake.transport.handle('apply_fix', async (sent, core) => {
      await answer.promise
      return core.reply(sent)
    })
    const [first] = within(banner()).getAllByTestId('repair-candidate-button')
    await user.click(first ?? banner())
    await settleAll()
    expect(inspector()).toHaveAttribute('data-preview', 'pending')
    for (const button of within(banner()).getAllByTestId('repair-candidate-button')) {
      expect(button).toHaveAttribute('aria-disabled', 'true')
      expect(button).toHaveAccessibleDescription(
        expect.stringContaining(MESSAGES.preview_pending()),
      )
    }
    answer.resolve()
    await settleAll()
    expect(inspector()).not.toHaveAttribute('data-preview', 'pending')
  })

  it('U-128 the command takes the edges the fix wrote, and one Mod+Z restores the stored edge', async () => {
    const { fake, statuses } = await editingR16()
    const user = userEvent.setup()
    delete statuses['f-fil1']
    const [first] = within(banner()).getAllByTestId('repair-candidate-button')
    await user.click(first ?? banner())
    await settleAll()
    expect(activeCommand(fake.app).params['edges']).toEqual([R16_FRONT_CUT_EDGE])
    await act(async () => {
      await undoCommandChange(fake.app)
      await settle()
    })
    expect(activeCommand(fake.app).params['edges']).toEqual([R16_STORED_EDGE])
  })

  it('U-128 the fix joins the transaction: no undo entry of its own, and the edit stays open', async () => {
    const { fake } = await editingR16()
    const user = userEvent.setup()
    const [first] = within(banner()).getAllByTestId('repair-candidate-button')
    await user.click(first ?? banner())
    await settleAll()
    const handle = currentDocument(fake.app).documentHandle
    expect(fake.transport.core.document(handle).transaction).toBe('Edit Fillet 1')
    expect(fake.transport.core.document(handle).undoLabels).toEqual([])
  })
})

/** "Use 2.3 mm" of the fillet of cube-features.ptsm. */
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
  message: 'Fillet radius 15 mm is too large for the selected edges. Try 2.3 mm or less.',
  field: '/radius',
  suggestion: '2.3 mm',
  actions: [{ label: 'Use 2.3 mm', run: { kind: 'apply', fix: SMALLER_FIX } }],
}

describe('a fix of a value in a command (3.7, U-87)', () => {
  it('U-87 "Use 2.3 mm" takes the record\'s radius from the fix\'s events and focuses the radius input', async () => {
    const fillet = featuresOf(CUBE_FEATURES_DOCUMENT)[2]
    if (fillet === undefined) {
      throw new Error('cube-features.ptsm has no fillet.')
    }
    const statuses: Record<string, FakeFeatureStatus> = {
      'f-fil1': { status: 'error', messages: [TOO_LARGE] },
    }
    const fake = await startWithDocument('cube-features.ptsm', CUBE_FEATURES_DOCUMENT, {
      core: {
        statuses,
        fixes: [
          {
            feature: 'f-fil1',
            code: 'fillet_too_large',
            field: '/radius',
            index: 0,
            label: 'Use 2.3 mm',
            edits: [{ type: 'update_feature', feature: { ...fillet, radius: '2.3 mm' } }],
          },
        ],
      },
    })
    renderWithApp(fake.app)
    await act(async () => {
      await editFeature(fake.app, 'f-fil1')
      await settle()
    })
    const user = userEvent.setup()
    delete statuses['f-fil1']
    await user.click(within(banner()).getByRole('button', { name: 'Use 2.3 mm' }))
    await settleAll()
    expect(fake.transport.commandsOfType('apply_fix').map((sent) => sent.fix)).toEqual([
      SMALLER_FIX,
    ])
    expect(activeCommand(fake.app).params['radius']).toBe('2.3 mm')
    expect(within(field('radius')).getByRole('combobox')).toHaveFocus()
  })
})

const REVERSE_FIX: FixRef = {
  feature: 'f-ext1',
  code: 'no_effect',
  cause: false,
  index: 0,
  revision: 1,
}

const NO_EFFECT: Diagnostic = {
  severity: 'warning',
  code: 'no_effect',
  message: "This cut does not touch body 'Cube', so nothing was removed.",
  actions: [{ label: 'Turn on Reverse', run: { kind: 'apply', fix: REVERSE_FIX } }],
}

/** The parametric cube with Extrude 1 cutting, its no_effect warning, and "Turn on Reverse" as the fake core applies it. */
async function editingCut(extraEdits: FakeFix['edits'] = []): Promise<EditedApp> {
  const cut = { ...CUBE_EXTRUDE, extent: { kind: 'distance', expr: 'size' }, op: 'cut' }
  const document: Document = {
    ...PARAMETRIC_DOCUMENT,
    features: featuresOf(PARAMETRIC_DOCUMENT).map((feature) =>
      feature.id === cut.id ? cut : feature,
    ),
  }
  const statuses: Record<string, FakeFeatureStatus> = {
    'f-ext1': { status: 'warning', messages: [NO_EFFECT] },
  }
  const fake = await startWithDocument('parametric.ptsm', document, {
    core: {
      statuses,
      solutions: { 'f-sk1': CUBE_SKETCH_SOLUTION },
      fixes: [
        {
          feature: 'f-ext1',
          code: 'no_effect',
          index: 0,
          label: 'Turn on Reverse',
          edits: [{ type: 'update_feature', feature: { ...cut, reverse: true } }, ...extraEdits],
        },
      ],
    },
  })
  renderWithApp(fake.app)
  await act(async () => {
    await editFeature(fake.app, 'f-ext1')
    await settle()
  })
  return { fake, statuses }
}

describe('fixes during commands (3.7, U-107 and U-145)', () => {
  it('U-107 "Turn on Reverse" sets the command\'s Reverse from the record and focuses Distance', async () => {
    const { fake, statuses } = await editingCut()
    const user = userEvent.setup()
    delete statuses['f-ext1']
    await user.click(within(banner()).getByRole('button', { name: 'Turn on Reverse' }))
    await settleAll()
    expect(fake.transport.commandsOfType('apply_fix').map((sent) => sent.fix)).toEqual([
      REVERSE_FIX,
    ])
    expect(activeCommand(fake.app).params['reverse']).toBe(true)
    expect(within(field('extent.expr')).getByRole('combobox')).toHaveFocus()
  })

  it('U-145 Mod+Z after the fix sets Reverse back and previews again', async () => {
    const { fake, statuses } = await editingCut()
    const user = userEvent.setup()
    delete statuses['f-ext1']
    await user.click(within(banner()).getByRole('button', { name: 'Turn on Reverse' }))
    await settleAll()
    const previews = fake.transport.commandsOfType('apply_edits').length
    await act(async () => {
      await undoCommandChange(fake.app)
      await settle()
    })
    expect(activeCommand(fake.app).params['reverse'] ?? false).toBe(false)
    expect(fake.transport.commandsOfType('apply_edits').length).toBeGreaterThan(previews)
  })

  it("U-145 a fix that also changes a parameter: Mod+Z undoes only the record's fields, Cancel reverts both", async () => {
    const { fake, statuses } = await editingCut([
      { type: 'update_parameter', parameter: { id: 'p-size', name: 'size', expr: '25 mm' } },
    ])
    const user = userEvent.setup()
    delete statuses['f-ext1']
    await user.click(within(banner()).getByRole('button', { name: 'Turn on Reverse' }))
    await settleAll()
    const parameterExpression = () =>
      (currentDocument(fake.app).recipe.parameters ?? []).find(
        (parameter) => parameter.id === 'p-size',
      )?.expr
    expect(parameterExpression()).toBe('25 mm')
    await act(async () => {
      await undoCommandChange(fake.app)
      await settle()
    })
    expect(activeCommand(fake.app).params['reverse'] ?? false).toBe(false)
    expect(parameterExpression()).toBe('25 mm')
    await act(async () => {
      await cancelCommand(fake.app)
      await settle()
    })
    expect(parameterExpression()).toBe('20 mm')
    const extrude = featuresOf(currentDocument(fake.app).recipe).find(
      (feature) => feature.id === 'f-ext1',
    )
    expect(extrude?.['reverse'] ?? false).toBe(false)
  })

  it('U-145 during a command an apply action of another feature in Problems waits with finish_command_first', async () => {
    const statuses: Record<string, FakeFeatureStatus> = {
      'f-fil1': { status: 'error', messages: [TOO_LARGE] },
    }
    const fake = await startWithDocument('cube-features.ptsm', CUBE_FEATURES_DOCUMENT, {
      core: { statuses },
    })
    fake.app.store.setState((state) => ({ panels: { ...state.panels, side: 'problems' } }))
    renderWithApp(fake.app)
    await act(async () => {
      await startFeatureCommand(fake.app, 'extrude')
      await settle()
    })
    const use = within(screen.getByTestId('problems-panel')).getByRole('button', {
      name: 'Use 2.3 mm',
    })
    expect(use).toHaveAttribute('aria-disabled', 'true')
    expect(use).toHaveAccessibleDescription(
      expect.stringContaining(MESSAGES.finish_command_first('Extrude')),
    )
    const user = userEvent.setup()
    await user.click(use)
    await settleAll()
    expect(fake.transport.commandsOfType('apply_fix')).toEqual([])
  })

  it('U-145 a show of another feature cancels the command with command_canceled', async () => {
    const fake = await startWithDocument('cube-features.ptsm', CUBE_FEATURES_DOCUMENT)
    await editFeature(fake.app, 'f-fil1')
    await settle()
    setCommandField(fake.app, 'radius', '3 mm')
    await settle()
    await runFixAction(
      fake.app,
      {
        label: "Show 'Sketch 1'",
        run: { kind: 'show', feature: 'f-sk1' },
      },
      'banner',
    )
    await settle()
    expect(fake.app.store.getState().command).toBeNull()
    expect(fake.app.store.getState().notifications).toMatchObject([
      {
        severity: 'info',
        code: 'command_canceled',
        message: MESSAGES.command_canceled('Fillet 1'),
      },
    ])
  })
})
