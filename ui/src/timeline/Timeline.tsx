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
import { openSidePanel, togglePanel } from '../app/actions/panels'
import {
  deleteFeatures,
  moveFeature,
  renameFeature,
  setRollback,
  setSuppressed,
  startRollbackDrag,
} from '../app/actions/timeline'
import { useApp, useAppState } from '../app/AppProvider'
import type { App } from '../app/createApp'
import { reportFailure } from '../app/report'
import { markerPosition } from '../core/recipe'
import { FEATURE_ICONS } from '../commands/icons'
import { IconButton } from '../components/IconButton'
import { Menu, type MenuEntry } from '../components/Menu'
import { Tooltip } from '../components/Tooltip'
import { featureLabel } from '../core/labels'
import type { FeatureRecord, FeatureStatus } from '../generated/protocol'
import { featureKey, featureSelectionItem, parseSelectionKey } from '../inspector/selection-values'
import { SIDE_PANEL_ID } from '../layout/panel-ids'
import { activatesRow, pickRow, togglesSelection } from '../layout/pick-row'
import { RenameField } from '../layout/RenameField'
import { useUiRequest } from '../layout/requests'
import { arrowTarget, rovingTabStop } from '../layout/roving'
import { keepInViewHorizontally } from '../layout/scroll'
import { counted } from '../messages'
import { selectCommandBlock, selectProblems } from '../state/selectors'
import { COLOR_CLASSES } from '../theme/roles'
import { itemStatus, markerText, statusText } from './strip'
import { useStripDrag } from './use-strip-drag'

// The timeline (ui.md 3.9): the features in timeline order with the rollback marker in its gap,
// reordering by drag, inline rename, a context menu, and the problems count. The strip is a
// toolbar with one tab stop: Left and Right move between the items and the marker.

const NO_FEATURES: readonly FeatureRecord[] = []

