import { act, cleanup, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { installPopoverPolyfill } from '../components/popover-polyfill.testing'
import type { Document } from '../generated/protocol'
import { documentState, readyState } from '../layout/fixtures.testing'
import { createModelApp, renderWithApp, stubMatchMedia } from '../layout/test-app.testing'
import { MESSAGES } from '../messages'
import { requestUi } from '../layout/requests'
import { INITIAL_STATE, type AppState, type DocumentState } from '../state/store'
import { coreErrorFor } from '../test/fakes/core-api'
import { PARAMETRIC_DOCUMENT } from '../test/fakes/documents'
import { settle } from '../test/fakes/settle'
import { COLOR_CLASSES, FONT_CLASSES } from '../theme/roles'

beforeEach(installPopoverPolyfill)

beforeEach(() => {
  stubMatchMedia()
})

afterEach(() => {
  cleanup()
  vi.unstubAllGlobals()
})

const SIZE_PARAMETER = { id: 'p-size', name: 'size', expr: '20 mm' }

function parametric(
  overrides: Partial<DocumentState> = {},
  recipe: Document = PARAMETRIC_DOCUMENT,
): DocumentState {
  return documentState(recipe, {
    name: 'parametric',
    parameterValues: { 'p-size': { id: 'p-size', value: 20, dimension: { length: 1, angle: 0 } } },
    ...overrides,
  })
}

function renderPanel(document: DocumentState = parametric()) {
  const test = createModelApp({
    state: readyState(document, {
      panels: { ...INITIAL_STATE.panels, browserOpen: true, side: 'parameters' },
    }),
  })
  renderWithApp(test.app)
  const panel = screen.getByTestId('parameters-panel')
  return { ...test, user: userEvent.setup(), panel }
}

function sizeRow(): HTMLElement {
  const found = screen
    .getAllByTestId('parameter-row')
    .find((element) => element.getAttribute('data-id') === 'p-size')
  if (found === undefined) {
    throw new Error('No row for p-size')
  }
  return found
}

describe('the Parameters panel (3.11)', () => {
  it('is titled "Parameters" with one row per parameter', () => {
    const { panel } = renderPanel()
    expect(panel).toHaveTextContent('Parameters')
    expect(
      screen.getAllByTestId('parameter-row').map((element) => element.getAttribute('data-id')),
    ).toEqual(['p-size'])
  })

  it('with no parameters, explains them above "Add parameter" in the muted text role', () => {
    const { panel } = renderPanel(parametric({}, { ...PARAMETRIC_DOCUMENT, parameters: [] }))
    const empty = within(panel).getByTestId('parameters-empty')
    expect(empty).toHaveTextContent(
      "Parameters are named values, such as wall = 2 mm. Type a parameter's name in any size or angle field to use it.",
    )
    expect(empty).toHaveClass(COLOR_CLASSES.textMuted.foreground)
    const add = within(panel).getByRole('button', { name: 'Add parameter' })
    expect(empty.compareDocumentPosition(add) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy()
  })

  it('with parameters, shows no explanation', () => {
    const { panel } = renderPanel()
    expect(within(panel).queryByTestId('parameters-empty')).toBeNull()
  })

  it('has name, expression and comment fields and a delete button named after the parameter', () => {
    renderPanel()
    const row = sizeRow()
    expect(within(row).getByRole('textbox', { name: 'Name of size' })).toHaveValue('size')
    expect(within(row).getByRole('combobox', { name: 'Expression of size' })).toHaveValue('20 mm')
    expect(within(row).getByRole('textbox', { name: 'Comment of size' })).toHaveValue('')
    expect(within(row).getByRole('button', { name: 'Delete size' })).toBeInTheDocument()
  })

  it('shows the recomputed value formatted by its dimension, in the values font', () => {
    renderPanel()
    const value = within(sizeRow()).getByTestId('parameter-value')
    expect(value).toHaveTextContent('20 mm')
    expect(value).toHaveClass(FONT_CLASSES.values)
  })

  it('shows the evaluation error instead of a value, in the error text role', () => {
    renderPanel(
      parametric({ parameterValues: { 'p-size': { id: 'p-size', error: "Unknown name 'wal'." } } }),
    )
    const value = within(sizeRow()).getByTestId('parameter-value')
    expect(value).toHaveTextContent("Unknown name 'wal'.")
    const carriers = [value, ...value.querySelectorAll('*')]
    const roleClasses = COLOR_CLASSES.errorText.foreground.split(' ')
    expect(
      carriers.some((element) => roleClasses.every((name) => element.classList.contains(name))),
    ).toBe(true)
    expect(value.querySelector('.lucide-circle-x')).not.toBeNull()
  })

  it('formats other dimensions in internal units with their exponents', () => {
    renderPanel(
      parametric({
        parameterValues: {
          'p-size': { id: 'p-size', value: 6, dimension: { length: 2, angle: 0 } },
        },
      }),
    )
    expect(within(sizeRow()).getByTestId('parameter-value')).toHaveTextContent('6 mm^2')
  })

  it('U-85 committing a new name calls renameParameter and sends no update_parameter', async () => {
    const { core, user } = renderPanel()
    const name = within(sizeRow()).getByRole('textbox', { name: 'Name of size' })
    await user.clear(name)
    await user.type(name, 'width{Enter}')
    await settle()
    expect(core.callsTo('renameParameter')).toEqual([[1, 'p-size', 'width']])
    expect(core.callsTo('applyEdits')).toEqual([])
  })

  it('renames on blur too, and only when the name changed', async () => {
    const { core, user } = renderPanel()
    const name = within(sizeRow()).getByRole('textbox', { name: 'Name of size' })
    await user.click(name)
    await user.tab()
    await settle()
    expect(core.callsTo('renameParameter')).toEqual([])
    await user.clear(name)
    await user.type(name, 'width')
    await user.tab()
    await settle()
    expect(core.callsTo('renameParameter')).toEqual([[1, 'p-size', 'width']])
  })

  it('refuses a name that breaks the name rules without sending it', async () => {
    const { core, user } = renderPanel()
    const name = within(sizeRow()).getByRole('textbox', { name: 'Name of size' })
    await user.clear(name)
    await user.type(name, '2x{Enter}')
    await settle()
    expect(sizeRow()).toHaveTextContent(MESSAGES.parameter_name())
    expect(core.callsTo('renameParameter')).toEqual([])
  })

  it('refuses a name longer than 64 characters', async () => {
    const { core, user } = renderPanel()
    const name = within(sizeRow()).getByRole('textbox', { name: 'Name of size' })
    await user.clear(name)
    await user.type(name, `${'a'.repeat(65)}{Enter}`)
    await settle()
    expect(sizeRow()).toHaveTextContent(MESSAGES.parameter_name())
    expect(core.callsTo('renameParameter')).toEqual([])
  })

  it('shows the core’s refusal of a name under the field', async () => {
    const taken = await coreErrorFor({
      code: 'edit_rejected',
      message: "The name 'pi' is reserved.",
    })
    const { core, user } = renderPanel()
    core.stub('renameParameter', () => Promise.reject(taken))
    const name = within(sizeRow()).getByRole('textbox', { name: 'Name of size' })
    await user.clear(name)
    await user.type(name, 'pi{Enter}')
    await waitFor(() => {
      expect(sizeRow()).toHaveTextContent("The name 'pi' is reserved.")
    })
  })

  it('reverts the name with Esc', async () => {
    const { core, user } = renderPanel()
    const name = within(sizeRow()).getByRole('textbox', { name: 'Name of size' })
    await user.clear(name)
    await user.type(name, 'width{Escape}')
    await settle()
    expect(name).toHaveValue('size')
    expect(core.callsTo('renameParameter')).toEqual([])
  })

  it('commits a changed valid expression with "Change {name}"', async () => {
    const { core, user } = renderPanel()
    const expression = within(sizeRow()).getByRole('combobox', { name: 'Expression of size' })
    await user.clear(expression)
    await user.type(expression, '25 mm')
    await waitFor(() => {
      expect(within(sizeRow()).getByTestId('expression-result')).toHaveTextContent('= 25 mm')
    })
    await user.keyboard('{Enter}')
    await settle()
    expect(core.callsTo('applyEdits')).toEqual([
      [
        1,
        'Change size',
        [{ type: 'update_parameter', parameter: { ...SIZE_PARAMETER, expr: '25 mm' } }],
      ],
    ])
  })

  it('reverts the expression with Esc', async () => {
    const { core, user } = renderPanel()
    const expression = within(sizeRow()).getByRole('combobox', { name: 'Expression of size' })
    await user.clear(expression)
    await user.type(expression, '25 mm{Escape}')
    await settle()
    expect(expression).toHaveValue('20 mm')
    expect(core.callsTo('applyEdits')).toEqual([])
  })

  it('commits a comment with "Change comment of {name}" and removes an empty one', async () => {
    const { core, user } = renderPanel()
    const comment = within(sizeRow()).getByRole('textbox', { name: 'Comment of size' })
    await user.type(comment, 'Outer size{Enter}')
    await settle()
    expect(core.callsTo('applyEdits')).toEqual([
      [
        1,
        'Change comment of size',
        [{ type: 'update_parameter', parameter: { ...SIZE_PARAMETER, comment: 'Outer size' } }],
      ],
    ])
    cleanup()
    const commented: Document = {
      ...PARAMETRIC_DOCUMENT,
      parameters: [{ ...SIZE_PARAMETER, comment: 'Outer size' }],
    }
    const second = renderPanel(parametric({}, commented))
    const field = within(sizeRow()).getByRole('textbox', { name: 'Comment of size' })
    await second.user.clear(field)
    await second.user.keyboard('{Enter}')
    await settle()
    expect(second.core.callsTo('applyEdits')).toEqual([
      [1, 'Change comment of size', [{ type: 'update_parameter', parameter: SIZE_PARAMETER }]],
    ])
  })

  it('adds a parameter from a draft row: name, Enter, expression, Enter', async () => {
    const { core, user, panel } = renderPanel()
    await user.click(within(panel).getByRole('button', { name: 'Add parameter' }))
    const draftName = document.activeElement as HTMLInputElement
    expect(draftName).toHaveValue('')
    await user.type(draftName, 'depth{Enter}')
    const draftExpression = document.activeElement as HTMLInputElement
    expect(draftExpression).not.toBe(draftName)
    expect(draftExpression).toHaveValue('')
    await user.type(draftExpression, '5 mm')
    await waitFor(() => {
      expect(
        screen
          .getAllByTestId('expression-result')
          .some((result) => result.textContent?.includes('= 5 mm')),
      ).toBe(true)
    })
    await user.keyboard('{Enter}')
    await settle()
    expect(core.callsTo('applyEdits')).toEqual([
      [
        1,
        'Add parameter depth',
        [
          {
            type: 'add_parameter',
            index: 1,
            parameter: {
              id: expect.stringMatching(/^p-[0-9a-f]{12}$/),
              name: 'depth',
              expr: '5 mm',
            },
          },
        ],
      ],
    ])
  })

  it('discards the draft row with Esc', async () => {
    const { core, user, panel } = renderPanel()
    await user.click(within(panel).getByRole('button', { name: 'Add parameter' }))
    await user.type(document.activeElement as HTMLInputElement, 'depth{Escape}')
    await settle()
    expect(screen.getAllByTestId('parameter-row')).toHaveLength(1)
    expect(core.callsTo('applyEdits')).toEqual([])
  })

  it('deletes a parameter with "Delete parameter {name}"', async () => {
    const { core, user } = renderPanel()
    await user.click(within(sizeRow()).getByRole('button', { name: 'Delete size' }))
    await settle()
    expect(core.callsTo('applyEdits')).toEqual([
      [1, 'Delete parameter size', [{ type: 'remove_parameter', id: 'p-size' }]],
    ])
  })

  it('closes with its close button', async () => {
    const { app, user, panel } = renderPanel()
    await user.click(within(panel).getByRole('button', { name: /^Close/ }))
    expect(app.store.getState().panels.side).toBeNull()
  })
})

describe('the Parameters panel during a command and from a field (3.7)', () => {
  const EXTRUDE: NonNullable<AppState['command']> = {
    featureType: 'extrude',
    featureId: 'f-ext9',
    editing: false,
    title: 'Extrude',
    params: { extent: { kind: 'distance', expr: 'wall' }, op: 'new' },
    activeField: null,
    fieldErrors: {},
    applied: false,
    newBody: null,
    helper: null,
    changedPaths: [],
    history: [],
    future: [],
    preview: 'none',
  }

  it('says under its title that changes are part of the open command', () => {
    const test = createModelApp({
      state: readyState(parametric(), {
        command: EXTRUDE,
        panels: { ...INITIAL_STATE.panels, side: 'parameters' },
      }),
    })
    renderWithApp(test.app)
    expect(screen.getByTestId('parameters-transaction')).toHaveTextContent(
      'Changes are part of Extrude until you press OK.',
    )
    expect(screen.getByTestId('parameters-transaction')).toHaveClass(
      COLOR_CLASSES.textMuted.foreground,
    )
  })

  it('shows no transaction line without a command', () => {
    renderPanel()
    expect(screen.queryByTestId('parameters-transaction')).toBeNull()
  })

  it('opens a draft named after a field’s request, adds it on Enter and gives focus back', async () => {
    const { app, core, user } = renderPanel()
    const field = document.createElement('input')
    field.id = 'asking-field'
    document.body.append(field)
    act(() => {
      requestUi(app, {
        kind: 'parameter_draft',
        name: 'wall',
        expression: '2 mm',
        returnFocus: 'asking-field',
      })
    })
    const draft = screen.getByTestId('parameter-draft')
    expect(within(draft).getByRole('textbox', { name: 'Name of the new parameter' })).toHaveValue(
      'wall',
    )
    const expression = within(draft).getByRole('combobox', {
      name: 'Expression of the new parameter',
    })
    expect(expression).toHaveValue('2 mm')
    expect(expression).toHaveFocus()
    await waitFor(() => {
      expect(within(draft).getByTestId('expression-result')).toHaveTextContent('= 2 mm')
    })
    await user.keyboard('{Enter}')
    await waitFor(() => {
      expect(screen.queryByTestId('parameter-draft')).toBeNull()
    })
    expect(core.callsTo('applyEdits')[0]?.[1]).toBe('Add parameter wall')
    expect(field).toHaveFocus()
    field.remove()
  })

  it('discards the draft with Esc and gives focus back', async () => {
    const { app, user } = renderPanel()
    const field = document.createElement('input')
    field.id = 'asking-field'
    document.body.append(field)
    act(() => {
      requestUi(app, {
        kind: 'parameter_draft',
        name: 'wall',
        expression: '',
        returnFocus: 'asking-field',
      })
    })
    await user.keyboard('{Escape}')
    expect(screen.queryByTestId('parameter-draft')).toBeNull()
    expect(field).toHaveFocus()
    field.remove()
  })
})
