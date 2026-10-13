import { Shapes } from 'lucide-react'
import { editFeature } from '../../app/actions/command'
import { setSuppressed } from '../../app/actions/timeline'
import { useApp } from '../../app/AppProvider'
import { reportFailure } from '../../app/report'
import { FEATURE_ICONS } from '../../commands/icons'
import { Button } from '../../components/Button'
import { Tooltip } from '../../components/Tooltip'
import { featureLabel } from '../../core/labels'
import { MESSAGES } from '../../messages'
import { selectEditLabel } from '../../state/selectors'
import type { AppState, DocumentState, SelectionItem } from '../../state/store'
import { COLOR_CLASSES } from '../../theme/roles'
import { UseSuggestion } from '../UseSuggestion'
import { Row, Suggestions, Title } from './parts'

// The selection inspector of one feature (ui.md 3.8): its type, its problems, Edit and Suppress.

/** The section of one selected feature or datum plane. */
export function FeatureSection({
  state,
  document,
  featureId,
  items,
}: {
  readonly state: AppState
  readonly document: DocumentState
  readonly featureId: string
  readonly items: readonly SelectionItem[]
}) {
  const app = useApp()
  const record = document.recipe.features?.find((feature) => feature.id === featureId)
  const label = featureLabel(document.recipe, featureId, state.catalog)
  if (record === undefined) {
    return <Title>{label}</Title>
  }
  const manifest = state.catalog?.byType[record.type]?.manifest
  const Icon = FEATURE_ICONS[manifest?.icon ?? ''] ?? Shapes
  const status = document.featureStatus[featureId]
  const messages = (status?.messages ?? []).filter(
    (message) => message.severity === 'error' || message.severity === 'warning',
  )
  const suppressed = record.suppressed === true
  const importFeature = record.type === 'import'
  return (
    <>
      <Title>{label}</Title>
      <Row label="Type">
        <span className="inline-flex items-center gap-2">
          <Icon aria-hidden="true" strokeWidth={1.75} className="size-4" />
          {manifest?.label ?? record.type}
        </span>
      </Row>
      {messages.length === 0 ? null : (
        <ul className="flex flex-col gap-2 text-sm">
          {messages.map((message, index) => (
            <li
              // Messages carry no id; their order is the core's.
              // oxlint-disable-next-line react/no-array-index-key
              key={index}
              className="flex flex-col gap-1"
            >
              <span
                className={
                  message.severity === 'error'
                    ? COLOR_CLASSES.errorText.foreground
                    : COLOR_CLASSES.warningText.foreground
                }
              >
                {message.message}
              </span>
              <UseSuggestion record={record} message={message} />
            </li>
          ))}
        </ul>
      )}
      <div className="flex flex-wrap gap-2">
        {importFeature ? (
          <Tooltip label={MESSAGES.import_not_editable()}>
            <Button aria-disabled="true">Edit</Button>
          </Tooltip>
        ) : (
          <Button
            onClick={() => {
              editFeature(app, featureId).catch(reportFailure('Editing the feature'))
            }}
          >
            {selectEditLabel(state, featureId)}
          </Button>
        )}
        <Button
          onClick={() => {
            setSuppressed(app, [featureId], !suppressed).catch(
              reportFailure('Suppressing the feature'),
            )
          }}
        >
          {suppressed ? 'Unsuppress' : 'Suppress'}
        </Button>
      </div>
      <Suggestions items={items} />
    </>
  )
}
