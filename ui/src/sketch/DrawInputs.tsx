import { clsx } from 'clsx/lite'
import { useEffect, useId, useRef, type KeyboardEvent, type Ref } from 'react'
import { COLOR_CLASSES, FONT_CLASSES } from '../theme/roles'
import type { DrawInputModel, SketchController } from './session'

// Dimension as you draw (ui.md 3.16.4): text inputs beside the rubber band that lock its length,
// angle, width, height, diameter or radius to a typed expression.

export type DrawInputsProps = {
  /** The positioned container, which the sketch controller moves next to the rubber band. */
  readonly ref?: Ref<HTMLFieldSetElement>
  readonly controller: SketchController
  readonly inputs: readonly DrawInputModel[]
  /** Increases each time typing on the canvas asks the first input for focus. */
  readonly focusRequest: number
}

export function DrawInputs({ ref, controller, inputs, focusRequest }: DrawInputsProps) {
  const fields = useRef<(HTMLInputElement | null)[]>([])
  const handled = useRef(focusRequest)
  const errorId = useId()

  // Typing a value on the canvas puts it in the first input, with the caret after it.
  useEffect(() => {
    if (focusRequest === handled.current) {
      return
    }
    handled.current = focusRequest
    const first = fields.current[0]
    if (first !== null && first !== undefined) {
      first.focus()
      const end = first.value.length
      first.setSelectionRange(end, end)
    }
  }, [focusRequest])

  function onKeyDown(event: KeyboardEvent<HTMLInputElement>, index: number): void {
    switch (event.key) {
      case 'Enter':
        event.preventDefault()
        controller.commitDrawInputs().catch((error: unknown) => {
          console.error('Committing the typed shape failed.', error)
        })
        return
      case 'Escape':
        event.preventDefault()
        controller.clearDrawInputs()
        return
      case 'Tab': {
        // Tab cycles between the draw inputs.
        event.preventDefault()
        const step = event.shiftKey ? -1 : 1
        const next = fields.current[(index + step + inputs.length) % inputs.length]
        next?.focus()
        next?.select()
        return
      }
      default:
    }
  }

  return (
    <fieldset ref={ref} className="pointer-events-auto absolute top-0 left-0 flex gap-1">
      <legend className="sr-only">Dimensions as you draw</legend>
      {inputs.map((input, index) => (
        <span key={input.name} className="flex">
          <input
            ref={(element) => {
              fields.current[index] = element
            }}
            type="text"
            data-testid="draw-input"
            data-field={input.name}
            aria-label={input.name}
            aria-invalid={input.error === null ? undefined : true}
            aria-describedby={input.error === null ? undefined : `${errorId}-${input.name}`}
            value={input.text}
            spellCheck={false}
            autoComplete="off"
            onChange={(event) => {
              controller.setDrawText(input.name, event.currentTarget.value)
            }}
            onKeyDown={(event) => {
              onKeyDown(event, index)
            }}
            className={clsx(
              'h-6 w-20 rounded-sm border px-1.5 text-xs shadow-sm',
              FONT_CLASSES.values,
              COLOR_CLASSES.field.background,
              COLOR_CLASSES.text.foreground,
              COLOR_CLASSES.focusRing.outline,
              input.error === null
                ? COLOR_CLASSES.borderControl.border
                : COLOR_CLASSES.errorMark.border,
            )}
          />
          {input.error === null ? null : (
            <span id={`${errorId}-${input.name}`} className="sr-only">
              {input.error}
            </span>
          )}
        </span>
      ))}
    </fieldset>
  )
}
