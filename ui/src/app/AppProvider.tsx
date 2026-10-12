import { createContext, useContext, type ReactNode } from 'react'
import { useStore } from 'zustand'
import type { AppState } from '../state/store'
import type { App } from './createApp'

// React access to the app (ui.md 2.1): components read state through narrow selectors, so each
// re-renders only for the slice it shows, and change it only through actions.

const AppContext = createContext<App | null>(null)

export type AppProviderProps = {
  readonly app: App
  readonly children?: ReactNode
}

export function AppProvider({ app, children }: AppProviderProps): React.JSX.Element {
  return <AppContext value={app}>{children}</AppContext>
}

/** The app of the nearest AppProvider. */
// The hooks live with their provider (ui.md 2.1); editing this file reloads instead of hot-swapping.
// oxlint-disable-next-line react/only-export-components
export function useApp(): App {
  const app = useContext(AppContext)
  if (app === null) {
    throw new Error('useApp needs an AppProvider above it.')
  }
  return app
}

/**
 * A slice of the app state; the component re-renders only when the selected value changes
 * (`Object.is`). The selector must return the same value for the same state, as the selectors of
 * state/selectors.ts do.
 */
// oxlint-disable-next-line react/only-export-components
export function useAppState<Selected>(selector: (state: AppState) => Selected): Selected {
  return useStore(useApp().store, selector)
}
