import { clsx } from 'clsx/lite'
import { ChevronDown, Ellipsis, type LucideIcon } from 'lucide-react'
import {
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
  type KeyboardEvent,
  type ReactNode,
  type Ref,
  type RefObject,
} from 'react'
import { setCheckHidden, setPrinter } from '../app/actions/print'
import { useApp, useAppState } from '../app/AppProvider'
import type { App } from '../app/createApp'
import { PRIMARY_COMMANDS, TOOLBARS, type ToolbarEntry } from '../commands/toolbars'
import { Menu, type MenuEntry } from '../components/Menu'
import { Tooltip } from '../components/Tooltip'
import { ChecksPopover } from '../print/ChecksPopover'
import { PrinterMenu } from '../print/PrinterMenu'
import { printerButtonText } from '../print/printer'
import { selectHealthBadge } from '../state/selectors'
import type { AppContext, AppState } from '../state/store'
import { COLOR_CLASSES } from '../theme/roles'
import { useUiRequest } from './requests'
import { EMPTY_MEMORY, FULL_FIT, nextFit, type FitMemory, type ToolbarFit } from './toolbar-fit'
import { commandMenuItem, runCommand, useCommandViews, type CommandView } from './use-commands'

// The context toolbar of the top bar (ui.md 3.2, 3.3): one short toolbar per context, built from
// TOOLBARS. It measures the room the top bar leaves it: buttons show icon and label when all of
// them fit, else the primary buttons keep their label, else every button shows its icon only (the
// label stays the accessible name and is in the tooltip); when even the icons do not fit, the last
// buttons that are not primary move into a More tools menu, so no command is out of sight. Arrow
// keys, Home and End move focus (roving tab index, 3.24).

const TOOLBAR_NAMES: Readonly<Record<AppContext, string>> = {
  model: 'Model tools',
  sketch: 'Sketch tools',
  print: 'Print tools',
}

/** Commands the Print toolbar draws as its own controls (3.17). */
const PRINTER_COMMAND = 'print.printer'
const CHECKS_COMMAND = 'print.checks'
const PRINT_CHECK_COMMAND = 'view.health'

type ButtonLook = {
  readonly label: string
  readonly icon: LucideIcon | null
  readonly disabledReason: string | null
  readonly pressed?: boolean | undefined
  readonly primary?: boolean | undefined
  readonly popup?: 'menu' | 'dialog' | undefined
  readonly expanded?: boolean | undefined
  /** Text after the label, such as a count. */
  readonly extra?: ReactNode
}

type ToolbarButtonProps = ButtonLook & {
  readonly shortcut: string | null
  /** What the command does, on the tooltip's second line (2.10, 3.24). */
  readonly description?: string | null | undefined
  readonly onClick: () => void
  readonly buttonRef?: Ref<HTMLButtonElement> | undefined
  /** Whether the label shows beside the icon (3.2). */
  readonly showLabel: boolean
  /** The button's place among the buttons that can move into More tools, which the fit measures; null for others. */
  readonly movable?: number | null | undefined
}

