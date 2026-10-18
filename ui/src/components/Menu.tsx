import { clsx } from 'clsx/lite'
import { Check, ChevronRight, type LucideIcon } from 'lucide-react'
import {
  useCallback,
  useId,
  useLayoutEffect,
  useRef,
  useState,
  type KeyboardEvent,
  type PointerEvent,
  type RefObject,
} from 'react'
import { COLOR_CLASSES } from '../theme/roles'
import { focusOnPointerMove, nextIndex, typeaheadIndex, useMenuPopover } from './menu-navigation'
import type { Placement } from './placement'

type EntryBase = {
  readonly label: string
  readonly disabled?: boolean
  /** A `data-testid` for the item (ui.md 6.1), such as `keep-sizes`. */
  readonly testId?: string
}

/** One entry of a menu, in display order. */
export type MenuEntry =
  | (EntryBase & {
      readonly kind: 'item'
      readonly icon?: LucideIcon
      /** The formatted shortcut (formatShortcut). */
      readonly shortcut?: string
      readonly onSelect: () => void
    })
  | (EntryBase & {
      readonly kind: 'checkbox'
      readonly checked: boolean
      readonly onSelect: () => void
      /** Toggling it by any means keeps the menu open, for an option of the menu's other choices (3.21 Keep sizes). */
      readonly keepsOpen?: boolean
    })
  | (EntryBase & {
      readonly kind: 'radio'
      readonly checked: boolean
      readonly onSelect: () => void
    })
  | (EntryBase & {
      readonly kind: 'submenu'
      readonly icon?: LucideIcon
      readonly entries: readonly MenuEntry[]
    })
  | { readonly kind: 'separator' }

type ChoiceEntry = Extract<MenuEntry, { readonly onSelect: () => void }>
type FocusableEntry = Exclude<MenuEntry, { readonly kind: 'separator' }>

export type MenuProps = {
  /** The menu's accessible name, usually its button's label. */
  readonly label: string
  readonly open: boolean
  /** The menu button: the menu opens under it and returns focus to it. */
  readonly anchor: RefObject<HTMLElement | null>
  readonly entries: readonly MenuEntry[]
  /** Called with false whenever the menu closes: a choice, Esc, Tab or a press outside. */
  readonly onOpenChange: (open: boolean) => void
  /** A `data-testid` for the root menu element (ui.md 6.1 names `document-menu` and `context-menu`). */
  readonly testId?: string
  /** Where focus goes when the menu closes by a choice or a key; the anchor when absent. */
  readonly returnFocus?: RefObject<HTMLElement | null>
}

/**
 * A menu in a native light-dismiss popover under its button (WAI-ARIA menu button pattern).
 * Opening focuses the first item; arrow keys, Home, End and a typed first letter move focus;
 * Enter, Space or a click chooses; ArrowRight and ArrowLeft open and close submenus; Esc closes
 * the innermost menu, and closing the menu returns focus to its button.
 */
export function Menu({
  label,
  open,
  anchor,
  entries,
  onOpenChange,
  testId,
  returnFocus,
}: MenuProps) {
  if (!open) {
    return null
  }
  const closeAndFocusButton = (): void => {
    const target = returnFocus ?? anchor
    target.current?.focus()
    onOpenChange(false)
  }
  const choose = (entry: ChoiceEntry): void => {
    // Focus returns first, so a dialog the choice opens can take it.
    closeAndFocusButton()
    entry.onSelect()
  }
  const dismiss = (): void => {
    onOpenChange(false)
  }
  return (
    <MenuList
      name={{ label }}
      testId={testId}
      entries={entries}
      anchor={anchor}
      placement="below"
      choose={choose}
      close={closeAndFocusButton}
      dismiss={dismiss}
      dismissAll={dismiss}
      focusRequest={1}
    />
  )
}

