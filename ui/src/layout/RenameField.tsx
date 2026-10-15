import { clsx } from 'clsx/lite'
import { useLayoutEffect, useRef, useState } from 'react'
import { COLOR_CLASSES } from '../theme/roles'

// The inline rename field of timeline items and browser rows (ui.md 3.9, 3.10): it takes focus
// with its text selected; Enter or leaving it commits, Esc cancels.

export type RenameFieldProps = {
  readonly initial: string
  /** Called once: with the typed name, or null when canceled. */
  readonly onDone: (name: string | null) => void
  /** Layout classes for where the field sits. */
  readonly className?: string
}

export function RenameField({ initial, onDone, className }: RenameFieldProps) {
  const input = useRef<HTMLInputElement>(null)
  const [value, setValue] = useState(initial)
  const done = useRef(false)

  useLayoutEffect(() => {
    input.current?.focus()
    input.current?.select()
  }, [])

  function finish(name: string | null): void {
    if (!done.current) {
      done.current = true
      onDone(name)
    }
  }

  return (
    <input
      ref={input}
      type="text"
      aria-label="Name"
      value={value}
      spellCheck={false}
      autoComplete="off"
      // The field's clicks and keys belong to it, not to the row or item around it.
      onClick={(event) => {
        event.stopPropagation()
      }}
      onDoubleClick={(event) => {
        event.stopPropagation()
      }}
      onPointerDown={(event) => {
        event.stopPropagation()
      }}
      onChange={(event) => {
        setValue(event.currentTarget.value)
      }}
      onKeyDown={(event) => {
        event.stopPropagation()
        if (event.key === 'Enter') {
          event.preventDefault()
          finish(value)
        } else if (event.key === 'Escape') {
          event.preventDefault()
          finish(null)
        }
      }}
      onBlur={() => {
        finish(value)
      }}
      className={clsx(
        'h-7 rounded-md border px-2 text-sm',
        COLOR_CLASSES.field.background,
        COLOR_CLASSES.accent.border,
        COLOR_CLASSES.text.foreground,
        COLOR_CLASSES.focusRing.outline,
        className,
      )}
    />
  )
}
