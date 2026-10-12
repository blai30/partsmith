import { clsx } from 'clsx/lite'
import {
  Box,
  Check,
  History,
  Search,
  Shapes,
  SlidersHorizontal,
  type LucideIcon,
} from 'lucide-react'
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
import { updateSettings } from '../app/actions/settings'
import { useApp, useAppState } from '../app/AppProvider'
import type { App } from '../app/createApp'
import { rankPaletteItems, type PaletteItem } from '../commands/fuzzy'
import { FEATURE_ICONS } from '../commands/icons'
import { paletteItems } from '../commands/palette'
import { measureTiming, TIMING_NAMES } from '../performance/marks'
import { PALETTE_RECENT_LIMIT } from '../settings/settings'
import { COLOR_CLASSES } from '../theme/roles'
import { keepInViewVertically } from './scroll'

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
      const id = item.id.slice('feature:'.length)
      const type = state.document?.recipe.features?.find((feature) => feature.id === id)?.type
      const icon = type === undefined ? undefined : state.catalog?.byType[type]?.manifest.icon
      return icon === undefined ? Shapes : (FEATURE_ICONS[icon] ?? Shapes)
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
): readonly PaletteItem[] {
  const start = performance.now()
  const ranked = rankPaletteItems(query, items, recentIds)
  measureTiming(TIMING_NAMES.palette, start)
  return ranked
}

function wrap(index: number, count: number): number {
  return ((index % count) + count) % count
}

function OpenPalette() {
  const app = useApp()
  const listId = useId()
  const optionPrefix = useId()
  const palette = useRef<HTMLDialogElement>(null)
  const input = useRef<HTMLInputElement>(null)
  const list = useRef<HTMLUListElement>(null)
  const recentIds = useAppState((state) => state.settings.paletteRecent)
  const [items, setItems] = useState<readonly PaletteItem[]>(() => paletteItems(app, []))
  const [query, setQuery] = useState('')
  const ranked = useMemo(() => rank(query, items, recentIds), [query, items, recentIds])
  // The first option is active whenever the list changes (3.4).
  const [chosen, setChosen] = useState({ ranked, index: 0 })
  const active = chosen.ranked === ranked ? chosen.index : 0
  const [footer, setFooter] = useState('')

  const close = (): void => {
    app.store.setState((state) => ({ panels: { ...state.panels, paletteOpen: false } }))
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

  // Recent files join the items once they are read.
  useEffect(() => {
    let current = true
    app.recent.list().then(
      (recent) => {
        if (current && recent.length > 0) {
          setItems(paletteItems(app, recent))
        }
      },
      (error: unknown) => {
        console.error('Reading the recent files failed.', error)
      },
    )
    return () => {
      current = false
    }
  }, [app])

  // A press outside closes the palette.
  useEffect(() => {
    const onPointerDown = (event: PointerEvent): void => {
      if (event.target instanceof Node && palette.current?.contains(event.target) !== true) {
        app.store.setState((state) => ({ panels: { ...state.panels, paletteOpen: false } }))
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
    const recent = app.store.getState().settings.paletteRecent
    updateSettings(app, {
      paletteRecent: [item.id, ...recent.filter((id) => id !== item.id)].slice(
        0,
        PALETTE_RECENT_LIMIT,
      ),
    })
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
      <dialog
        ref={palette}
        open
        aria-label="Command palette"
        data-testid="palette"
        className={clsx(
          'pointer-events-auto relative m-0 flex w-140 max-w-full flex-col overflow-hidden rounded-md border p-0 shadow-lg transition-opacity duration-150 ease-out starting:opacity-0 motion-reduce:transition-none',
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
            placeholder="Search commands, bodies and parameters"
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
          {ranked.map((item, index) => {
            const Icon = iconFor(app, item)
            const checked = checkedOf(app, item)
            const disabled = item.disabledReason !== null
            const tag = KIND_TAGS[item.kind]
            const detail = item.disabledReason ?? item.detail
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
                  {Icon === null ? null : (
                    <Icon aria-hidden="true" strokeWidth={1.75} className="size-4" />
                  )}
                </span>
                <span className="min-w-0 truncate">{item.title}</span>
                {detail === null ? null : (
                  <span
                    className={clsx(
                      'min-w-0 shrink truncate text-xs',
                      COLOR_CLASSES.textMuted.foreground,
                    )}
                  >
                    {detail}
                  </span>
                )}
                <span className="ml-auto flex shrink-0 items-center gap-3">
                  {tag === null ? null : (
                    <span className={clsx('text-xs', COLOR_CLASSES.textMuted.foreground)}>
                      {tag}
                    </span>
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
          })}
        </ul>
        <output
          className={clsx(
            'block shrink-0 border-t px-3 py-2 text-xs empty:hidden',
            COLOR_CLASSES.border.border,
            COLOR_CLASSES.textMuted.foreground,
          )}
        >
          {footer}
        </output>
      </dialog>
    </div>
  )
}

/** The palette while `panels.paletteOpen` is set; nothing otherwise. */
export function CommandPalette() {
  const open = useAppState((state) => state.panels.paletteOpen)
  return open ? <OpenPalette /> : null
}
