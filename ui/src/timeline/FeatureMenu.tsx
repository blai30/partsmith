import type { RefObject } from 'react'
import { editFeature } from '../app/actions/command'
import { select } from '../app/actions/selection'
import { deleteFeatures, setRollback, setSuppressed } from '../app/actions/timeline'
import { useApp, useAppState } from '../app/AppProvider'
import type { App } from '../app/createApp'
import { reportFailure } from '../app/report'
import { Menu, type MenuEntry } from '../components/Menu'
import type { FeatureRecord } from '../generated/protocol'
import { featureSelectionItem } from '../inspector/selection-values'
import { selectFeatureLabel } from '../state/selectors'
import type { AppState } from '../state/store'
import { usedByText, usersOf, usesAreCurrent } from './uses'

// The context menu of a timeline item (ui.md 3.9).

type FeatureMenuOptions = {
  /** The features Suppress and Delete cover. */
  readonly targets: readonly string[]
  /** An open feature command blocks every timeline edit (3.7). */
  readonly blocked: boolean
  /** "Used by {labels}" (3.9), or null when nothing uses the feature or its uses are not known. */
  readonly usedBy: string | null
  readonly onRename: (id: string) => void
}

/**
 * The `used_by` text of the features that use `feature` (3.9), once the uses of the current
 * revision are known; null before, and when nothing uses it.
 */
function featureUsedBy(state: AppState, feature: string): string | null {
  const users = usesAreCurrent(state) ? usersOf(state, feature) : []
  return users.length === 0 ? null : usedByText(state, users)
}

/** Selects the features that use `feature`, which highlights what they made (3.6, 3.9). */
function selectUsers(app: App, feature: string): void {
  const state = app.store.getState()
  select(
    app,
    usersOf(state, feature).map((user) =>
      featureSelectionItem(user, selectFeatureLabel(state, user)),
    ),
    'replace',
  )
}

/** The entries of the menu of the timeline item of `record`, at `index` in the timeline. */
function featureMenuEntries(
  app: App,
  record: FeatureRecord,
  index: number,
  { targets, blocked, usedBy, onRename }: FeatureMenuOptions,
): MenuEntry[] {
  const suppressed = record.suppressed === true
  const usedByEntries: MenuEntry[] =
    usedBy === null
      ? []
      : [
          {
            kind: 'item',
            label: usedBy,
            onSelect: () => {
              selectUsers(app, record.id)
            },
          },
        ]
  return [
    {
      kind: 'item',
      label: 'Edit',
      disabled: record.type === 'import',
      onSelect: () => {
        editFeature(app, record.id).catch(reportFailure('Editing the feature'))
      },
    },
    {
      kind: 'item',
      label: 'Rename',
      disabled: blocked,
      onSelect: () => {
        onRename(record.id)
      },
    },
    {
      kind: 'item',
      label: suppressed ? 'Unsuppress' : 'Suppress',
      disabled: blocked,
      onSelect: () => {
        setSuppressed(app, targets, !suppressed).catch(reportFailure('Suppressing'))
      },
    },
    ...usedByEntries,
    {
      kind: 'item',
      label: 'Roll back to here',
      disabled: blocked,
      onSelect: () => {
        setRollback(app, index + 1).catch(reportFailure('Rolling back'))
      },
    },
    {
      kind: 'item',
      label: 'Delete',
      disabled: blocked,
      onSelect: () => {
        deleteFeatures(app, targets).catch(reportFailure('Deleting'))
      },
    },
  ]
}

type FeatureMenuProps = {
  /** The feature of the timeline item that opened the menu. */
  readonly feature: FeatureRecord
  /** The feature's index in the timeline. */
  readonly index: number
  /** The item's button: the menu opens under it and returns focus to it. */
  readonly anchor: RefObject<HTMLButtonElement | null>
  readonly targets: readonly string[]
  readonly blocked: boolean
  readonly onRename: (id: string) => void
  /** Called whenever the menu closes. */
  readonly onClose: () => void
}

/** The open context menu of a timeline item (3.9). */
export function FeatureMenu({
  feature,
  index,
  anchor,
  targets,
  blocked,
  onRename,
  onClose,
}: FeatureMenuProps) {
  const app = useApp()
  // The menu lists the features that use its feature, once their uses are known (3.9).
  const usedBy = useAppState((state) => featureUsedBy(state, feature.id))
  return (
    <Menu
      label="Feature"
      testId="context-menu"
      open
      anchor={anchor}
      entries={featureMenuEntries(app, feature, index, { targets, blocked, usedBy, onRename })}
      onOpenChange={(open) => {
        if (!open) {
          onClose()
        }
      }}
    />
  )
}
