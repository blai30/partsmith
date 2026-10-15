import { clsx } from 'clsx/lite'
import { Component, type ErrorInfo, type ReactNode } from 'react'
import { Button } from '../components/Button'
import { MESSAGES } from '../messages'
import { COLOR_CLASSES } from '../theme/roles'

// A React error boundary around one region of the window (ui.md 4.4): a crash in the browser, the
// viewport or any other region shows a short explanation with "Reload" in its place, and the other
// regions keep working. React offers error boundaries only as class components.

export type RegionBoundaryProps = {
  /** The region's name, for the console report. */
  readonly region: string
  /** Layout and shape classes for the fallback, so it keeps the region's place and island shape in the window (3.2). */
  readonly fallbackClassName?: string
  readonly children: ReactNode
}

type RegionBoundaryState = { readonly crashed: boolean }

export class RegionBoundary extends Component<RegionBoundaryProps, RegionBoundaryState> {
  override state: RegionBoundaryState = { crashed: false }

  static getDerivedStateFromError(): RegionBoundaryState {
    return { crashed: true }
  }

  override componentDidCatch(error: unknown, info: ErrorInfo): void {
    console.error(`The ${this.props.region} region crashed.`, error, info.componentStack)
  }

  override render(): ReactNode {
    if (!this.state.crashed) {
      return this.props.children
    }
    return (
      <div
        role="alert"
        className={clsx(
          'flex min-h-0 min-w-0 flex-col items-start gap-3 overflow-hidden p-4 text-sm',
          COLOR_CLASSES.surface.background,
          COLOR_CLASSES.border.border,
          COLOR_CLASSES.elevation.shadow,
          COLOR_CLASSES.text.foreground,
          this.props.fallbackClassName,
        )}
      >
        <p>{MESSAGES.region_crashed()}</p>
        <Button
          onClick={() => {
            location.reload()
          }}
        >
          Reload
        </Button>
      </div>
    )
  }
}
