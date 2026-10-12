import { clsx } from 'clsx/lite'
import { Check } from 'lucide-react'
import {
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
  type KeyboardEvent,
  type RefObject,
} from 'react'
import { showPopover, useAnchoredPosition } from '../components/floating'
import type { PrinterProfile, Technology } from '../generated/protocol'
import { COLOR_CLASSES } from '../theme/roles'
import { printerGroups } from './printers'

export type PrinterMenuProps = {
  readonly open: boolean
  /** The printer button the menu opens under and returns focus to. */
  readonly anchor: RefObject<HTMLElement | null>
  /** Called with false whenever the menu closes: a choice, Esc, Tab or a press outside. */
  readonly onOpenChange: (open: boolean) => void
  /** The profiles of `listPrinterProfiles`, in its order. */
  readonly printers: readonly PrinterProfile[]
  /** The document's printer id, or null when it has none. */
  readonly current: string | null
  /** Chooses a profile by id, or null for "No printer" (`setPrinter`). */
  readonly onChoose: (profileId: string | null) => void
}

/** One radio item of the menu: a profile under its technology, or "No printer" with the id null. */
type Choice = {
  readonly id: string | null
  readonly label: string
  readonly technology: Technology | null
}

/**
 * The printer menu (`printer-menu`, ui.md 3.17) under the Print toolbar's printer button and the
 * status bar's: radio items for every profile under the headings "FDM" and "Resin", then "No
 * printer". It follows the WAI-ARIA menu button pattern like `Menu`, which has no groups.
 */
export function PrinterMenu(props: PrinterMenuProps) {
  return props.open ? <PrinterMenuList {...props} /> : null
}

/** An index wrapped into [0, count). */
function wrap(index: number, count: number): number {
  return ((index % count) + count) % count
}

