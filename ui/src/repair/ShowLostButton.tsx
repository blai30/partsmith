import { showLost } from '../app/actions/fixes'
import { useApp, useAppState } from '../app/AppProvider'
import { reportFailure } from '../app/report'
import { Button } from '../components/Button'
import { MESSAGES } from '../messages'

/** "Show what '{label}' lost", which opens repair mode (3.7); nothing while the feature has no repair. */
export function ShowLostButton({
  featureId,
  label,
}: {
  readonly featureId: string
  readonly label: string
}) {
  const app = useApp()
  const hasRepairs = useAppState(
    (state) => (state.document?.featureStatus[featureId]?.repairs ?? []).length > 0,
  )
  if (!hasRepairs) {
    return null
  }
  return (
    <Button
      onClick={() => {
        showLost(app, featureId).catch(reportFailure('Showing what was lost'))
      }}
    >
      {MESSAGES.show_lost(label)}
    </Button>
  )
}
