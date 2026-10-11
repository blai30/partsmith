import { clsx } from 'clsx/lite'
import type { LucideIcon } from 'lucide-react'
import { useRef, useState, type KeyboardEvent } from 'react'
import { COLOR_CLASSES } from '../theme/roles'

export type SegmentedOption<Value extends string> = {
  readonly value: Value
  readonly label: string
  readonly icon?: LucideIcon
}

export type SegmentedControlProps<Value extends string> = {
  /** The group's accessible name, such as "Context". */
  readonly label: string
  readonly options: readonly SegmentedOption<Value>[]
  readonly value: Value
  readonly onChange: (value: Value) => void
  readonly disabled?: boolean
  readonly className?: string
}

/** The option index an arrow, Home or End key moves focus to, or null for other keys. */
function targetIndex(key: string, current: number, count: number): number | null {
  switch (key) {
    case 'ArrowRight':
    case 'ArrowDown':
      return (current + 1) % count
    case 'ArrowLeft':
    case 'ArrowUp':
      return (current - 1 + count) % count
    case 'Home':
      return 0
    case 'End':
      return count - 1
    default:
      return null
  }
}

/**
 * One choice among a few options, shown as joined segments: a radio group with one tab stop.
 * Arrow keys, Home and End move focus only (roving tab index, ui.md 3.5); Space, Enter or a click
 * chooses the focused option.
 */
export function SegmentedControl<Value extends string>({
  label,
  options,
  value,
  onChange,
  disabled = false,
  className,
}: SegmentedControlProps<Value>) {
  const buttons = useRef<(HTMLButtonElement | null)[]>([])
  // The option holding the tab stop while focus is inside the group; the checked one otherwise.
  const [focused, setFocused] = useState<Value | null>(null)
  const checkedIndex = options.findIndex((option) => option.value === value)
  const tabStop = focused ?? options[Math.max(checkedIndex, 0)]?.value

  function onKeyDown(event: KeyboardEvent<HTMLDivElement>): void {
    const current = options.findIndex((option) => option.value === (focused ?? value))
    const next = targetIndex(event.key, Math.max(current, 0), options.length)
    if (next === null) {
      return
    }
    event.preventDefault()
    buttons.current[next]?.focus()
  }

  return (
    <div
      role="radiogroup"
      aria-label={label}
      aria-disabled={disabled ? true : undefined}
      onKeyDown={onKeyDown}
      onBlur={(event) => {
        if (!event.currentTarget.contains(event.relatedTarget)) {
          setFocused(null)
        }
      }}
      className={clsx(
        'inline-flex h-8 shrink-0 items-stretch gap-0.5 rounded-sm border p-0.5',
        COLOR_CLASSES.field.background,
        COLOR_CLASSES.border.border,
        className,
      )}
    >
      {options.map((option, index) => {
        const checked = option.value === value
        const Icon = option.icon
        return (
          <button
            key={option.value}
            ref={(button) => {
              buttons.current[index] = button
            }}
            type="button"
            role="radio"
            aria-checked={checked}
            tabIndex={option.value === tabStop ? 0 : -1}
            disabled={disabled}
            onFocus={() => {
              setFocused(option.value)
            }}
            onClick={() => {
              if (!checked) {
                onChange(option.value)
              }
            }}
            className={clsx(
              'flex items-center gap-2 rounded-xs px-3 text-sm whitespace-nowrap transition-colors duration-150 motion-reduce:transition-none',
              COLOR_CLASSES.focusRing.outline,
              disabled && COLOR_CLASSES.textDisabled.foreground,
              disabled && 'cursor-not-allowed',
              !disabled && checked && COLOR_CLASSES.selection.background,
              !disabled && checked && COLOR_CLASSES.text.foreground,
              !disabled && checked && 'font-semibold',
              !disabled && !checked && COLOR_CLASSES.textMuted.foreground,
              !disabled && !checked && COLOR_CLASSES.hover.background,
            )}
          >
            {Icon === undefined ? null : (
              <Icon aria-hidden="true" strokeWidth={1.75} className="size-4 shrink-0" />
            )}
            {option.label}
          </button>
        )
      })}
    </div>
  )
}
