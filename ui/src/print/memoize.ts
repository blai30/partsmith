import type { AppState } from '../state/store'

/**
 * A derivation of the state that returns its last result again while every input is the same
 * object as last time, so `useAppState` and the scene see a stable value until the inputs change.
 */
export function memoized<Inputs extends unknown[], Result>(
  inputsOf: (state: AppState) => readonly [...Inputs],
  derive: (...inputs: Inputs) => Result,
): (state: AppState) => Result {
  let last: { readonly inputs: readonly [...Inputs]; readonly result: Result } | null = null
  return (state) => {
    const inputs = inputsOf(state)
    if (
      last !== null &&
      last.inputs.length === inputs.length &&
      last.inputs.every((input, index) => Object.is(input, inputs[index]))
    ) {
      return last.result
    }
    const result = derive(...inputs)
    last = { inputs, result }
    return result
  }
}
