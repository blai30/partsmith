import { clsx } from 'clsx/lite'
import { ChevronDown, ChevronRight } from 'lucide-react'
import { useId, useRef, useState, type FocusEvent } from 'react'
import { editFeature } from '../app/actions/command'
import { blurRepair, focusRepair, runFixAction } from '../app/actions/fixes'
import { select } from '../app/actions/selection'
import { useApp, useAppState } from '../app/AppProvider'
import type { App } from '../app/createApp'
import { reportFailure } from '../app/report'
import { Button } from '../components/Button'
import { useOverflowEdges } from '../components/overflow-edges'
import type { Severity } from '../generated/protocol'
import { featureSelectionItem } from '../inspector/selection-values'
import { PanelHeader } from '../layout/PanelHeader'
import { SEVERITY_NAMES } from '../print/health'
import { SeverityIcon } from '../print/SeverityIcon'
import { MessageActions } from '../repair/MessageActions'
import { ShowLostButton } from '../repair/ShowLostButton'
import { selectFeatureLabel, selectProblems } from '../state/selectors'
import { COLOR_CLASSES } from '../theme/roles'
import {
  problemSeverity,
  rowSeverity,
  type NestedProblem,
  type ProblemNote,
  type ProblemRow,
} from './problems'

// The Problems panel (ui.md 3.9): one row per root cause in timeline order, the features that
// failed because of it nested under it, then the document's diagnostics, each with the core's
// fixes; info notices are listed under a collapsed "Notes" heading, never as problems.

function StatusIcon({ severity }: { readonly severity: Severity }) {
  return <SeverityIcon severity={severity} label={SEVERITY_NAMES[severity]} className="mt-0.5" />
}

/** A feature's label as a button that selects it (3.9). */
function FeatureLabel({ featureId }: { readonly featureId: string }) {
  const app = useApp()
  const label = useAppState((state) => selectFeatureLabel(state, featureId))
  return (
    <button
      type="button"
      onClick={() => {
        select(app, [featureSelectionItem(featureId, label)], 'replace')
      }}
      className={clsx(
        'min-w-0 truncate self-start rounded-xs text-left text-sm font-medium hover:underline',
        COLOR_CLASSES.focusRing.outline,
      )}
    >
      {label}
    </button>
  )
}

/** A nested row: its label, its messages without their actions, and "Show '{root}'" (3.9). */
function NestedRow({ problem }: { readonly problem: NestedProblem }) {
  const app = useApp()
  const rootLabel = useAppState((state) => selectFeatureLabel(state, problem.root))
  const showRoot = `Show '${rootLabel}'`
  return (
    <li
      data-testid="problem-row"
      data-feature-id={problem.feature}
      data-nested="true"
      className="flex gap-2"
    >
      <StatusIcon severity={problemSeverity(problem.messages)} />
      <div className="flex min-w-0 flex-1 flex-col gap-1">
        <FeatureLabel featureId={problem.feature} />
        {problem.messages.map((message, index) => (
          // Messages carry no id; their order is the core's.
          // oxlint-disable-next-line react/no-array-index-key
          <p key={index} className="text-sm">
            {message.message}
          </p>
        ))}
        <Button
          className="self-start"
          onClick={() => {
            runFixAction(
              app,
              { label: showRoot, run: { kind: 'show', feature: problem.root } },
              'panel',
            ).catch(reportFailure('Showing the root cause'))
          }}
        >
          {showRoot}
        </Button>
      </div>
    </li>
  )
}

/**
 * Shows repair mode for the feature of a row while focus is inside it and no feature command is
 * active, and ends it when focus leaves the row (3.7, 3.9).
 */
function rowFocusHandlers(app: App, featureId: string) {
  return {
    onFocus: () => {
      focusRepair(app, featureId)
    },
    onBlur: (event: FocusEvent<HTMLElement>) => {
      const next = event.relatedTarget
      if (next instanceof Node && event.currentTarget.contains(next)) {
        return
      }
      blurRepair(app, featureId)
    },
  }
}

