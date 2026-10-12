import { clsx } from 'clsx/lite'
import { ChevronDown, ChevronUp, CircleX, Shapes, TriangleAlert } from 'lucide-react'
import {
  useEffect,
  useId,
  useLayoutEffect,
  useRef,
  useState,
  type KeyboardEvent,
  type PointerEvent,
} from 'react'
import { editFeature } from '../app/actions/command'
import {
  deleteFeatures,
  moveFeature,
  renameFeature,
  setRollback,
  setSuppressed,
} from '../app/actions/timeline'
import { useApp, useAppState } from '../app/AppProvider'
import type { App } from '../app/createApp'
import { FEATURE_ICONS } from '../commands/icons'
import { Button } from '../components/Button'
import { IconButton } from '../components/IconButton'
import { Menu, type MenuEntry } from '../components/Menu'
import { Tooltip } from '../components/Tooltip'
import { featureLabel } from '../core/labels'
import type { FeatureRecord, FeatureStatus } from '../generated/protocol'
import { pickRow, togglesSelection } from '../layout/pick-row'
import { useUiRequest } from '../layout/requests'
import { keepInViewHorizontally } from '../layout/scroll'
import { MESSAGES } from '../messages'
import { selectProblems } from '../state/selectors'
import type { AppState, SelectionItem } from '../state/store'
import { COLOR_CLASSES } from '../theme/roles'
import { positionElement } from '../viewport/overlay'
import { startRollbackDrag, type RollbackDrag } from './rollback-drag'

// The timeline (ui.md 3.9): the features in timeline order with the rollback marker in its gap,
// reordering by drag, inline rename, a context menu, and the problems count. The strip is a
// toolbar with one tab stop: Left and Right move between the items and the marker.

/** A press must move this far (CSS pixels) before it drags an item or the marker. */
const DRAG_THRESHOLD_PIXELS = 4

function plural(count: number, singular: string, several: string): string {
  return `${String(count)} ${count === 1 ? singular : several}`
}

function featureItem(id: string, label: string): SelectionItem {
  return {
    key: `feature:${id}`,
    kind: 'feature',
    label,
    body: null,
    feature: id,
    element: null,
    sketchItem: null,
    origin: null,
  }
}

function report(action: string): (error: unknown) => void {
  return (error) => {
    console.error(`${action} failed.`, error)
  }
}

/** The rollback marker position p: features 1 to p are active (3.9). */
function markerPosition(records: readonly FeatureRecord[], rollback: string | null): number {
  const index = rollback === null ? -1 : records.findIndex((record) => record.id === rollback)
  return index === -1 ? records.length : index
}

/** The status the item shows: the latest recomputed one, or pending before it (3.9). */
function itemStatus(status: FeatureStatus | undefined): FeatureStatus['status'] | 'pending' {
  return status?.status ?? 'pending'
}

/** The hidden description of an item's status, or null for ok and pending. */
function statusText(status: FeatureStatus | undefined, suppressed: boolean): string | null {
  const first = status?.messages[0]?.message
  switch (status?.status) {
    case 'error':
      return first === undefined ? 'Error' : `Error: ${first}`
    case 'warning':
      return first === undefined ? 'Warning' : `Warning: ${first}`
    case 'suppressed':
      return 'Suppressed'
    case 'rolled_back':
      return 'Rolled back'
    default:
      return suppressed ? 'Suppressed' : null
  }
}

/** The x positions (client CSS pixels) of the gaps 0 to N around the strip's items. */
function gapPositions(strip: HTMLElement): readonly number[] {
  const rectangles = [
    ...strip.querySelectorAll<HTMLElement>('[data-testid="timeline-item"]'),
  ].map((item) => item.getBoundingClientRect())
  const [first] = rectangles
  const last = rectangles.at(-1)
  if (first === undefined || last === undefined) {
    return [strip.getBoundingClientRect().left]
  }
  const second = rectangles[1]
  const spacing = second === undefined ? 0 : second.left - first.right
  const gaps = [first.left - spacing / 2]
  for (let i = 1; i < rectangles.length; i++) {
    const before = rectangles[i - 1]
    const after = rectangles[i]
    if (before !== undefined && after !== undefined) {
      gaps.push((before.right + after.left) / 2)
    }
  }
  gaps.push(last.right + spacing / 2)
  return gaps
}

