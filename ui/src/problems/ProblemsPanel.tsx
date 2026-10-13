import { clsx } from 'clsx/lite'
import { useId } from 'react'
import { editFeature } from '../app/actions/command'
import { select } from '../app/actions/selection'
import { useApp, useAppState } from '../app/AppProvider'
import { reportFailure } from '../app/report'
import { Button } from '../components/Button'
import type { Severity } from '../generated/protocol'
import { featureSelectionItem } from '../inspector/selection-values'
import { UseSuggestion } from '../inspector/UseSuggestion'
import { PanelHeader } from '../layout/PanelHeader'
import { SEVERITY_NAMES } from '../print/health'
import { SeverityIcon } from '../print/SeverityIcon'
import { selectEditLabel, selectProblems } from '../state/selectors'
import { COLOR_CLASSES } from '../theme/roles'

// The Problems panel (ui.md 3.9): features with warnings or errors in timeline order, then the
// document's diagnostics, each with the way to fix it.

function StatusIcon({ severity }: { readonly severity: Severity }) {
  return <SeverityIcon severity={severity} label={SEVERITY_NAMES[severity]} className="mt-0.5" />
}

/** "Edit", or "Re-pick {field}" for a feature with a lost reference (4.2). */
function EditLabel({ featureId }: { readonly featureId: string }) {
  return useAppState((state) => selectEditLabel(state, featureId))
}

export function ProblemsPanel() {
  const app = useApp()
  const titleId = useId()
  const problems = useAppState(selectProblems)
  const records = useAppState((state) => state.document?.recipe.features)
  return (
    <section
      data-testid="problems-panel"
      aria-labelledby={titleId}
      className="flex min-h-0 flex-1 flex-col"
    >
      <PanelHeader title="Problems" titleId={titleId} />
      <div className="min-h-0 flex-1 overflow-y-auto px-4 pb-4">
        {problems.length === 0 ? (
          <p className={clsx('text-sm', COLOR_CLASSES.textMuted.foreground)}>No problems.</p>
        ) : (
          <ul className="flex flex-col gap-4">
            {problems.map((problem, index) =>
              problem.kind === 'feature' ? (
                <li key={problem.featureId} className="flex gap-2">
                  <StatusIcon severity={problem.status} />
                  <div className="flex min-w-0 flex-1 flex-col gap-1">
                    <div className="flex items-center justify-between gap-2">
                      <button
                        type="button"
                        onClick={() => {
                          select(
                            app,
                            [featureSelectionItem(problem.featureId, problem.label)],
                            'replace',
                          )
                        }}
                        className={clsx(
                          'min-w-0 truncate rounded-xs text-left text-sm font-medium hover:underline',
                          COLOR_CLASSES.focusRing.outline,
                        )}
                      >
                        {problem.label}
                      </button>
                      <Button
                        onClick={() => {
                          editFeature(app, problem.featureId).catch(
                            reportFailure('Editing the feature'),
                          )
                        }}
                      >
                        <EditLabel featureId={problem.featureId} />
                      </Button>
                    </div>
                    {problem.messages.map((message, messageIndex) => {
                      const record = records?.find((entry) => entry.id === problem.featureId)
                      return (
                        // Messages carry no id; their order is the core's.
                        // oxlint-disable-next-line react/no-array-index-key
                        <div key={messageIndex} className="flex flex-col gap-1">
                          <p className="text-sm">{message.message}</p>
                          {record === undefined ? null : (
                            <UseSuggestion record={record} message={message} />
                          )}
                        </div>
                      )
                    })}
                  </div>
                </li>
              ) : (
                // Diagnostics carry no id; their order is the core's.
                // oxlint-disable-next-line react/no-array-index-key
                <li key={`diagnostic-${String(index)}`} className="flex gap-2">
                  <StatusIcon severity={problem.diagnostic.severity} />
                  <p className="min-w-0 flex-1 text-sm">{problem.diagnostic.message}</p>
                </li>
              ),
            )}
          </ul>
        )}
      </div>
    </section>
  )
}