type MenuListProps = {
  /** The element id, which a submenu's item names as its popover target. */
  readonly id?: string
  /** The root menu is named by a label; a submenu by its item. */
  readonly name: { readonly label: string } | { readonly labelledBy: string }
  readonly entries: readonly MenuEntry[]
  readonly anchor: RefObject<HTMLElement | null>
  readonly placement: Placement
  /** Runs a choice and closes every menu. */
  readonly choose: (entry: ChoiceEntry) => void
  /** Closes this menu by keyboard (Esc, or ArrowLeft in a submenu), focusing what opened it. */
  readonly close: () => void
  /** This menu was light-dismissed by the browser. */
  readonly dismiss: () => void
  /** Closes every menu without moving focus (Tab). */
  readonly dismissAll: () => void
  /** Each increase focuses the first item; 0 leaves focus where it is (a submenu opened by hovering). */
  readonly focusRequest: number
  readonly testId?: string | undefined
}

/** Whether choosing the entry leaves its menu open. */
function keepsOpen(entry: ChoiceEntry): boolean {
  return entry.kind === 'checkbox' && entry.keepsOpen === true
}

function roleOf(entry: FocusableEntry): 'menuitem' | 'menuitemcheckbox' | 'menuitemradio' {
  switch (entry.kind) {
    case 'checkbox':
      return 'menuitemcheckbox'
    case 'radio':
      return 'menuitemradio'
    case 'item':
    case 'submenu':
      return 'menuitem'
  }
}

/** The index of the next entry that is not a separator from `start` in `direction`, wrapping around. */
function nextFocusable(entries: readonly MenuEntry[], start: number, direction: 1 | -1): number {
  return nextIndex(
    entries.length,
    start,
    direction,
    (index) => entries[index]?.kind !== 'separator',
  )
}

