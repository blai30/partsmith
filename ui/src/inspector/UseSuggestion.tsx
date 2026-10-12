import { commitEdits } from '../app/actions/edits'
import { useApp } from '../app/AppProvider'
import { Button } from '../components/Button'
import type { Diagnostic, FeatureRecord } from '../generated/protocol'
import { suggestedRecord } from './status'

/**
 * "Use {suggestion}" for a feature message at rest (4.2): one `update_feature` with the value at the
 * message's field replaced, labeled "Change {label}". Nothing for a message without a suggestion.
 */
export function UseSuggestion({
  record,
  label,
  message,
}: {
  readonly record: FeatureRecord
  readonly label: string
  readonly message: Diagnostic
}) {
  const app = useApp()
  const updated = suggestedRecord(record, message)
  if (updated === null) {
    return null
  }
  return (
    <Button
      className="self-start"
      onClick={() => {
        commitEdits(app, `Change ${label}`, [{ type: 'update_feature', feature: updated }]).catch(
          (error: unknown) => {
            console.error('Applying the suggestion failed.', error)
          },
        )
      }}
    >
      Use {message.suggestion}
    </Button>
  )
}
