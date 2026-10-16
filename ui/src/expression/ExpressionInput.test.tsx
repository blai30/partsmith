import { act, fireEvent, renderHook, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { useState, type ReactNode } from 'react'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { AppProvider } from '../app/AppProvider'
import type { App } from '../app/createApp'
import { installPopoverPolyfill, isPopoverOpen } from '../components/popover-polyfill.testing'
import type { Dimension } from '../generated/protocol'
import { cubeDocumentState, readyState } from '../layout/fixtures.testing'
import { createModelApp, renderWithApp } from '../layout/test-app.testing'
import { coreErrorFor, type FakeCoreApi } from '../test/fakes/core-api'
import { deferred, type Deferred } from '../test/fakes/deferred'
import { settle } from '../test/fakes/settle'
import { hasTabularFigures, isCode, NO_CONTEXTUAL_LIGATURES } from '../test/type-roles'
import { ExpressionInput, type ExpressionInputProps, type ExpressionState } from './ExpressionInput'
import { useExpression } from './useExpression'

// The name suggestions are a popover in the top layer (2.13).
beforeEach(installPopoverPolyfill)

const LENGTH: Dimension = { length: 1, angle: 0 }

type Evaluation = { value: number; dimension: Dimension }

type HarnessProps = Partial<Omit<ExpressionInputProps, 'value'>> & { readonly initial: string }

// A controlled ExpressionInput that keeps the typed text, as its owners do.
function Harness({ initial, onChange, ...rest }: HarnessProps) {
  const [value, setValue] = useState(initial)
  return (
    <ExpressionInput
      id="distance"
      label="Distance"
      unit="length"
      {...rest}
      value={value}
      onChange={(next, state) => {
        setValue(next)
        onChange?.(next, state)
      }}
    />
  )
}

function setUp(props: HarnessProps, prepare: (core: FakeCoreApi) => void = () => undefined) {
  const test = createModelApp({ state: readyState(cubeDocumentState()) })
  prepare(test.core)
  renderWithApp(test.app, <Harness {...props} />)
  const input = screen.getByRole<HTMLInputElement>('combobox', { name: 'Distance' })
  return { ...test, input, user: userEvent.setup() }
}

/** Makes `evaluate` reject every expression with this protocol error. */
async function rejectEvaluations(
  core: FakeCoreApi,
  error: {
    readonly message: string
    readonly span?: { readonly start: number; readonly end: number }
  },
): Promise<void> {
  const coreError = await coreErrorFor({ code: 'expression_error', ...error })
  core.stub('evaluate', () => Promise.reject(coreError))
}

function evaluations(core: FakeCoreApi): readonly unknown[] {
  return core.callsTo('evaluate').map(([, expression]) => expression)
}

describe('ExpressionInput', () => {
  it('U-42 typing 2 * 5 shows = 10 mm', async () => {
    const { input, user } = setUp({ initial: '' })
    await user.type(input, '2 * 5')
    await waitFor(() => {
      expect(screen.getByTestId('expression-result')).toHaveTextContent('= 10 mm')
    })
  })

  it('U-42 typing 10 shows the unit symbol mm', async () => {
    const { input, user } = setUp({ initial: '' })
    await user.type(input, '10')
    await waitFor(() => {
      expect(screen.getByTestId('expression-result')).toHaveTextContent(/^mm$/)
    })
  })

  it('U-42 a rejected evaluation shows the core message in field-error', async () => {
    const { input, user, core } = setUp({ initial: '' })
    await rejectEvaluations(core, { message: "Unknown name 'wal'." })
    await user.type(input, 'wal')
    await waitFor(() => {
      expect(screen.getByTestId('field-error')).toHaveTextContent("Unknown name 'wal'.")
    })
  })

  it('references field-error from the input with aria-describedby', async () => {
    const { input } = setUp({ initial: '' })
    await settle()
    const fieldError = screen.getByTestId('field-error')
    expect(fieldError.id).not.toBe('')
    expect(input.getAttribute('aria-describedby')?.split(' ')).toContain(fieldError.id)
    expect(fieldError).toHaveAttribute('aria-live', 'polite')
  })

  it('U-42 shows the result for 12 when the result for 1 arrives after it', async () => {
    const { input, user, core } = setUp({ initial: '' })
    const pending = new Map<string, Deferred<Evaluation>>()
    core.stub('evaluate', (_documentHandle, expression) => {
      const answer = deferred<Evaluation>()
      pending.set(expression, answer)
      return answer.promise
    })
    const twelveFails = await coreErrorFor({
      code: 'expression_error',
      message: 'Twelve does not fit here.',
    })
    await user.type(input, '1')
    await waitFor(() => {
      expect(pending.has('1')).toBe(true)
    })
    await user.type(input, '2')
    await settle()
    // Both may be in flight (12 answers first, then the stale 1), or 12 may wait until 1 settles.
    const bothInFlight = pending.has('12')
    if (bothInFlight) {
      pending.get('12')?.reject(twelveFails)
      await settle()
    }
    pending.get('1')?.resolve({ value: 1, dimension: LENGTH })
    await settle()
    pending.get('12')?.reject(twelveFails)
    await settle()
    expect(screen.getByTestId('field-error')).toHaveTextContent('Twelve does not fit here.')
  })

  it('U-43 Enter calls onCommit in the valid state', async () => {
    const onCommit = vi.fn<(value: string, state: ExpressionState) => void>()
    const { input, user } = setUp({ initial: '10 mm', onCommit })
    await waitFor(() => {
      expect(screen.getByTestId('expression-result')).toHaveTextContent('= 10 mm')
    })
    await user.click(input)
    await user.keyboard('{Enter}')
    expect(onCommit).toHaveBeenCalledWith('10 mm', { state: 'valid', value: 10, dimension: LENGTH })
  })

  it('a held Enter commits once: repeated presses call no onCommit', async () => {
    const onCommit = vi.fn<(value: string, state: ExpressionState) => void>()
    const { input, user } = setUp({ initial: '10 mm', onCommit })
    await waitFor(() => {
      expect(screen.getByTestId('expression-result')).toHaveTextContent('= 10 mm')
    })
    await user.click(input)
    fireEvent.keyDown(input, { key: 'Enter' })
    fireEvent.keyDown(input, { key: 'Enter', repeat: true })
    expect(onCommit).toHaveBeenCalledTimes(1)
  })

  it('U-43 Enter calls no onCommit in the invalid state', async () => {
    const onCommit = vi.fn<(value: string, state: ExpressionState) => void>()
    const { input, user } = setUp({ initial: 'wal', onCommit })
    await waitFor(() => {
      expect(screen.getByTestId('field-error')).toHaveTextContent("Unknown name 'wal'.")
    })
    await user.click(input)
    await user.keyboard('{Enter}')
    expect(onCommit).not.toHaveBeenCalled()
  })

  it('Enter calls no onCommit while the evaluation is pending', async () => {
    const onCommit = vi.fn<(value: string, state: ExpressionState) => void>()
    const { input, user } = setUp({ initial: '10 mm', onCommit }, (core) => {
      core.stub('evaluate', () => deferred<Evaluation>().promise)
    })
    await user.click(input)
    await user.keyboard('{Enter}')
    expect(onCommit).not.toHaveBeenCalled()
  })

  it('Enter pressed while the evaluation is pending commits once the result arrives', async () => {
    const onCommit = vi.fn<(value: string, state: ExpressionState) => void>()
    const result = deferred<Evaluation>()
    const { input, user } = setUp({ initial: '25', onCommit }, (core) => {
      core.stub('evaluate', () => result.promise)
    })
    await user.click(input)
    await user.keyboard('{Enter}')
    expect(onCommit).not.toHaveBeenCalled()
    await act(async () => {
      result.resolve({ value: 25, dimension: { length: 0, angle: 0 } })
      await settle()
    })
    expect(onCommit).toHaveBeenCalledTimes(1)
    expect(onCommit).toHaveBeenCalledWith('25', { state: 'valid', value: 25, dimension: LENGTH })
  })

  it('Enter pressed while pending is dropped when the text changes before the result', async () => {
    const onCommit = vi.fn<(value: string, state: ExpressionState) => void>()
    const results: Deferred<Evaluation>[] = []
    const { input, user } = setUp({ initial: '25', onCommit }, (core) => {
      core.stub('evaluate', () => {
        const result = deferred<Evaluation>()
        results.push(result)
        return result.promise
      })
    })
    await user.click(input)
    await user.keyboard('{Enter}0')
    await act(async () => {
      for (const result of results) {
        result.resolve({ value: 250, dimension: { length: 0, angle: 0 } })
      }
      await settle()
    })
    expect(input).toHaveValue('250')
    expect(onCommit).not.toHaveBeenCalled()
  })

  it('Enter pressed while pending on an expression that fails selects the offending range', async () => {
    const onCommit = vi.fn<(value: string, state: ExpressionState) => void>()
    const result = deferred<Evaluation>()
    const { input, user } = setUp({ initial: '2 * x', onCommit }, (core) => {
      core.stub('evaluate', () => result.promise)
    })
    const coreError = await coreErrorFor({
      code: 'expression_error',
      message: "Unknown name 'x'.",
      span: { start: 4, end: 5 },
    })
    await user.click(input)
    await user.keyboard('{Enter}')
    await act(async () => {
      result.reject(coreError)
      await settle()
    })
    expect(onCommit).not.toHaveBeenCalled()
    expect(input.selectionStart).toBe(4)
    expect(input.selectionEnd).toBe(5)
  })

  it('U-43 Esc calls onCancel', async () => {
    const onCancel = vi.fn<() => void>()
    const { input, user } = setUp({ initial: '10 mm', onCancel })
    await user.click(input)
    await user.keyboard('{Escape}')
    expect(onCancel).toHaveBeenCalledTimes(1)
  })

  it('U-43 ArrowUp on 10 changes the value to 11', async () => {
    const onChange = vi.fn<(value: string, state: ExpressionState) => void>()
    const { input, user } = setUp({ initial: '10', onChange })
    await user.click(input)
    await user.keyboard('{ArrowUp}')
    expect(input).toHaveValue('11')
    expect(onChange).toHaveBeenCalledWith('11', expect.anything())
  })

  it('ArrowDown steps down and Shift steps coarse', async () => {
    const { input, user } = setUp({ initial: '2.5 mm' })
    await user.click(input)
    await user.keyboard('{Shift>}{ArrowUp}{/Shift}')
    expect(input).toHaveValue('12.5 mm')
    await user.keyboard('{ArrowDown}')
    expect(input).toHaveValue('11.5 mm')
  })

  it('ArrowUp does nothing for an expression that is not a plain number', async () => {
    const { input, user } = setUp({ initial: '2 * wall' })
    await user.click(input)
    await user.keyboard('{ArrowUp}')
    expect(input).toHaveValue('2 * wall')
  })

  it('U-92 Enter on an error with a span selects its UTF-16 range and keeps focus', async () => {
    const { input, user, core } = setUp({ initial: '' })
    await rejectEvaluations(core, { message: "Unknown name 'x'.", span: { start: 9, end: 10 } })
    await user.type(input, '2 * Ø + x')
    await waitFor(() => {
      expect(screen.getByTestId('field-error')).toHaveTextContent("Unknown name 'x'.")
    })
    await user.keyboard('{Enter}')
    expect(input).toHaveFocus()
    expect(input.selectionStart).toBe(8)
    expect(input.selectionEnd).toBe(9)
    expect(screen.getByTestId('field-error')).toHaveTextContent("Unknown name 'x'.")
  })

  it('shows "{label} needs a value." for an empty required field', async () => {
    const { input, user } = setUp({ initial: '10 mm' })
    await user.clear(input)
    await waitFor(() => {
      expect(screen.getByTestId('field-error')).toHaveTextContent('Distance needs a value.')
    })
  })

  it('shows no error for an empty optional field', async () => {
    const { input, user } = setUp({ initial: '10 mm', optional: true })
    await user.clear(input)
    await settle()
    expect(screen.getByTestId('field-error').textContent).toBe('')
  })

  it('shows no result while the text is empty, hidden so it takes no room in the row', async () => {
    setUp({ initial: '', optional: true })
    await settle()
    const result = screen.getByTestId('expression-result')
    expect(result.textContent).toBe('')
    expect(result).toHaveClass('empty:hidden')
    expect(result.childNodes).toHaveLength(0)
  })

  it('explains a value of the wrong dimension for its unit', async () => {
    const { input, user } = setUp({ initial: '' })
    await user.type(input, '10 deg')
    await waitFor(() => {
      expect(screen.getByTestId('field-error')).toHaveTextContent(
        "Distance must be a length, but '10 deg' is an angle. Use a value such as 10 mm.",
      )
    })
  })

  it('with unit any keeps the returned dimension and formats it', async () => {
    const { input, user } = setUp({ initial: '', unit: 'any' })
    await user.type(input, '3 mm * 2 mm')
    await waitFor(() => {
      expect(screen.getByTestId('expression-result')).toHaveTextContent('= 6 mm^2')
    })
    await user.clear(input)
    await user.type(input, '2 * 45 deg')
    await waitFor(() => {
      expect(screen.getByTestId('expression-result')).toHaveTextContent('= 90°')
    })
  })

  it('evaluates against the open document', async () => {
    const { input, user, core } = setUp({ initial: '' })
    await user.type(input, '5 mm')
    await waitFor(() => {
      expect(evaluations(core)).toContain('5 mm')
    })
    expect(core.callsTo('evaluate').every(([documentHandle]) => documentHandle === 1)).toBe(true)
  })
})

function wrapperFor(app: App) {
  return function Wrapper({ children }: { readonly children: ReactNode }) {
    return <AppProvider app={app}>{children}</AppProvider>
  }
}

describe('useExpression', () => {
  it('is invalid with "{label} needs a value." for empty text', () => {
    const { app } = createModelApp({ state: readyState(cubeDocumentState()) })
    const { result } = renderHook(() => useExpression('', 'length', 'Distance'), {
      wrapper: wrapperFor(app),
    })
    expect(result.current).toEqual({
      state: 'invalid',
      message: 'Distance needs a value.',
      range: null,
    })
  })

  it('is pending while the core evaluates, then valid with the converted value', async () => {
    const { app } = createModelApp({ state: readyState(cubeDocumentState()) })
    const { result } = renderHook(() => useExpression('2 * 5', 'length', 'Distance'), {
      wrapper: wrapperFor(app),
    })
    expect(result.current).toEqual({ state: 'pending' })
    await waitFor(() => {
      expect(result.current).toEqual({ state: 'valid', value: 10, dimension: LENGTH })
    })
  })

  it('evaluates again when the document revision changes', async () => {
    const { app, core } = createModelApp({ state: readyState(cubeDocumentState()) })
    renderHook(() => useExpression('size', 'length', 'Distance'), { wrapper: wrapperFor(app) })
    await waitFor(() => {
      expect(evaluations(core)).toEqual(['size'])
    })
    act(() => {
      const document = app.store.getState().document
      app.store.setState({ document: document && { ...document, revision: document.revision + 1 } })
    })
    await waitFor(() => {
      expect(evaluations(core)).toEqual(['size', 'size'])
    })
  })
})

/** cube.ptsm with the parameters `size` = 20 mm and `slot` = 3 mm. */
function withParameters() {
  const document = cubeDocumentState()
  return {
    ...document,
    recipe: {
      ...document.recipe,
      parameters: [
        { id: 'p-size', name: 'size', expr: '20 mm' },
        { id: 'p-slot', name: 'slot', expr: '3 mm' },
      ],
    },
    parameterValues: {
      'p-size': { id: 'p-size', value: 20, dimension: LENGTH },
      'p-slot': { id: 'p-slot', value: 3, dimension: LENGTH },
    },
  }
}

/** The texts of the open suggestion list's options. */
function options(): string[] {
  return within(screen.getByTestId('expression-suggestions'))
    .getAllByRole('option')
    .map((option) => option.textContent ?? '')
}

/** An ExpressionInput in a document with `size` and `slot`, whose evaluate answers in mm. */
function setUpNames(props: Partial<HarnessProps> = {}) {
  const test = createModelApp({ state: readyState(withParameters()) })
  test.core.stub('evaluate', (_handle, expression) =>
    Promise.resolve({ value: expression === 'size' ? 20 : 10, dimension: LENGTH }),
  )
  const onCommit = vi.fn<(value: string, state: ExpressionState) => void>()
  const onCancel = vi.fn<() => void>()
  renderWithApp(test.app, <Harness initial="" onCommit={onCommit} onCancel={onCancel} {...props} />)
  const input = screen.getByRole<HTMLInputElement>('combobox', { name: 'Distance' })
  return { ...test, input, onCommit, onCancel, user: userEvent.setup() }
}

describe('parameter name completion (2.13, U-118)', () => {
  it('U-118 typing si lists size first with no option active, and Tab inserts it without committing', async () => {
    const { input, user, onCommit } = setUpNames()
    await user.type(input, 'si')
    expect(options()[0]).toBe('size = 20 mm')
    // U-188: the list is a popup.
    expect(screen.getByTestId('expression-suggestions')).toHaveClass(
      'rounded-lg',
      'border',
      'shadow-lg',
    )
    expect(input).toHaveAttribute('aria-expanded', 'true')
    expect(input).not.toHaveAttribute('aria-activedescendant')
    await user.keyboard('{Tab}')
    expect(input).toHaveValue('size')
    expect(input).toHaveFocus()
    expect(screen.queryByTestId('expression-suggestions')).toBeNull()
    expect(onCommit).not.toHaveBeenCalled()
  })

  it('U-118 a full name opens nothing, so Enter commits at once', async () => {
    const { input, user, onCommit } = setUpNames()
    await user.type(input, 'size')
    expect(screen.queryByTestId('expression-suggestions')).toBeNull()
    await user.keyboard('{Enter}')
    await waitFor(() => {
      expect(onCommit).toHaveBeenCalledWith('size', expect.anything())
    })
  })

  it('U-118 ArrowDown then Enter inserts the chosen name without committing', async () => {
    const { input, user, onCommit } = setUpNames()
    await user.type(input, 'si')
    await user.keyboard('{ArrowDown}')
    expect(input.getAttribute('aria-activedescendant')).not.toBeNull()
    await user.keyboard('{Enter}')
    expect(input).toHaveValue('size')
    await settle()
    expect(onCommit).not.toHaveBeenCalled()
  })

  it('U-118 numbers with units and pi open nothing, and ArrowUp still steps a number', async () => {
    const { input, user } = setUpNames()
    await user.type(input, '10')
    expect(screen.queryByTestId('expression-suggestions')).toBeNull()
    await user.keyboard('{ArrowUp}')
    expect(input).toHaveValue('11')
    await user.clear(input)
    await user.type(input, '1 in')
    expect(screen.queryByTestId('expression-suggestions')).toBeNull()
    await user.clear(input)
    await user.type(input, '2 * pi')
    expect(screen.queryByTestId('expression-suggestions')).toBeNull()
  })

  it('U-118 Esc closes an open list without calling onCancel', async () => {
    const { input, user, onCancel } = setUpNames()
    await user.type(input, 'sl')
    expect(options()).toContain('slot = 3 mm')
    await user.keyboard('{Escape}')
    expect(screen.queryByTestId('expression-suggestions')).toBeNull()
    expect(onCancel).not.toHaveBeenCalled()
    expect(input).toHaveValue('sl')
  })

  it('shows the list as a popover in the top layer, placed at the input, so no panel cuts it off', async () => {
    const { input, user } = setUpNames()
    await user.type(input, 'sl')
    const list = screen.getByTestId('expression-suggestions')
    expect(list).toHaveAttribute('popover', 'manual')
    expect(isPopoverOpen(list)).toBe(true)
    expect(list).toHaveAttribute('data-positioned')
    await user.keyboard('{Escape}')
    expect(isPopoverOpen(list)).toBe(false)
  })

  it('U-118 lists printer.nozzle first for printer.n with an FDM profile', async () => {
    const { input, user } = setUpNames()
    await user.type(input, 'printer.n')
    expect(options()[0]).toBe('printer.nozzle')
  })

  it('inserts a clicked option in place of the identifier and keeps the rest of the text', async () => {
    const { input, user } = setUpNames()
    await user.type(input, '2 * sl')
    await user.click(within(screen.getByTestId('expression-suggestions')).getByText('slot = 3 mm'))
    expect(input).toHaveValue('2 * slot')
    expect(input).toHaveFocus()
  })
})

describe('Add parameter {name} (3.7)', () => {
  it('offers to add a name the text uses that no parameter has, with the last valid value', async () => {
    const test = createModelApp({ state: readyState(cubeDocumentState()) })
    const missing = await coreErrorFor({
      code: 'expression_error',
      message: "Unknown name 'wall'.",
      span: { start: 0, end: 4 },
    })
    test.core.stub('evaluate', (_handle, expression) =>
      expression === 'wall'
        ? Promise.reject(missing)
        : Promise.resolve({ value: 2, dimension: LENGTH }),
    )
    renderWithApp(test.app, <Harness initial="2 mm" offerParameter />)
    const input = screen.getByRole<HTMLInputElement>('combobox', { name: 'Distance' })
    const user = userEvent.setup()
    await waitFor(() => {
      expect(screen.getByTestId('expression-result')).toHaveTextContent('= 2 mm')
    })
    await user.clear(input)
    await user.type(input, 'wall')
    const add = await screen.findByRole('button', { name: 'Add parameter wall' })
    await user.click(add)
    expect(test.app.store.getState().panels.side).toBe('parameters')
  })

  it('offers nothing for a name that is a parameter or without a range', async () => {
    const test = createModelApp({ state: readyState(cubeDocumentState()) })
    const failed = await coreErrorFor({ code: 'expression_error', message: 'Something failed.' })
    test.core.stub('evaluate', () => Promise.reject(failed))
    renderWithApp(test.app, <Harness initial="wall" offerParameter />)
    await waitFor(() => {
      expect(screen.getByTestId('field-error')).toHaveTextContent('Something failed.')
    })
    expect(screen.queryByRole('button', { name: /^Add parameter/ })).toBeNull()
  })
})

describe('ps:expression (2.18)', () => {
  const EXPRESSION = 'ps:expression'
  const PLAIN: Dimension = { length: 0, angle: 0 }

  /** One evaluate call: the text it was asked for and the answer the test gives by hand. */
  type PendingEvaluation = { readonly expression: string; readonly answer: Deferred<Evaluation> }

  function expressionMeasures(): PerformanceEntryList {
    return performance.getEntriesByName(EXPRESSION, 'measure')
  }

  /** Waits for an evaluate call of `expression` after the first `skip` of them and answers it. */
  async function answer(
    asked: readonly PendingEvaluation[],
    expression: string,
    result: Evaluation,
    skip = 0,
  ): Promise<void> {
    let calls: PendingEvaluation[] = []
    await waitFor(() => {
      calls = asked.filter((evaluation) => evaluation.expression === expression)
      expect(calls.length).toBeGreaterThan(skip)
    })
    await act(async () => {
      calls[skip]?.answer.resolve(result)
      await settle()
    })
  }

  it('U-151 measures from a changed text to its result shown, once per text whose result shows', async () => {
    performance.clearMeasures(EXPRESSION)
    const asked: PendingEvaluation[] = []
    const { app, input, user } = setUp({ initial: '' }, (core) => {
      core.stub('evaluate', (_documentHandle, expression) => {
        const pending = deferred<Evaluation>()
        asked.push({ expression, answer: pending })
        return pending.promise
      })
    })

    // 2: nothing while its evaluation is pending, one measure once its result shows.
    await user.type(input, '2')
    await waitFor(() => {
      expect(asked.map((evaluation) => evaluation.expression)).toContain('2')
    })
    await settle()
    expect(expressionMeasures()).toHaveLength(0)
    await answer(asked, '2', { value: 2, dimension: PLAIN })
    await waitFor(() => {
      expect(screen.getByTestId('expression-result')).toHaveTextContent(/^mm$/)
    })
    await waitFor(() => {
      expect(expressionMeasures()).toHaveLength(1)
    })

    // 23 replaced by 234 before its result shows: one measure, for 234, from its change.
    await user.type(input, '3')
    await waitFor(() => {
      expect(asked.map((evaluation) => evaluation.expression)).toContain('23')
    })
    const beforeChange = performance.now()
    await user.type(input, '4')
    const afterChange = performance.now()
    await answer(asked, '23', { value: 23, dimension: PLAIN })
    await answer(asked, '234', { value: 234, dimension: PLAIN })
    await waitFor(() => {
      expect(expressionMeasures()).toHaveLength(2)
    })
    const latest = expressionMeasures()[1]
    expect(latest?.startTime).toBeGreaterThanOrEqual(beforeChange)
    expect(latest?.startTime).toBeLessThanOrEqual(afterChange)

    // The same text at a newer revision: evaluated again, no new measure.
    act(() => {
      const document = app.store.getState().document
      app.store.setState({ document: document && { ...document, revision: document.revision + 1 } })
    })
    await answer(asked, '234', { value: 234, dimension: PLAIN }, 1)
    await settle()
    expect(expressionMeasures()).toHaveLength(2)

    // 2345, then back to 234 before its result shows: 234's result shows at once, so evaluating
    // 234 again records no measure.
    await user.type(input, '5')
    await waitFor(() => {
      expect(asked.map((evaluation) => evaluation.expression)).toContain('2345')
    })
    await user.type(input, '{Backspace}')
    expect(screen.getByTestId('expression-result')).toHaveTextContent(/^mm$/)
    await answer(asked, '2345', { value: 2345, dimension: PLAIN })
    await answer(asked, '234', { value: 234, dimension: PLAIN }, 2)
    await settle()
    expect(expressionMeasures()).toHaveLength(2)

    // A newly shown field measures its first text.
    renderWithApp(app, <Harness initial="10 mm" id="height" label="Height" />)
    await answer(asked, '10 mm', { value: 10, dimension: LENGTH })
    await waitFor(() => {
      expect(expressionMeasures()).toHaveLength(3)
    })
  })
})

/** The suggestion options for `si`, after typing it into a field with or without `code`. */
async function suggestionOptions(code: boolean): Promise<HTMLElement[]> {
  const { input, user } = setUpNames({ code })
  await user.type(input, 'si')
  return within(screen.getByTestId('expression-suggestions')).getAllByRole('option')
}

describe('the type roles of expression fields (2.16.7, U-205)', () => {
  it('U-205 sets the input in tabular figures, not code, without the ligature class', () => {
    const { input } = setUpNames()
    expect(hasTabularFigures(input)).toBe(true)
    expect(isCode(input)).toBe(false)
    expect(input).not.toHaveClass(NO_CONTEXTUAL_LIGATURES)
  })

  it('U-205 sets the result of 10, "mm", in tabular figures, not code', async () => {
    const { input, user } = setUpNames()
    await user.type(input, '10')
    const result = screen.getByTestId('expression-result')
    await waitFor(() => {
      expect(result).toHaveTextContent(/^mm$/)
    })
    expect(hasTabularFigures(result)).toBe(true)
    expect(isCode(result)).toBe(false)
  })

  it('U-205 sets the name suggestions in tabular figures, not code', async () => {
    const listed = await suggestionOptions(false)
    expect(listed.length).toBeGreaterThan(0)
    for (const option of listed) {
      expect(hasTabularFigures(option)).toBe(true)
      expect(isCode(option)).toBe(false)
    }
  })

  it('U-205 with code, sets the input as editable code without tabular-nums', () => {
    const { input } = setUpNames({ code: true })
    expect(isCode(input)).toBe(true)
    expect(input).toHaveClass(NO_CONTEXTUAL_LIGATURES)
    expect(input).not.toHaveClass('tabular-nums')
  })

  it('U-205 with code, keeps the result in tabular figures, not code', async () => {
    const { input, user } = setUpNames({ code: true })
    await user.type(input, '10')
    const result = screen.getByTestId('expression-result')
    await waitFor(() => {
      expect(result).toHaveTextContent(/^mm$/)
    })
    expect(hasTabularFigures(result)).toBe(true)
    expect(isCode(result)).toBe(false)
  })

  it('U-205 with code, sets the name suggestions as code', async () => {
    const listed = await suggestionOptions(true)
    expect(listed.length).toBeGreaterThan(0)
    for (const option of listed) {
      expect(isCode(option)).toBe(true)
    }
  })
})
