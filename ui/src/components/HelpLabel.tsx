import type { ReactElement } from 'react'
import { Tooltip } from './Tooltip'

export type HelpLabelProps = {
  /** The field's help (FIELD_HELP), or null for a label without help. */
  readonly help: string | null
  /** The id of the visually hidden help, which the field's control lists in `aria-describedby`. */
  readonly helpId: string
  /** The label element; give it `cursor-help` while there is help, the only visible cue. */
  readonly children: ReactElement<{ readonly 'aria-describedby'?: string | undefined }>
}

/**
 * A field label with hover help (ui.md 3.7): resting the pointer on the label shows the help in a
 * tooltip, and a visually hidden copy describes the control for assistive technology, so the
 * layout gains nothing. Without help, the label is shown as it is.
 */
export function HelpLabel({ help, helpId, children }: HelpLabelProps) {
  if (help === null) {
    return children
  }
  return (
    <>
      <Tooltip label={help}>{children}</Tooltip>
      <span id={helpId} className="sr-only">
        {help}
      </span>
    </>
  )
}
