import { clsx } from 'clsx/lite'
import { ChevronDown } from 'lucide-react'
import { useEffect, useLayoutEffect, useRef, useState, type ReactNode, type RefObject } from 'react'
import { setCommandField } from '../../app/actions/command'
import { useApp } from '../../app/AppProvider'
import { HelpLabel } from '../../components/HelpLabel'
import { SegmentedControl } from '../../components/SegmentedControl'
import { COLOR_CLASSES } from '../../theme/roles'
import type { InspectorField } from '../model'
import { fieldInputId } from './field-id'

export type ChoiceFieldProps = {
  readonly field: InspectorField
  readonly options: readonly { readonly value: string; readonly label: string }[]
  readonly presentation: 'segmented' | 'select'
  /** An inline toggle at the end of the row, such as Reverse (3.7). */
  readonly trailing?: ReactNode
}

/**
 * Whether a segmented row fits its field: measured before the browser paints and whenever the
 * segmented control changes size (the chosen option is drawn bold), and tried again from the
 * segmented control once the fonts have loaded or the labels change. A row that overflowed stays a
 * select until then, so choosing an option with a wider bold label cannot flip it back and forth.
 */
function useSegmentsFit(labels: string): {
  readonly fits: boolean
  readonly rowRef: RefObject<HTMLDivElement | null>
} {
  const rowRef = useRef<HTMLDivElement>(null)
  // The labels that overflowed the row, or null while the segmented control is tried.
  const [overflowed, setOverflowed] = useState<string | null>(null)
  const fits = overflowed !== labels
  useLayoutEffect(() => {
    const row = rowRef.current
    if (!fits || row === null) {
      return undefined
    }
    const measure = () => {
      if (row.scrollWidth > row.clientWidth) {
        setOverflowed(labels)
      }
    }
    measure()
    const group = row.firstElementChild
    if (group === null || typeof ResizeObserver !== 'function') {
      return undefined
    }
    const observer = new ResizeObserver(measure)
    observer.observe(group)
    return () => {
      observer.disconnect()
    }
  }, [fits, labels])
  useEffect(() => {
    if (!('fonts' in document)) {
      return undefined
    }
    const retry = () => {
      setOverflowed(null)
    }
    document.fonts.addEventListener('loadingdone', retry)
    return () => {
      document.fonts.removeEventListener('loadingdone', retry)
    }
  }, [])
  return { fits, rowRef }
}

/**
 * One of a few options (3.7): a segmented control up to 4 options when its segments fit the field,
 * else a native select, so no option is ever clipped.
 */
export function ChoiceField({ field, options, presentation, trailing = null }: ChoiceFieldProps) {
  const app = useApp()
  const value = typeof field.value === 'string' ? field.value : (options[0]?.value ?? '')
  const id = fieldInputId(field.path)
  const helpId = `${id}-help`
  const help = field.help
  const { fits, rowRef } = useSegmentsFit(options.map((option) => option.label).join('\n'))
  if (presentation === 'segmented' && fits) {
    return (
      <div className="flex min-w-0 flex-col gap-1">
        <HelpLabel help={help} helpId={helpId}>
          <span
            aria-hidden="true"
            className={clsx(
              'self-start text-xs font-medium',
              COLOR_CLASSES.textMuted.foreground,
              help !== null && 'cursor-help',
            )}
          >
            {field.label}
          </span>
        </HelpLabel>
        <div ref={rowRef} className="flex min-w-0 items-center justify-between gap-2">
          <SegmentedControl
            stretch
            label={field.label}
            options={options}
            value={value}
            describedBy={help === null ? undefined : helpId}
            // A status message about this field moves focus to its checked option (4.2).
            checkedId={id}
            onChange={(next) => {
              setCommandField(app, field.path, next)
            }}
            className="self-start"
          />
          {trailing}
        </div>
      </div>
    )
  }
  return (
    <div className="flex min-w-0 flex-col gap-1">
      <HelpLabel help={help} helpId={helpId}>
        <label
          htmlFor={id}
          className={clsx(
            'self-start text-xs font-medium',
            COLOR_CLASSES.textMuted.foreground,
            help !== null && 'cursor-help',
          )}
        >
          {field.label}
        </label>
      </HelpLabel>
      <div className="flex min-w-0 items-center gap-2">
        {/* The browser's arrow ignores the UI scale, so the select draws its own (3.7). */}
        <div className="relative flex min-w-0 flex-1 items-center">
          <select
            id={id}
            value={value}
            aria-describedby={help === null ? undefined : helpId}
            onChange={(event) => {
              setCommandField(app, field.path, event.currentTarget.value)
            }}
            className={clsx(
              'h-8 w-full appearance-none rounded-md border pr-8 pl-2 text-sm',
              COLOR_CLASSES.field.background,
              COLOR_CLASSES.text.foreground,
              COLOR_CLASSES.borderControl.border,
              COLOR_CLASSES.focusRing.outline,
            )}
          >
            {options.map((option) => (
              <option key={option.value} value={option.value}>
                {option.label}
              </option>
            ))}
          </select>
          <ChevronDown
            aria-hidden="true"
            strokeWidth={1.75}
            className={clsx(
              'pointer-events-none absolute right-2 size-4',
              COLOR_CLASSES.textMuted.foreground,
            )}
          />
        </div>
        {trailing}
      </div>
    </div>
  )
}