/** The gap nearest to a client x position. */
function nearestGap(gaps: readonly number[], x: number): number {
  let best = 0
  gaps.forEach((gap, index) => {
    if (Math.abs(gap - x) < Math.abs((gaps[best] ?? gap) - x)) {
      best = index
    }
  })
  return best
}

/** Everything the strip draws, read from the store with one subscription per slice. */
function useTimelineState() {
  const recipe = useAppState((state) => state.document?.recipe ?? null)
  const statuses = useAppState((state) => state.document?.featureStatus ?? null)
  const catalog = useAppState((state) => state.catalog)
  const selection = useAppState((state) => state.selection)
  const editing = useAppState((state) =>
    state.command?.editing === true ? state.command.featureId : null,
  )
  const editedSketch = useAppState((state) =>
    state.context === 'sketch' ? (state.sketch?.sketchId ?? null) : null,
  )
  return { recipe, statuses, catalog, selection, editing, editedSketch }
}

type Press = {
  readonly pointerId: number
  readonly startX: number
  /** The dragged feature, or null for the rollback marker. */
  readonly featureId: string | null
  readonly index: number
  dragging: boolean
  gaps: readonly number[]
  gap: number
  rollback: RollbackDrag | null
}

type ItemProps = {
  readonly record: FeatureRecord
  readonly index: number
  readonly label: string
  readonly status: FeatureStatus | undefined
  readonly rolledBack: boolean
  readonly dimmed: boolean
  readonly selected: boolean
  readonly outlined: boolean
  readonly renaming: boolean
  readonly onPress: (event: PointerEvent<HTMLDivElement>, featureId: string, index: number) => void
  readonly onPick: (record: FeatureRecord, label: string, toggle: boolean) => void
  readonly onEdit: (featureId: string) => void
  readonly onMenu: (featureId: string, button: HTMLButtonElement) => void
  readonly onRename: (featureId: string | null) => void
}

function TimelineItem({
  record,
  index,
  label,
  status,
  rolledBack,
  dimmed,
  selected,
  outlined,
  renaming,
  onPress,
  onPick,
  onEdit,
  onMenu,
  onRename,
}: ItemProps) {
  const app = useApp()
  const descriptionId = useId()
  const catalog = useAppState((state) => state.catalog)
  const suppressed = record.suppressed === true
  const description = statusText(status, suppressed)
  const iconName = catalog?.byType[record.type]?.manifest.icon
  const Icon = iconName === undefined ? Shapes : (FEATURE_ICONS[iconName] ?? Shapes)
  const state = status?.status
  const messages = status?.messages.map((message) => message.message) ?? []
  const tooltip = [label, ...messages].join(' ')

  function onKeyDown(event: KeyboardEvent<HTMLButtonElement>): void {
    if (event.key === 'Enter') {
      event.preventDefault()
      onEdit(record.id)
    } else if (event.key === ' ') {
      event.preventDefault()
      onPick(record, label, togglesSelection(app, event))
    } else if (event.key === 'F2') {
      event.preventDefault()
      onRename(record.id)
    } else if ((event.key === 'F10' && event.shiftKey) || event.key === 'ContextMenu') {
      event.preventDefault()
      onMenu(record.id, event.currentTarget)
    }
  }

  return (
    <div
      data-testid="timeline-item"
      data-feature-id={record.id}
      data-status={itemStatus(status)}
      data-suppressed={suppressed ? 'true' : 'false'}
      data-rolled-back={rolledBack ? 'true' : 'false'}
      onPointerDown={(event) => {
        onPress(event, record.id, index)
      }}
      className="relative shrink-0 touch-none"
    >
      <Tooltip label={tooltip}>
        <button
          type="button"
          data-strip-item=""
          aria-label={label}
          aria-describedby={description === null ? undefined : descriptionId}
          onClick={(event) => {
            onPick(record, label, togglesSelection(app, event))
          }}
          onDoubleClick={() => {
            onEdit(record.id)
          }}
          onKeyDown={onKeyDown}
          onContextMenu={(event) => {
            event.preventDefault()
            onMenu(record.id, event.currentTarget)
          }}
          className={clsx(
            'relative flex size-10 items-center justify-center rounded-sm transition-colors duration-150 motion-reduce:transition-none',
            COLOR_CLASSES.focusRing.outline,
            COLOR_CLASSES.accent.outline,
            COLOR_CLASSES.text.foreground,
            outlined && 'outline-2 -outline-offset-2',
            selected ? COLOR_CLASSES.selection.background : COLOR_CLASSES.hover.background,
          )}
        >
          <Icon
            aria-hidden="true"
            strokeWidth={1.75}
            className={clsx('size-5', (suppressed || dimmed) && 'opacity-40')}
          />
          {suppressed ? (
            <span
              aria-hidden="true"
              className={clsx(
                'absolute inset-x-1 top-1/2 h-0.5 -rotate-45 bg-current',
                COLOR_CLASSES.textMuted.foreground,
              )}
            />
          ) : null}
          {state === 'error' ? (
            <CircleX
              aria-hidden="true"
              strokeWidth={2}
              className={clsx(
                'absolute right-0 bottom-0 size-3.5 rounded-full',
                COLOR_CLASSES.surfaceRaised.background,
                COLOR_CLASSES.errorMark.foreground,
              )}
            />
          ) : state === 'warning' ? (
            <TriangleAlert
              aria-hidden="true"
              strokeWidth={2}
              className={clsx(
                'absolute right-0 bottom-0 size-3.5',
                COLOR_CLASSES.warningMark.fill,
                COLOR_CLASSES.warningMark.foreground,
              )}
            />
          ) : null}
        </button>
      </Tooltip>
      {description === null ? null : (
        <span id={descriptionId} hidden>
          {description}
        </span>
      )}
      {renaming ? (
        <RenameField
          initial={label}
          onDone={(name) => {
            onRename(null)
            if (name !== null && name.trim() !== label) {
              renameFeature(app, record.id, name).catch(report('Renaming the feature'))
            }
          }}
        />
      ) : null}
    </div>
  )
}

