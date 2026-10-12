import { clsx } from 'clsx/lite'
import { GripHorizontal, Shapes, X } from 'lucide-react'
import {
  useEffect,
  useId,
  useLayoutEffect,
  useRef,
  type KeyboardEvent,
  type PointerEvent,
  type ReactNode,
} from 'react'
import { cancelCommand, confirmCommand } from '../app/actions/command'
import { clearSelection } from '../app/actions/selection'
import { useApp, useAppState } from '../app/AppProvider'
import type { App } from '../app/createApp'
import { FEATURE_ICONS } from '../commands/icons'
import { IconButton } from '../components/IconButton'
import type { SelectionItem } from '../state/store'
import { COLOR_CLASSES } from '../theme/roles'
import { positionElement } from '../viewport/overlay'
import { commandSelectionFields } from '../viewport/scene'
import { CommandInspector } from './CommandInspector'
import { buildInspectorModel } from './model'
import { placeInspector } from './placement'
import { SelectionInspector } from './SelectionInspector'
import { itemsFromValue } from './selection-values'

// The floating inspector (ui.md 3.7, 3.8): one panel inside the viewport for the active feature
// command or the selection, placed beside what it is about and dragged by its header. Its
// position is a runtime translate (3.12), never a style prop.

type Position = { readonly x: number; readonly y: number }

/** The items the panel is about, for its placement: the selection, or the command's picks. */
function anchorItems(app: App): readonly SelectionItem[] {
  const state = app.store.getState()
  if (state.command === null) {
    return state.context === 'sketch' ? (state.sketch?.selection ?? []) : state.selection
  }
  return commandSelectionFields(state).flatMap((field) =>
    itemsFromValue(field.accepts, field.value, state, field.siblings),
  )
}

/** Elements whose own Enter must not confirm the command (3.5 Enter ladder). */
function handlesEnter(target: EventTarget): boolean {
  return (
    target instanceof HTMLElement &&
    target.closest('button, a, select, textarea, [role="button"], [role="menu"]') !== null
  )
}

type PanelProps = {
  readonly mode: 'command' | 'selection'
  readonly heading: ReactNode
  /** The accessible name: the command title, or "Selection". */
  readonly name: string | null
  readonly preview?: string
  readonly closeLabel: string
  readonly onClose: () => void
  readonly onEnter?: () => void
  readonly children: ReactNode
}

