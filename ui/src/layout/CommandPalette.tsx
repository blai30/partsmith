import { clsx } from 'clsx/lite'
import { Box, Check, History, Search, SlidersHorizontal, type LucideIcon } from 'lucide-react'
import {
  useEffect,
  useId,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
  type ChangeEvent,
  type KeyboardEvent,
} from 'react'
import { recordPaletteRecent } from '../app/actions/settings'
import { setPaletteOpen } from '../app/actions/panels'
import { useApp, useAppState } from '../app/AppProvider'
import type { App } from '../app/createApp'
import { rankPaletteItems, recentItems, type PaletteItem } from '../commands/fuzzy'
import { featureIcon } from '../commands/icons'
import { paletteItems } from '../commands/palette'
import { parseSelectionKey } from '../inspector/selection-values'
import { measureTiming, TIMING_NAMES } from '../performance/marks'
import type { AppContext } from '../state/store'
import { COLOR_CLASSES } from '../theme/roles'
import { keepInViewVertically } from './scroll'
import { useRecentFiles } from './use-recent-files'

// The command palette (ui.md 3.4): a dialog with a combobox over a listbox of commands, features,
// bodies, parameters, recent files and settings, ranked on every keystroke. Focus stays in the
// input; the arrow keys move the active option.

/** How far PageUp and PageDown move the active option. */
const PAGE_STEP = 8

const KIND_TAGS: Readonly<Record<PaletteItem['kind'], string | null>> = {
  command: null,
  setting: null,
  feature: 'Feature',
  body: 'Body',
  parameter: 'Parameter',
  recent: 'Recent file',
}

/** The icon of an item: its command's, its feature type's, or one per kind. */
function iconFor(app: App, item: PaletteItem): LucideIcon | null {
  switch (item.kind) {
    case 'command':
    case 'setting':
      return app.commands.get(item.id)?.icon ?? null
    case 'feature': {
      const state = app.store.getState()
      const id = parseSelectionKey(item.id)?.id
      const type = state.document?.recipe.features?.find((feature) => feature.id === id)?.type
      const icon = type === undefined ? undefined : state.catalog?.byType[type]?.manifest.icon
      return featureIcon(icon)
    }
    case 'body':
      return Box
    case 'parameter':
      return SlidersHorizontal
    case 'recent':
      return History
  }
}

/** The toggle state of a command item with `checked`, else null. */
function checkedOf(app: App, item: PaletteItem): boolean | null {
  if (item.kind !== 'command' && item.kind !== 'setting') {
    return null
  }
  return app.commands.get(item.id)?.checked?.(app) ?? null
}

/** Ranks the items for a query and records the time it took as `ps:palette`. */
function rank(
  query: string,
  items: readonly PaletteItem[],
  recentIds: readonly string[],
  context: AppContext,
): readonly PaletteItem[] {
  const start = performance.now()
  const ranked = rankPaletteItems(query, items, recentIds, context)
  measureTiming(TIMING_NAMES.palette, start)
  return ranked
}

/** A run of options under one heading, with each option's index in the whole list. */
type OptionGroup = {
  readonly heading: string | null
  readonly options: readonly { readonly item: PaletteItem; readonly index: number }[]
}

/**
 * The headed groups of the list: with an empty query "Recent" for the recent items, then one per
 * category in the ranked order (3.4); with a query, one group without a heading.
 */
function groupsOf(
  ranked: readonly PaletteItem[],
  query: string,
  items: readonly PaletteItem[],
  recentIds: readonly string[],
): readonly OptionGroup[] {
  const indexed = ranked.map((item, index) => ({ item, index }))
  if (query.trim() !== '') {
    return [{ heading: null, options: indexed }]
  }
  const recentCount = recentItems(items, recentIds).length
  const groups: { heading: string | null; options: { item: PaletteItem; index: number }[] }[] = []
  for (const option of indexed) {
    const heading = option.index < recentCount ? 'Recent' : option.item.category
    const last = groups.at(-1)
    if (last !== undefined && last.heading === heading) {
      last.options.push(option)
    } else {
      groups.push({ heading, options: [option] })
    }
  }
  return groups
}

function wrap(index: number, count: number): number {
  return ((index % count) + count) % count
}

