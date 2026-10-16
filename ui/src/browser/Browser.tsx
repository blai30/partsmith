import { clsx } from 'clsx/lite'
import {
  ChevronDown,
  ChevronRight,
  Dot,
  Eye,
  EyeOff,
  Lock,
  LockOpen,
  MoveRight,
  SlidersHorizontal,
  Square,
  type LucideIcon,
} from 'lucide-react'
import {
  useLayoutEffect,
  useRef,
  useState,
  type KeyboardEvent,
  type MouseEvent,
  type ReactNode,
} from 'react'
import { renameBody, setBodyVisible, setLocked, setSessionVisible } from '../app/actions/bodies'
import { editFeature } from '../app/actions/command'
import { openSidePanel, setBrowserOpen } from '../app/actions/panels'
import { enterSketch } from '../app/actions/sketch'
import { renameFeature } from '../app/actions/timeline'
import { useApp, useAppState } from '../app/AppProvider'
import type { App } from '../app/createApp'
import { reportFailure } from '../app/report'
import { featureIcon } from '../commands/icons'
import { explainDisabled } from '../commands/registry'
import { Tooltip } from '../components/Tooltip'
import { featureLabel, type WorldAxis } from '../core/labels'
import { formatParameterValue } from '../expression/evaluate'
import { isPlainNumber } from '../expression/step'
import type { Body, FeatureRecord, Parameter } from '../generated/protocol'
import { activatesRow, pickRow, togglesSelection } from '../layout/pick-row'
import { RenameField } from '../layout/RenameField'
import { BROWSER_PANEL_ID } from '../layout/panel-ids'
import { requestUi, useUiRequest } from '../layout/requests'
import {
  bodyKey,
  bodySelectionItem,
  featureKey,
  featureSelectionItem,
  originItem,
  parameterKey,
  parameterSelectionItem,
  parseSelectionKey,
} from '../inspector/selection-values'
import { rovingTabStop } from '../layout/roving'
import { useOverlayDismiss } from '../layout/use-overlay-dismiss'
import { SEVERITY_NAMES } from '../print/health'
import { SeverityIcon } from '../print/SeverityIcon'
import { selectCommandBlock } from '../state/selectors'
import type { AppState, OriginName, SelectionItem } from '../state/store'
import { isSessionVisible } from '../state/visibility'
import { AXIS_ROLES, COLOR_CLASSES } from '../theme/roles'
import { BodyColorButton } from './BodyColor'

// The browser (ui.md 3.10): a tree of the origin, bodies, sketches, datum planes and parameters.
// Its treeitems form one flat list with levels, so each row is one focusable line; row buttons stay
// out of the tab order, and their actions are also reachable by keyboard elsewhere (3.10).

type GroupId = 'origin' | 'bodies' | 'sketches' | 'datums' | 'parameters'

type RowKind =
  | 'origin_plane'
  | 'origin_axis'
  | 'origin_point'
  | 'body'
  | 'sketch'
  | 'datum'
  | 'parameter'

/** The group that shows the row a rename names: bodies, sketches or datum planes; null for anything else. */
function renameGroup(
  key: string,
  bodies: readonly Body[],
  features: readonly FeatureRecord[],
): GroupId | null {
  const parsed = parseSelectionKey(key)
  if (parsed?.kind === 'body') {
    return bodies.some((body) => body.id === parsed.id) ? 'bodies' : null
  }
  const id = parsed?.kind === 'feature' ? parsed.id : null
  const record = features.find((feature) => feature.id === id)
  switch (record?.type) {
    case 'sketch':
      return 'sketches'
    case 'datum_plane':
      return 'datums'
    default:
      return null
  }
}

const DEFAULT_EXPANDED: Readonly<Record<GroupId, boolean>> = {
  origin: false,
  bodies: true,
  sketches: true,
  datums: true,
  parameters: true,
}

/** Group expansion, kept for the session even while the browser is closed (3.10). */
const sessionExpansion = new WeakMap<App, Record<GroupId, boolean>>()

