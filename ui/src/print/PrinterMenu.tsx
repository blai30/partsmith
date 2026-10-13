import { clsx } from 'clsx/lite'
import { Check } from 'lucide-react'
import { Fragment, useId, useRef, useState, type KeyboardEvent, type RefObject } from 'react'
import {
  focusOnPointerMove,
  nextIndex,
  typeaheadIndex,
  useMenuPopover,
} from '../components/menu-navigation'
import type { PrinterProfile, Technology } from '../generated/protocol'
import { COLOR_CLASSES } from '../theme/roles'
import { defaultPrinterName, printerDetail, printerGroups } from './printer'

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
  /** Chooses a profile by id, or null for the default item, which clears the printer (`setPrinter`). */
  readonly onChoose: (profileId: string | null) => void
}

/** One radio item of the menu: a profile under its technology, or the default item with the id null. */
type Choice = {
  readonly id: string | null
  readonly label: string
  /** The profile's key numbers (`printerDetail`); null for the default item. */
  readonly detail: string | null
  readonly technology: Technology | null
}

/**
 * The printer menu (`printer-menu`, ui.md 3.17) under the Print toolbar's printer button and the
 * status bar's: radio items for every profile with its key numbers, under the headings "FDM" and
 * "Resin", then "Default ({default name})". It follows the WAI-ARIA menu button pattern like `Menu`.
 * The radio items are one set, so the headings are visual labels that each item's description
 * repeats, not groups, which would split the set.
 */
export function PrinterMenu(props: PrinterMenuProps) {
  return props.open ? <PrinterMenuList {...props} /> : null
}

function PrinterMenuList({ anchor, onOpenChange, printers, current, onChoose }: PrinterMenuProps) {
  const element = useRef<HTMLDivElement>(null)
  const items = useRef<(HTMLButtonElement | null)[]>([])
  const [active, setActive] = useState<number | null>(null)
  const idPrefix = useId()

  const groups = printerGroups(printers)
  // Every item in display order, so an item's index is its position for the arrow keys.
  const choices: readonly Choice[] = [
    ...groups.flatMap((group) =>
      group.printers.map((printer) => ({
        id: printer.id,
        label: printer.name,
        detail: printerDetail(printer),
        technology: group.technology,
      })),
    ),
    {
      id: null,
      label: `Default (${defaultPrinterName(printers)})`,
      detail: null,
      technology: null,
    },
  ]

  function headingId(technology: Technology): string {
    return `${idPrefix}-${technology}`
  }

  function detailId(index: number): string {
    return `${idPrefix}-detail-${String(index)}`
  }

  // The browser light-dismisses the menu on a press outside it; the owner learns it closed.
  // Showing it focuses the first item.
  useMenuPopover(
    element,
    anchor,
    'below',
    () => {
      onOpenChange(false)
    },
    () => {
      items.current[0]?.focus()
    },
  )

  function focusItem(index: number): void {
    items.current[index]?.focus()
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
    const index = typeaheadIndex(
      choices.map((choice) => choice.label),
      from,
      character,
    )
    if (index !== null) {
      focusItem(index)
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
        focusItem(nextIndex(choices.length, position, 1))
        return
      case 'ArrowUp':
        handled()
        focusItem(nextIndex(choices.length, position === -1 ? choices.length : position, -1))
        return
      case 'Home':
        handled()
        focusItem(0)
        return
      case 'End':
        handled()
        focusItem(choices.length - 1)
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
    const technology = choice.technology
    return (
      <button
        key={choice.id ?? 'default'}
        ref={(button) => {
          items.current[index] = button
        }}
        type="button"
        role="menuitemradio"
        tabIndex={-1}
        aria-checked={checked}
        aria-describedby={
          technology === null ? undefined : `${headingId(technology)} ${detailId(index)}`
        }
        onFocus={() => {
          setActive(index)
        }}
        onBlur={() => {
          setActive((previous) => (previous === index ? null : previous))
        }}
        onPointerMove={focusOnPointerMove}
        onClick={() => {
          choose(choice)
        }}
        className={clsx(
          'flex min-h-8 w-full shrink-0 items-center gap-2 rounded-sm px-2 py-1.5 text-left whitespace-nowrap transition-colors duration-150 motion-reduce:transition-none',
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
        <span className="flex min-w-0 flex-1 flex-col">
          <span className="truncate">{choice.label}</span>
          {choice.detail === null ? null : (
            // Read as the item's description, not as part of its name.
            <span
              id={detailId(index)}
              aria-hidden="true"
              className={clsx('truncate text-xs', COLOR_CLASSES.textMuted.foreground)}
            >
              {choice.detail}
            </span>
          )}
        </span>
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
        <Fragment key={group.technology}>
          {/* A visual label; each item under it names it in its description. */}
          <div
            id={headingId(group.technology)}
            aria-hidden="true"
            className={clsx(
              'px-2 pt-2 pb-1 text-xs font-medium',
              COLOR_CLASSES.textMuted.foreground,
            )}
          >
            {group.title}
          </div>
          {choices.map((choice, index) =>
            choice.technology === group.technology ? item(choice, index) : null,
          )}
        </Fragment>
      ))}
      {groups.length === 0 ? null : (
        // Presentational: a separator would end the set of radio items.
        <hr aria-hidden="true" className={clsx('-mx-1 my-1', COLOR_CLASSES.border.border)} />
      )}
      {choices.map((choice, index) => (choice.technology === null ? item(choice, index) : null))}
    </div>
  )
}
