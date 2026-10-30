import { clsx } from 'clsx/lite'
import { GripHorizontal, Shapes, X } from 'lucide-react'
import { useEffect, useId, useLayoutEffect, useRef, type PointerEvent, type ReactNode } from 'react'
import { useShallow } from 'zustand/react/shallow'
import { cancelCommand } from '../app/actions/command'
import { clearSelection } from '../app/actions/selection'
import { useApp, useAppState } from '../app/AppProvider'
import type { App } from '../app/createApp'
import { reportFailure } from '../app/report'
import { FEATURE_ICONS } from '../commands/icons'
import { IconButton } from '../components/IconButton'
import { useOverflowEdges } from '../components/overflow-edges'
import { Tooltip } from '../components/Tooltip'
import { focusedElement, focusLost, returnFocus } from '../layout/focus'
import { selectInspectedItems } from '../state/selectors'
import type { AppState, SelectionItem } from '../state/store'
import { COLOR_CLASSES } from '../theme/roles'
import { framedArea } from '../viewport/camera'
import { positionElement } from '../viewport/overlay'
import { commandSelectionFields } from '../viewport/picking-rules'
import { CommandInspector } from './CommandInspector'
import { fieldInputId, focusFieldInput } from './fields/field-id'
import { activeCommandModel, fieldsOf, isEmptyValue } from './model'
import { inspectorBounds, placeInspector, type Rectangle } from './placement'
import { SelectionInspector } from './SelectionInspector'
import { itemsFromValue } from './selection-values'

// The floating inspector (ui.md 3.7, 3.8): one card in the free area for the active feature
// command or the selection, placed beside what it is about and dragged by its header. Its
// position is a runtime translate (3.12), never a style prop.

type Position = { readonly x: number; readonly y: number }

/**
 * A position moved into the free-area layer so the whole panel lies inside it (3.7): a stored or
 * dragged position, or the current one after the layer shrank or the panel grew.
 */
function clampedInto(layer: HTMLElement, panel: HTMLElement, position: Position): Position {
  const box = layer.getBoundingClientRect()
  return {
    x: Math.min(Math.max(position.x, 0), Math.max(0, box.width - panel.offsetWidth)),
    y: Math.min(Math.max(position.y, 0), Math.max(0, box.height - panel.offsetHeight)),
  }
}

/** An element's box relative to the free-area layer's box. */
function boxIn(element: Element, layer: DOMRect): Rectangle {
  const box = element.getBoundingClientRect()
  return { x: box.left - layer.left, y: box.top - layer.top, width: box.width, height: box.height }
}

/**
 * The view cube's reach in the free-area layer `layer`: its box widened to its Home view button,
 * which lies outside the cube's top-left corner, clear of the turned cube (3.14).
 */
function cubeReach(layer: HTMLElement, layerBox: DOMRect): Rectangle | null {
  const cube = layer.querySelector('[data-testid="view-cube"]')
  if (cube === null) {
    return null
  }
  const box = boxIn(cube, layerBox)
  const home = cube.querySelector('button[aria-label="Home view"]')
  if (home === null) {
    return box
  }
  const homeBox = boxIn(home, layerBox)
  const x = Math.min(box.x, homeBox.x)
  const y = Math.min(box.y, homeBox.y)
  return {
    x,
    y,
    width: Math.max(box.x + box.width, homeBox.x + homeBox.width) - x,
    height: Math.max(box.y + box.height, homeBox.y + homeBox.height) - y,
  }
}

/**
 * Where an opening inspector goes in the free-area layer `layer` (3.7): beside the items it is
 * about, whose canvas rectangle moves by the free area's offset in the canvas, inside the free
 * area clear of the viewport controls, the view cube and the top-center slot's content where it
 * fits.
 */
