import { clsx } from 'clsx/lite'
import { useApp, useAppState } from '../app/AppProvider'
import { runCommand } from '../commands/registry'
import { formatShortcut } from '../commands/shortcuts'
import { Button } from '../components/Button'
import { useCommandViews } from '../layout/use-commands'
import { selectEmptyState } from '../state/selectors'
import { COLOR_CLASSES } from '../theme/roles'
import { navigationLegend } from './navigation'
import { TopCenterSlot } from './TopCenterSlot'

// The empty-document first step (ui.md 3.2): a card in the free area's top-center slot while the
// document has no features. It follows the document only, with no first-run flag and no setting,
// so it shows in every empty document and nowhere else.

const FIRST_STEPS: readonly string[] = ['feature.sketch', 'feature.box']

/** The first-step card while `selectEmptyState` holds; nothing otherwise. */
export function EmptyState() {
  const app = useApp()
  const shown = useAppState(selectEmptyState)
  const navigation = useAppState((state) => state.settings.navigation)
  // Box is left out when the catalog has none.
  const steps = useCommandViews(app, FIRST_STEPS)
  if (!shown) {
    return null
  }
  return (
    <TopCenterSlot>
      <fieldset
        data-testid="empty-state"
        aria-label="First step"
        className={clsx(
          'pointer-events-auto flex max-w-140 flex-col gap-2 rounded-xl border p-3 shadow-md',
          COLOR_CLASSES.surfaceRaised.background,
          COLOR_CLASSES.border.border,
          COLOR_CLASSES.elevation.shadow,
          COLOR_CLASSES.text.foreground,
        )}
      >
        <div className="flex flex-wrap items-center gap-2 text-sm">
          <p>Start with a sketch on a plane, or add a box.</p>
          {steps.map((step) => (
            <Button
              key={step.definition.id}
              onClick={() => {
                runCommand(app, step.definition.id, 'toolbar')
              }}
            >
              {step.definition.id === 'feature.sketch' ? 'Sketch' : 'Box'}
            </Button>
          ))}
        </div>
        <p className={clsx('text-xs', COLOR_CLASSES.textMuted.foreground)}>
          {`${navigationLegend(navigation)} Search every command: ${formatShortcut('Mod+K', app.platform)}.`}
        </p>
      </fieldset>
    </TopCenterSlot>
  )
}