const ORIGIN_ROWS: readonly {
  readonly origin: OriginName
  readonly kind: 'origin_plane' | 'origin_axis' | 'origin_point'
  readonly icon: LucideIcon
}[] = [
  { origin: 'XY', kind: 'origin_plane', icon: Square },
  { origin: 'XZ', kind: 'origin_plane', icon: Square },
  { origin: 'YZ', kind: 'origin_plane', icon: Square },
  { origin: 'X', kind: 'origin_axis', icon: MoveRight },
  { origin: 'Y', kind: 'origin_axis', icon: MoveRight },
  { origin: 'Z', kind: 'origin_axis', icon: MoveRight },
  { origin: 'O', kind: 'origin_point', icon: Dot },
]

function isWorldAxis(origin: OriginName): origin is WorldAxis {
  return origin === 'X' || origin === 'Y' || origin === 'Z'
}

/**
 * An origin row's icon color: an axis row's in its axis role, beside the name that carries its
 * letter; a plane's name carries the letters of its own two axes, so its icon stays muted (3.10).
 */
function originIconColor(origin: OriginName): string {
  return isWorldAxis(origin)
    ? COLOR_CLASSES[AXIS_ROLES[origin]].foreground
    : COLOR_CLASSES.textMuted.foreground
}

/** Stops a row button's click from also selecting the row. */
function own(handler: () => void): (event: MouseEvent) => void {
  return (event) => {
    event.stopPropagation()
    handler()
  }
}

type RowButtonProps = {
  readonly label: string
  readonly icon: LucideIcon
  readonly pressed?: boolean
  /** Why the button cannot act now, shown as its tooltip; a click shows it as a notification. */
  readonly disabledReason?: string | null
  readonly onClick: () => void
}

/** A small icon button inside a row, outside the tab order (3.10). */
function RowButton({ label, icon: Icon, pressed, disabledReason = null, onClick }: RowButtonProps) {
  const app = useApp()
  const button = (
    <button
      type="button"
      tabIndex={-1}
      aria-label={label}
      aria-pressed={pressed}
      aria-disabled={disabledReason === null ? undefined : true}
      onClick={own(() => {
        if (disabledReason !== null) {
          explainDisabled(app, { reason: disabledReason, code: 'finish_command_first' })
          return
        }
        onClick()
      })}
      onDoubleClick={(event) => {
        event.stopPropagation()
      }}
      className={clsx(
        'inline-flex size-6 shrink-0 items-center justify-center rounded-md transition-colors duration-150 motion-reduce:transition-none',
        disabledReason === null
          ? COLOR_CLASSES.textMuted.foreground
          : COLOR_CLASSES.textDisabled.foreground,
        disabledReason === null ? COLOR_CLASSES.hover.background : 'cursor-not-allowed',
      )}
    >
      <Icon aria-hidden="true" strokeWidth={1.75} className="size-4" />
    </button>
  )
  return disabledReason === null ? button : <Tooltip label={disabledReason}>{button}</Tooltip>
}

type RowProps = {
  readonly kind: RowKind
  readonly id: string
  readonly selection: SelectionItem
  readonly selected: boolean
  readonly icon?: ReactNode
  readonly children: ReactNode
  readonly actions?: ReactNode
  /** Double-click or Enter. */
  readonly onActivate?: () => void
  /** F2, for rows that rename. */
  readonly onRename?: () => void
}