/** One toolbar button: icon, a label when the toolbar has room for it, and a tooltip with the shortcut. */
function ToolbarButton({
  label,
  icon: Icon,
  disabledReason,
  pressed,
  primary = false,
  popup,
  expanded,
  extra,
  shortcut,
  description = null,
  onClick,
  buttonRef,
  showLabel,
  movable = null,
}: ToolbarButtonProps) {
  const disabled = disabledReason !== null
  const accent = primary && !disabled
  const selected = pressed === true && !disabled
  return (
    <Tooltip
      label={disabled ? `${label}: ${disabledReason}` : label}
      shortcut={shortcut}
      description={description}
    >
      <button
        ref={buttonRef}
        type="button"
        data-toolbar-item=""
        data-toolbar-movable={movable ?? undefined}
        aria-label={label}
        aria-disabled={disabled ? true : undefined}
        aria-pressed={pressed}
        aria-haspopup={popup}
        aria-expanded={popup === undefined ? undefined : expanded === true}
        onClick={onClick}
        className={clsx(
          'inline-flex shrink-0 items-center justify-center gap-2 rounded-sm text-sm font-medium whitespace-nowrap transition-colors duration-150 motion-reduce:transition-none',
          showLabel ? 'h-10 px-3' : 'h-9 min-w-9 px-2',
          COLOR_CLASSES.focusRing.outline,
          COLOR_CLASSES.accent.outline,
          accent && COLOR_CLASSES.accent.background,
          accent && COLOR_CLASSES.accent.foreground,
          accent && COLOR_CLASSES.accentHover.background,
          selected && COLOR_CLASSES.selection.background,
          selected && COLOR_CLASSES.text.foreground,
          !accent && !selected && !disabled && COLOR_CLASSES.text.foreground,
          !accent && !selected && !disabled && COLOR_CLASSES.hover.background,
          disabled && COLOR_CLASSES.textDisabled.foreground,
          disabled && 'cursor-not-allowed',
        )}
      >
        {Icon === null ? null : (
          <Icon aria-hidden="true" strokeWidth={1.75} className="size-5 shrink-0" />
        )}
        {showLabel ? <span>{label}</span> : null}
        {extra}
        {popup === 'menu' && showLabel ? (
          <ChevronDown aria-hidden="true" strokeWidth={1.75} className="size-4" />
        ) : null}
      </button>
    </Tooltip>
  )
}

function disabledReasonOf(view: CommandView): string | null {
  return view.availability.state === 'disabled' ? view.availability.reason : null
}

/** How a control shows in the toolbar now: its label or not, and its place among the movable ones. */
type Placement = {
  readonly showLabel: boolean
  readonly movable: number | null
}

function CommandButton({
  view,
  placement,
}: {
  readonly view: CommandView
  readonly placement: Placement
}) {
  const app = useApp()
  const { definition } = view
  return (
    <ToolbarButton
      label={definition.title}
      icon={definition.icon}
      disabledReason={disabledReasonOf(view)}
      pressed={view.checked ?? undefined}
      primary={PRIMARY_COMMANDS.has(definition.id)}
      shortcut={view.shortcut}
      description={definition.description ?? null}
      showLabel={placement.showLabel}
      movable={placement.movable}
      onClick={() => {
        runCommand(app, definition.id, 'toolbar')
      }}
    />
  )
}

type GroupEntry = Extract<ToolbarEntry, { readonly kind: 'group' }>

/** A group's reason to be disabled: its first command's, when all of its commands are disabled (3.3). */
function groupReason(views: readonly CommandView[]): string | null {
  const [first] = views
  return first !== undefined && views.every((view) => disabledReasonOf(view) !== null)
    ? disabledReasonOf(first)
    : null
}

/** A group button opening a menu of its commands with icon, title and shortcut (3.3). */
function GroupButton({
  entry,
  views,
  placement,
}: {
  readonly entry: GroupEntry
  readonly views: readonly CommandView[]
  readonly placement: Placement
}) {
  const app = useApp()
  const anchor = useRef<HTMLButtonElement>(null)
  const [open, setOpen] = useState(false)
  const pressed = views.some((view) => view.checked === true)
  const entries: MenuEntry[] = views.map((view) => commandMenuItem(app, view))
  const [first] = views
  const reason = groupReason(views)
  return (
    <>
      <ToolbarButton
        buttonRef={anchor}
        label={entry.title}
        icon={entry.icon}
        disabledReason={reason}
        pressed={pressed ? true : undefined}
        popup="menu"
        expanded={open}
        shortcut={null}
        showLabel={placement.showLabel}
        movable={placement.movable}
        onClick={() => {
          if (reason !== null && first !== undefined) {
            runCommand(app, first.definition.id, 'toolbar')
            return
          }
          setOpen((current) => !current)
        }}
      />
      <Menu
        label={entry.title}
        open={open}
        anchor={anchor}
        entries={entries}
        onOpenChange={setOpen}
      />
    </>
  )
}

function selectPrinterText(state: AppState): string {
  return printerButtonText(state.printers, state.document?.recipe.printer)
}

