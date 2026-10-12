import { clsx } from 'clsx/lite'
import { useAppState } from '../app/AppProvider'
import { MESSAGES } from '../messages'
import type { AppState } from '../state/store'
import { COLOR_CLASSES } from '../theme/roles'

// The print hint (ui.md 3.17): while Lay on face waits for a face, a chip in the viewport's
// top-center slot (3.2) says what to click, or why the last face was refused.

function selectPicking(state: AppState): boolean {
  return state.context === 'print' && state.print.pickingFace
}

function selectRefusal(state: AppState): string | null {
  return state.pickHint !== null && state.pickHint.field === null ? state.pickHint.message : null
}

/** The print hint chip while Lay on face picks a face; nothing otherwise. */
export function PrintHint() {
  const picking = useAppState(selectPicking)
  const refusal = useAppState(selectRefusal)
  if (!picking) {
    return null
  }
  return (
    <div className="pointer-events-none absolute inset-x-32 top-4 z-10 flex flex-col items-center gap-2">
      <output
        data-testid="print-hint"
        className={clsx(
          'max-w-full rounded-md border px-3 py-1 text-center text-xs text-pretty shadow-sm',
          COLOR_CLASSES.surfaceRaised.background,
          COLOR_CLASSES.border.border,
          refusal === null ? COLOR_CLASSES.textMuted.foreground : COLOR_CLASSES.text.foreground,
        )}
      >
        {refusal ?? MESSAGES.print_pick_face()}
      </output>
    </div>
  )
}
