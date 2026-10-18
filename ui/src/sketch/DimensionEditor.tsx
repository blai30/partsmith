import { clsx } from 'clsx/lite'
import { useEffect, useId, useRef } from 'react'
import { reportFailure } from '../app/report'
import { Button } from '../components/Button'
import { SegmentedControl } from '../components/SegmentedControl'
import { ExpressionInput } from '../expression/ExpressionInput'
import { MESSAGES } from '../messages'
import { COLOR_CLASSES } from '../theme/roles'
import { ReferenceOffer } from './ReferenceOffer'
import type { DimensionEditorModel, SketchController } from './session'

// The dimension editor (ui.md 3.16.7): the value of a new or stored dimension as an expression,
// whether it only measures, and for circles and arcs whether it is a diameter or a radius.

export type DimensionEditorProps = {
  readonly controller: SketchController
  readonly editor: DimensionEditorModel
}

const ROUND_OPTIONS = [
  { value: 'diameter', label: 'Diameter' },
  { value: 'radius', label: 'Radius' },
] as const

/** The editor, centered on its position: the overlay moves a wrapper onto the label (3.12). */
export function DimensionEditor({ controller, editor }: DimensionEditorProps) {
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
      data-testid="dimension-editor"
      aria-busy={editor.busy}
      className={clsx(
        'pointer-events-auto flex w-64 -translate-x-1/2 -translate-y-1/2 flex-col gap-2 rounded-lg border p-2 shadow-lg',
        COLOR_CLASSES.surface.background,
        COLOR_CLASSES.forcedEdge.border,
        COLOR_CLASSES.elevation.shadow,
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
        completion={false}
        unit={editor.unit}
        value={editor.text}
        selectOnFocus
        error={editor.message}
        onChange={(value) => {
          controller.setEditorText(value)
        }}
        onCommit={() => {
          controller.commitEditor().catch(reportFailure('Committing the dimension'))
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
        >
          Reference
        </Button>
      </div>
      {editor.problem === null ? null : (
        <ReferenceOffer
          onAddReference={() => {
            controller
              .commitEditor({ reference: true })
              .catch(reportFailure('Committing the dimension'))
          }}
          onCancel={() => {
            controller.closeEditor()
          }}
        >
          {editor.problem.kind === 'over_constrained' ? (
            <p>{MESSAGES.dimension_over_constrained()}</p>
          ) : (
            <p className={COLOR_CLASSES.errorText.foreground}>{editor.problem.message}</p>
          )}
        </ReferenceOffer>
      )}
    </fieldset>
  )
}