function PrinterMenuList({ anchor, onOpenChange, printers, current, onChoose }: PrinterMenuProps) {
  const element = useRef<HTMLDivElement>(null)
  const items = useRef<(HTMLButtonElement | null)[]>([])
  const [active, setActive] = useState<number | null>(null)

  const groups = printerGroups(printers)
  // Every item in display order, so an item's index is its position for the arrow keys.
  const choices: readonly Choice[] = [
    ...groups.flatMap((group) =>
      group.printers.map((printer) => ({
        id: printer.id,
        label: printer.name,
        technology: group.technology,
      })),
    ),
    { id: null, label: 'No printer', technology: null },
  ]

  // The browser light-dismisses the menu on a press outside it; the owner learns it closed.
  const latestOnOpenChange = useRef(onOpenChange)
  useLayoutEffect(() => {
    latestOnOpenChange.current = onOpenChange
  })
  useEffect(() => {
    const menu = element.current
    if (menu === null) {
      return undefined
    }
    const onToggle = (event: ToggleEvent): void => {
      if (event.newState === 'closed') {
        latestOnOpenChange.current(false)
      }
    }
    menu.addEventListener('toggle', onToggle)
    return () => {
      menu.removeEventListener('toggle', onToggle)
    }
  }, [])

  useLayoutEffect(() => {
    const menu = element.current
    if (menu === null) {
      return
    }
    showPopover(menu, anchor.current)
    items.current[0]?.focus()
  }, [anchor])
  useAnchoredPosition(element, anchor, true, 'below')

  function focusItem(index: number): void {
    items.current[wrap(index, choices.length)]?.focus()
  }

  function choose(choice: Choice | undefined): void {
    if (choice === undefined) {
      return
    }
    // Focus returns first, so whatever the choice opens can take it.
    anchor.current?.focus()
    onOpenChange(false)
    onChoose(choice.id)
  }

  function typeahead(character: string, from: number): void {
    const wanted = character.toLocaleLowerCase()
    for (let step = 1; step <= choices.length; step++) {
      const index = wrap(from + step, choices.length)
      if (choices[index]?.label.toLocaleLowerCase().startsWith(wanted) === true) {
        focusItem(index)
        return
      }
    }
  }

  function onKeyDown(event: KeyboardEvent<HTMLDivElement>): void {
    const position = active ?? -1
    // Handled keys are default-prevented, so the shortcut dispatcher and the Esc ladder skip them.
    const handled = (): void => {
      event.preventDefault()
    }
    switch (event.key) {
      case 'ArrowDown':
        handled()
        focusItem(position + 1)
        return
      case 'ArrowUp':
        handled()
        focusItem(position === -1 ? -1 : position - 1)
        return
      case 'Home':
        handled()
        focusItem(0)
        return
      case 'End':
        handled()
        focusItem(-1)
        return
      case 'Enter':
        handled()
        choose(choices[position])
        return
      case ' ': {
        handled()
        // Space checks a radio item without closing the menu (WAI-ARIA menu pattern).
        const choice = choices[position]
        if (choice !== undefined) {
          onChoose(choice.id)
        }
        return
      }
      case 'Escape':
        handled()
        anchor.current?.focus()
        onOpenChange(false)
        return
      case 'Tab':
        // Focus moves on to the next element as usual; the menu closes behind it.
        onOpenChange(false)
        return
      default:
        if (event.key.length === 1 && !event.ctrlKey && !event.metaKey && !event.altKey) {
          handled()
          typeahead(event.key, position)
        }
    }
  }

  function item(choice: Choice, index: number) {
    const checked = choice.id === current
    return (
      <button
        key={choice.id ?? 'none'}
        ref={(button) => {
          items.current[index] = button
        }}
        type="button"
        role="menuitemradio"
        tabIndex={-1}
        aria-checked={checked}
        onFocus={() => {
          setActive(index)
        }}
        onBlur={() => {
          setActive((previous) => (previous === index ? null : previous))
        }}
        onPointerMove={(event) => {
          if (
            event.pointerType !== 'touch' &&
            event.currentTarget.ownerDocument.activeElement !== event.currentTarget
          ) {
            event.currentTarget.focus({ preventScroll: true })
          }
        }}
        onClick={() => {
          choose(choice)
        }}
        className={clsx(
          'flex h-8 w-full shrink-0 items-center gap-2 rounded-sm px-2 text-left whitespace-nowrap transition-colors duration-150 motion-reduce:transition-none',
          COLOR_CLASSES.focusRing.outline,
          // A resting outline color equal to the ring's, so transition-colors does not fade the
          // ring in from the text color.
          COLOR_CLASSES.accent.outline,
          active === index ? COLOR_CLASSES.hover.active : COLOR_CLASSES.hover.background,
        )}
      >
        <span
          aria-hidden="true"
          className={clsx(
            'flex size-4 shrink-0 items-center justify-center',
            COLOR_CLASSES.textMuted.foreground,
          )}
        >
          {checked ? <Check aria-hidden="true" strokeWidth={1.75} className="size-4" /> : null}
        </span>
        <span className="min-w-0 flex-1 truncate">{choice.label}</span>
      </button>
    )
  }

  return (
    <div
      ref={element}
      role="menu"
      tabIndex={-1}
      popover="auto"
      aria-label="Printer"
      data-testid="printer-menu"
      onKeyDown={onKeyDown}
      // `hidden open:flex`: a display class would override the browser's hiding of a closed popover.
      className={clsx(
        'fixed top-0 left-0 m-0 hidden max-h-dvh min-w-56 max-w-80 flex-col open:flex overflow-y-auto rounded-md border p-1 text-sm shadow-lg transition-opacity duration-150 ease-out starting:opacity-0 motion-reduce:transition-none',
        COLOR_CLASSES.surfaceRaised.background,
        COLOR_CLASSES.border.border,
        COLOR_CLASSES.text.foreground,
      )}
    >
      {groups.map((group) => (
        <fieldset key={group.technology} className="flex min-w-0 flex-col">
          <legend
            className={clsx(
              'px-2 pt-2 pb-1 text-xs font-medium',
              COLOR_CLASSES.textMuted.foreground,
            )}
          >
            {group.title}
          </legend>
          {choices.map((choice, index) =>
            choice.technology === group.technology ? item(choice, index) : null,
          )}
        </fieldset>
      ))}
      {groups.length === 0 ? null : (
        <hr className={clsx('-mx-1 my-1', COLOR_CLASSES.border.border)} />
      )}
      {choices.map((choice, index) => (choice.technology === null ? item(choice, index) : null))}
    </div>
  )
}