/** `MouseEvent.button` of the secondary (right) button. */
const SECONDARY_BUTTON_INDEX = 2
/** The secondary button's bit in `MouseEvent.buttons`. */
const SECONDARY_BUTTON_BIT = 2

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
  // The selection inspector's "Made by" row hovers the feature it names (3.8).
  const hovered = useAppState((state) => state.hover?.key === featureKey(record.id))
  const catalog = useAppState((state) => state.catalog)
  const suppressed = record.suppressed === true
  const description = statusText(status, suppressed)
  const iconName = catalog?.byType[record.type]?.manifest.icon
  const Icon = iconName === undefined ? Shapes : (FEATURE_ICONS[iconName] ?? Shapes)
  const state = status?.status
  const messages = status?.messages.map((message) => message.message) ?? []
  const tooltip = [label, ...messages].join(' ')
  // On Linux and macOS the browser fires `contextmenu` on the right button's press. A menu opened
  // then would be light-dismissed by the release, so it opens on the release instead (3.13).
  const menuOnRelease = useRef(false)

  function onKeyDown(event: KeyboardEvent<HTMLButtonElement>): void {
    if (activatesRow(event)) {
      event.preventDefault()
      // A held key edits once.
      if (!event.repeat) {
        onEdit(record.id)
      }
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
          aria-pressed={selected}
          aria-describedby={description === null ? undefined : descriptionId}
          onClick={(event) => {
            onPick(record, label, togglesSelection(app, event))
          }}
          onDoubleClick={() => {
            onEdit(record.id)
          }}
          onKeyDown={onKeyDown}
          onPointerDown={() => {
            menuOnRelease.current = false
          }}
          onPointerUp={(event) => {
            if (event.button === SECONDARY_BUTTON_INDEX && menuOnRelease.current) {
              menuOnRelease.current = false
              onMenu(record.id, event.currentTarget)
            }
          }}
          onContextMenu={(event) => {
            event.preventDefault()
            if ((event.buttons & SECONDARY_BUTTON_BIT) !== 0) {
              menuOnRelease.current = true
              return
            }
            onMenu(record.id, event.currentTarget)
          }}
          className={clsx(
            'relative flex size-10 items-center justify-center rounded-sm transition-colors duration-150 motion-reduce:transition-none',
            COLOR_CLASSES.focusRing.outline,
            COLOR_CLASSES.accent.outline,
            COLOR_CLASSES.text.foreground,
            outlined && 'outline-2 -outline-offset-2',
            selected
              ? COLOR_CLASSES.selection.background
              : hovered
                ? COLOR_CLASSES.hover.active
                : COLOR_CLASSES.hover.background,
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
              strokeWidth={1.75}
              className={clsx(
                'absolute right-0 bottom-0 size-3.5 rounded-full',
                COLOR_CLASSES.surfaceRaised.background,
                COLOR_CLASSES.errorMark.foreground,
              )}
            />
          ) : state === 'warning' ? (
            <TriangleAlert
              aria-hidden="true"
              strokeWidth={1.75}
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
          className="absolute top-1/2 left-0 z-10 w-40 -translate-y-1/2 shadow-lg"
          onDone={(name) => {
            onRename(null)
            if (name !== null && name.trim() !== label) {
              renameFeature(app, record.id, name).catch(reportFailure('Renaming the feature'))
            }
          }}
        />
      ) : null}
    </div>
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
      setRollback(app, target).catch(reportFailure('Moving the rollback marker'))
    }
  }

  return (
    <div
      // A slider over the strip's gaps: every step applies its own edit and a drag follows the
      // items, which a native range input cannot do.
      // oxlint-disable-next-line jsx-a11y/prefer-tag-over-role
      role="slider"
      tabIndex={-1}
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

/** A timeline item's context menu (3.9); Suppress and Delete cover `targets`. */
function featureMenuEntries(
  app: App,
  record: FeatureRecord,
  records: readonly FeatureRecord[],
  targets: readonly string[],
  blocked: boolean,
  setRenaming: (id: string) => void,
): MenuEntry[] {
  const suppressed = record.suppressed === true
  const index = records.indexOf(record)
  return [
    {
      kind: 'item',
      label: 'Edit',
      disabled: record.type === 'import',
      onSelect: () => {
        editFeature(app, record.id).catch(reportFailure('Editing the feature'))
      },
    },
    {
      kind: 'item',
      label: 'Rename',
      disabled: blocked,
      onSelect: () => {
        setRenaming(record.id)
      },
    },
    {
      kind: 'item',
      label: suppressed ? 'Unsuppress' : 'Suppress',
      disabled: blocked,
      onSelect: () => {
        setSuppressed(app, targets, !suppressed).catch(reportFailure('Suppressing'))
      },
    },
    {
      kind: 'item',
      label: 'Roll back to here',
      disabled: blocked,
      onSelect: () => {
        setRollback(app, index + 1).catch(reportFailure('Rolling back'))
      },
    },
    {
      kind: 'item',
      label: 'Delete',
      disabled: blocked,
      onSelect: () => {
        deleteFeatures(app, targets).catch(reportFailure('Deleting'))
      },
    },
  ]
}

/** The strip of items and the marker, with reordering, renaming and the context menu. */
function Strip() {
  const app = useApp()
  const { recipe, statuses, catalog, selection, editing, editedSketch } = useTimelineState()
  const strip = useRef<HTMLDivElement>(null)
  const lastFocused = useRef<Element | null>(null)
  const menuAnchor = useRef<HTMLButtonElement | null>(null)
  const [menuFor, setMenuFor] = useState<string | null>(null)
  const [renaming, setRenaming] = useState<string | null>(null)
  // Timeline edits wait for an open feature command (3.7); editing another feature ends it.
  const blocked = useAppState(selectCommandBlock) !== null
  const records = recipe?.features ?? NO_FEATURES
  const rollback = recipe?.rollback ?? null
  const position = markerPosition(records, rollback)
  const editingIndex = editing === null ? -1 : records.findIndex((record) => record.id === editing)
  const selectedKeys = new Set(selection.map((item) => item.key))

  useUiRequest(app, 'rename', (request) => {
    const parsed = parseSelectionKey(request.key)
    if (
      request.where !== 'timeline' ||
      parsed?.kind !== 'feature' ||
      !records.some((record) => record.id === parsed.id)
    ) {
      return false
    }
    setRenaming(parsed.id)
    return true
  })

  // A feature chosen in the palette scrolls into view (3.4); a strip that is not shown drops it.
  useUiRequest(app, 'reveal', (request) => {
    const element = strip.current
    const target =
      element?.querySelector<HTMLElement>(`[data-feature-id="${request.feature}"]`) ?? null
    if (element !== null && target !== null) {
      keepInViewHorizontally(element, target)
    }
    return true
  })

  // One item, or the marker, keeps the strip's tab stop (3.24).
  useLayoutEffect(() => {
    const element = strip.current
    if (element === null) {
      return
    }
    rovingTabStop(element, '[data-strip-item]', lastFocused.current)
  })

  // A newly added item and the moved marker scroll into view (3.9).
  const shown = useRef({ ids: new Set(records.map((record) => record.id)), position })
  useEffect(() => {
    const element = strip.current
    if (element === null) {
      return
    }
    const previous = shown.current
    shown.current = { ids: new Set(records.map((record) => record.id)), position }
    const added = records.find((record) => !previous.ids.has(record.id))
    const target =
      added !== undefined
        ? element.querySelector<HTMLElement>(`[data-feature-id="${added.id}"]`)
        : position === previous.position
          ? null
          : element.querySelector<HTMLElement>('[role="slider"]')
    if (target !== null) {
      keepInViewHorizontally(element, target)
    }
  }, [records, position])

  const { dragging, bar, ghost, startPress, onPointerMove, onPointerUp, onPointerCancel } =
    useStripDrag(strip, {
      // The timeline drag and the marker do not start during a feature command (3.7).
      canPress: () => renaming === null && app.store.getState().command === null,
      onRollback: (index) => startRollbackDrag(app, index),
      onMove: (featureId, target) => {
        moveFeature(app, featureId, target).catch(reportFailure('Reordering'))
      },
    })

  function labelOf(id: string): string {
    return recipe === null ? id : featureLabel(recipe, id, catalog)
  }

  /** The features an action on an item covers: the selected features when it is one of them. */
  function targetsOf(id: string): readonly string[] {
    if (!selectedKeys.has(featureKey(id))) {
      return [id]
    }
    return records
      .filter((record) => selectedKeys.has(featureKey(record.id)))
      .map((record) => record.id)
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
    const target = arrowTarget(event.key, index, items.length - 1, { wrap: false })
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
      : featureMenuEntries(app, menuRecord, records, targetsOf(menuRecord.id), blocked, setRenaming)

  const markerElement = (
    <Marker
      key="marker"
      position={position}
      count={records.length}
      valueText={markerText(records, position, labelOf)}
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
        selected={selectedKeys.has(featureKey(record.id))}
        outlined={record.id === editing || record.id === editedSketch}
        renaming={renaming === record.id}
        onPress={startPress}
        onPick={(picked, label, toggle) => {
          pickRow(app, featureSelectionItem(picked.id, label), toggle)
        }}
        onEdit={(featureId) => {
          editFeature(app, featureId).catch(reportFailure('Editing the feature'))
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
      tabIndex={-1}
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
      {records.length === 0 ? (
        <p
          data-testid="timeline-empty"
          className={clsx('px-2 text-xs', COLOR_CLASSES.textMuted.foreground)}
        >
          Each step you add appears here. Double-click a step to change it later.
        </p>
      ) : null}
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
            'pointer-events-none absolute top-2 left-0 flex size-10 items-center justify-center rounded-sm border opacity-80 shadow-lg',
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
  const problems = useAppState(selectProblems)
  const count = problems.length
  if (count === 0) {
    return null
  }
  // The icon names the worst problem, as status icons do everywhere (3.24).
  const error = problems.some((problem) =>
    problem.kind === 'feature'
      ? problem.status === 'error'
      : problem.diagnostic.severity === 'error',
  )
  return (
    <button
      type="button"
      data-testid="timeline-problems"
      aria-controls={SIDE_PANEL_ID}
      onClick={() => {
        openSidePanel(app, 'problems')
      }}
      className={clsx(
        'inline-flex h-6 shrink-0 items-center gap-1.5 rounded-sm px-2 text-xs whitespace-nowrap tabular-nums transition-colors duration-150 motion-reduce:transition-none',
        COLOR_CLASSES.text.foreground,
        COLOR_CLASSES.hover.background,
        COLOR_CLASSES.focusRing.outline,
        COLOR_CLASSES.accent.outline,
      )}
    >
      {error ? (
        <CircleX
          aria-hidden="true"
          strokeWidth={1.75}
          className={clsx('size-4', COLOR_CLASSES.errorMark.foreground)}
        />
      ) : (
        <TriangleAlert
          aria-hidden="true"
          strokeWidth={1.75}
          className={clsx(
            'size-4',
            COLOR_CLASSES.warningMark.fill,
            COLOR_CLASSES.warningMark.foreground,
          )}
        />
      )}
      {counted(count, 'problem')}
    </button>
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
        'relative col-span-3 row-start-3 flex min-w-0 items-center gap-2 border-t px-2',
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
          togglePanel(app, 'timelineCollapsed')
        }}
      />
      {collapsed ? (
        <span className={clsx('flex-1 text-xs tabular-nums', COLOR_CLASSES.textMuted.foreground)}>
          {counted(count, 'feature')}
        </span>
      ) : (
        <Strip />
      )}
      <ProblemsButton />
    </section>
  )
}