/** The inline rename field over an item: Enter or leaving it commits, Esc cancels (3.9). */
function RenameField({
  initial,
  onDone,
}: {
  readonly initial: string
  readonly onDone: (name: string | null) => void
}) {
  const input = useRef<HTMLInputElement>(null)
  const [value, setValue] = useState(initial)
  const done = useRef(false)

  useLayoutEffect(() => {
    input.current?.focus()
    input.current?.select()
  }, [])

  function finish(name: string | null): void {
    if (!done.current) {
      done.current = true
      onDone(name)
    }
  }

  return (
    <input
      ref={input}
      type="text"
      aria-label="Name"
      value={value}
      onChange={(event) => {
        setValue(event.currentTarget.value)
      }}
      onKeyDown={(event) => {
        if (event.key === 'Enter') {
          event.preventDefault()
          finish(value)
        } else if (event.key === 'Escape') {
          event.preventDefault()
          finish(null)
        }
      }}
      onBlur={() => {
        finish(value)
      }}
      className={clsx(
        'absolute top-1/2 left-0 z-10 h-8 w-40 -translate-y-1/2 rounded-sm border px-2 text-sm shadow-lg',
        COLOR_CLASSES.field.background,
        COLOR_CLASSES.accent.border,
        COLOR_CLASSES.text.foreground,
        COLOR_CLASSES.focusRing.outline,
      )}
    />
  )
}

type MarkerProps = {
  readonly position: number
  readonly count: number
  readonly valueText: string
  readonly onPress: (event: PointerEvent<HTMLDivElement>) => void
}

/** The rollback marker: a slider over the gaps 0 to N (3.9). */
function Marker({ position, count, valueText, onPress }: MarkerProps) {
  const app = useApp()

  function onKeyDown(event: KeyboardEvent<HTMLDivElement>): void {
    const target =
      event.key === 'ArrowLeft'
        ? position - 1
        : event.key === 'ArrowRight'
          ? position + 1
          : event.key === 'Home'
            ? 0
            : event.key === 'End'
              ? count
              : null
    if (target === null) {
      return
    }
    event.preventDefault()
    if (target >= 0 && target <= count && target !== position) {
      setRollback(app, target).catch(report('Moving the rollback marker'))
    }
  }

  return (
    <div
      role="slider"
      data-testid="timeline-marker"
      data-strip-item=""
      aria-label="Rollback marker"
      aria-orientation="horizontal"
      aria-valuemin={0}
      aria-valuemax={count}
      aria-valuenow={position}
      aria-valuetext={valueText}
      onKeyDown={onKeyDown}
      onPointerDown={onPress}
      className={clsx(
        'flex h-10 w-2 shrink-0 cursor-ew-resize touch-none justify-center rounded-sm',
        COLOR_CLASSES.focusRing.outline,
        COLOR_CLASSES.hover.background,
      )}
    >
      <span className={clsx('h-full w-0.5', COLOR_CLASSES.accent.background)} />
    </div>
  )
}

