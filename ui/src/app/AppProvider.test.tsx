import { act, render, screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import { createTestApp } from '../test/fakes/app'
import { AppProvider, useApp, useAppState } from './AppProvider'
import type { App } from './createApp'

function AppProbe(props: { readonly onRender: (app: App) => void }) {
  props.onRender(useApp())
  return null
}

function ContextProbe(props: { readonly onRender?: () => void }) {
  props.onRender?.()
  const context = useAppState((state) => state.context)
  return <p data-testid="context">{context}</p>
}

describe('AppProvider', () => {
  it('useApp gives the provided app', () => {
    const app = createTestApp()
    const onRender = vi.fn<(app: App) => void>()
    render(
      <AppProvider app={app}>
        <AppProbe onRender={onRender} />
      </AppProvider>,
    )
    expect(onRender).toHaveBeenCalledWith(app)
  })

  it('useAppState reads a slice and follows its changes', () => {
    const app = createTestApp()
    render(
      <AppProvider app={app}>
        <ContextProbe />
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
    const onRender = vi.fn<() => void>()
    render(
      <AppProvider app={app}>
        <ContextProbe onRender={onRender} />
      </AppProvider>,
    )
    const afterMount = onRender.mock.calls.length
    act(() => {
      app.store.setState({ hover: { key: 'mesh:1:face:0', kind: 'face' } })
    })
    expect(onRender).toHaveBeenCalledTimes(afterMount)
    act(() => {
      app.store.setState({ context: 'sketch' })
    })
    expect(onRender).toHaveBeenCalledTimes(afterMount + 1)
  })
})