function Row({
  kind,
  id,
  selection,
  selected,
  icon,
  children,
  actions,
  onActivate,
  onRename,
}: RowProps) {
  const app = useApp()
  return (
    <li
      role="treeitem"
      aria-level={2}
      aria-selected={selected}
      data-testid="browser-row"
      data-tree-item=""
      data-kind={kind}
      data-id={id}
      tabIndex={-1}
      onClick={(event) => {
        pickRow(app, selection, togglesSelection(app, event))
      }}
      onDoubleClick={() => {
        onActivate?.()
      }}
      onKeyDown={(event) => {
        if (event.target !== event.currentTarget) {
          return
        }
        // Enter with Ctrl, Cmd or Alt stays a shortcut (Mod+Enter finishes a sketch, 3.5), and a
        // row with nothing to activate leaves Enter to the Enter ladder.
        if (activatesRow(event)) {
          if (onActivate !== undefined) {
            event.preventDefault()
            onActivate()
          }
        } else if (event.key === ' ') {
          event.preventDefault()
          pickRow(app, selection, togglesSelection(app, event))
        } else if (event.key === 'F2' && onRename !== undefined) {
          event.preventDefault()
          onRename()
        }
      }}
      className={clsx(
        'flex h-8 cursor-default items-center gap-2 rounded-md pr-1 pl-7 text-sm',
        COLOR_CLASSES.focusRing.outline,
        // The transparent state outline, which forced colors draw on a selected row (3.23).
        COLOR_CLASSES.forcedEdge.outline,
        selected && 'outline -outline-offset-1',
        selected ? COLOR_CLASSES.selection.background : COLOR_CLASSES.hover.background,
      )}
    >
      {icon}
      <span className="flex min-w-0 flex-1 items-center gap-2">{children}</span>
      {actions}
    </li>
  )
}

/** The name of a row, or the inline rename field in its place. */
function RowName({
  name,
  renaming,
  onRenamed,
}: {
  readonly name: string
  readonly renaming: boolean
  readonly onRenamed: (name: string | null) => void
}) {
  if (renaming) {
    return <RenameField initial={name} onDone={onRenamed} className="min-w-0 flex-1" />
  }
  return <span className="truncate">{name}</span>
}

type GroupProps = {
  readonly id: GroupId
  readonly title: string
  readonly expanded: boolean
  readonly onToggle: (id: GroupId) => void
  readonly actions?: ReactNode
}

/** A group's treeitem: its title, a chevron and its actions. Its rows follow it in the list. */
function GroupHeader({ id, title, expanded, onToggle, actions }: GroupProps) {
  const Chevron = expanded ? ChevronDown : ChevronRight
  return (
    <li
      role="treeitem"
      aria-level={1}
      aria-expanded={expanded}
      aria-selected={false}
      data-tree-item=""
      data-group={id}
      tabIndex={-1}
      onClick={() => {
        onToggle(id)
      }}
      onKeyDown={(event) => {
        if (event.target === event.currentTarget && (activatesRow(event) || event.key === ' ')) {
          event.preventDefault()
          onToggle(id)
        }
      }}
      className={clsx(
        'mt-2 flex h-8 cursor-default items-center gap-1 rounded-md px-1 text-xs font-semibold first:mt-0',
        COLOR_CLASSES.textMuted.foreground,
        COLOR_CLASSES.focusRing.outline,
        COLOR_CLASSES.accent.outline,
        COLOR_CLASSES.hover.background,
      )}
    >
      <Chevron aria-hidden="true" strokeWidth={1.75} className="size-4 shrink-0" />
      <span className="flex-1">{title}</span>
      {actions}
    </li>
  )
}

/** The status badge of a sketch or datum row, as in the timeline (3.10). */
function StatusBadge({ featureId }: { readonly featureId: string }) {
  const status = useAppState((state) => state.document?.featureStatus[featureId]?.status)
  return status === 'error' || status === 'warning' ? (
    <SeverityIcon severity={status} label={SEVERITY_NAMES[status]} />
  ) : null
}

function rowIcon(Icon: LucideIcon, color: string = COLOR_CLASSES.textMuted.foreground): ReactNode {
  return <Icon aria-hidden="true" strokeWidth={1.75} className={clsx('size-4 shrink-0', color)} />
}

function useSelectedKeys(): ReadonlySet<string> {
  const selection = useAppState((state) => state.selection)
  return new Set(selection.map((selected) => selected.key))
}

/** The session eye of a sketch, datum plane or origin item. */
function SessionEye({ itemKey, name }: { readonly itemKey: string; readonly name: string }) {
  const app = useApp()
  const visible = useAppState((state: AppState) => isSessionVisible(state, itemKey))
  return (
    <RowButton
      label={`${visible ? 'Hide' : 'Show'} ${name}`}
      icon={visible ? Eye : EyeOff}
      onClick={() => {
        setSessionVisible(app, itemKey, !visible)
      }}
    />
  )
}

