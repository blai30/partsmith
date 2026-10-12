import { applySuggestion } from '../app/actions/timeline'
import { useApp, useAppState } from '../app/AppProvider'
import { Button } from '../components/Button'
import { Tooltip } from '../components/Tooltip'
import type { Diagnostic, FeatureRecord } from '../generated/protocol'
import { selectCommandBlock } from '../state/selectors'
import { suggestedRecord } from './status'

/**
 * "Use {suggestion}" for a feature message at rest (4.2): one `update_feature` with the value at the
 * message's field replaced, labeled "Change {label}". Nothing for a message without a suggestion.
 * During a feature command it is a document edit that waits (3.7): `aria-disabled` with the reason.
 */
export function UseSuggestion({
  record,
  message,
}: {
  readonly record: FeatureRecord
  readonly message: Diagnostic
}) {
  const app = useApp()
  const blocked = useAppState(selectCommandBlock)
  if (suggestedRecord(record, message) === null) {
    return null
  }
  return (
    <Tooltip label={blocked}>
      <Button
        className="self-start"
        aria-disabled={blocked === null ? undefined : true}
        onClick={() => {
          applySuggestion(app, record.id, message).catch((error: unknown) => {
            console.error('Applying the suggestion failed.', error)
          })
        }}
      >
        Use {message.suggestion}
      </Button>
    </Tooltip>
  )
}
