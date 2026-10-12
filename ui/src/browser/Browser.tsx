import { clsx } from 'clsx/lite'
import {
  ChevronDown,
  ChevronRight,
  CircleX,
  Dot,
  Eye,
  EyeOff,
  Lock,
  LockOpen,
  MoveRight,
  Shapes,
  SlidersHorizontal,
  Square,
  TriangleAlert,
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
import { enterSketch } from '../app/actions/sketch'
import { renameFeature } from '../app/actions/timeline'
import { useApp, useAppState } from '../app/AppProvider'
import type { App } from '../app/createApp'
import { FEATURE_ICONS } from '../commands/icons'
import { featureLabel } from '../core/labels'
import { formatEvaluated } from '../expression/evaluate'
import { isPlainNumber } from '../expression/step'
import type { Body, FeatureRecord, Parameter } from '../generated/protocol'
import { pickRow, togglesSelection } from '../layout/pick-row'
import { RenameField } from '../layout/RenameField'
import { requestUi, useUiRequest } from '../layout/requests'
import type { AppState, OriginName, SelectionItem, SelectionItemKind } from '../state/store'
import { COLOR_CLASSES, FONT_CLASSES } from '../theme/roles'
import { isSessionVisible } from '../viewport/scene'
import { BodyColorButton } from './BodyColor'

// The browser (ui.md 3.10): a tree of the origin, bodies, sketches, datum planes and parameters.
// Its treeitems form one flat list with levels, so each row is one focusable line; row buttons stay
// out of the tab order, and their actions are also reachable by keyboard elsewhere (3.10).

type GroupId = 'origin' | 'bodies' | 'sketches' | 'construction' | 'parameters'

type RowKind =
  | 'origin_plane'
  | 'origin_axis'
  | 'origin_point'
  | 'body'
  | 'sketch'
  | 'datum'
  | 'parameter'

const DEFAULT_EXPANDED: Readonly<Record<GroupId, boolean>> = {
  origin: false,
  bodies: true,
  sketches: true,
  construction: true,
  parameters: true,
}

/** Group expansion, kept for the session even while the browser is closed (3.10). */
const sessionExpansion = new WeakMap<App, Record<GroupId, boolean>>()

const ORIGIN_ROWS: readonly {
  readonly origin: OriginName
  readonly kind: 'origin_plane' | 'origin_axis' | 'origin_point'
  readonly name: string
  readonly icon: LucideIcon
}[] = [
  { origin: 'XY', kind: 'origin_plane', name: 'XY plane', icon: Square },
  { origin: 'XZ', kind: 'origin_plane', name: 'XZ plane', icon: Square },
  { origin: 'YZ', kind: 'origin_plane', name: 'YZ plane', icon: Square },
  { origin: 'X', kind: 'origin_axis', name: 'X axis', icon: MoveRight },
  { origin: 'Y', kind: 'origin_axis', name: 'Y axis', icon: MoveRight },
  { origin: 'Z', kind: 'origin_axis', name: 'Z axis', icon: MoveRight },
  { origin: 'O', kind: 'origin_point', name: 'Origin', icon: Dot },
]

function item(
  key: string,
  kind: SelectionItemKind,
  label: string,
  data: Partial<Pick<SelectionItem, 'body' | 'feature' | 'origin'>> = {},
): SelectionItem {
  return {
    key,
    kind,
    label,
    body: data.body ?? null,
    feature: data.feature ?? null,
    element: null,
    sketchItem: null,
    origin: data.origin ?? null,
  }
}

function report(action: string): (error: unknown) => void {
  return (error) => {
    console.error(`${action} failed.`, error)
  }
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
  readonly onClick: () => void
}

/** A small icon button inside a row, outside the tab order (3.10). */
function RowButton({ label, icon: Icon, pressed, onClick }: RowButtonProps) {
  return (
    <button
      type="button"
      tabIndex={-1}
      aria-label={label}
      aria-pressed={pressed}
      onClick={own(onClick)}
      onDoubleClick={(event) => {
        event.stopPropagation()
      }}
      className={clsx(
        'inline-flex size-6 shrink-0 items-center justify-center rounded-sm transition-colors duration-150 motion-reduce:transition-none',
        COLOR_CLASSES.textMuted.foreground,
        COLOR_CLASSES.hover.background,
      )}
    >
      <Icon aria-hidden="true" strokeWidth={1.75} className="size-4" />
    </button>
  )
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
        if (event.key === 'Enter') {
          event.preventDefault()
          onActivate?.()
        } else if (event.key === ' ') {
          event.preventDefault()
          pickRow(app, selection, togglesSelection(app, event))
        } else if (event.key === 'F2' && onRename !== undefined) {
          event.preventDefault()
          onRename()
        }
      }}
      className={clsx(
        'flex h-8 cursor-default items-center gap-2 rounded-sm pr-1 pl-7 text-sm',
        COLOR_CLASSES.focusRing.outline,
        COLOR_CLASSES.accent.outline,
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
        if (event.target === event.currentTarget && (event.key === 'Enter' || event.key === ' ')) {
          event.preventDefault()
          onToggle(id)
        }
      }}
      className={clsx(
        'mt-2 flex h-8 cursor-default items-center gap-1 rounded-sm px-1 text-xs font-semibold first:mt-0',
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
  if (status === 'error') {
    return (
      <CircleX
        aria-label="Error"
        strokeWidth={1.75}
        className={clsx('size-4 shrink-0', COLOR_CLASSES.errorMark.foreground)}
      />
    )
  }
  if (status === 'warning') {
    return (
      <TriangleAlert
        aria-label="Warning"
        strokeWidth={1.75}
        className={clsx(
          'size-4 shrink-0',
          COLOR_CLASSES.warningMark.fill,
          COLOR_CLASSES.warningMark.foreground,
        )}
      />
    )
  }
  return null
}

function rowIcon(Icon: LucideIcon): ReactNode {
  return (
    <Icon
      aria-hidden="true"
      strokeWidth={1.75}
      className={clsx('size-4 shrink-0', COLOR_CLASSES.textMuted.foreground)}
    />
  )
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
    const key = `origin:${row.origin}`
    return (
      <Row
        key={key}
        kind={row.kind}
        id={row.origin}
        selection={item(key, row.kind, row.name, { origin: row.origin })}
        selected={selected.has(key)}
        icon={rowIcon(row.icon)}
        actions={<SessionEye itemKey={key} name={row.name} />}
      >
        <span className="truncate">{row.name}</span>
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
  const visible = body.visible !== false
  const key = `body:${body.id}`
  return (
    <Row
      kind="body"
      id={body.id}
      selection={item(key, 'body', body.name, { body: body.id })}
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
            onClick={() => {
              setBodyVisible(app, body.id, !visible).catch(report('Changing the visibility'))
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
            renameBody(app, body.id, name).catch(report('Renaming the body'))
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
  const Icon = iconName === undefined ? Shapes : (FEATURE_ICONS[iconName] ?? Shapes)
  const key = `feature:${record.id}`
  const selectionKind = kind === 'sketch' ? 'feature' : 'datum_plane'
  return (
    <Row
      kind={kind}
      id={record.id}
      selection={item(key, selectionKind, label, { feature: record.id })}
      selected={selected}
      icon={rowIcon(Icon)}
      onActivate={() => {
        if (kind === 'sketch') {
          enterSketch(app, record.id).catch(report('Entering the sketch'))
        } else {
          editFeature(app, record.id).catch(report('Editing the datum plane'))
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
            renameFeature(app, record.id, name).catch(report('Renaming the feature'))
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
  const evaluated =
    isPlainNumber(parameter.expr) || value === undefined
      ? null
      : typeof value.value === 'number' && value.dimension != null
        ? formatEvaluated(value.value, value.dimension, units)
        : null
  return (
    <Row
      kind="parameter"
      id={parameter.id}
      selection={item(`parameter:${parameter.id}`, 'parameter', parameter.name)}
      selected={selected}
      icon={rowIcon(SlidersHorizontal)}
      onActivate={() => {
        app.store.setState((state) => ({ panels: { ...state.panels, side: 'parameters' } }))
        requestUi(app, { kind: 'parameter', id: parameter.id })
      }}
    >
      <span className="truncate">{text}</span>
      {evaluated === null ? null : (
        <span
          className={clsx(
            'shrink-0 text-xs',
            FONT_CLASSES.values,
            COLOR_CLASSES.textMuted.foreground,
          )}
        >
          {evaluated}
        </span>
      )}
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
  const tree = useRef<HTMLUListElement>(null)
  const lastFocused = useRef<Element | null>(null)
  const [expanded, setExpanded] = useState<Record<GroupId, boolean>>(
    () => sessionExpansion.get(app) ?? DEFAULT_EXPANDED,
  )
  const [renaming, setRenaming] = useState<string | null>(null)

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
    const group: GroupId = request.key.startsWith('body:') ? 'bodies' : 'sketches'
    if (!expanded[group] && group === 'bodies') {
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
    const items = [...element.querySelectorAll<HTMLElement>('[data-tree-item]')]
    const stop = items.find((candidate) => candidate === lastFocused.current) ?? items[0]
    for (const candidate of items) {
      candidate.tabIndex = candidate === stop ? 0 : -1
    }
  })

  function labelOf(record: FeatureRecord): string {
    return recipe === null ? record.id : featureLabel(recipe, record.id, catalog)
  }

  return (
    <nav
      aria-label="Browser"
      data-testid="browser"
      className={clsx(
        'flex w-66 shrink-0 flex-col overflow-y-auto border-r max-lg:absolute max-lg:inset-y-0 max-lg:left-0 max-lg:z-30 max-lg:shadow-lg',
        COLOR_CLASSES.surface.background,
        COLOR_CLASSES.border.border,
        COLOR_CLASSES.text.foreground,
      )}
    >
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
        className="flex flex-col p-2"
      >
        <GroupHeader id="origin" title="Origin" expanded={expanded.origin} onToggle={toggle} />
        {expanded.origin ? <OriginRows selected={selected} /> : null}
        <GroupHeader id="bodies" title="Bodies" expanded={expanded.bodies} onToggle={toggle} />
        {expanded.bodies
          ? bodies.map((body) => (
              <BodyRow
                key={body.id}
                body={body}
                selected={selected.has(`body:${body.id}`)}
                renaming={renaming === `body:${body.id}`}
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
                selected={selected.has(`feature:${record.id}`)}
                renaming={renaming === `feature:${record.id}`}
                onRename={setRenaming}
              />
            ))
          : null}
        <GroupHeader
          id="construction"
          title="Construction"
          expanded={expanded.construction}
          onToggle={toggle}
        />
        {expanded.construction
          ? datums.map((record) => (
              <FeatureRow
                key={record.id}
                record={record}
                kind="datum"
                label={labelOf(record)}
                selected={selected.has(`feature:${record.id}`)}
                renaming={renaming === `feature:${record.id}`}
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
                app.store.setState((state) => ({ panels: { ...state.panels, side: 'parameters' } }))
              }}
            />
          }
        />
        {expanded.parameters
          ? parameters.map((parameter) => (
              <ParameterRow
                key={parameter.id}
                parameter={parameter}
                selected={selected.has(`parameter:${parameter.id}`)}
              />
            ))
          : null}
      </ul>
    </nav>
  )
}