function OriginRows({ selected }: { readonly selected: ReadonlySet<string> }) {
  return ORIGIN_ROWS.map((row) => {
    const selection = originItem(row.origin)
    return (
      <Row
        key={selection.key}
        kind={row.kind}
        id={row.origin}
        selection={selection}
        selected={selected.has(selection.key)}
        icon={rowIcon(row.icon, originIconColor(row.origin))}
        actions={<SessionEye itemKey={selection.key} name={selection.label} />}
      >
        <span className="truncate">{selection.label}</span>
      </Row>
    )
  })
}

function BodyRow({
  body,
  selected,
  renaming,
  onRename,
}: {
  readonly body: Body
  readonly selected: boolean
  readonly renaming: boolean
  readonly onRename: (key: string | null) => void
}) {
  const app = useApp()
  const empty = useAppState((state) => state.document?.meshes[body.id] === undefined)
  const locked = useAppState((state) => state.view.locked.includes(body.id))
  // Visibility is a document edit, which waits for an open command (3.7).
  const blocked = useAppState(selectCommandBlock)
  const visible = body.visible !== false
  const key = bodyKey(body.id)
  return (
    <Row
      kind="body"
      id={body.id}
      selection={bodySelectionItem(body.id, body.name)}
      selected={selected}
      icon={<BodyColorButton body={body} tabbable={false} />}
      onActivate={() => {
        onRename(key)
      }}
      onRename={() => {
        onRename(key)
      }}
      actions={
        <>
          <RowButton
            label={`${locked ? 'Unlock' : 'Lock'} ${body.name}`}
            icon={locked ? Lock : LockOpen}
            pressed={locked}
            onClick={() => {
              setLocked(app, body.id, !locked)
            }}
          />
          <RowButton
            label={`${visible ? 'Hide' : 'Show'} ${body.name}`}
            icon={visible ? Eye : EyeOff}
            disabledReason={blocked}
            onClick={() => {
              setBodyVisible(app, body.id, !visible).catch(reportFailure('Changing the visibility'))
            }}
          />
        </>
      }
    >
      <RowName
        name={body.name}
        renaming={renaming}
        onRenamed={(name) => {
          onRename(null)
          if (name !== null && name.trim() !== '' && name.trim() !== body.name) {
            renameBody(app, body.id, name).catch(reportFailure('Renaming the body'))
          }
        }}
      />
      {empty ? (
        <span className={clsx('shrink-0 text-xs', COLOR_CLASSES.textMuted.foreground)}>
          (empty)
        </span>
      ) : null}
    </Row>
  )
}

function FeatureRow({
  record,
  kind,
  label,
  selected,
  renaming,
  onRename,
}: {
  readonly record: FeatureRecord
  readonly kind: 'sketch' | 'datum'
  readonly label: string
  readonly selected: boolean
  readonly renaming: boolean
  readonly onRename: (key: string | null) => void
}) {
  const app = useApp()
  const iconName = useAppState((state) => state.catalog?.byType[record.type]?.manifest.icon)
  const Icon = featureIcon(iconName)
  const key = featureKey(record.id)
  const selectionKind = kind === 'sketch' ? 'feature' : 'datum_plane'
  return (
    <Row
      kind={kind}
      id={record.id}
      selection={featureSelectionItem(record.id, label, selectionKind)}
      selected={selected}
      icon={rowIcon(Icon)}
      onActivate={() => {
        if (kind === 'sketch') {
          enterSketch(app, record.id).catch(reportFailure('Entering the sketch'))
        } else {
          editFeature(app, record.id).catch(reportFailure('Editing the datum plane'))
        }
      }}
      onRename={() => {
        onRename(key)
      }}
      actions={<SessionEye itemKey={key} name={label} />}
    >
      <RowName
        name={label}
        renaming={renaming}
        onRenamed={(name) => {
          onRename(null)
          if (name !== null && name.trim() !== label) {
            renameFeature(app, record.id, name).catch(reportFailure('Renaming the feature'))
          }
        }}
      />
      <StatusBadge featureId={record.id} />
    </Row>
  )
}