/** A top-level row of a feature: its problems with their fixes, then Show what it lost and Edit (3.9). */
function FeatureRow({ row, featureId }: { readonly row: ProblemRow; readonly featureId: string }) {
  const app = useApp()
  const severity = useAppState((state) => rowSeverity(row, state.document?.featureStatus ?? {}))
  const label = useAppState((state) => selectFeatureLabel(state, featureId))
  return (
    <li
      data-testid="problem-row"
      data-feature-id={featureId}
      className="flex flex-col gap-3"
      {...rowFocusHandlers(app, featureId)}
    >
      <div className="flex gap-2">
        <StatusIcon severity={severity} />
        <div className="flex min-w-0 flex-1 flex-col gap-2">
          <FeatureLabel featureId={featureId} />
          {row.messages.map((message, index) => (
            // Messages carry no id; their order is the core's.
            // oxlint-disable-next-line react/no-array-index-key
            <div key={index} className="flex flex-col gap-2 text-sm">
              <p>{message.message}</p>
              <MessageActions message={message} place="panel" />
            </div>
          ))}
          <div className="flex flex-wrap gap-2">
            <ShowLostButton featureId={featureId} label={label} />
            <Button
              onClick={() => {
                editFeature(app, featureId).catch(reportFailure('Editing the feature'))
              }}
            >
              Edit
            </Button>
          </div>
        </div>
      </div>
      {row.nested.length === 0 ? null : (
        <ul className="flex flex-col gap-3 pl-6">
          {row.nested.map((problem) => (
            <NestedRow key={problem.feature} problem={problem} />
          ))}
        </ul>
      )}
    </li>
  )
}

/** The info notices under a disclosure that starts closed (3.9). */
function Notes({ notes }: { readonly notes: readonly ProblemNote[] }) {
  const [open, setOpen] = useState(false)
  const listId = useId()
  return (
    <div className="flex flex-col gap-3">
      <button
        type="button"
        data-testid="problems-notes"
        aria-expanded={open}
        aria-controls={listId}
        onClick={() => {
          setOpen(!open)
        }}
        className={clsx(
          'inline-flex h-8 items-center gap-1 self-start rounded-md px-1 text-sm font-medium',
          COLOR_CLASSES.hover.background,
          COLOR_CLASSES.focusRing.outline,
        )}
      >
        {open ? (
          <ChevronDown aria-hidden="true" strokeWidth={1.75} className="size-4 shrink-0" />
        ) : (
          <ChevronRight aria-hidden="true" strokeWidth={1.75} className="size-4 shrink-0" />
        )}
        Notes
        <span className={clsx('ml-1 tabular-nums', COLOR_CLASSES.textMuted.foreground)}>
          {notes.length}
        </span>
      </button>
      {open ? (
        <ul id={listId} className="flex flex-col gap-3">
          {notes.map((note, index) => (
            // Notes carry no id; their order is the timeline's and the core's.
            // oxlint-disable-next-line react/no-array-index-key
            <li key={index} className="flex gap-2">
              <StatusIcon severity="info" />
              <div className="flex min-w-0 flex-1 flex-col gap-2 text-sm">
                <FeatureLabel featureId={note.feature} />
                <p>{note.message.message}</p>
                <MessageActions message={note.message} place="panel" />
              </div>
            </li>
          ))}
        </ul>
      ) : null}
    </div>
  )
}

export function ProblemsPanel() {
  const titleId = useId()
  const body = useRef<HTMLDivElement>(null)
  useOverflowEdges(body)
  const { rows, notes } = useAppState(selectProblems)
  return (
    <section
      data-testid="problems-panel"
      aria-labelledby={titleId}
      className="flex min-h-0 flex-1 flex-col"
    >
      <PanelHeader title="Problems" titleId={titleId} />
      {/* Its rule shows while problems are scrolled under the header (3.2). */}
      <div
        ref={body}
        className={clsx(
          'flex min-h-0 flex-1 flex-col gap-4 overflow-y-auto border-t p-4',
          COLOR_CLASSES.forcedEdge.border,
          COLOR_CLASSES.divider.overflowTop,
        )}
      >
        {rows.length === 0 ? (
          <p className={clsx('text-sm', COLOR_CLASSES.textMuted.foreground)}>No problems.</p>
        ) : (
          <ul className="flex flex-col gap-4">
            {rows.map((row, index) =>
              row.feature === null ? (
                // Diagnostics carry no id; their order is the core's.
                // oxlint-disable-next-line react/no-array-index-key
                <li key={`diagnostic-${String(index)}`} className="flex gap-2">
                  {row.messages.map((message, messageIndex) => (
                    // oxlint-disable-next-line react/no-array-index-key
                    <div key={messageIndex} className="flex min-w-0 flex-1 gap-2">
                      <StatusIcon severity={message.severity} />
                      <div className="flex min-w-0 flex-1 flex-col gap-2 text-sm">
                        <p>{message.message}</p>
                        <MessageActions message={message} place="panel" />
                      </div>
                    </div>
                  ))}
                </li>
              ) : (
                <FeatureRow key={row.feature} row={row} featureId={row.feature} />
              ),
            )}
          </ul>
        )}
        {notes.length === 0 ? null : <Notes notes={notes} />}
      </div>
    </section>
  )
}