function OpenPalette() {
  const app = useApp()
  const listId = useId()
  const optionPrefix = useId()
  const palette = useRef<HTMLDivElement>(null)
  const input = useRef<HTMLInputElement>(null)
  const list = useRef<HTMLUListElement>(null)
  const descriptionId = useId()
  const recentIds = useAppState((state) => state.settings.paletteRecent)
  const context = useAppState((state) => state.context)
  // Recent files join the items once they are read.
  const recent = useRecentFiles(app, true)
  const items = useMemo(() => paletteItems(app, recent), [app, recent])
  const [query, setQuery] = useState('')
  const ranked = useMemo(
    () => rank(query, items, recentIds, context),
    [query, items, recentIds, context],
  )
  const groups = useMemo(
    () => groupsOf(ranked, query, items, recentIds),
    [ranked, query, items, recentIds],
  )
  // The first option is active whenever the list changes (3.4).
  const [chosen, setChosen] = useState({ ranked, index: 0 })
  const active = chosen.ranked === ranked ? chosen.index : 0
  const [footer, setFooter] = useState('')

  const close = (): void => {
    setPaletteOpen(app, false)
  }

  // Focus moves into the input and returns, on closing, to where it was (3.4).
  useLayoutEffect(() => {
    const previous = document.activeElement instanceof HTMLElement ? document.activeElement : null
    input.current?.focus()
    return () => {
      if (previous?.isConnected === true) {
        previous.focus()
      }
    }
  }, [])

  // A press outside closes the palette.
  useEffect(() => {
    const onPointerDown = (event: PointerEvent): void => {
      if (event.target instanceof Node && palette.current?.contains(event.target) !== true) {
        setPaletteOpen(app, false)
      }
    }
    document.addEventListener('pointerdown', onPointerDown, true)
    return () => {
      document.removeEventListener('pointerdown', onPointerDown, true)
    }
  }, [app])

  const activeItem = ranked[active]
  const activeId = activeItem === undefined ? undefined : `${optionPrefix}-${String(active)}`

  useEffect(() => {
    const option = activeId === undefined ? null : document.getElementById(activeId)
    if (list.current !== null && option !== null) {
      keepInViewVertically(list.current, option)
    }
  }, [activeId])

  function onChange(event: ChangeEvent<HTMLInputElement>): void {
    setFooter('')
    setQuery(event.currentTarget.value)
  }

  function setActive(index: number): void {
    setFooter('')
    setChosen({ ranked, index })
  }

  function runItem(item: PaletteItem): void {
    if (item.disabledReason !== null) {
      setFooter(item.disabledReason)
      return
    }
    recordPaletteRecent(app, item.id)
    close()
    item.run()
  }

  function onKeyDown(event: KeyboardEvent<HTMLInputElement>): void {
    const count = ranked.length
    switch (event.key) {
      case 'ArrowDown':
        event.preventDefault()
        if (count > 0) {
          setActive(wrap(active + 1, count))
        }
        return
      case 'ArrowUp':
        event.preventDefault()
        if (count > 0) {
          setActive(wrap(active - 1, count))
        }
        return
      case 'PageDown':
        event.preventDefault()
        if (count > 0) {
          setActive(Math.min(active + PAGE_STEP, count - 1))
        }
        return
      case 'PageUp':
        event.preventDefault()
        if (count > 0) {
          setActive(Math.max(active - PAGE_STEP, 0))
        }
        return
      case 'Enter':
        event.preventDefault()
        if (activeItem !== undefined) {
          runItem(activeItem)
        }
        return
      case 'Escape':
        event.preventDefault()
        close()
        return
      case 'Tab':
        event.preventDefault()
        return
      default:
    }
  }

  return (
    <div className="pointer-events-none fixed inset-x-4 top-1/6 z-50 flex justify-center">
      <div
        ref={palette}
        // The spec names the dialog role on this element (ui.md 3.4, 6.1); a native dialog element
        // would bring browser placement styles to a panel the layer above positions.
        // oxlint-disable-next-line jsx-a11y/prefer-tag-over-role
        role="dialog"
        aria-label="Command palette"
        data-testid="palette"
        className={clsx(
          'pointer-events-auto flex w-140 max-w-full flex-col overflow-hidden rounded-md border shadow-lg transition-opacity duration-150 ease-out starting:opacity-0 motion-reduce:transition-none',
          COLOR_CLASSES.surfaceRaised.background,
          COLOR_CLASSES.border.border,
          COLOR_CLASSES.text.foreground,
        )}
      >
        <div
          className={clsx(
            'flex h-12 shrink-0 items-center gap-2 border-b px-3',
            COLOR_CLASSES.border.border,
          )}
        >
          <Search
            aria-hidden="true"
            strokeWidth={1.75}
            className={clsx('size-4 shrink-0', COLOR_CLASSES.textMuted.foreground)}
          />
          <input
            ref={input}
            type="text"
            role="combobox"
            data-testid="palette-input"
            aria-label="Search"
            aria-expanded="true"
            aria-controls={listId}
            aria-autocomplete="list"
            aria-activedescendant={activeId}
            placeholder="Search commands, features, bodies, parameters and settings"
            autoComplete="off"
            spellCheck={false}
            value={query}
            onChange={onChange}
            onKeyDown={onKeyDown}
            className="h-full min-w-0 flex-1 bg-transparent text-sm outline-none"
          />
        </div>
        <ul
          ref={list}
          id={listId}
          // The combobox pattern needs a listbox of rich options, which a native select cannot hold.
          // oxlint-disable-next-line jsx-a11y/prefer-tag-over-role, jsx-a11y/no-noninteractive-element-to-interactive-role
          role="listbox"
          aria-label="Results"
          className="relative max-h-96 overflow-y-auto p-1"
        >
          {ranked.length === 0 && query.trim() !== '' ? (
            // The one row of an empty result is not an option (3.4).
            <li
              role="presentation"
              data-testid="palette-empty"
              className={clsx('px-2 py-2 text-sm', COLOR_CLASSES.textMuted.foreground)}
            >
              {`Nothing matches ${query.trim()}.`}
            </li>
          ) : null}
          {groups.map((group) =>
            group.heading === null ? (
              group.options.map(({ item, index }) => renderOption(item, index))
            ) : (
              <li
                key={`${group.heading} ${String(group.options[0]?.index ?? 0)}`}
                // A listbox groups its options with the group role (3.4).
                // oxlint-disable-next-line jsx-a11y/prefer-tag-over-role, jsx-a11y/no-noninteractive-element-to-interactive-role
                role="group"
                aria-label={group.heading}
              >
                <div
                  data-testid="palette-heading"
                  aria-hidden="true"
                  className={clsx('px-2 pt-2 pb-1 text-xs', COLOR_CLASSES.textMuted.foreground)}
                >
                  {group.heading}
                </div>
                <ul role="none">
                  {group.options.map(({ item, index }) => renderOption(item, index))}
                </ul>
              </li>
            ),
          )}
        </ul>
        <div
          data-testid="palette-footer"
          className={clsx(
            'flex min-h-8 shrink-0 flex-col justify-center border-t px-3 py-2 text-xs',
            COLOR_CLASSES.border.border,
            COLOR_CLASSES.textMuted.foreground,
          )}
        >
          <p
            id={descriptionId}
            hidden={footer !== '' || (activeItem?.description ?? null) === null}
          >
            {activeItem?.description ?? ''}
          </p>
          <output className="block empty:hidden">{footer}</output>
        </div>
      </div>
    </div>
  )

  function renderOption(item: PaletteItem, index: number) {
    const Icon = iconFor(app, item)
    const checked = checkedOf(app, item)
    const disabled = item.disabledReason !== null
    const tag = KIND_TAGS[item.kind]
    const detail = item.disabledReason ?? item.detail
    const described = index === active && item.description !== null
    return (
      // Options take no focus or keys: the input keeps both and points at the active option.
      // oxlint-disable-next-line jsx-a11y/click-events-have-key-events
      <li
        key={item.id}
        id={`${optionPrefix}-${String(index)}`}
        // oxlint-disable-next-line jsx-a11y/prefer-tag-over-role, jsx-a11y/no-noninteractive-element-to-interactive-role
        role="option"
        data-testid="palette-option"
        data-item-id={item.id}
        data-kind={item.kind}
        aria-selected={index === active}
        aria-disabled={disabled ? true : undefined}
        aria-checked={checked ?? undefined}
        aria-describedby={described ? descriptionId : undefined}
        onPointerMove={() => {
          if (index !== active) {
            setActive(index)
          }
        }}
        onPointerDown={(event) => {
          // Keeps focus in the input.
          event.preventDefault()
        }}
        onClick={() => {
          runItem(item)
        }}
        className={clsx(
          'flex h-9 cursor-default items-center gap-3 rounded-sm px-2 text-sm',
          index === active && COLOR_CLASSES.hover.active,
          disabled && COLOR_CLASSES.textDisabled.foreground,
        )}
      >
        <span className="flex size-4 shrink-0 items-center justify-center">
          {Icon === null ? null : <Icon aria-hidden="true" strokeWidth={1.75} className="size-4" />}
        </span>
        <span className="min-w-0 truncate">{item.title}</span>
        {detail === null ? null : (
          <span
            className={clsx('min-w-0 shrink truncate text-xs', COLOR_CLASSES.textMuted.foreground)}
          >
            {detail}
          </span>
        )}
        <span className="ml-auto flex shrink-0 items-center gap-3">
          {tag === null ? null : (
            <span className={clsx('text-xs', COLOR_CLASSES.textMuted.foreground)}>{tag}</span>
          )}
          {item.shortcut === null ? null : (
            <span
              data-testid="palette-shortcut"
              className={clsx('text-xs', COLOR_CLASSES.textMuted.foreground)}
            >
              {item.shortcut}
            </span>
          )}
          {checked === true ? (
            <Check aria-hidden="true" strokeWidth={1.75} className="size-4" />
          ) : null}
        </span>
      </li>
    )
  }
}

/** The palette while `panels.paletteOpen` is set; nothing otherwise. */
export function CommandPalette() {
  const open = useAppState((state) => state.panels.paletteOpen)
  return open ? <OpenPalette /> : null
}