function markerText(app: App, recipe: AppState['document'], position: number): string {
  const records = recipe?.recipe.features ?? []
  if (position === 0) {
    return 'At the start'
  }
  if (position >= records.length) {
    return 'At the end'
  }
  const record = records[position - 1]
  return record === undefined || recipe === null
    ? 'At the end'
    : `After ${featureLabel(recipe.recipe, record.id, app.store.getState().catalog)}`
}

/** The strip of items and the marker, with reordering, renaming and the context menu. */
function Strip() {
  const app = useApp()
  const { recipe, statuses, catalog, selection, editing, editedSketch } = useTimelineState()
  const document = useAppState((state) => state.document)
  const strip = useRef<HTMLDivElement>(null)
  const ghost = useRef<HTMLDivElement>(null)
  const bar = useRef<HTMLDivElement>(null)
  const press = useRef<Press | null>(null)
  const lastFocused = useRef<Element | null>(null)
  const menuAnchor = useRef<HTMLButtonElement | null>(null)
  const [menuFor, setMenuFor] = useState<string | null>(null)
  const [renaming, setRenaming] = useState<string | null>(null)
  const [dragging, setDragging] = useState<string | 'marker' | null>(null)
  const records = recipe?.features ?? []
  const rollback = recipe?.rollback ?? null
  const position = markerPosition(records, rollback)
  const editingIndex = editing === null ? -1 : records.findIndex((record) => record.id === editing)
  const selectedKeys = new Set(selection.map((item) => item.key))

  useUiRequest(app, 'rename', (request) => {
    const id = request.key.startsWith('feature:') ? request.key.slice('feature:'.length) : null
    if (request.where !== 'timeline' || id === null || !records.some((record) => record.id === id)) {
      return false
    }
    setRenaming(id)
    return true
  })

  // One item, or the marker, keeps the strip's tab stop (3.24).
  useLayoutEffect(() => {
    const element = strip.current
    if (element === null) {
      return
    }
    const items = [...element.querySelectorAll<HTMLElement>('[data-strip-item]')]
    const stop = items.find((item) => item === lastFocused.current) ?? items[0]
    for (const item of items) {
      item.tabIndex = item === stop ? 0 : -1
    }
  })

  // A newly added item and the moved marker scroll into view (3.9).
  const knownIds = useRef<ReadonlySet<string>>(new Set(records.map((record) => record.id)))
  useEffect(() => {
    const element = strip.current
    if (element === null) {
      return
    }
    const added = records.find((record) => !knownIds.current.has(record.id))
    knownIds.current = new Set(records.map((record) => record.id))
    const target =
      added === undefined
        ? element.querySelector<HTMLElement>('[data-testid="timeline-marker"]')
        : element.querySelector<HTMLElement>(`[data-feature-id="${added.id}"]`)
    if (target !== null) {
      keepInViewHorizontally(element, target)
    }
  }, [records, position])

  // Esc cancels a drag (3.9).
  useEffect(() => {
    if (dragging === null) {
      return undefined
    }
    const onKeyDown = (event: KeyboardEvent | globalThis.KeyboardEvent): void => {
      if (event.key === 'Escape') {
        event.preventDefault()
        const current = press.current
        press.current = null
        setDragging(null)
        current?.rollback?.cancel().catch(report('Canceling the roll back'))
      }
    }
    window.addEventListener('keydown', onKeyDown, true)
    return () => {
      window.removeEventListener('keydown', onKeyDown, true)
    }
  }, [dragging])

  function labelOf(id: string): string {
    return recipe === null ? id : featureLabel(recipe, id, catalog)
  }

  /** The features an action on an item covers: the selected features when it is one of them. */
  function targetsOf(id: string): readonly string[] {
    if (!selectedKeys.has(`feature:${id}`)) {
      return [id]
    }
    return records
      .filter((record) => selectedKeys.has(`feature:${record.id}`))
      .map((record) => record.id)
  }

  function startPress(event: PointerEvent<HTMLElement>, featureId: string | null, index: number): void {
    if (event.button !== 0 || renaming !== null) {
      return
    }
    press.current = {
      pointerId: event.pointerId,
      startX: event.clientX,
      featureId,
      index,
      dragging: false,
      gaps: [],
      gap: index,
      rollback: null,
    }
    // jsdom has no pointer capture; browsers send the drag's moves to the item.
    event.currentTarget.setPointerCapture?.(event.pointerId)
  }

  function onPointerMove(event: PointerEvent<HTMLDivElement>): void {
    const current = press.current
    const element = strip.current
    if (current === null || element === null || current.pointerId !== event.pointerId) {
      return
    }
    if (!current.dragging) {
      if (Math.abs(event.clientX - current.startX) <= DRAG_THRESHOLD_PIXELS) {
        return
      }
      current.dragging = true
      current.gaps = gapPositions(element)
      if (current.featureId === null) {
        current.rollback = startRollbackDrag(app, current.index)
      }
      setDragging(current.featureId ?? 'marker')
    }
    current.gap = nearestGap(current.gaps, event.clientX)
    current.rollback?.move(current.gap)
    const origin = element.getBoundingClientRect()
    if (bar.current !== null) {
      positionElement(bar.current, (current.gaps[current.gap] ?? origin.left) - origin.left, 0)
    }
    if (ghost.current !== null) {
      positionElement(ghost.current, event.clientX - origin.left, 0)
    }
  }

  function onPointerUp(event: PointerEvent<HTMLDivElement>): void {
    const current = press.current
    if (current === null || current.pointerId !== event.pointerId) {
      return
    }
    press.current = null
    if (!current.dragging) {
      return
    }
    setDragging(null)
    if (current.featureId === null) {
      current.rollback?.drop().catch(report('Rolling back'))
      return
    }
    // Dropping at gap g moves the feature to g before its own index, else to g - 1 (3.9).
    const target = current.gap < current.index ? current.gap : current.gap - 1
    if (target !== current.index) {
      moveFeature(app, current.featureId, target).catch(report('Reordering'))
    }
  }

  function onPointerCancel(event: PointerEvent<HTMLDivElement>): void {
    const current = press.current
    if (current === null || current.pointerId !== event.pointerId) {
      return
    }
    press.current = null
    setDragging(null)
    current.rollback?.cancel().catch(report('Canceling the roll back'))
  }

  function moveFocus(event: KeyboardEvent<HTMLDivElement>): void {
    if (event.defaultPrevented || strip.current === null) {
      return
    }
    const items = [...strip.current.querySelectorAll<HTMLElement>('[data-strip-item]')]
    const index = items.findIndex((item) => item === event.target)
    if (index === -1) {
      return
    }
    const last = items.length - 1
    const target =
      event.key === 'ArrowRight'
        ? Math.min(index + 1, last)
        : event.key === 'ArrowLeft'
          ? Math.max(index - 1, 0)
          : event.key === 'Home'
            ? 0
            : event.key === 'End'
              ? last
              : null
    if (target === null) {
      return
    }
    event.preventDefault()
    items[target]?.focus()
  }

  const menuRecord = menuFor === null ? undefined : records.find((record) => record.id === menuFor)
  const menuEntries: MenuEntry[] =
    menuRecord === undefined
      ? []
      : (() => {
          const targets = targetsOf(menuRecord.id)
          const suppressed = menuRecord.suppressed === true
          const index = records.indexOf(menuRecord)
          return [
            {
              kind: 'item',
              label: 'Edit',
              disabled: menuRecord.type === 'import',
              onSelect: () => {
                editFeature(app, menuRecord.id).catch(report('Editing the feature'))
              },
            },
            {
              kind: 'item',
              label: 'Rename',
              onSelect: () => {
                setRenaming(menuRecord.id)
              },
            },
            {
              kind: 'item',
              label: suppressed ? 'Unsuppress' : 'Suppress',
              onSelect: () => {
                setSuppressed(app, targets, !suppressed).catch(report('Suppressing'))
              },
            },
            {
              kind: 'item',
              label: 'Roll back to here',
              onSelect: () => {
                setRollback(app, index + 1).catch(report('Rolling back'))
              },
            },
            {
              kind: 'item',
              label: 'Delete',
              onSelect: () => {
                deleteFeatures(app, targets).catch(report('Deleting'))
              },
            },
          ]
        })()

  const markerElement = (
    <Marker
      key="marker"
      position={position}
      count={records.length}
      valueText={markerText(app, document, position)}
      onPress={(event) => {
        startPress(event, null, position)
      }}
    />
  )

  const children = records.flatMap((record, index) => {
    const item = (
      <TimelineItem
        key={record.id}
        record={record}
        index={index}
        label={labelOf(record.id)}
        status={statuses?.[record.id]}
        rolledBack={index >= position}
        dimmed={index >= position || (editingIndex !== -1 && index > editingIndex)}
        selected={selectedKeys.has(`feature:${record.id}`)}
        outlined={record.id === editing || record.id === editedSketch}
        renaming={renaming === record.id}
        onPress={(event, featureId, itemIndex) => {
          startPress(event, featureId, itemIndex)
        }}
        onPick={(picked, label, toggle) => {
          pickRow(app, featureItem(picked.id, label), toggle)
        }}
        onEdit={(featureId) => {
          editFeature(app, featureId).catch(report('Editing the feature'))
        }}
        onMenu={(featureId, button) => {
          menuAnchor.current = button
          setMenuFor(featureId)
        }}
        onRename={setRenaming}
      />
    )
    return index === position ? [markerElement, item] : [item]
  })
  if (position >= records.length) {
    children.push(markerElement)
  }

  return (
    <div
      ref={strip}
      role="toolbar"
      aria-label="Features"
      onKeyDown={moveFocus}
      onFocus={(event) => {
        if (event.target.hasAttribute('data-strip-item')) {
          lastFocused.current = event.target
        }
      }}
      onPointerMove={onPointerMove}
      onPointerUp={onPointerUp}
      onPointerCancel={onPointerCancel}
      className="relative flex h-full min-w-0 flex-1 items-center gap-1 overflow-x-auto px-1"
    >
      {children}
      {dragging === null ? null : (
        <div
          ref={bar}
          aria-hidden="true"
          className={clsx(
            'pointer-events-none absolute top-2 left-0 h-10 w-0.5',
            COLOR_CLASSES.accent.background,
          )}
        />
      )}
      {dragging === null || dragging === 'marker' ? null : (
        <div
          ref={ghost}
          aria-hidden="true"
          className={clsx(
            'pointer-events-none absolute top-2 left-0 flex size-10 -translate-x-1/2 items-center justify-center rounded-sm border opacity-80 shadow-lg',
            COLOR_CLASSES.surfaceRaised.background,
            COLOR_CLASSES.accent.border,
          )}
        />
      )}
      <Menu
        label="Feature"
        testId="context-menu"
        open={menuRecord !== undefined}
        anchor={menuAnchor}
        entries={menuEntries}
        onOpenChange={(open) => {
          if (!open) {
            setMenuFor(null)
          }
        }}
      />
    </div>
  )
}