/**
 * The Print toolbar's printer menu button, "Printer: {name}" (3.17). Moved into More tools, it
 * draws no button and opens its menu under More tools instead (`collapsedTo`).
 */
function PrinterButton({
  view,
  placement,
  collapsedTo,
}: {
  readonly view: CommandView
  readonly placement: Placement
  readonly collapsedTo: RefObject<HTMLButtonElement | null> | null
}) {
  const app = useApp()
  const text = useAppState(selectPrinterText)
  const printers = useAppState((state) => state.printers)
  const current = useAppState((state) => state.document?.recipe.printer ?? null)
  const anchor = useRef<HTMLButtonElement>(null)
  const [open, setOpen] = useState(false)
  const reason = disabledReasonOf(view)
  useUiRequest(app, 'printer', () => {
    setOpen(true)
    return true
  })
  return (
    <>
      {collapsedTo === null ? (
        <ToolbarButton
          buttonRef={anchor}
          label={text}
          icon={view.definition.icon}
          disabledReason={reason}
          popup="menu"
          expanded={open}
          shortcut={view.shortcut}
          showLabel={placement.showLabel}
          movable={placement.movable}
          onClick={() => {
            if (reason === null) {
              setOpen((value) => !value)
            } else {
              runCommand(app, view.definition.id, 'toolbar')
            }
          }}
        />
      ) : null}
      <PrinterMenu
        open={open}
        anchor={collapsedTo ?? anchor}
        onOpenChange={setOpen}
        printers={printers}
        current={current}
        onChoose={(profileId) => {
          setPrinter(app, profileId).catch((error: unknown) => {
            console.error('Changing the printer failed.', error)
          })
        }}
      />
    </>
  )
}

/**
 * The Print toolbar's Checks button and its popover of check switches (3.17). Moved into More
 * tools, it draws no button and opens its popover under More tools instead (`collapsedTo`).
 */
function ChecksButton({
  view,
  placement,
  collapsedTo,
}: {
  readonly view: CommandView
  readonly placement: Placement
  readonly collapsedTo: RefObject<HTMLButtonElement | null> | null
}) {
  const app = useApp()
  const hiddenChecks = useAppState((state) => state.settings.hiddenChecks)
  const anchor = useRef<HTMLButtonElement>(null)
  const [open, setOpen] = useState(false)
  useUiRequest(app, 'checks', () => {
    setOpen(true)
    return true
  })
  return (
    <>
      {collapsedTo === null ? (
        <ToolbarButton
          buttonRef={anchor}
          label={view.definition.title}
          icon={view.definition.icon}
          disabledReason={disabledReasonOf(view)}
          popup="dialog"
          expanded={open}
          shortcut={view.shortcut}
          showLabel={placement.showLabel}
          movable={placement.movable}
          onClick={() => {
            setOpen((value) => !value)
          }}
        />
      ) : null}
      {open ? (
        <ChecksPopover
          open
          anchor={collapsedTo ?? anchor}
          onOpenChange={setOpen}
          hiddenChecks={hiddenChecks}
          onHiddenChange={(check, hidden) => {
            setCheckHidden(app, check, hidden)
          }}
        />
      ) : null}
    </>
  )
}

/** The shown finding count of the last report, or null before the first report. */
function selectFindingCount(state: AppState): number | null {
  const badge = selectHealthBadge(state)
  return state.print.report === null ? null : badge.errors + badge.warnings
}

/**
 * The Print toolbar's Print check button with the shown finding count (3.17). Its command opens the
 * print check panel as the status bar badge does, checking a missing or stale report again.
 */
