import { clsx } from 'clsx/lite'
import { ChevronDown, type LucideIcon } from 'lucide-react'
import {
  useLayoutEffect,
  useRef,
  useState,
  type KeyboardEvent,
  type ReactNode,
  type Ref,
} from 'react'
import { setCheckHidden, setPrinter } from '../app/actions/print'
import { useApp, useAppState } from '../app/AppProvider'
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
import { commandMenuItem, runCommand, useCommandViews, type CommandView } from './use-commands'

// The context toolbar of the top bar (ui.md 3.3): one short toolbar per context, built from
// TOOLBARS. Buttons show icon and label when the top bar is at least 80rem wide and the icon only
// below, with the label kept as the accessible name and in the tooltip. Arrow keys, Home and End
// move focus (roving tab index, 3.24).

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
  readonly onClick: () => void
  readonly buttonRef?: Ref<HTMLButtonElement> | undefined
}

/** One toolbar button: icon, a label shown on wide top bars, and a tooltip with the shortcut. */
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
  onClick,
  buttonRef,
}: ToolbarButtonProps) {
  const disabled = disabledReason !== null
  const accent = primary && !disabled
  const selected = pressed === true && !disabled
  return (
    <Tooltip label={disabled ? `${label}: ${disabledReason}` : label} shortcut={shortcut}>
      <button
        ref={buttonRef}
        type="button"
        data-toolbar-item=""
        aria-label={label}
        aria-disabled={disabled ? true : undefined}
        aria-pressed={pressed}
        aria-haspopup={popup}
        aria-expanded={popup === undefined ? undefined : expanded === true}
        onClick={onClick}
        className={clsx(
          'inline-flex h-9 min-w-9 shrink-0 items-center justify-center gap-2 rounded-sm px-2 text-sm font-medium whitespace-nowrap transition-colors duration-150 motion-reduce:transition-none @7xl:h-10 @7xl:px-3',
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
        <span className="hidden @7xl:inline">{label}</span>
        {extra}
        {popup === 'menu' ? (
          <ChevronDown aria-hidden="true" strokeWidth={1.75} className="hidden size-4 @7xl:block" />
        ) : null}
      </button>
    </Tooltip>
  )
}

function disabledReasonOf(view: CommandView): string | null {
  return view.availability.state === 'disabled' ? view.availability.reason : null
}

function CommandButton({ view }: { readonly view: CommandView }) {
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
      onClick={() => {
        runCommand(app, definition.id, 'toolbar')
      }}
    />
  )
}

type GroupEntry = Extract<ToolbarEntry, { readonly kind: 'group' }>

/** A group button opening a menu of its commands with icon, title and shortcut (3.3); nothing when all of them are hidden. */
function GroupButton({ entry }: { readonly entry: GroupEntry }) {
  const app = useApp()
  const views = useCommandViews(app, entry.commands).filter(
    (view) => view.availability.state !== 'hidden',
  )
  const anchor = useRef<HTMLButtonElement>(null)
  const [open, setOpen] = useState(false)
  if (views.length === 0) {
    return null
  }
  const pressed = views.some((view) => view.checked === true)
  const entries: MenuEntry[] = views.map((view) => commandMenuItem(app, view))
  return (
    <>
      <ToolbarButton
        buttonRef={anchor}
        label={entry.title}
        icon={entry.icon}
        disabledReason={null}
        pressed={pressed ? true : undefined}
        popup="menu"
        expanded={open}
        shortcut={null}
        onClick={() => {
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

/** The Print toolbar's printer menu button, "Printer: {name}" (3.17). */
function PrinterButton({ view }: { readonly view: CommandView }) {
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
      <ToolbarButton
        buttonRef={anchor}
        label={text}
        icon={view.definition.icon}
        disabledReason={reason}
        popup="menu"
        expanded={open}
        shortcut={view.shortcut}
        onClick={() => {
          if (reason === null) {
            setOpen((value) => !value)
          } else {
            runCommand(app, view.definition.id, 'toolbar')
          }
        }}
      />
      <PrinterMenu
        open={open}
        anchor={anchor}
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

/** The Print toolbar's Checks button and its popover of check switches (3.17). */
function ChecksButton({ view }: { readonly view: CommandView }) {
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
      <ToolbarButton
        buttonRef={anchor}
        label={view.definition.title}
        icon={view.definition.icon}
        disabledReason={disabledReasonOf(view)}
        popup="dialog"
        expanded={open}
        shortcut={view.shortcut}
        onClick={() => {
          setOpen((value) => !value)
        }}
      />
      {open ? (
        <ChecksPopover
          open
          anchor={anchor}
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
function PrintCheckButton({ view }: { readonly view: CommandView }) {
  const app = useApp()
  const count = useAppState(selectFindingCount)
  return (
    <ToolbarButton
      label={view.definition.title}
      icon={view.definition.icon}
      disabledReason={disabledReasonOf(view)}
      shortcut={view.shortcut}
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

function EntryControl({ view }: { readonly view: CommandView }) {
  switch (view.definition.id) {
    case PRINTER_COMMAND:
      return <PrinterButton view={view} />
    case CHECKS_COMMAND:
      return <ChecksButton view={view} />
    case PRINT_CHECK_COMMAND:
      return <PrintCheckButton view={view} />
    default:
      return <CommandButton view={view} />
  }
}

/** The command ids a toolbar's command entries name. */
function commandIds(entries: readonly ToolbarEntry[]): readonly string[] {
  return entries.flatMap((entry) => (entry.kind === 'command' ? [entry.command] : []))
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

export function ContextToolbar() {
  const app = useApp()
  const context = useAppState((state) => state.context)
  const entries = TOOLBARS[context]
  const views = useCommandViews(app, commandIds(entries))
  const byId = new Map(views.map((view) => [view.definition.id, view]))
  const toolbar = useRef<HTMLDivElement>(null)
  const lastFocused = useRef<Element | null>(null)
  // Buttons come and go with the context and the catalog; one of them always keeps the tab stop.
  useLayoutEffect(() => {
    if (toolbar.current !== null) {
      rove(toolbar.current, lastFocused.current)
    }
  })

  const controls: ReactNode[] = []
  entries.forEach((entry, index) => {
    if (entry.kind === 'separator') {
      controls.push(
        <hr
          key={`separator-${String(index)}`}
          aria-orientation="vertical"
          className={clsx('mx-1 h-6 w-0 shrink-0 border-0 border-l', COLOR_CLASSES.border.border)}
        />,
      )
      return
    }
    if (entry.kind === 'group') {
      controls.push(<GroupButton key={entry.id} entry={entry} />)
      return
    }
    const view = byId.get(entry.command)
    if (view !== undefined && view.availability.state !== 'hidden') {
      controls.push(<EntryControl key={entry.command} view={view} />)
    }
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
      className="flex min-w-0 flex-1 items-center gap-1 overflow-x-auto"
    >
      {controls}
    </div>
  )
}