function Panel({ mode, heading, name, preview, closeLabel, onClose, onEnter, children }: PanelProps) {
  const app = useApp()
  const panel = useRef<HTMLDivElement>(null)
  const titleId = useId()
  const position = useRef<Position>({ x: 0, y: 0 })
  const drag = useRef<{
    readonly pointerId: number
    readonly startX: number
    readonly startY: number
    readonly from: Position
  } | null>(null)

  // Placed once when it opens; it then stays put while open (3.7).
  useLayoutEffect(() => {
    const element = panel.current
    if (element === null) {
      return
    }
    const container = element.parentElement
    const bounds = {
      x: 0,
      y: 0,
      width: container?.clientWidth ?? 0,
      height: container?.clientHeight ?? 0,
    }
    const placed =
      app.store.getState().panels.inspectorPosition ??
      placeInspector({
        anchor: app.viewport()?.screenRectangle(anchorItems(app)) ?? null,
        panel: { width: element.offsetWidth, height: element.offsetHeight },
        bounds,
      })
    position.current = placed
    positionElement(element, placed.x, placed.y)
  }, [app])

  // A command's inspector takes focus when it opens and gives it back when it closes (3.24).
  useEffect(() => {
    const element = panel.current
    if (mode !== 'command' || element === null) {
      return undefined
    }
    const previous = document.activeElement instanceof HTMLElement ? document.activeElement : null
    const state = app.store.getState()
    const target =
      state.command?.activeField === null
        ? element.querySelector<HTMLInputElement>('[data-testid="inspector-field"] input[type="text"]')
        : element.querySelector<HTMLElement>('[data-testid="selection-field"][data-active="true"]')
    if (target !== null) {
      target.focus()
      if (target instanceof HTMLInputElement) {
        target.select()
      }
    }
    return () => {
      const active = document.activeElement
      const lost = active === null || active === document.body || element.contains(active)
      if (lost && previous?.isConnected === true) {
        previous.focus()
      }
    }
  }, [app, mode])

  function onHeaderPointerDown(event: PointerEvent<HTMLDivElement>): void {
    if (event.button !== 0 || (event.target instanceof Element && event.target.closest('button'))) {
      return
    }
    drag.current = {
      pointerId: event.pointerId,
      startX: event.clientX,
      startY: event.clientY,
      from: position.current,
    }
    try {
      event.currentTarget.setPointerCapture(event.pointerId)
    } catch {
      // The pointer is gone; the drag ends with it.
    }
  }

  function onHeaderPointerMove(event: PointerEvent<HTMLDivElement>): void {
    const current = drag.current
    const element = panel.current
    if (current === null || element === null || current.pointerId !== event.pointerId) {
      return
    }
    const container = element.parentElement
    const maximumX = Math.max(0, (container?.clientWidth ?? 0) - element.offsetWidth)
    const maximumY = Math.max(0, (container?.clientHeight ?? 0) - element.offsetHeight)
    const next = {
      x: Math.min(Math.max(current.from.x + event.clientX - current.startX, 0), maximumX),
      y: Math.min(Math.max(current.from.y + event.clientY - current.startY, 0), maximumY),
    }
    position.current = next
    positionElement(element, next.x, next.y)
  }

  function onHeaderPointerUp(event: PointerEvent<HTMLDivElement>): void {
    const current = drag.current
    if (current === null || current.pointerId !== event.pointerId) {
      return
    }
    drag.current = null
    const moved = position.current.x !== current.from.x || position.current.y !== current.from.y
    if (moved) {
      // The dragged position is used for every inspector until reload (3.7).
      const inspectorPosition = position.current
      app.store.setState((state) => ({ panels: { ...state.panels, inspectorPosition } }))
    }
  }

  function onKeyDown(event: KeyboardEvent<HTMLDivElement>): void {
    if (event.key !== 'Enter' || event.defaultPrevented || onEnter === undefined) {
      return
    }
    if (handlesEnter(event.target)) {
      return
    }
    event.preventDefault()
    onEnter()
  }

  return (
    <div
      ref={panel}
      role="dialog"
      aria-modal="false"
      aria-labelledby={name === null ? undefined : titleId}
      aria-label={name === null ? 'Selection' : undefined}
      data-testid="inspector"
      data-mode={mode}
      data-preview={preview}
      onKeyDown={onKeyDown}
      className={clsx(
        'pointer-events-auto absolute top-0 left-0 z-20 flex max-h-[calc(100%-2rem)] w-80 flex-col gap-4 overflow-y-auto rounded-md border p-4 text-sm shadow-lg transition-opacity duration-150 ease-out starting:opacity-0 motion-reduce:transition-none',
        COLOR_CLASSES.surfaceRaised.background,
        COLOR_CLASSES.border.border,
        COLOR_CLASSES.text.foreground,
      )}
    >
      <div
        className="-m-1 flex cursor-move touch-none items-center gap-2 p-1"
        onPointerDown={onHeaderPointerDown}
        onPointerMove={onHeaderPointerMove}
        onPointerUp={onHeaderPointerUp}
        onPointerCancel={onHeaderPointerUp}
      >
        {heading}
        {name === null ? null : (
          <h2 id={titleId} className="min-w-0 flex-1 truncate text-base font-semibold">
            {name}
          </h2>
        )}
        <IconButton label={closeLabel} icon={X} size="compact" onClick={onClose} />
      </div>
      {children}
    </div>
  )
}

function report(error: unknown): void {
  console.error('An inspector action failed.', error)
}

export function Inspector() {
  const app = useApp()
  const command = useAppState((state) => state.command)
  const catalog = useAppState((state) => state.catalog)
  const context = useAppState((state) => state.context)
  const selection = useAppState((state) => state.selection)
  const sketch = useAppState((state) => state.sketch)

  if (command !== null) {
    const feature = catalog?.byType[command.featureType]
    if (feature === undefined) {
      return null
    }
    const Icon = FEATURE_ICONS[feature.manifest.icon] ?? Shapes
    return (
      <Panel
        key={`command:${command.featureId}`}
        mode="command"
        heading={<Icon aria-hidden="true" strokeWidth={1.75} className="size-5 shrink-0" />}
        name={command.title}
        preview={command.preview}
        closeLabel="Cancel"
        onClose={() => {
          cancelCommand(app).catch(report)
        }}
        onEnter={() => {
          const current = app.store.getState().command
          const complete =
            current !== null &&
            buildInspectorModel({
              feature,
              params: current.params,
              errors: current.fieldErrors,
            }).complete
          if (complete) {
            confirmCommand(app).catch(report)
          }
        }}
      >
        <CommandInspector command={command} feature={feature} />
      </Panel>
    )
  }

  const items =
    context === 'sketch' ? (sketch !== null && sketch.tool === null ? sketch.selection : []) : selection
  if (items.length === 0) {
    return null
  }
  return (
    <Panel
      key="selection"
      mode="selection"
      heading={
        <span className={clsx('flex min-w-0 flex-1 items-center gap-2 text-xs font-medium', COLOR_CLASSES.textMuted.foreground)}>
          <GripHorizontal aria-hidden="true" strokeWidth={1.75} className="size-4" />
          Selection
        </span>
      }
      name={null}
      closeLabel="Clear selection"
      onClose={() => {
        if (context === 'sketch') {
          app.store.setState((state) =>
            state.sketch === null ? {} : { sketch: { ...state.sketch, selection: [] } },
          )
          return
        }
        clearSelection(app)
      }}
    >
      <SelectionInspector items={items} />
    </Panel>
  )
}