function PrintCheckButton({
  view,
  placement,
}: {
  readonly view: CommandView
  readonly placement: Placement
}) {
  const app = useApp()
  const count = useAppState(selectFindingCount)
  return (
    <ToolbarButton
      label={view.definition.title}
      icon={view.definition.icon}
      disabledReason={disabledReasonOf(view)}
      shortcut={view.shortcut}
      showLabel={placement.showLabel}
      movable={placement.movable}
      extra={
        count === null ? null : (
          <span
            className={clsx(
              'rounded-sm px-1.5 text-xs tabular-nums',
              COLOR_CLASSES.hover.active,
              COLOR_CLASSES.text.foreground,
            )}
          >
            {count}
          </span>
        )
      }
      onClick={() => {
        runCommand(app, view.definition.id, 'toolbar')
      }}
    />
  )
}

/**
 * One command's control: its toolbar button, or, moved into More tools, nothing but what opens
 * from there (the printer menu and the checks popover, under the More tools button).
 */
function EntryControl({
  view,
  placement,
  collapsedTo,
}: {
  readonly view: CommandView
  readonly placement: Placement
  readonly collapsedTo: RefObject<HTMLButtonElement | null> | null
}) {
  switch (view.definition.id) {
    case PRINTER_COMMAND:
      return <PrinterButton view={view} placement={placement} collapsedTo={collapsedTo} />
    case CHECKS_COMMAND:
      return <ChecksButton view={view} placement={placement} collapsedTo={collapsedTo} />
    case PRINT_CHECK_COMMAND:
      return collapsedTo === null ? <PrintCheckButton view={view} placement={placement} /> : null
    default:
      return collapsedTo === null ? <CommandButton view={view} placement={placement} /> : null
  }
}

/** One control of a toolbar, in toolbar order. */
type ToolbarItem =
  | { readonly kind: 'separator'; readonly key: string }
  | {
      readonly kind: 'group'
      readonly key: string
      readonly entry: GroupEntry
      readonly views: readonly CommandView[]
      readonly movable: number
    }
  | {
      readonly kind: 'command'
      readonly key: string
      readonly view: CommandView
      readonly primary: boolean
      /** Its place among the buttons that can move into More tools; null for a primary button, which stays. */
      readonly movable: number | null
    }

/** The toolbar's controls: shown commands and groups with a shown command, numbered as they may move. */
function toolbarItems(
  entries: readonly ToolbarEntry[],
  views: ReadonlyMap<string, CommandView>,
): readonly ToolbarItem[] {
  const items: ToolbarItem[] = []
  let movable = 0
  entries.forEach((entry, index) => {
    if (entry.kind === 'separator') {
      items.push({ kind: 'separator', key: `separator-${String(index)}` })
      return
    }
    if (entry.kind === 'group') {
      const groupViews = entry.commands.flatMap((id) => {
        const view = views.get(id)
        return view === undefined || view.availability.state === 'hidden' ? [] : [view]
      })
      if (groupViews.length > 0) {
        items.push({ kind: 'group', key: entry.id, entry, views: groupViews, movable: movable++ })
      }
      return
    }
    const view = views.get(entry.command)
    if (view === undefined || view.availability.state === 'hidden') {
      return
    }
    const primary = PRIMARY_COMMANDS.has(entry.command)
    items.push({
      kind: 'command',
      key: entry.command,
      view,
      primary,
      movable: primary ? null : movable++,
    })
  })
  return items
}

/** The More tools menu entry of a control moved there: a command item (a toggle as a checkbox) or a group's submenu. */
function overflowEntry(app: App, item: ToolbarItem): MenuEntry | null {
  if (item.kind === 'separator') {
    return null
  }
  if (item.kind === 'group') {
    return {
      kind: 'submenu',
      label: item.entry.title,
      icon: item.entry.icon,
      entries: item.views.map((view) => commandMenuItem(app, view)),
    }
  }
  const { view } = item
  if (view.checked === null) {
    return commandMenuItem(app, view)
  }
  return {
    kind: 'checkbox',
    label: view.definition.title,
    checked: view.checked,
    disabled: view.availability.state === 'disabled',
    onSelect: () => {
      runCommand(app, view.definition.id, 'menu')
    },
  }
}