function openingPosition(app: App, layer: HTMLElement, panel: HTMLElement): Position {
  const layerBox = layer.getBoundingClientRect()
  const controls = layer.querySelector('[data-testid="viewport-controls"]')
  // Only a slot that shows a card or chip has a height; an empty one holds a hidden live region.
  const slots = Array.from(layer.querySelectorAll('[data-testid="top-center-slot"]'), (slot) =>
    boxIn(slot, layerBox),
  ).filter((box) => box.height > 0)
  const slot =
    slots.length === 0
      ? null
      : slots.reduce((lowest, box) =>
          box.y + box.height > lowest.y + lowest.height ? box : lowest,
        )
  const size = { width: panel.offsetWidth, height: panel.offsetHeight }
  const bounds = inspectorBounds({
    area: { x: 0, y: 0, width: layerBox.width, height: layerBox.height },
    controls: controls === null ? null : boxIn(controls, layerBox),
    cube: cubeReach(layer, layerBox),
    slot,
    panel: size,
  })
  const viewport = app.viewport()
  const items = viewport?.screenRectangle(anchorItems(app)) ?? null
  const free = viewport === null ? null : framedArea(viewport.viewWindow())
  const anchor =
    items === null || free === null ? null : { ...items, x: items.x - free.x, y: items.y - free.y }
  return placeInspector({ anchor, panel: size, bounds })
}

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
  /** What goes under the header: a scrolling body, and a footer that stays in view. */
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

  // Placed once when it opens; it then stays put while open, inside the free area (3.7).
  useLayoutEffect(() => {
    const element = panel.current
    const layer = element?.parentElement ?? null
    if (element === null || layer === null) {
      return undefined
    }
    const place = (wanted: Position): void => {
      const placed = clampedInto(layer, element, wanted)
      position.current = placed
      positionElement(element, placed.x, placed.y)
    }
    place(app.store.getState().panels.inspectorPosition ?? openingPosition(app, layer, element))
    if (typeof ResizeObserver !== 'function') {
      return undefined
    }
    // A free area that shrinks under the card (a docked island opens, the window narrows) or a
    // card that grows moves it back inside, so it never ends up under an island.
    const observer = new ResizeObserver(() => {
      if (drag.current === null) {
        place(position.current)
      }
    })
    observer.observe(layer)
    observer.observe(element)
    return () => {
      observer.disconnect()
    }
  }, [app])

  // A command's inspector takes focus when it opens and gives it back when it closes, to the 3D
  // view when the control that had it is gone, such as a suggestion of the selection the command
  // took (3.24).
  useEffect(() => {
    const element = panel.current
    if (mode !== 'command' || element === null) {
      return undefined
    }
    const previous = focusedElement()
    // The first of the opening fields that is shown takes focus.
    const target =
      openingFocus(app.store.getState())
        .map((path) => document.getElementById(fieldInputId(path)))
        .find((candidate) => candidate !== null) ?? null
    focusFieldInput(target)
    if (target instanceof HTMLInputElement) {
      target.select()
    }
    return () => {
      if (focusLost() || element.contains(document.activeElement)) {
        returnFocus(previous)
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
    const layer = element.parentElement
    if (layer === null) {
      return
    }
    // The free-area layer bounds the drag (3.7).
    const next = clampedInto(layer, element, {
      x: current.from.x + event.clientX - current.startX,
      y: current.from.y + event.clientY - current.startY,
    })
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
      data-inspector=""
      // A card of the free area: labels under it are hidden (3.12).
      data-card=""
      data-mode={mode}
      data-preview={preview}
      // The card never scrolls as a whole: its content's body does, under the header (3.2).
      className={clsx(
        'pointer-events-auto absolute top-0 left-0 z-20 flex max-h-[calc(100%-2rem)] w-80 flex-col overflow-hidden rounded-xl border text-sm shadow-lg transition-opacity duration-150 ease-out starting:opacity-0 motion-reduce:transition-none',
        COLOR_CLASSES.surface.background,
        COLOR_CLASSES.forcedEdge.border,
        COLOR_CLASSES.elevation.shadow,
        COLOR_CLASSES.text.foreground,
      )}
    >
      <div
        className="m-3 mb-0 flex shrink-0 cursor-move touch-none items-center gap-2 p-1"
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
  const active = activeCommandModel(state)
  if (active === null) {
    return []
  }
  const { command, model } = active
  const activeField = fieldsOf(model).find((field) => field.path === command.activeField)
  const waiting =
    activeField !== undefined && activeField.required && isEmptyValue(activeField.value)
  const expression = model.essential.find((field) => field.control.type === 'expression')
  const paths =
    waiting || expression === undefined
      ? [command.activeField, expression?.path]
      : [expression.path, command.activeField]
  return paths.filter((path): path is string => path !== null && path !== undefined)
}

export function Inspector() {
  const app = useApp()
  const command = useAppState((state) => state.command)
  const catalog = useAppState((state) => state.catalog)
  const context = useAppState((state) => state.context)
  // Compared by value: the selector makes a new empty list on every store change.
  const items = useAppState(useShallow(selectInspectedItems))

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
          cancelCommand(app).catch(reportFailure('An inspector action'))
        }}
      >
        <CommandInspector command={command} feature={feature} />
      </Panel>
    )
  }

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
      <SelectionBody items={items} />
    </Panel>
  )
}

/** The selection inspector's body, which scrolls under the header with its rule (3.2, 3.8). */
function SelectionBody({ items }: { readonly items: readonly SelectionItem[] }) {
  const body = useRef<HTMLDivElement>(null)
  useOverflowEdges(body)
  return (
    <div
      ref={body}
      className={clsx(
        'flex min-h-0 flex-1 flex-col gap-4 overflow-y-auto border-t px-4 pt-3 pb-4',
        COLOR_CLASSES.forcedEdge.border,
        COLOR_CLASSES.divider.overflowTop,
      )}
    >
      <SelectionInspector items={items} />
    </div>
  )
}
