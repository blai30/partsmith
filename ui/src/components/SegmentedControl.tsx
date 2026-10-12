import { clsx } from 'clsx/lite'
import type { LucideIcon } from 'lucide-react'
import { useId, useRef, type KeyboardEvent, type Ref } from 'react'
import { COLOR_CLASSES } from '../theme/roles'
import { Tooltip } from './Tooltip'

export type SegmentedOption<Value extends string> = {
  readonly value: Value
  readonly label: string
  readonly icon?: LucideIcon
  /** A tooltip naming what choosing the option does; null hides it for now (Tooltip). */
  readonly tooltip?: string | null
}

export type SegmentedControlProps<Value extends string> = {
  /** The group's accessible name, such as "Context". */
  readonly label: string
  readonly options: readonly SegmentedOption<Value>[]
  readonly value: Value
  readonly onChange: (value: Value) => void
  readonly disabled?: boolean
  readonly className?: string
  /** Ids of elements that describe the group, such as a field's help. */
  readonly describedBy?: string | undefined
  /** An id for the checked option's radio, so other controls can move focus to the group. */
  readonly checkedId?: string | undefined
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
 * One choice among a few options, shown as joined segments: native radio buttons, so the group
 * is one tab stop on the checked option. Arrow keys, Home and End only move focus (ui.md 3.5),
 * because choosing can start work such as a new sketch; Space or a click chooses.
 */
export function SegmentedControl<Value extends string>({
  label,
  options,
  value,
  onChange,
  disabled = false,
  className,
  describedBy,
  checkedId,
}: SegmentedControlProps<Value>) {
  const name = useId()
  const radios = useRef<(HTMLInputElement | null)[]>([])

  function onKeyDown(event: KeyboardEvent<HTMLInputElement>, index: number): void {
    const next = targetIndex(event.key, index, options.length)
    if (next === null) {
      return
    }
    // Native radios check the option an arrow key reaches; here the key only moves focus.
    event.preventDefault()
    radios.current[next]?.focus()
  }

  return (
    <div
      role="radiogroup"
      aria-label={label}
      aria-describedby={describedBy}
      className={clsx(
        'inline-flex h-8 shrink-0 items-stretch gap-0.5 rounded-sm border p-0.5',
        COLOR_CLASSES.field.background,
        COLOR_CLASSES.border.border,
        className,
      )}
    >
      {options.map((option, index) => {
        const segment = (
          <Segment
            key={option.value}
            option={option}
            name={name}
            id={option.value === value ? checkedId : undefined}
            checked={option.value === value}
            disabled={disabled}
            radioRef={(radio) => {
              radios.current[index] = radio
            }}
            onChange={() => {
              onChange(option.value)
            }}
            onKeyDown={(event) => {
              onKeyDown(event, index)
            }}
          />
        )
        return option.tooltip === undefined ? (
          segment
        ) : (
          <Tooltip key={option.value} label={option.tooltip}>
            {segment}
          </Tooltip>
        )
      })}
    </div>
  )
}

type SegmentProps<Value extends string> = {
  readonly option: SegmentedOption<Value>
  readonly name: string
  readonly id: string | undefined
  readonly checked: boolean
  readonly disabled: boolean
  readonly radioRef: Ref<HTMLInputElement>
  readonly onChange: () => void
  readonly onKeyDown: (event: KeyboardEvent<HTMLInputElement>) => void
  /** From a Tooltip around the segment, for the radio. */
  readonly 'aria-describedby'?: string | undefined
}

/** One option: a transparent native radio over its label. */
function Segment<Value extends string>({
  option,
  name,
  id,
  checked,
  disabled,
  radioRef,
  onChange,
  onKeyDown,
  'aria-describedby': describedBy,
}: SegmentProps<Value>) {
  const Icon = option.icon
  return (
    <label
      className={clsx(
        'relative flex items-center gap-2 rounded-xs px-3 text-sm whitespace-nowrap transition-colors duration-150 motion-reduce:transition-none',
        disabled && COLOR_CLASSES.textDisabled.foreground,
        !disabled && checked && COLOR_CLASSES.selection.background,
        !disabled && checked && COLOR_CLASSES.text.foreground,
        !disabled && checked && 'font-semibold',
        !disabled && !checked && COLOR_CLASSES.textMuted.foreground,
        !disabled && !checked && COLOR_CLASSES.hover.background,
      )}
    >
      {/* The transparent radio covers its segment, so the focus ring outlines the segment. */}
      <input
        ref={radioRef}
        id={id}
        type="radio"
        name={name}
        value={option.value}
        checked={checked}
        disabled={disabled}
        aria-describedby={describedBy}
        onChange={onChange}
        onKeyDown={onKeyDown}
        className={clsx(
          'absolute inset-0 m-0 size-full appearance-none rounded-xs',
          COLOR_CLASSES.focusRing.outline,
          disabled && 'cursor-not-allowed',
        )}
      />
      {Icon === undefined ? null : (
        <Icon aria-hidden="true" strokeWidth={1.75} className="size-4 shrink-0" />
      )}
      {option.label}
    </label>
  )
}