function ParameterRow({
  parameter,
  selected,
}: {
  readonly parameter: Parameter
  readonly selected: boolean
}) {
  const app = useApp()
  const value = useAppState((state) => state.document?.parameterValues[parameter.id])
  const units = useAppState((state) => state.document?.recipe.units ?? 'mm')
  const text = `${parameter.name} = ${parameter.expr}`
  const evaluated = isPlainNumber(parameter.expr) ? null : formatParameterValue(value, units)
  return (
    <Row
      kind="parameter"
      id={parameter.id}
      selection={parameterSelectionItem(parameter.id, parameter.name)}
      selected={selected}
      icon={rowIcon(SlidersHorizontal)}
      onActivate={() => {
        openSidePanel(app, 'parameters')
        requestUi(app, { kind: 'parameter', id: parameter.id })
      }}
    >
      {/* The code and the smaller value share a baseline, which centering them would not give. */}
      <span className="flex min-w-0 items-baseline gap-2">
        <span className="truncate font-mono">{text}</span>
        {evaluated === null ? null : (
          <span
            className={clsx('shrink-0 text-xs tabular-nums', COLOR_CLASSES.textMuted.foreground)}
          >
            {evaluated}
          </span>
        )}
      </span>
    </Row>
  )
}

/** Moves focus through the visible treeitems (WAI-ARIA tree pattern, no type-ahead). */
function treeKeyDown(
  event: KeyboardEvent<HTMLUListElement>,
  expanded: Readonly<Record<GroupId, boolean>>,
  toggle: (id: GroupId) => void,
): void {
  const items = [...event.currentTarget.querySelectorAll<HTMLElement>('[data-tree-item]')]
  const current = items.findIndex((element) => element === event.target)
  if (current === -1) {
    return
  }
  const element = items[current]
  const group = element?.dataset['group'] as GroupId | undefined
  const focus = (index: number): void => {
    event.preventDefault()
    items[Math.min(Math.max(index, 0), items.length - 1)]?.focus()
  }
  switch (event.key) {
    case 'ArrowDown':
      focus(current + 1)
      return
    case 'ArrowUp':
      focus(current - 1)
      return
    case 'Home':
      focus(0)
      return
    case 'End':
      focus(items.length - 1)
      return
    case 'ArrowRight':
      if (group !== undefined) {
        event.preventDefault()
        if (expanded[group]) {
          focus(current + 1)
        } else {
          toggle(group)
        }
      }
      return
    case 'ArrowLeft': {
      event.preventDefault()
      if (group !== undefined) {
        if (expanded[group]) {
          toggle(group)
        }
        return
      }
      // A row goes to its group's header.
      const parent = items
        .slice(0, current)
        .findLastIndex((candidate) => candidate.dataset['group'] !== undefined)
      if (parent !== -1) {
        focus(parent)
      }
      return
    }
    default:
  }
}

