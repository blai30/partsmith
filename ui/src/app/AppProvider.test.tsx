import { act, cleanup, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it } from 'vitest'
import { createTestApp } from '../test/fakes/app'
import { AppProvider, useApp, useAppState } from './AppProvider'
import type { App } from './createApp'

afterEach(() => {
  cleanup()
})

describe('AppProvider', () => {
  it('useApp gives the provided app', () => {
    const app = createTestApp()
    let seen: App | null = null
    function Probe() {
      seen = useApp()
      return null
    }
    render(
      <AppProvider app={app}>
        <Probe />
      </AppProvider>,
    )
    expect(seen).toBe(app)
  })

  it('useAppState reads a slice and follows its changes', () => {
    const app = createTestApp()
    function Context() {
      const context = useAppState((state) => state.context)
      return <p data-testid="context">{context}</p>
    }
    render(
      <AppProvider app={app}>
        <Context />
      </AppProvider>,
    )
    expect(screen.getByTestId('context')).toHaveTextContent('model')
    act(() => {
      app.store.setState({ context: 'print' })
    })
    expect(screen.getByTestId('context')).toHaveTextContent('print')
  })

  it('useAppState re-renders only when its slice changes', () => {
    const app = createTestApp()
    let renders = 0
    function Context() {
      renders += 1
      const context = useAppState((state) => state.context)
      return <p>{context}</p>
    }
    render(
      <AppProvider app={app}>
        <Context />
      </AppProvider>,
    )
    const afterMount = renders
    act(() => {
      app.store.setState({ hover: { key: 'mesh:1:face:0', kind: 'face' } })
    })
    expect(renders).toBe(afterMount)
    act(() => {
      app.store.setState({ context: 'sketch' })
    })
    expect(renders).toBe(afterMount + 1)
  })
})
