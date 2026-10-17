import { clsx } from 'clsx/lite'
import { useApp, useAppState } from '../app/AppProvider'
import { Tooltip } from '../components/Tooltip'
import { capitalized } from '../messages'
import { selectFixBlock } from '../state/selectors'
import { COLOR_CLASSES } from '../theme/roles'
import { candidateHandlers } from './candidates'
import type { RepairCandidateView } from './repair'

// The candidates of the current repair in the inspector banner (ui.md 3.7) and, while a sketch is
// re-attached, the sketch header (3.16.1): numbered buttons "{n}. {Description}" with the relation
// on a second line, in the order received; never a score.

function CandidateButton({
  candidate,
  onRan,
}: {
  readonly candidate: RepairCandidateView
  readonly onRan?: (() => void) | undefined
}) {
  const app = useApp()
  const action = candidate.action
  const blocked = useAppState((state) =>
    action === null ? null : selectFixBlock(state, action, 'banner'),
  )
  const inactive = action === null || blocked !== null
  return (
    <Tooltip label={blocked}>
      <button
        type="button"
        data-testid="repair-candidate-button"
        aria-disabled={inactive ? true : undefined}
        {...candidateHandlers(app, candidate, onRan)}
        className={clsx(
          'flex flex-col items-start gap-0.5 rounded-md border px-3 py-1.5 text-left text-sm transition-colors duration-150 motion-reduce:transition-none',
          COLOR_CLASSES.focusRing.outline,
          COLOR_CLASSES.accent.outline,
          COLOR_CLASSES.forcedEdge.border,
          COLOR_CLASSES.tonal.banner,
          inactive ? COLOR_CLASSES.textDisabled.foreground : COLOR_CLASSES.hover.banner,
          inactive ? 'cursor-not-allowed' : COLOR_CLASSES.text.foreground,
        )}
      >
        <span className="font-medium">
          <span className="tabular-nums">{candidate.number}.</span>{' '}
          {capitalized(candidate.description)}
        </span>
        {candidate.relation === '' ? null : (
          <span className={COLOR_CLASSES.textMuted.foreground}>{candidate.relation}</span>
        )}
      </button>
    </Tooltip>
  )
}

/** The candidates of the current repair as buttons, in the order received (3.7). */
export function RepairCandidates({
  candidates,
  onRan,
}: {
  readonly candidates: readonly RepairCandidateView[]
  readonly onRan?: (() => void) | undefined
}) {
  return (
    <div className="flex flex-col items-start gap-2">
      {candidates.map((candidate) => (
        <CandidateButton key={candidate.number} candidate={candidate} onRan={onRan} />
      ))}
    </div>
  )
}
