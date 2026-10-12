import { act, cleanup, renderHook, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { useState, type ReactNode } from 'react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { AppProvider } from '../app/AppProvider'
import type { App } from '../app/createApp'
import type { Dimension } from '../generated/protocol'
import { cubeDocumentState, readyState } from '../layout/fixtures.testing'
import { createModelApp, deferred, renderWithApp, type Deferred } from '../layout/test-app.testing'
import { coreErrorFor, type FakeCoreApi } from '../test/fakes/core-api'
import { settle } from '../test/fakes/settle'
import { FONT_CLASSES } from '../theme/roles'
import {
  ExpressionInput,
  type ExpressionInputProps,
  type ExpressionState,
} from './ExpressionInput'
import { useExpression } from './useExpression'

afterEach(() => {
  cleanup()
})

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
  const input = screen.getByRole<HTMLInputElement>('textbox', { name: 'Distance' })
  return { ...test, input, user: userEvent.setup() }
}

/** Makes `evaluate` reject every expression with this protocol error. */
async function rejectEvaluations(
  core: FakeCoreApi,
  error: { readonly message: string; readonly span?: { readonly start: number; readonly end: number } },
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

  it('shows no result while the text is empty', async () => {
    setUp({ initial: '', optional: true })
    await settle()
    expect(screen.getByTestId('expression-result').textContent).toBe('')
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

  it('writes the expression and its result in the values font', async () => {
    const { input } = setUp({ initial: '10 mm' })
    await settle()
    expect(input.className).toContain(FONT_CLASSES.values)
    expect(screen.getByTestId('expression-result').className).toContain(FONT_CLASSES.values)
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