export function Browser() {
  const app = useApp()
  const recipe = useAppState((state) => state.document?.recipe ?? null)
  const catalog = useAppState((state) => state.catalog)
  const selected = useSelectedKeys()
  const panel = useRef<HTMLElement>(null)
  const tree = useRef<HTMLUListElement>(null)
  const lastFocused = useRef<Element | null>(null)
  const [expanded, setExpanded] = useState<Record<GroupId, boolean>>(
    () => sessionExpansion.get(app) ?? DEFAULT_EXPANDED,
  )
  const [renaming, setRenaming] = useState<string | null>(null)

  useOverlayDismiss(panel, () => {
    setBrowserOpen(app, false)
  })

  // Absent lists are empty; anything else is used as the protocol gives it.
  const { bodies = [], features = [], parameters = [] } = recipe ?? {}
  const sketches = features.filter((record) => record.type === 'sketch')
  const datums = features.filter((record) => record.type === 'datum_plane')

  function toggle(id: GroupId): void {
    setExpanded((current) => {
      const next = { ...current, [id]: !current[id] }
      sessionExpansion.set(app, next)
      return next
    })
  }

  useUiRequest(app, 'rename', (request) => {
    if (request.where !== 'browser') {
      return false
    }
    const group = renameGroup(request.key, bodies, features)
    if (group === null) {
      return false
    }
    if (!expanded[group]) {
      toggle(group)
    }
    setRenaming(request.key)
    return true
  })

  // One treeitem keeps the tree's tab stop: the last focused one while it is shown, else the first.
  useLayoutEffect(() => {
    const element = tree.current
    if (element === null) {
      return
    }
    rovingTabStop(element, '[data-tree-item]', lastFocused.current)
  })

  function labelOf(record: FeatureRecord): string {
    return recipe === null ? record.id : featureLabel(recipe, record.id, catalog)
  }

  return (
    <nav
      ref={panel}
      id={BROWSER_PANEL_ID}
      aria-label="Browser"
      data-testid="browser"
      // A frame island (3.2), docked in the first column from 65rem and floating over the free
      // area's cell below it, a step raised over the cards there; its tree scrolls inside it.
      className={clsx(
        'relative z-10 col-start-1 row-start-2 mr-2 flex min-h-0 w-66 max-w-full flex-col overflow-hidden rounded-xl border shadow-md @max-5xl/app:absolute @max-5xl/app:inset-y-0 @max-5xl/app:left-0 @max-5xl/app:z-20 @max-5xl/app:col-start-2 @max-5xl/app:col-end-3 @max-5xl/app:row-end-3 @max-5xl/app:shadow-lg',
        COLOR_CLASSES.surface.background,
        COLOR_CLASSES.surfaceRaised.overlay,
        COLOR_CLASSES.forcedEdge.border,
        COLOR_CLASSES.elevation.shadow,
        COLOR_CLASSES.text.foreground,
      )}
    >
      <div className="min-h-0 flex-1 overflow-y-auto">
        <ul
          ref={tree}
          role="tree"
          aria-label="Browser"
          onKeyDown={(event) => {
            treeKeyDown(event, expanded, toggle)
          }}
          onFocus={(event) => {
            if (event.target.hasAttribute('data-tree-item')) {
              lastFocused.current = event.target
            }
          }}
          className="flex flex-col p-1"
        >
          <GroupHeader id="origin" title="Origin" expanded={expanded.origin} onToggle={toggle} />
          {expanded.origin ? <OriginRows selected={selected} /> : null}
          <GroupHeader id="bodies" title="Bodies" expanded={expanded.bodies} onToggle={toggle} />
          {expanded.bodies
            ? bodies.map((body) => (
                <BodyRow
                  key={body.id}
                  body={body}
                  selected={selected.has(bodyKey(body.id))}
                  renaming={renaming === bodyKey(body.id)}
                  onRename={setRenaming}
                />
              ))
            : null}
          <GroupHeader
            id="sketches"
            title="Sketches"
            expanded={expanded.sketches}
            onToggle={toggle}
          />
          {expanded.sketches
            ? sketches.map((record) => (
                <FeatureRow
                  key={record.id}
                  record={record}
                  kind="sketch"
                  label={labelOf(record)}
                  selected={selected.has(featureKey(record.id))}
                  renaming={renaming === featureKey(record.id)}
                  onRename={setRenaming}
                />
              ))
            : null}
          <GroupHeader
            id="datums"
            title="Datum planes"
            expanded={expanded.datums}
            onToggle={toggle}
          />
          {expanded.datums
            ? datums.map((record) => (
                <FeatureRow
                  key={record.id}
                  record={record}
                  kind="datum"
                  label={labelOf(record)}
                  selected={selected.has(featureKey(record.id))}
                  renaming={renaming === featureKey(record.id)}
                  onRename={setRenaming}
                />
              ))
            : null}
          <GroupHeader
            id="parameters"
            title="Parameters"
            expanded={expanded.parameters}
            onToggle={toggle}
            actions={
              <RowButton
                label="Edit parameters"
                icon={SlidersHorizontal}
                onClick={() => {
                  openSidePanel(app, 'parameters')
                }}
              />
            }
          />
          {expanded.parameters
            ? parameters.map((parameter) => (
                <ParameterRow
                  key={parameter.id}
                  parameter={parameter}
                  selected={selected.has(parameterKey(parameter.id))}
                />
              ))
            : null}
        </ul>
      </div>
    </nav>
  )
}