/** The More tools button at the toolbar's end and its menu of the controls that did not fit (3.2). */
function MoreTools({
  entries,
  anchor,
}: {
  readonly entries: readonly MenuEntry[]
  readonly anchor: RefObject<HTMLButtonElement | null>
}) {
  const [open, setOpen] = useState(false)
  return (
    <>
      <ToolbarButton
        buttonRef={anchor}
        label="More tools"
        icon={Ellipsis}
        disabledReason={null}
        popup="menu"
        expanded={open}
        shortcut={null}
        showLabel={false}
        onClick={() => {
          setOpen((current) => !current)
        }}
      />
      <Menu
        label="More tools"
        open={open}
        anchor={anchor}
        entries={entries}
        onOpenChange={setOpen}
      />
    </>
  )
}

/** The command ids a toolbar's command and group entries name. */
function commandIds(entries: readonly ToolbarEntry[]): readonly string[] {
  return entries.flatMap((entry) =>
    entry.kind === 'command' ? [entry.command] : entry.kind === 'group' ? entry.commands : [],
  )
}

/** Moves focus between the toolbar's buttons with arrow keys, Home and End. */
function moveFocus(event: KeyboardEvent<HTMLDivElement>): void {
  const items = [...event.currentTarget.querySelectorAll<HTMLButtonElement>('[data-toolbar-item]')]
  const index = items.findIndex((item) => item === document.activeElement)
  if (index === -1) {
    return
  }
  const last = items.length - 1
  const target =
    event.key === 'ArrowRight'
      ? index === last
        ? 0
        : index + 1
      : event.key === 'ArrowLeft'
        ? index === 0
          ? last
          : index - 1
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

/** Makes one button the toolbar's tab stop: the last focused one while it exists, else the first. */
function rove(container: HTMLElement, focused: Element | null): void {
  const items = [...container.querySelectorAll<HTMLButtonElement>('[data-toolbar-item]')]
  const stop = items.find((item) => item === focused) ?? items[0]
  for (const item of items) {
    item.tabIndex = item === stop ? 0 : -1
  }
}

/** The toolbar's inner width, the width its buttons take, the movable buttons' widths and the More tools width, in CSS pixels. */
function measureToolbar(toolbar: HTMLElement) {
  const style = getComputedStyle(toolbar)
  const gap = Number.parseFloat(style.columnGap) || 0
  const padding =
    (Number.parseFloat(style.paddingLeft) || 0) + (Number.parseFloat(style.paddingRight) || 0)
  const parts = [...toolbar.querySelectorAll<HTMLElement>('[data-toolbar-item], hr')]
  const rectangles = parts.map((part) => part.getBoundingClientRect())
  const left = Math.min(...rectangles.map((rectangle) => rectangle.left))
  const right = Math.max(...rectangles.map((rectangle) => rectangle.right))
  const movable = [...toolbar.querySelectorAll<HTMLElement>('[data-toolbar-movable]')].map(
    (button) => button.offsetWidth + gap,
  )
  // The More tools button is an icon button: as wide as the buttons are tall.
  const more = (parts.find((part) => part.tagName === 'BUTTON')?.offsetHeight ?? 0) + gap
  return {
    available: toolbar.clientWidth - padding,
    needed: parts.length === 0 ? 0 : right - left,
    movable,
    more,
  }
}

/**
 * The fit of the toolbar's buttons to its width (3.2), measured after every render and whenever
 * the toolbar is resized. A change of `content` (the context, labels, counts or UI scale) starts
 * again from every label shown.
 */
function useToolbarFit(toolbar: RefObject<HTMLDivElement | null>, content: string): ToolbarFit {
  const [state, setState] = useState<{ readonly content: string; readonly fit: ToolbarFit }>({
    content,
    fit: FULL_FIT,
  })
  const fit = state.content === content ? state.fit : FULL_FIT
  const shown = useRef({ content, fit })
  const memory = useRef<{ content: string; memory: FitMemory }>({ content, memory: EMPTY_MEMORY })
  const measure = useRef(() => {
    const element = toolbar.current
    // A toolbar that is not laid out (hidden, or a test without layout) keeps what it has.
    if (element === null || element.clientWidth === 0) {
      return
    }
    const current = shown.current
    if (memory.current.content !== current.content) {
      memory.current = { content: current.content, memory: EMPTY_MEMORY }
    }
    const measured = measureToolbar(element)
    const next = nextFit(current.fit, memory.current.memory, {
      ...measured,
      movable: current.fit.labels === 'none' && current.fit.kept === null ? measured.movable : null,
    })
    memory.current = { content: current.content, memory: next.memory }
    if (next.fit !== current.fit) {
      setState({ content: current.content, fit: next.fit })
    }
  })
  useLayoutEffect(() => {
    shown.current = { content, fit }
    measure.current()
  })
  useEffect(() => {
    const element = toolbar.current
    if (element === null || typeof ResizeObserver !== 'function') {
      return undefined
    }
    const observer = new ResizeObserver(() => {
      measure.current()
    })
    observer.observe(element)
    // Labels are measured again once the interface font has loaded.
    if ('fonts' in document) {
      void document.fonts.ready.then(() => {
        measure.current()
      })
    }
    return () => {
      observer.disconnect()
    }
  }, [toolbar])
  return fit
}

export function ContextToolbar() {
  const app = useApp()
  const context = useAppState((state) => state.context)
  const printerText = useAppState(selectPrinterText)
  const findingCount = useAppState(selectFindingCount)
  const uiScale = useAppState((state) => state.settings.uiScale)
  const entries = TOOLBARS[context]
  const views = useCommandViews(app, commandIds(entries))
  const items = toolbarItems(entries, new Map(views.map((view) => [view.definition.id, view])))
  const toolbar = useRef<HTMLDivElement>(null)
  const more = useRef<HTMLButtonElement>(null)
  const lastFocused = useRef<Element | null>(null)
  // What the buttons show: when it changes, the fit is measured from the start again.
  const content = JSON.stringify([
    context,
    uiScale,
    printerText,
    findingCount,
    items.map((item) => item.key),
  ])
  const fit = useToolbarFit(toolbar, content)
  // Buttons come and go with the context, the catalog and the fit; one of them always keeps the tab stop.
  useLayoutEffect(() => {
    if (toolbar.current !== null) {
      rove(toolbar.current, lastFocused.current)
    }
  })

  const moved = (item: ToolbarItem): boolean =>
    item.kind !== 'separator' &&
    item.movable !== null &&
    fit.kept !== null &&
    item.movable >= fit.kept
  const overflow = items.filter(moved).flatMap((item) => {
    const entry = overflowEntry(app, item)
    return entry === null ? [] : [entry]
  })
  const controls = items.map((item) => {
    if (item.kind === 'separator') {
      return (
        <hr
          key={item.key}
          aria-orientation="vertical"
          className={clsx('mx-1 h-6 w-0 shrink-0 border-0 border-l', COLOR_CLASSES.border.border)}
        />
      )
    }
    const placement: Placement = {
      showLabel:
        fit.labels === 'all' ||
        (fit.labels === 'primary' && item.kind === 'command' && item.primary),
      movable: item.movable,
    }
    if (item.kind === 'group') {
      return moved(item) ? null : (
        <GroupButton key={item.key} entry={item.entry} views={item.views} placement={placement} />
      )
    }
    return (
      <EntryControl
        key={item.key}
        view={item.view}
        placement={placement}
        collapsedTo={moved(item) ? more : null}
      />
    )
  })

  return (
    <div
      ref={toolbar}
      role="toolbar"
      aria-label={TOOLBAR_NAMES[context]}
      tabIndex={-1}
      onKeyDown={moveFocus}
      onFocus={(event) => {
        if (toolbar.current !== null && event.target.hasAttribute('data-toolbar-item')) {
          lastFocused.current = event.target
          rove(toolbar.current, event.target)
        }
      }}
      // Nothing scrolls: what does not fit is in More tools. The padding keeps focus rings whole.
      className="flex min-w-28 flex-1 items-center gap-1 overflow-hidden p-1"
    >
      {controls}
      {overflow.length === 0 ? null : <MoreTools entries={overflow} anchor={more} />}
    </div>
  )
}
