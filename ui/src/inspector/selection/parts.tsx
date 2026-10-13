import { clsx } from 'clsx/lite'
import { Shapes, type LucideIcon } from 'lucide-react'
import { useEffect, useSyncExternalStore, type ReactNode } from 'react'
import { useApp, useAppState } from '../../app/AppProvider'
import { reportFailure } from '../../app/report'
import { runCommand } from '../../commands/registry'
import { ariaKeyShortcut, formatShortcut } from '../../commands/shortcuts'
import { Button } from '../../components/Button'
import type { SelectionItem } from '../../state/store'
import { COLOR_CLASSES, FONT_CLASSES } from '../../theme/roles'
import { measuredBody, measureSelection, selectionMeasurements, volumeMeter } from '../measurements'
import { suggestedCommands } from '../suggestions'

// The building blocks of the selection inspector's sections (ui.md 3.8): the title, label and value
// rows, the measure rows and the suggested commands.

/** The section's title: what the selection is. */
export function Title({ children }: { readonly children: ReactNode }) {
  return (
    <h2 data-testid="selection-title" className="text-base font-semibold tabular-nums">
      {children}
    </h2>
  )
}

/** A row of a label and its value. */
export function Row({ label, children }: { readonly label: string; readonly children: ReactNode }) {
  return (
    <div className="flex min-h-8 items-center justify-between gap-3 text-sm">
      <span className={COLOR_CLASSES.textMuted.foreground}>{label}</span>
      <span className="min-w-0 text-right">{children}</span>
    </div>
  )
}

/** A measured value in the value font. */
export function Value({
  testId,
  children,
}: {
  readonly testId?: string
  readonly children: ReactNode
}) {
  return (
    <span data-testid={testId} className={clsx('tabular-nums', FONT_CLASSES.values)}>
      {children}
    </span>
  )
}

/** A shortcut shown on a suggestion button: seen, but given to assistive technology through `aria-keyshortcuts`. */
function ShortcutHint({ text }: { readonly text: string }) {
  return (
    <span aria-hidden="true" className={clsx('text-xs', COLOR_CLASSES.textMuted.foreground)}>
      {text}
    </span>
  )
}

/** A button that runs a command with the selection preselected: its icon, title and shortcut (3.8). */
export function InspectorCommandButton({
  id,
  icon: Icon,
  title,
}: {
  readonly id: string
  readonly icon: LucideIcon
  readonly title: string
}) {
  const app = useApp()
  const [shortcut] = app.commands.get(id)?.shortcuts ?? []
  return (
    <Button
      aria-keyshortcuts={
        shortcut === undefined ? undefined : ariaKeyShortcut(shortcut, app.platform)
      }
      onClick={() => {
        runCommand(app, id, 'inspector')
      }}
    >
      <Icon aria-hidden="true" strokeWidth={1.75} className="size-4" />
      {title}
      {shortcut === undefined ? null : (
        <ShortcutHint text={formatShortcut(shortcut, app.platform)} />
      )}
    </Button>
  )
}

/** A command the selection suggests, run with the selection preselected (3.8). */
function SuggestedCommand({ id }: { readonly id: string }) {
  const app = useApp()
  const definition = app.commands.get(id)
  // A suggestion whose command is not available now is left out (2.10, 3.8).
  const available = useAppState(() => definition?.availability(app).state === 'enabled')
  if (definition === undefined || !available) {
    return null
  }
  return (
    <InspectorCommandButton id={id} icon={definition.icon ?? Shapes} title={definition.title} />
  )
}

/** The suggested commands of 3.8 for the selection, in order. */
export function Suggestions({ items }: { readonly items: readonly SelectionItem[] }) {
  const ids = suggestedCommands(useApp().store.getState(), items)
  if (ids.length === 0) {
    return null
  }
  return (
    <div className="flex flex-wrap gap-2">
      {ids.map((id) => (
        <SuggestedCommand key={id} id={id} />
      ))}
    </div>
  )
}

/**
 * The measure rows of the selection (3.8, `measure-row`): selecting measures, with no tool. A
 * body's volume comes from the core once per model mesh; its row is empty until then.
 */
export function MeasureRows({ items }: { readonly items: readonly SelectionItem[] }) {
  const app = useApp()
  const meter = volumeMeter(app)
  const meshId = useAppState((state) => measuredBody(state, items)?.meshId ?? null)
  // Re-renders when the measured volume arrives.
  useSyncExternalStore(meter.subscribe, () => (meshId === null ? null : meter.volume(meshId)))
  useEffect(() => {
    if (meshId !== null) {
      measureSelection(app, items).catch(reportFailure('Measuring the selection'))
    }
  }, [app, items, meshId])
  const rows = selectionMeasurements(app, items)
  return rows.map((row) => (
    <div
      key={row.measure}
      data-testid="measure-row"
      data-measure={row.measure}
      className="flex min-h-8 items-center justify-between gap-3 text-sm"
    >
      <span className={COLOR_CLASSES.textMuted.foreground}>{row.label}</span>
      <span className={clsx('min-w-0 text-right tabular-nums select-text', FONT_CLASSES.values)}>
        {row.value}
      </span>
    </div>
  ))
}
