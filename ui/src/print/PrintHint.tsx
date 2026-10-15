import { clsx } from 'clsx/lite'
import { useAppState } from '../app/AppProvider'
import { MESSAGES } from '../messages'
import type { AppState } from '../state/store'
import { COLOR_CLASSES } from '../theme/roles'
import { TopCenterSlot } from '../viewport/TopCenterSlot'

// The print hint (ui.md 3.17): while Lay on face waits for a face, a chip in the free area's
// top-center slot (3.2) says what to click, or why the last face was refused.

function selectPicking(state: AppState): boolean {
  return state.context === 'print' && state.print.pickingFace
}

function selectRefusal(state: AppState): string | null {
  return state.pickHint !== null && state.pickHint.field === null ? state.pickHint.message : null
}

/**
 * The print hint chip while Lay on face picks a face. The live region stays mounted, empty and
 * visually hidden otherwise, so the prompt is announced when a pick starts (3.24).
 */
export function PrintHint() {
  const picking = useAppState(selectPicking)
  const refusal = useAppState(selectRefusal)
  return (
    <TopCenterSlot>
      <output
        data-testid="print-hint"
        className={clsx(
          !picking && 'sr-only',
          picking &&
            'max-w-full rounded-lg border px-3 py-1 text-center text-xs text-pretty shadow-md',
          picking && COLOR_CLASSES.surfaceRaised.background,
          picking && COLOR_CLASSES.border.border,
          picking && COLOR_CLASSES.elevation.shadow,
          picking && refusal === null && COLOR_CLASSES.textMuted.foreground,
          picking && refusal !== null && COLOR_CLASSES.text.foreground,
        )}
      >
        {picking ? (refusal ?? MESSAGES.print_pick_face()) : null}
      </output>
    </TopCenterSlot>
  )
}
