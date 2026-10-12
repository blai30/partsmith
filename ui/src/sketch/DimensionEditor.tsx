import { clsx } from 'clsx/lite'
import { useEffect, useId, useRef, type Ref } from 'react'
import { Button } from '../components/Button'
import { SegmentedControl } from '../components/SegmentedControl'
import { ExpressionInput } from '../expression/ExpressionInput'
import { MESSAGES } from '../messages'
import { COLOR_CLASSES } from '../theme/roles'
import type { DimensionEditorModel, SketchController } from './session'

// The dimension editor (ui.md 3.16.7): the value of a new or stored dimension as an expression,
// whether it only measures, and for circles and arcs whether it is a diameter or a radius.

export type DimensionEditorProps = {
  /** The positioned container, which the sketch controller moves onto the label. */
  readonly ref?: Ref<HTMLFieldSetElement>
  readonly controller: SketchController
  readonly editor: DimensionEditorModel
}

const ROUND_OPTIONS = [
  { value: 'diameter', label: 'Diameter' },
  { value: 'radius', label: 'Radius' },
] as const

function report(error: unknown): void {
  console.error('Committing the dimension failed.', error)
}

export function DimensionEditor({ ref, controller, editor }: DimensionEditorProps) {
  const inputId = useId()
  const focused = useRef<number | null>(null)

  // Each opened editor takes focus with its value selected, so typing replaces it (3.16.7).
  useEffect(() => {
    if (focused.current === editor.key) {
      return
    }
    focused.current = editor.key
    const input = document.getElementById(inputId)
    if (input instanceof HTMLInputElement) {
      input.focus()
      input.select()
    }
  }, [editor.key, inputId])

  return (
    <fieldset
      ref={ref}
      data-testid="dimension-editor"
      aria-busy={editor.busy}
      className={clsx(
        'pointer-events-auto absolute top-0 left-0 flex w-64 flex-col gap-2 rounded-md border p-2 shadow-lg',
        COLOR_CLASSES.surfaceRaised.background,
        COLOR_CLASSES.border.border,
        COLOR_CLASSES.text.foreground,
      )}
    >
      <legend className="sr-only">
        {editor.mode === 'new' ? 'New dimension' : 'Change dimension'}
      </legend>
      <ExpressionInput
        key={editor.key}
        id={inputId}
        label="Dimension value"
        hideLabel
        unit={editor.unit}
        value={editor.text}
        selectOnFocus
        error={editor.message}
        onChange={(value) => {
          controller.setEditorText(value)
        }}
        onCommit={() => {
          controller.commitEditor().catch(report)
        }}
        onCancel={() => {
          controller.closeEditor()
        }}
      />
      <div className="flex items-center gap-2">
        {editor.round === null ? null : (
          <SegmentedControl
            label="Dimension kind"
            options={ROUND_OPTIONS}
            value={editor.round}
            onChange={(round) => {
              controller.setEditorRound(round)
            }}
          />
        )}
        <Button
          aria-pressed={editor.reference}
          onClick={() => {
            controller.setEditorReference(!editor.reference)
          }}
          className={clsx(editor.reference && COLOR_CLASSES.selection.background)}
        >
          Reference
        </Button>
      </div>
      {editor.problem === 'over_constrained' ? (
        <div role="alert" className="flex flex-col gap-2 text-sm">
          <p>{MESSAGES.dimension_over_constrained()}</p>
          <div className="flex justify-end gap-2">
            <Button
              variant="primary"
              onClick={() => {
                controller.commitEditor({ reference: true }).catch(report)
              }}
            >
              Add reference
            </Button>
            <Button
              onClick={() => {
                controller.closeEditor()
              }}
            >
              Cancel
            </Button>
          </div>
        </div>
      ) : null}
      {editor.problem === 'unsolvable' ? (
        <p role="alert" className={clsx('text-sm', COLOR_CLASSES.errorText.foreground)}>
          {MESSAGES.dimension_unsolvable()}
        </p>
      ) : null}
    </fieldset>
  )
}