function ProblemsButton() {
  const app = useApp()
  const count = useAppState((state) => selectProblems(state).length)
  if (count === 0) {
    return null
  }
  return (
    <Button
      data-testid="timeline-problems"
      className="h-6 shrink-0 gap-1.5 px-2 text-xs tabular-nums"
      onClick={() => {
        app.store.setState((state) => ({ panels: { ...state.panels, side: 'problems' } }))
      }}
    >
      <TriangleAlert
        aria-hidden="true"
        strokeWidth={1.75}
        className={clsx('size-4', COLOR_CLASSES.warningMark.fill, COLOR_CLASSES.warningMark.foreground)}
      />
      {plural(count, 'problem', 'problems')}
    </Button>
  )
}

export function Timeline() {
  const app = useApp()
  const collapsed = useAppState((state) => state.panels.timelineCollapsed)
  const count = useAppState((state) => state.document?.recipe.features?.length ?? 0)
  return (
    <section
      aria-label="Timeline"
      data-testid="timeline"
      className={clsx(
        'relative flex shrink-0 items-center gap-2 border-t px-2',
        collapsed ? 'h-7' : 'h-14',
        COLOR_CLASSES.surface.background,
        COLOR_CLASSES.border.border,
      )}
    >
      <IconButton
        label={collapsed ? 'Expand timeline' : 'Collapse timeline'}
        icon={collapsed ? ChevronUp : ChevronDown}
        size="compact"
        onClick={() => {
          app.store.setState((state) => ({
            panels: { ...state.panels, timelineCollapsed: !state.panels.timelineCollapsed },
          }))
        }}
      />
      {collapsed ? (
        <span className={clsx('flex-1 text-xs tabular-nums', COLOR_CLASSES.textMuted.foreground)}>
          {plural(count, 'feature', 'features')}
        </span>
      ) : (
        <Strip />
      )}
      <ProblemsButton />
      {count === 0 && !collapsed ? (
        <span className="sr-only">{MESSAGES.core_not_running.length > 0 ? '' : ''}</span>
      ) : null}
    </section>
  )
}