function MenuList({
  id,
  name,
  entries,
  anchor,
  placement,
  choose,
  close,
  dismiss,
  dismissAll,
  focusRequest,
  testId,
}: MenuListProps) {
  const element = useRef<HTMLDivElement>(null)
  const items = useRef<(HTMLButtonElement | null)[]>([])
  const submenuAnchor = useRef<HTMLButtonElement | null>(null)
  const idPrefix = useId()
  const [active, setActive] = useState<number | null>(null)
  const [submenu, setSubmenu] = useState<{ index: number; focusRequest: number } | null>(null)

  const focusItem = useCallback((index: number) => {
    items.current[index]?.focus()
  }, [])
  const firstItem = nextFocusable(entries, -1, 1)

  // The browser light-dismisses a menu on a press outside it; the owner learns it closed.
  useMenuPopover(element, anchor, placement, dismiss)
  useLayoutEffect(() => {
    if (focusRequest > 0) {
      focusItem(firstItem)
    }
  }, [focusRequest, firstItem, focusItem])

  const closeSubmenu = useCallback(() => {
    setSubmenu(null)
  }, [])

  function openSubmenu(index: number, focus: boolean): void {
    submenuAnchor.current = items.current[index] ?? null
    setSubmenu((previous) => {
      const requests = previous?.index === index ? previous.focusRequest : 0
      return { index, focusRequest: focus ? requests + 1 : requests }
    })
  }

  function activate(index: number, source: 'enter' | 'space' | 'pointer'): void {
    const entry = entries[index]
    if (entry === undefined || entry.kind === 'separator' || entry.disabled === true) {
      return
    }
    if (entry.kind === 'submenu') {
      openSubmenu(index, source !== 'pointer')
      return
    }
    // Space checks a checkbox or radio item without closing the menu (WAI-ARIA menu pattern).
    if ((source === 'space' && entry.kind !== 'item') || keepsOpen(entry)) {
      entry.onSelect()
      return
    }
    choose(entry)
  }

  function typeahead(character: string, from: number): void {
    const labels = entries.map((entry) => (entry.kind === 'separator' ? null : entry.label))
    const index = typeaheadIndex(labels, from, character)
    if (index !== null) {
      focusItem(index)
    }
  }

  function onKeyDown(event: KeyboardEvent<HTMLDivElement>): void {
    // Keys pressed in a submenu are its own; it is nested inside this menu's element.
    if (
      !(event.target instanceof Element) ||
      event.target.closest('[role="menu"]') !== element.current
    ) {
      return
    }
    const current = active ?? -1
    // Handled keys are default-prevented, so the shortcut dispatcher and the Esc ladder skip them.
    const handled = (): void => {
      event.preventDefault()
    }
    switch (event.key) {
      case 'ArrowDown':
        handled()
        focusItem(nextFocusable(entries, current, 1))
        return
      case 'ArrowUp':
        handled()
        focusItem(nextFocusable(entries, current === -1 ? entries.length : current, -1))
        return
      case 'Home':
        handled()
        focusItem(nextFocusable(entries, -1, 1))
        return
      case 'End':
        handled()
        focusItem(nextFocusable(entries, entries.length, -1))
        return
      case 'ArrowRight':
        handled()
        if (entries[current]?.kind === 'submenu') {
          activate(current, 'enter')
        }
        return
      case 'ArrowLeft':
        handled()
        if (placement === 'right') {
          close()
        }
        return
      case 'Enter':
        handled()
        activate(current, 'enter')
        return
      case ' ':
        handled()
        activate(current, 'space')
        return
      case 'Escape':
        handled()
        close()
        return
      case 'Tab':
        // Focus moves on to the next element as usual; the menus close behind it.
        dismissAll()
        return
      default:
        if (event.key.length === 1 && !event.ctrlKey && !event.metaKey && !event.altKey) {
          handled()
          typeahead(event.key, current)
        }
    }
  }

  return (
    <div
      ref={element}
      id={id}
      role="menu"
      data-testid={testId}
      tabIndex={-1}
      popover="auto"
      aria-label={'label' in name ? name.label : undefined}
      aria-labelledby={'labelledBy' in name ? name.labelledBy : undefined}
      onKeyDown={onKeyDown}
      // `hidden open:flex`: a display class would override the browser's hiding of a closed popover.
      // `pointer-events-auto`: the top layer still inherits pointer-events from DOM parents.
      className={clsx(
        'pointer-events-auto fixed top-0 left-0 m-0 hidden max-h-dvh min-w-48 max-w-80 flex-col open:flex overflow-y-auto rounded-lg border p-1 text-sm shadow-lg transition-opacity duration-150 ease-out starting:opacity-0 motion-reduce:transition-none',
        COLOR_CLASSES.surfaceRaised.background,
        COLOR_CLASSES.forcedEdge.border,
        COLOR_CLASSES.elevation.shadow,
        COLOR_CLASSES.text.foreground,
      )}
    >
      {entries.map((entry, index) => {
        if (entry.kind === 'separator') {
          return (
            <hr
              key={`separator-${String(index)}`}
              className={clsx('-mx-1 my-1', COLOR_CLASSES.divider.border)}
            />
          )
        }
        const expanded = submenu?.index === index
        const itemId = `${idPrefix}-${String(index)}`
        const submenuId = `${itemId}-menu`
        const item = (
          <MenuItem
            key={itemId}
            entry={entry}
            id={itemId}
            active={active === index}
            expanded={expanded}
            submenuId={submenuId}
            buttonRef={(button) => {
              items.current[index] = button
            }}
            onFocus={() => {
              setActive(index)
            }}
            onBlur={() => {
              setActive((previous) => (previous === index ? null : previous))
            }}
            onPointerMove={(event) => {
              // Touch never hovers, so it neither moves focus nor opens a submenu.
              if (event.pointerType === 'touch') {
                return
              }
              focusOnPointerMove(event)
              if (entry.kind === 'submenu' && entry.disabled !== true && !expanded) {
                openSubmenu(index, false)
              } else if (entry.kind !== 'submenu' && submenu !== null) {
                setSubmenu(null)
              }
            }}
            onClick={() => {
              activate(index, 'pointer')
            }}
          />
        )
        if (entry.kind !== 'submenu') {
          return item
        }
        // The submenu sits inside this menu's element, so the browser treats it as nested.
        return (
          <div key={itemId} role="none" className="contents">
            {item}
            {expanded ? (
              <MenuList
                id={submenuId}
                name={{ labelledBy: itemId }}
                entries={entry.entries}
                anchor={submenuAnchor}
                placement="right"
                choose={choose}
                close={() => {
                  closeSubmenu()
                  focusItem(index)
                }}
                dismiss={closeSubmenu}
                dismissAll={dismissAll}
                focusRequest={submenu?.focusRequest ?? 0}
              />
            ) : null}
          </div>
        )
      })}
    </div>
  )
}

