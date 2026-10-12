import { clsx } from 'clsx/lite'
import { GripHorizontal, Shapes, X } from 'lucide-react'
import { useEffect, useId, useLayoutEffect, useRef, type PointerEvent, type ReactNode } from 'react'
import { cancelCommand } from '../app/actions/command'
import { clearSelection } from '../app/actions/selection'
import { useApp, useAppState } from '../app/AppProvider'
import type { App } from '../app/createApp'
import { FEATURE_ICONS } from '../commands/icons'
import { IconButton } from '../components/IconButton'
import { Tooltip } from '../components/Tooltip'
import type { AppState, SelectionItem } from '../state/store'
import { COLOR_CLASSES } from '../theme/roles'
import { positionElement } from '../viewport/overlay'
import { commandSelectionFields } from '../viewport/scene'
import { CommandInspector } from './CommandInspector'
import { fieldInputId } from './fields/field-id'
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

type PanelProps = {
  readonly mode: 'command' | 'selection'
  readonly heading: ReactNode
  /** The accessible name: the command title, or "Selection". */
  readonly name: string | null
  /** The command's description (2.10), the title's tooltip, or null. */
  readonly description?: string | null
  readonly preview?: string
  readonly closeLabel: string
  readonly onClose: () => void
  readonly children: ReactNode
}

function Panel({
  mode,
  heading,
  name,
  description = null,
  preview,
  closeLabel,
  onClose,
  children,
}: PanelProps) {
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
    // The first of the opening fields that is shown takes focus.
    const target =
      openingFocus(app.store.getState())
        .map((path) => document.getElementById(fieldInputId(path)))
        .find((element) => element !== null) ?? null
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

  return (
    <div
      ref={panel}
      // The spec names the dialog role on this element (ui.md 3.7, 6.1). A native dialog element
      // would bring browser placement styles to a panel the app positions itself.
      // oxlint-disable-next-line jsx-a11y/prefer-tag-over-role
      role="dialog"
      aria-modal="false"
      aria-labelledby={name === null ? undefined : titleId}
      aria-label={name === null ? 'Selection' : undefined}
      data-testid="inspector"
      data-mode={mode}
      data-preview={preview}
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
        {name === null ? null : description === null ? (
          <h2 id={titleId} className="min-w-0 flex-1 truncate text-base font-semibold">
            {name}
          </h2>
        ) : (
          <Tooltip label={description}>
            <h2 id={titleId} className="min-w-0 flex-1 truncate text-base font-semibold">
              {name}
            </h2>
          </Tooltip>
        )}
        <IconButton label={closeLabel} icon={X} size="compact" onClick={onClose} />
      </div>
      {children}
    </div>
  )
}

/**
 * Where focus goes when a command's inspector opens (3.7 step 5), in order of preference: the
 * active field when it is a required empty selection field, else the first essential expression
 * field, else the active field. The first of them that is shown takes focus, so an active field
 * inside the closed More options gives way to the first essential expression field.
 */
function openingFocus(state: AppState): readonly string[] {
  const command = state.command
  const feature = command === null ? undefined : state.catalog?.byType[command.featureType]
  if (command === null || feature === undefined) {
    return []
  }
  const model = buildInspectorModel({ feature, params: command.params, errors: {} })
  const fields = [...model.essential, ...model.more]
  const active = fields.find((field) => field.path === command.activeField)
  const waiting = active !== undefined && active.required && isEmptyField(active.value)
  const expression = model.essential.find((field) => field.control.type === 'expression')
  const paths =
    waiting || expression === undefined
      ? [command.activeField, expression?.path]
      : [expression.path, command.activeField]
  return paths.filter((path): path is string => path !== null && path !== undefined)
}

function isEmptyField(value: unknown): boolean {
  if (value === undefined || value === null || value === '') {
    return true
  }
  if (Array.isArray(value)) {
    return value.length === 0
  }
  const regions: unknown = typeof value === 'object' ? Reflect.get(value, 'regions') : undefined
  return Array.isArray(regions) && regions.length === 0
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
        description={app.commands.get(`feature.${command.featureType}`)?.description ?? null}
        preview={command.preview}
        closeLabel="Cancel"
        onClose={() => {
          cancelCommand(app).catch(report)
        }}
      >
        <CommandInspector command={command} feature={feature} />
      </Panel>
    )
  }

  const items =
    context === 'sketch'
      ? sketch !== null && sketch.tool === null
        ? sketch.selection
        : []
      : selection
  if (items.length === 0) {
    return null
  }
  return (
    <Panel
      key="selection"
      mode="selection"
      heading={
        <span
          className={clsx(
            'flex min-w-0 flex-1 items-center gap-2 text-xs font-medium',
            COLOR_CLASSES.textMuted.foreground,
          )}
        >
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
