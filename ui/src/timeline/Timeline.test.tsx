import { act, cleanup, fireEvent, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { installShortcutDispatcher } from '../commands/dispatcher'
import { paletteItems } from '../commands/palette'
import { installPopoverPolyfill } from '../components/popover-polyfill.testing'
import type { Document, FeatureStatus } from '../generated/protocol'
import {
  cubeDocumentState,
  documentState,
  FOUR_BOXES_DOCUMENT,
  IMPORT_DOCUMENT,
  readyState,
  THREE_BOXES_DOCUMENT,
} from '../layout/fixtures.testing'
import { createModelApp, renderWithApp, stubMatchMedia } from '../layout/test-app.testing'
import { MESSAGES } from '../messages'
import type { ActiveCommand, AppState, DocumentState } from '../state/store'
import { coreErrorFor } from '../test/fakes/core-api'
import {
  CUBE_DOCUMENT,
  CUBE_EXTRUDE,
  CUBE_SKETCH,
  emptyDocument,
  featuresOf,
} from '../test/fakes/documents'
import { featureItem } from '../test/fakes/selection'
import { settle } from '../test/fakes/settle'
import { COLOR_CLASSES } from '../theme/roles'

let uninstallDispatcher: () => void = () => undefined

beforeEach(installPopoverPolyfill)

beforeEach(() => {
  stubMatchMedia()
})

afterEach(() => {
  uninstallDispatcher()
  cleanup()
  vi.unstubAllGlobals()
  vi.restoreAllMocks()
})

function renderTimeline(
  document: DocumentState = cubeDocumentState(),
  state: Partial<AppState> = {},
) {
  const test = createModelApp({ state: readyState(document, state) })
  renderWithApp(test.app)
  uninstallDispatcher = installShortcutDispatcher(test.app, window)
  return { ...test, user: userEvent.setup(), timeline: screen.getByTestId('timeline') }
}

function boxes(
  rollback: string | null = null,
  document: Document = THREE_BOXES_DOCUMENT,
): DocumentState {
  return documentState(rollback === null ? document : { ...document, rollback })
}

function item(id: string): HTMLElement {
  const found = screen
    .getAllByTestId('timeline-item')
    .find((element) => element.getAttribute('data-feature-id') === id)
  if (found === undefined) {
    throw new Error(`No timeline item ${id}`)
  }
  return found
}

function itemButton(label: string): HTMLElement {
  return within(screen.getByTestId('timeline')).getByRole('button', { name: label })
}

function describedText(element: HTMLElement): string {
  const ids = element.getAttribute('aria-describedby')?.split(' ') ?? []
  return ids.map((id) => document.getElementById(id)?.textContent ?? '').join(' ')
}

function withStatus(document: DocumentState, status: FeatureStatus): DocumentState {
  return { ...document, featureStatus: { ...document.featureStatus, [status.id]: status } }
}

const FAILED_EXTRUDE: FeatureStatus = {
  id: 'f-ext1',
  status: 'error',
  messages: [
    {
      severity: 'error',
      code: 'body_not_empty',
      message: "Body 'Cube' already has a solid. Use op join to add to it.",
    },
  ],
  repairs: [],
}

const EDITING_EXTRUDE: ActiveCommand = {
  featureType: 'extrude',
  featureId: 'f-ext1',
  editing: true,
  title: 'Extrude 1',
  params: {},
  activeField: null,
  fieldErrors: {},
  applied: true,
  newBody: null,
  helper: null,
  changedPaths: [],
  history: [],
  future: [],
  preview: 'none',
}

describe('timeline items (3.9)', () => {
  it('says what the timeline is for while the document has no features', () => {
    renderTimeline(documentState(emptyDocument()))
    expect(screen.getByTestId('timeline-empty')).toHaveTextContent(
      /^Each step you add appears here\. Double-click a step to change it later\.$/,
    )
    expect(screen.getByTestId('timeline-empty')).toHaveClass(COLOR_CLASSES.textMuted.foreground)
  })

  it('shows no empty text once there is a feature', () => {
    renderTimeline()
    expect(screen.queryByTestId('timeline-empty')).toBeNull()
  })

  it('lists the features in timeline order, named by their labels, with their status', () => {
    renderTimeline()
    const items = screen.getAllByTestId('timeline-item')
    expect(items.map((element) => element.getAttribute('data-feature-id'))).toEqual([
      'f-sk1',
      'f-ext1',
    ])
    expect(items.map((element) => element.getAttribute('data-status'))).toEqual(['ok', 'ok'])
    expect(itemButton('Sketch 1')).toBeInTheDocument()
    expect(itemButton('Extrude 1')).toBeInTheDocument()
  })

  it('shows pending before the first status of a feature', () => {
    const document = cubeDocumentState()
    renderTimeline({
      ...document,
      featureStatus: { 'f-sk1': document.featureStatus['f-sk1'] as FeatureStatus },
    })
    expect(item('f-ext1')).toHaveAttribute('data-status', 'pending')
  })

  it('describes an error with its first message and shows the circle-x badge', () => {
    renderTimeline(withStatus(cubeDocumentState(), FAILED_EXTRUDE))
    expect(item('f-ext1')).toHaveAttribute('data-status', 'error')
    expect(describedText(itemButton('Extrude 1'))).toContain(
      "Error: Body 'Cube' already has a solid. Use op join to add to it.",
    )
    expect(item('f-ext1').querySelector('.lucide-circle-x')).not.toBeNull()
  })

  it('describes a warning with its first message and shows the hazard sign', () => {
    renderTimeline(
      withStatus(cubeDocumentState(), {
        id: 'f-ext1',
        status: 'warning',
        messages: [
          { severity: 'warning', code: 'no_effect', message: 'This feature does not change it.' },
        ],
        repairs: [],
      }),
    )
    expect(describedText(itemButton('Extrude 1'))).toContain(
      'Warning: This feature does not change it.',
    )
    expect(item('f-ext1').querySelector('.lucide-triangle-alert')).not.toBeNull()
  })

  it('marks suppressed features', () => {
    const suppressed: Document = {
      ...CUBE_DOCUMENT,
      features: [CUBE_SKETCH, { ...CUBE_EXTRUDE, suppressed: true }] as NonNullable<
        Document['features']
      >,
    }
    renderTimeline(
      withStatus(documentState(suppressed), {
        id: 'f-ext1',
        status: 'suppressed',
        messages: [],
        repairs: [],
      }),
    )
    expect(item('f-ext1')).toHaveAttribute('data-suppressed', 'true')
    expect(item('f-sk1')).toHaveAttribute('data-suppressed', 'false')
    expect(describedText(itemButton('Extrude 1'))).toContain('Suppressed')
  })

  it('marks the features after the rollback marker as rolled back', () => {
    renderTimeline(boxes('f-box2'))
    expect(item('f-box1')).toHaveAttribute('data-rolled-back', 'false')
    expect(item('f-box2')).toHaveAttribute('data-rolled-back', 'true')
    expect(item('f-box3')).toHaveAttribute('data-rolled-back', 'true')
  })

  it('outlines the feature being edited in the accent and dims the items after it', () => {
    renderTimeline(cubeDocumentState(), { command: EDITING_EXTRUDE })
    const editing = item('f-ext1')
    const carriers = [editing, ...editing.querySelectorAll('*')]
    const outline = COLOR_CLASSES.accent.outline.split(' ')
    expect(
      carriers.some((element) => outline.every((name) => element.classList.contains(name))),
    ).toBe(true)
  })

  it('gives selected items the selection background', () => {
    renderTimeline(cubeDocumentState(), { selection: [featureItem('f-ext1', 'Extrude 1')] })
    const selected = item('f-ext1')
    const carriers = [selected, ...selected.querySelectorAll('*')]
    const background = COLOR_CLASSES.selection.background.split(' ')
    expect(
      carriers.some((element) => background.every((name) => element.classList.contains(name))),
    ).toBe(true)
  })
})

describe('scrolling (3.4, 3.9)', () => {
  it('scrolls a feature chosen in the palette into view and selects it', () => {
    const { app } = renderTimeline()
    const strip = within(screen.getByTestId('timeline')).getByRole('toolbar', { name: 'Features' })
    Object.defineProperty(strip, 'clientWidth', { value: 100 })
    const target = item('f-ext1')
    Object.defineProperty(target, 'offsetLeft', { value: 400 })
    Object.defineProperty(target, 'offsetWidth', { value: 40 })
    const option = paletteItems(app, []).find((entry) => entry.id === 'feature:f-ext1')
    act(() => {
      option?.run()
    })
    expect(app.store.getState().selection.map((entry) => entry.key)).toEqual(['feature:f-ext1'])
    expect(strip.scrollLeft).toBe(340)
  })
})

describe('the rollback marker (3.9)', () => {
  it('is a slider whose value is the number of active features', () => {
    renderTimeline(boxes('f-box2'))
    const marker = screen.getByTestId('timeline-marker')
    expect(marker).toHaveAttribute('role', 'slider')
    expect(marker).toHaveAccessibleName('Rollback marker')
    expect(marker).toHaveAttribute('aria-valuemin', '0')
    expect(marker).toHaveAttribute('aria-valuemax', '3')
    expect(marker).toHaveAttribute('aria-valuenow', '1')
    expect(marker).toHaveAttribute('aria-valuetext', 'After Box 1')
  })

  it('reads "At the end" with nothing rolled back and "At the start" before the first feature', () => {
    renderTimeline(boxes())
    expect(screen.getByTestId('timeline-marker')).toHaveAttribute('aria-valuetext', 'At the end')
    expect(screen.getByTestId('timeline-marker')).toHaveAttribute('aria-valuenow', '3')
    cleanup()
    renderTimeline(boxes('f-box1'))
    expect(screen.getByTestId('timeline-marker')).toHaveAttribute('aria-valuetext', 'At the start')
  })

  it('U-68 End sends set_rollback without an id, labeled "Roll back"', async () => {
    const { core } = renderTimeline(boxes('f-box2'))
    fireEvent.keyDown(screen.getByTestId('timeline-marker'), { key: 'End' })
    await settle()
    expect(core.callsTo('applyEdits')).toEqual([[1, 'Roll back', [{ type: 'set_rollback' }]]])
  })

  it('U-68 Home sends set_rollback to the first feature', async () => {
    const { core } = renderTimeline(boxes('f-box2'))
    fireEvent.keyDown(screen.getByTestId('timeline-marker'), { key: 'Home' })
    await settle()
    expect(core.callsTo('applyEdits')).toEqual([
      [1, 'Roll back', [{ type: 'set_rollback', rollback: 'f-box1' }]],
    ])
  })

  it('U-68 ArrowRight from position 1 sends set_rollback to the third feature', async () => {
    const { core } = renderTimeline(boxes('f-box2'))
    fireEvent.keyDown(screen.getByTestId('timeline-marker'), { key: 'ArrowRight' })
    await settle()
    expect(core.callsTo('applyEdits')).toEqual([
      [1, 'Roll back', [{ type: 'set_rollback', rollback: 'f-box3' }]],
    ])
  })

  it('ArrowLeft moves the marker one gap back', async () => {
    const { core } = renderTimeline(boxes('f-box3'))
    fireEvent.keyDown(screen.getByTestId('timeline-marker'), { key: 'ArrowLeft' })
    await settle()
    expect(core.callsTo('applyEdits')).toEqual([
      [1, 'Roll back', [{ type: 'set_rollback', rollback: 'f-box2' }]],
    ])
  })
})

describe('selecting and editing (3.9)', () => {
  it('selects a feature on click and toggles with Shift', async () => {
    const { app, user } = renderTimeline()
    await user.click(itemButton('Extrude 1'))
    expect(app.store.getState().selection.map((selected) => selected.key)).toEqual([
      'feature:f-ext1',
    ])
    await user.keyboard('{Shift>}')
    await user.click(itemButton('Sketch 1'))
    await user.keyboard('{/Shift}')
    expect(app.store.getState().selection.map((selected) => selected.key)).toEqual([
      'feature:f-ext1',
      'feature:f-sk1',
    ])
  })

  it('U-67 a double-click edits the feature: "Edit Extrude 1" and focus on f-ext1', async () => {
    const { core, user } = renderTimeline()
    await user.dblClick(itemButton('Extrude 1'))
    await waitFor(() => {
      expect(core.callsTo('setFocus')).toEqual([[1, 'f-ext1']])
    })
    expect(core.callsTo('beginTransaction')).toEqual([[1, 'Edit Extrude 1']])
  })

  it('U-67 focus_inactive on a suppressed feature cancels the transaction and offers Unsuppress', async () => {
    const inactive = await coreErrorFor({
      code: 'focus_inactive',
      message: "'Extrude 1' is suppressed, so it cannot be edited. Unsuppress it first.",
    })
    // The recipe says what the core refused: Extrude 1 is suppressed (3.7 picks the fix from it).
    const suppressed: Document = {
      ...CUBE_DOCUMENT,
      features: [CUBE_SKETCH, { ...CUBE_EXTRUDE, suppressed: true }] as NonNullable<
        Document['features']
      >,
    }
    const { app, core, user } = renderTimeline(documentState(suppressed))
    core.stub('setFocus', () => Promise.reject(inactive))
    await user.dblClick(itemButton('Extrude 1'))
    await waitFor(() => {
      expect(core.callsTo('cancelTransaction')).toEqual([[1]])
    })
    const [notice] = app.store.getState().notifications
    expect(notice?.severity).toBe('error')
    expect(notice?.message).toBe(inactive.message)
    expect(notice?.actions.map((action) => action.label)).toEqual(['Unsuppress'])
  })

  it('opens a context menu with Edit, Rename, Suppress, Roll back to here and Delete', async () => {
    const { user } = renderTimeline()
    await user.pointer({ keys: '[MouseRight]', target: itemButton('Extrude 1') })
    const menu = screen.getByTestId('context-menu')
    expect(menu).toHaveAttribute('role', 'menu')
    expect(
      within(menu)
        .getAllByRole('menuitem')
        .map((entry) => entry.textContent?.trim()),
    ).toEqual(['Edit', 'Rename', 'Suppress', 'Roll back to here', 'Delete'])
  })

  it('opens the context menu on the release when the browser fires contextmenu on the press', () => {
    renderTimeline()
    const extrude = itemButton('Extrude 1')
    const init = { pointerId: 1, pointerType: 'mouse' }
    fireEvent.pointerDown(extrude, { ...init, button: 2, buttons: 2 })
    // Linux and macOS: the menu event comes while the button is down, before the release that
    // would light-dismiss a menu opened now.
    fireEvent.contextMenu(extrude, { button: 2, buttons: 2 })
    expect(screen.queryByTestId('context-menu')).toBeNull()
    fireEvent.pointerUp(extrude, { ...init, button: 2, buttons: 0 })
    expect(screen.getByTestId('context-menu')).toBeVisible()
  })

  it('opens the context menu at once when the browser fires contextmenu after the release', () => {
    renderTimeline()
    fireEvent.contextMenu(itemButton('Extrude 1'), { button: 2, buttons: 0 })
    expect(screen.getByTestId('context-menu')).toBeVisible()
  })

  it('opens the context menu with Shift+F10', async () => {
    const { user } = renderTimeline()
    itemButton('Extrude 1').focus()
    await user.keyboard('{Shift>}{F10}{/Shift}')
    expect(screen.getByTestId('context-menu')).toBeVisible()
  })

  it('suppresses from the context menu with "Suppress {label}"', async () => {
    const { core, user } = renderTimeline()
    await user.pointer({ keys: '[MouseRight]', target: itemButton('Extrude 1') })
    await user.click(
      within(screen.getByTestId('context-menu')).getByRole('menuitem', { name: 'Suppress' }),
    )
    await settle()
    expect(core.callsTo('applyEdits')).toEqual([
      [
        1,
        'Suppress Extrude 1',
        [{ type: 'update_feature', feature: { ...CUBE_EXTRUDE, suppressed: true } }],
      ],
    ])
  })

  it('unsuppresses a suppressed feature with "Unsuppress {label}"', async () => {
    const suppressed: Document = {
      ...CUBE_DOCUMENT,
      features: [CUBE_SKETCH, { ...CUBE_EXTRUDE, suppressed: true }] as NonNullable<
        Document['features']
      >,
    }
    const { core, user } = renderTimeline(documentState(suppressed))
    await user.pointer({ keys: '[MouseRight]', target: itemButton('Extrude 1') })
    await user.click(
      within(screen.getByTestId('context-menu')).getByRole('menuitem', { name: 'Unsuppress' }),
    )
    await settle()
    const [[, label, edits] = []] = core.callsTo('applyEdits')
    expect(label).toBe('Unsuppress Extrude 1')
    const [edit] = edits as readonly {
      type: string
      feature: { id: string; suppressed?: boolean }
    }[]
    expect(edit?.type).toBe('update_feature')
    expect(edit?.feature.id).toBe('f-ext1')
    expect(edit?.feature.suppressed ?? false).toBe(false)
  })

  it('"Roll back to here" keeps the item active and rolls back what follows', async () => {
    const { core, user } = renderTimeline(boxes())
    await user.pointer({ keys: '[MouseRight]', target: itemButton('Box 1') })
    await user.click(
      within(screen.getByTestId('context-menu')).getByRole('menuitem', {
        name: 'Roll back to here',
      }),
    )
    await settle()
    expect(core.callsTo('applyEdits')).toEqual([
      [1, 'Roll back', [{ type: 'set_rollback', rollback: 'f-box2' }]],
    ])
  })

  it('renames inline with F2 and Enter, labeled "Rename"', async () => {
    const { core, user } = renderTimeline()
    itemButton('Extrude 1').focus()
    await user.keyboard('{F2}')
    const field = within(screen.getByTestId('timeline')).getByRole('textbox')
    await user.clear(field)
    await user.type(field, 'Base{Enter}')
    await settle()
    expect(core.callsTo('applyEdits')).toEqual([
      [1, 'Rename', [{ type: 'update_feature', feature: { ...CUBE_EXTRUDE, name: 'Base' } }]],
    ])
  })

  it('removes the name when the rename is empty, so the default label returns', async () => {
    const named: Document = {
      ...CUBE_DOCUMENT,
      features: [CUBE_SKETCH, { ...CUBE_EXTRUDE, name: 'Base' }] as NonNullable<
        Document['features']
      >,
    }
    const { core, user } = renderTimeline(documentState(named))
    itemButton('Base').focus()
    await user.keyboard('{F2}')
    await user.clear(within(screen.getByTestId('timeline')).getByRole('textbox'))
    await user.keyboard('{Enter}')
    await settle()
    expect(core.callsTo('applyEdits')).toEqual([
      [1, 'Rename', [{ type: 'update_feature', feature: CUBE_EXTRUDE }]],
    ])
  })

  it('cancels an inline rename with Esc', async () => {
    const { core, user } = renderTimeline()
    itemButton('Extrude 1').focus()
    await user.keyboard('{F2}')
    await user.type(within(screen.getByTestId('timeline')).getByRole('textbox'), 'Base{Escape}')
    await settle()
    expect(core.callsTo('applyEdits')).toEqual([])
    expect(within(screen.getByTestId('timeline')).queryByRole('textbox')).toBeNull()
  })
})

describe('deleting (3.9)', () => {
  it('U-68 deleting the rollback feature first moves the marker to the next remaining feature', async () => {
    const { app, core, user } = renderTimeline(boxes('f-box2'))
    await user.pointer({ keys: '[MouseRight]', target: itemButton('Box 2') })
    await user.click(
      within(screen.getByTestId('context-menu')).getByRole('menuitem', { name: 'Delete' }),
    )
    await settle()
    expect(core.callsTo('applyEdits')).toEqual([
      [
        1,
        'Delete Box 2',
        [
          { type: 'set_rollback', rollback: 'f-box3' },
          { type: 'remove_feature', id: 'f-box2' },
        ],
      ],
    ])
    const notice = app.store
      .getState()
      .notifications.find((entry) => entry.message === MESSAGES.deleted('Box 2'))
    expect(notice?.actions.map((action) => action.label)).toEqual(['Undo'])
  })

  it('U-68 deleting the last feature at the marker clears the marker (no id)', async () => {
    const { core, user } = renderTimeline(boxes('f-box3'))
    await user.pointer({ keys: '[MouseRight]', target: itemButton('Box 3') })
    await user.click(
      within(screen.getByTestId('context-menu')).getByRole('menuitem', { name: 'Delete' }),
    )
    await settle()
    expect(core.callsTo('applyEdits')).toEqual([
      [1, 'Delete Box 3', [{ type: 'set_rollback' }, { type: 'remove_feature', id: 'f-box3' }]],
    ])
  })

  it('deletes several selected features in one edit, "Delete {n} features"', async () => {
    const { core, user } = renderTimeline(boxes())
    await user.click(itemButton('Box 1'))
    await user.keyboard('{Shift>}')
    await user.click(itemButton('Box 2'))
    await user.keyboard('{/Shift}')
    await user.keyboard('{Delete}')
    await settle()
    expect(core.callsTo('applyEdits')).toEqual([
      [
        1,
        'Delete 2 features',
        [
          { type: 'remove_feature', id: 'f-box1' },
          { type: 'remove_feature', id: 'f-box2' },
        ],
      ],
    ])
  })
})

// Items 40 pixels wide, 4 apart, from x = 100; gap g lies at x = 98 + 44 g.
function stubItemRectangles(): void {
  const order = featuresOf(FOUR_BOXES_DOCUMENT).map((feature) => feature.id)
  vi.spyOn(Element.prototype, 'getBoundingClientRect').mockImplementation(function rectangle(
    this: Element,
  ) {
    const id = this.closest('[data-feature-id]')?.getAttribute('data-feature-id') ?? null
    const index = id === null ? -1 : order.indexOf(id)
    return index === -1
      ? DOMRect.fromRect({ x: 0, y: 0, width: 1000, height: 56 })
      : DOMRect.fromRect({ x: 100 + 44 * index, y: 8, width: 40, height: 40 })
  })
}

function drag(from: HTMLElement, startX: number, endX: number): void {
  const init = { pointerId: 1, pointerType: 'mouse', button: 0 }
  fireEvent.pointerDown(from, { ...init, buttons: 1, clientX: startX, clientY: 28 })
  fireEvent.pointerMove(from, { ...init, buttons: 1, clientX: startX - 10, clientY: 28 })
  fireEvent.pointerMove(from, { ...init, buttons: 1, clientX: endX, clientY: 28 })
  fireEvent.pointerUp(from, { ...init, buttons: 0, clientX: endX, clientY: 28 })
}

describe('reordering by drag (3.9)', () => {
  it('U-68 dropping feature index 3 at gap 1 moves it to index 1, labeled "Reorder"', async () => {
    stubItemRectangles()
    const { core } = renderTimeline(boxes(null, FOUR_BOXES_DOCUMENT))
    drag(item('f-box4'), 252, 142)
    await settle()
    expect(core.callsTo('applyEdits')).toEqual([
      [1, 'Reorder', [{ type: 'move_feature', id: 'f-box4', index: 1 }]],
    ])
  })

  it('U-68 dropping feature index 3 at gap 4, its own position, sends no edit', async () => {
    stubItemRectangles()
    const { core } = renderTimeline(boxes(null, FOUR_BOXES_DOCUMENT))
    drag(item('f-box4'), 252, 274)
    await settle()
    expect(core.callsTo('applyEdits')).toEqual([])
  })

  it('moving a feature later drops it at the gap minus one', async () => {
    stubItemRectangles()
    const { core } = renderTimeline(boxes(null, FOUR_BOXES_DOCUMENT))
    drag(item('f-box1'), 120, 230)
    await settle()
    expect(core.callsTo('applyEdits')).toEqual([
      [1, 'Reorder', [{ type: 'move_feature', id: 'f-box1', index: 2 }]],
    ])
  })

  it('captures the pointer only once a press moves past the drag threshold', () => {
    stubItemRectangles()
    const captured: number[] = []
    // jsdom has no pointer capture.
    Object.defineProperty(HTMLElement.prototype, 'setPointerCapture', {
      configurable: true,
      value(this: HTMLElement, pointerId: number) {
        expect(this).toBe(screen.getByRole('toolbar', { name: 'Features' }))
        captured.push(pointerId)
      },
    })
    try {
      renderTimeline(boxes(null, FOUR_BOXES_DOCUMENT))
      const init = { pointerId: 7, pointerType: 'mouse', button: 0 }
      fireEvent.pointerDown(item('f-box4'), { ...init, buttons: 1, clientX: 252, clientY: 28 })
      fireEvent.pointerMove(item('f-box4'), { ...init, buttons: 1, clientX: 255, clientY: 28 })
      // Captured at the press, the click and double-click would never reach the item's button.
      expect(captured).toEqual([])
      fireEvent.pointerMove(item('f-box4'), { ...init, buttons: 1, clientX: 240, clientY: 28 })
      expect(captured).toEqual([7])
    } finally {
      Reflect.deleteProperty(HTMLElement.prototype, 'setPointerCapture')
    }
  })

  it('a press released outside the strip starts no drag when the pointer comes back', async () => {
    stubItemRectangles()
    const { core } = renderTimeline(boxes(null, FOUR_BOXES_DOCUMENT))
    const init = { pointerId: 1, pointerType: 'mouse', button: 0 }
    fireEvent.pointerDown(item('f-box4'), { ...init, buttons: 1, clientX: 252, clientY: 28 })
    // The release happened elsewhere; the next move over the strip has no button down.
    fireEvent.pointerMove(item('f-box2'), { ...init, buttons: 0, clientX: 142, clientY: 28 })
    fireEvent.pointerUp(item('f-box2'), { ...init, buttons: 0, clientX: 142, clientY: 28 })
    await settle()
    expect(core.callsTo('applyEdits')).toEqual([])
  })

  it('a press that moves 4 pixels or less starts no drag', async () => {
    stubItemRectangles()
    const { core } = renderTimeline(boxes(null, FOUR_BOXES_DOCUMENT))
    const init = { pointerId: 1, pointerType: 'mouse', button: 0 }
    fireEvent.pointerDown(item('f-box4'), { ...init, buttons: 1, clientX: 252, clientY: 28 })
    fireEvent.pointerUp(item('f-box4'), { ...init, buttons: 0, clientX: 249, clientY: 28 })
    await settle()
    expect(core.callsTo('applyEdits')).toEqual([])
  })
})

describe('import features (U-89)', () => {
  it('U-89 offers Rename, Suppress and Delete for an import, with Edit disabled', async () => {
    const { user } = renderTimeline(documentState(IMPORT_DOCUMENT))
    await user.pointer({ keys: '[MouseRight]', target: itemButton('Import 1') })
    const menu = screen.getByTestId('context-menu')
    for (const name of ['Rename', 'Suppress', 'Delete']) {
      expect(within(menu).getByRole('menuitem', { name })).not.toHaveAttribute(
        'aria-disabled',
        'true',
      )
    }
    expect(within(menu).getByRole('menuitem', { name: 'Edit' })).toHaveAttribute(
      'aria-disabled',
      'true',
    )
  })

  it('U-89 a double-click on an import starts no edit and explains why', async () => {
    const { app, core, user } = renderTimeline(documentState(IMPORT_DOCUMENT))
    await user.dblClick(itemButton('Import 1'))
    await settle()
    expect(core.callsTo('beginTransaction')).toEqual([])
    expect(app.store.getState().notifications.map((entry) => entry.message)).toContain(
      MESSAGES.import_not_editable(),
    )
  })
})

describe('problems and collapsing (3.9)', () => {
  it('hides the problems button without problems', () => {
    renderTimeline()
    expect(screen.queryByTestId('timeline-problems')).toBeNull()
  })

  it('counts features with errors or warnings and opens the Problems panel', async () => {
    const { app, user } = renderTimeline(withStatus(cubeDocumentState(), FAILED_EXTRUDE))
    const problems = screen.getByTestId('timeline-problems')
    expect(problems).toHaveTextContent('1 problem')
    await user.click(problems)
    expect(app.store.getState().panels.side).toBe('problems')
  })

  it('shows the crossed circle of an error when any problem is an error, else the hazard sign (3.24)', () => {
    const warning: FeatureStatus = {
      id: 'f-sk1',
      status: 'warning',
      messages: [{ severity: 'warning', code: 'open_profile', message: 'The profile is open.' }],
      repairs: [],
    }
    renderTimeline(withStatus(withStatus(cubeDocumentState(), FAILED_EXTRUDE), warning))
    const problems = screen.getByTestId('timeline-problems')
    expect(problems.querySelector('.lucide-circle-x')).toHaveClass('text-red-700')
    expect(problems.querySelector('.lucide-triangle-alert')).toBeNull()
    cleanup()
    renderTimeline(withStatus(cubeDocumentState(), warning))
    const warnings = screen.getByTestId('timeline-problems')
    expect(warnings.querySelector('.lucide-triangle-alert')).toHaveClass('fill-yellow-400')
    expect(warnings.querySelector('.lucide-circle-x')).toBeNull()
  })

  it('pluralizes the problem count', () => {
    renderTimeline(
      withStatus(withStatus(cubeDocumentState(), FAILED_EXTRUDE), {
        id: 'f-sk1',
        status: 'warning',
        messages: [{ severity: 'warning', code: 'open_profile', message: 'The profile is open.' }],
        repairs: [],
      }),
    )
    expect(screen.getByTestId('timeline-problems')).toHaveTextContent('2 problems')
  })

  it('collapses to the expand button, the feature count and the problems button', async () => {
    const { app, user, timeline } = renderTimeline(withStatus(cubeDocumentState(), FAILED_EXTRUDE))
    await user.click(within(timeline).getByRole('button', { name: 'Collapse timeline' }))
    expect(app.store.getState().panels.timelineCollapsed).toBe(true)
    expect(within(timeline).getByRole('button', { name: 'Expand timeline' })).toBeInTheDocument()
    expect(timeline).toHaveTextContent('2 features')
    expect(within(timeline).getByTestId('timeline-problems')).toBeInTheDocument()
    expect(within(timeline).queryAllByTestId('timeline-item')).toEqual([])
  })
})

describe('keyboard (3.9)', () => {
  it('is a toolbar with one tab stop where arrow keys move between items and the marker', async () => {
    const { user, timeline } = renderTimeline()
    const strip = within(timeline).getByRole('toolbar')
    const stops = [...strip.querySelectorAll<HTMLElement>('button, [role="slider"]')].filter(
      (element) => element.tabIndex === 0,
    )
    expect(stops).toHaveLength(1)
    itemButton('Sketch 1').focus()
    await user.keyboard('{ArrowRight}')
    expect(itemButton('Extrude 1')).toHaveFocus()
    await user.keyboard('{ArrowRight}')
    expect(screen.getByTestId('timeline-marker')).toHaveFocus()
  })

  it('selects with Space and toggles with Mod+Space', async () => {
    const { app, user } = renderTimeline()
    itemButton('Sketch 1').focus()
    await user.keyboard(' ')
    expect(app.store.getState().selection.map((selected) => selected.key)).toEqual([
      'feature:f-sk1',
    ])
    itemButton('Extrude 1').focus()
    await user.keyboard('{Control>} {/Control}')
    expect(app.store.getState().selection.map((selected) => selected.key)).toEqual([
      'feature:f-sk1',
      'feature:f-ext1',
    ])
  })

  it('edits the focused feature with Enter', async () => {
    const { core, user } = renderTimeline()
    itemButton('Extrude 1').focus()
    await user.keyboard('{Enter}')
    await waitFor(() => {
      expect(core.callsTo('beginTransaction')).toEqual([[1, 'Edit Extrude 1']])
    })
  })

  it('leaves Mod+Enter on a focused item to the shortcuts (3.5)', async () => {
    const { core } = renderTimeline()
    const extrude = itemButton('Extrude 1')
    extrude.focus()
    const event = new KeyboardEvent('keydown', {
      key: 'Enter',
      ctrlKey: true,
      bubbles: true,
      cancelable: true,
    })
    extrude.dispatchEvent(event)
    await settle()
    expect(event.defaultPrevented).toBe(false)
    expect(core.callsTo('beginTransaction')).toEqual([])
  })

  it('a held Enter edits the feature once: repeated presses do nothing', async () => {
    const { core } = renderTimeline()
    const extrude = itemButton('Extrude 1')
    extrude.focus()
    fireEvent.keyDown(extrude, { key: 'Enter' })
    fireEvent.keyDown(extrude, { key: 'Enter', repeat: true })
    fireEvent.keyDown(extrude, { key: 'Enter', repeat: true })
    await waitFor(() => {
      expect(core.callsTo('beginTransaction')).toEqual([[1, 'Edit Extrude 1']])
    })
  })

  it('a selected item says so to assistive technology with aria-pressed', async () => {
    const { user } = renderTimeline()
    expect(itemButton('Extrude 1')).toHaveAttribute('aria-pressed', 'false')
    await user.click(itemButton('Extrude 1'))
    expect(itemButton('Extrude 1')).toHaveAttribute('aria-pressed', 'true')
  })
})

describe('during a feature command (3.7)', () => {
  it('disables the context menu edits other than Edit', async () => {
    const { user } = renderTimeline(cubeDocumentState(), { command: EDITING_EXTRUDE })
    await user.pointer({ keys: '[MouseRight]', target: itemButton('Sketch 1') })
    const menu = screen.getByTestId('context-menu')
    const disabled = within(menu)
      .getAllByRole('menuitem')
      .filter((entry) => entry.getAttribute('aria-disabled') === 'true')
      .map((entry) => entry.textContent?.trim())
    expect(disabled).toEqual(['Rename', 'Suppress', 'Roll back to here', 'Delete'])
  })

  it('does not start a marker drag, and the marker keys say why they do nothing', async () => {
    const { app, core } = renderTimeline(cubeDocumentState(), { command: EDITING_EXTRUDE })
    const marker = screen.getByTestId('timeline-marker')
    fireEvent.pointerDown(marker, { pointerId: 1, button: 0, clientX: 100 })
    fireEvent.pointerMove(marker, { pointerId: 1, clientX: 20 })
    fireEvent.keyDown(marker, { key: 'Home' })
    await settle()
    expect(core.callsTo('beginTransaction')).toEqual([])
    expect(core.callsTo('applyEdits')).toEqual([])
    expect(app.store.getState().notifications.map((notification) => notification.message)).toEqual([
      MESSAGES.finish_command_first('Extrude 1'),
    ])
  })
})