type MenuItemProps = {
  readonly entry: FocusableEntry
  readonly id: string
  /** Whether the item has focus, which highlights it. */
  readonly active: boolean
  /** Whether a submenu item's submenu is open. */
  readonly expanded: boolean
  /** The element id of a submenu item's submenu. */
  readonly submenuId: string
  readonly buttonRef: (button: HTMLButtonElement | null) => void
  readonly onFocus: () => void
  readonly onBlur: () => void
  readonly onPointerMove: (event: PointerEvent<HTMLButtonElement>) => void
  readonly onClick: () => void
}

/** One item of a menu: its mark, label and shortcut, or the chevron of a submenu item. */
function MenuItem({
  entry,
  id,
  active,
  expanded,
  submenuId,
  buttonRef,
  onFocus,
  onBlur,
  onPointerMove,
  onClick,
}: MenuItemProps) {
  const disabled = entry.disabled === true
  return (
    <button
      ref={buttonRef}
      id={id}
      type="button"
      role={roleOf(entry)}
      data-testid={entry.testId}
      tabIndex={-1}
      aria-checked={entry.kind === 'checkbox' || entry.kind === 'radio' ? entry.checked : undefined}
      aria-disabled={disabled ? true : undefined}
      aria-haspopup={entry.kind === 'submenu' ? 'menu' : undefined}
      aria-expanded={entry.kind === 'submenu' ? expanded : undefined}
      // The submenu's invoker: the browser counts a press on it as a press inside the open
      // submenu, so clicking the item does not light-dismiss the submenu that hovering opened.
      popoverTarget={entry.kind === 'submenu' ? submenuId : undefined}
      popoverTargetAction={entry.kind === 'submenu' ? 'show' : undefined}
      onFocus={onFocus}
      onBlur={onBlur}
      onPointerMove={onPointerMove}
      onClick={onClick}
      className={clsx(
        'flex h-8 w-full shrink-0 items-center gap-2 rounded-md px-2 text-left whitespace-nowrap transition-colors duration-150 motion-reduce:transition-none',
        COLOR_CLASSES.focusRing.outline,
        // The transparent state outline, at rest too, so transition-colors never fades a colored
        // outline in or out; forced colors draw it on the active item (3.23).
        COLOR_CLASSES.forcedEdge.outline,
        disabled && COLOR_CLASSES.textDisabled.foreground,
        disabled && 'cursor-not-allowed',
        !disabled && active && 'outline -outline-offset-1',
        !disabled && active && COLOR_CLASSES.hover.active,
        !disabled && !active && COLOR_CLASSES.hover.background,
      )}
    >
      <span
        aria-hidden="true"
        className={clsx(
          'flex size-4 shrink-0 items-center justify-center',
          !disabled && COLOR_CLASSES.textMuted.foreground,
        )}
      >
        <EntryMark entry={entry} />
      </span>
      <span className="min-w-0 flex-1 truncate">{entry.label}</span>
      {entry.kind === 'item' && entry.shortcut !== undefined ? (
        <span className={clsx('pl-4 text-xs', !disabled && COLOR_CLASSES.textMuted.foreground)}>
          {entry.shortcut}
        </span>
      ) : null}
      {entry.kind === 'submenu' ? (
        <ChevronRight
          aria-hidden="true"
          strokeWidth={1.75}
          className={clsx('size-4 shrink-0', !disabled && COLOR_CLASSES.textMuted.foreground)}
        />
      ) : null}
    </button>
  )
}

/** The mark before an entry's label: its icon, a check mark for a checked checkbox or radio item, or nothing. */
function EntryMark({ entry }: { readonly entry: FocusableEntry }) {
  switch (entry.kind) {
    case 'checkbox':
    case 'radio':
      return entry.checked ? (
        <Check aria-hidden="true" strokeWidth={1.75} className="size-4" />
      ) : null
    case 'item':
    case 'submenu': {
      const Icon = entry.icon
      return Icon === undefined ? null : (
        <Icon aria-hidden="true" strokeWidth={1.75} className="size-4" />
      )
    }
  }
}
