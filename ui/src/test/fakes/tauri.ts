import type { TauriInvokeGlobal } from '../../files/desktop-file-service'

// A `window.__TAURI__` for desktop tests (desktop.md section 6): its `invoke` records every call and
// answers from a script, as the desktop host's commands would.

/** How the fake host answers one invoke: resolve with a value, or reject with a value. */
export type FakeAnswer = { readonly resolve: unknown } | { readonly reject: unknown }

/** One invoke as the desktop host received it. */
export type FakeInvocation = {
  readonly command: string
  readonly payload: Parameters<TauriInvokeGlobal['core']['invoke']>[1]
  readonly options: Parameters<TauriInvokeGlobal['core']['invoke']>[2]
}

/** Per command, the answers in call order; the last answer repeats. */
export type FakeScript = Readonly<Record<string, readonly FakeAnswer[]>>

export type FakeTauri = {
  readonly tauri: TauriInvokeGlobal
  /** Every invoke so far, in call order. */
  readonly invocations: readonly FakeInvocation[]
}

/** Resolves an invoke with `value`. */
export function resolves(value: unknown): FakeAnswer {
  return { resolve: value }
}

/** Rejects an invoke with `value`, as a command error or a Tauri rejection would. */
export function rejects(value: unknown): FakeAnswer {
  return { reject: value }
}

/** A Tauri global whose invoke answers from `script`; a command the script does not answer rejects with an Error naming it. */
export function createFakeTauri(script: FakeScript = {}): FakeTauri {
  const invocations: FakeInvocation[] = []
  const answered = new Map<string, number>()
  const tauri: TauriInvokeGlobal = {
    core: {
      invoke(command, payload, options) {
        invocations.push({ command, payload, options })
        const answers = script[command] ?? []
        const count = answered.get(command) ?? 0
        answered.set(command, count + 1)
        const answer = answers[Math.min(count, answers.length - 1)]
        if (answer === undefined) {
          return Promise.reject(new Error(`The fake desktop host has no answer to ${command}.`))
        }
        return 'reject' in answer ? Promise.reject(answer.reject) : Promise.resolve(answer.resolve)
      },
    },
  }
  return { tauri, invocations }
}
