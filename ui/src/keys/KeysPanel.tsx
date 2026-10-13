import { clsx } from 'clsx/lite'
import { useId, type ReactNode } from 'react'
import { useApp, useAppState } from '../app/AppProvider'
import type { App } from '../app/createApp'
import {
  COMPONENT_KEYS,
  TYPING_KEYS,
  TYPING_NOTE,
  withPlatformKeys,
  type ComponentKey,
} from '../commands/component-keys'
import { runCommand } from '../commands/registry'
import { formatShortcut } from '../commands/shortcuts'
import { SegmentedControl } from '../components/SegmentedControl'
import { PanelHeader } from '../layout/PanelHeader'
import { COLOR_CLASSES } from '../theme/roles'
import { NAVIGATION_PRESETS, type NavigationPreset } from '../viewport/navigation'

// The keyboard and mouse reference (ui.md 3.5): the side panel F1 opens. Everything in it is
// rendered from the data the behavior uses (the navigation presets, the component keys), so it
// cannot drift from what the app does; every command and shortcut is in the palette.

const PRESET_OPTIONS = NAVIGATION_PRESETS.map((preset) => ({
  value: preset.id,
  label: preset.label,
}))

/** Two columns of rows: what to press or do, and what it does, with where it works under it. */
function KeyRows({
  rows,
}: {
  readonly rows: readonly {
    readonly key: string
    readonly effect: string
    readonly place?: string
  }[]
}) {
  return (
    <dl className="grid grid-cols-2 gap-x-3 gap-y-2 text-sm">
      {rows.map((row, index) => (
        // Rows repeat keys (Tab, digits) in other places, so their position is their identity.
        // oxlint-disable-next-line react/no-array-index-key
        <div key={index} data-testid="keys-row" className="col-span-2 grid grid-cols-subgrid">
          <dt className="font-medium">{row.key}</dt>
          <dd className="flex flex-col">
            <span>{row.effect}</span>
            {row.place === undefined ? null : (
              <span className={clsx('text-xs', COLOR_CLASSES.textMuted.foreground)}>
                {row.place}
              </span>
            )}
          </dd>
        </div>
      ))}
    </dl>
  )
}

function Section({ title, children }: { readonly title: string; readonly children: ReactNode }) {
  const headingId = useId()
  return (
    <section
      aria-labelledby={headingId}
      className={clsx('flex flex-col gap-3 border-t pt-4', COLOR_CLASSES.border.border)}
    >
      <h3 id={headingId} className="text-sm font-semibold">
        {title}
      </h3>
      {children}
    </section>
  )
}

/** Rows with `Mod` written as the platform's key. */
function platformRows(rows: readonly ComponentKey[], app: App) {
  return rows.map((row) => ({
    key: withPlatformKeys(row.key, app.platform),
    effect: withPlatformKeys(row.effect, app.platform),
    place: row.place,
  }))
}

export function KeysPanel() {
  const app = useApp()
  const titleId = useId()
  const navigation = useAppState((state) => state.settings.navigation)
  const preset = NAVIGATION_PRESETS.find((entry) => entry.id === navigation)
  const mouseRows = (preset?.rows ?? []).map((row) => ({ key: row.action, effect: row.gesture }))
  return (
    <section
      data-testid="keys-panel"
      aria-labelledby={titleId}
      className="flex min-h-0 flex-1 flex-col"
    >
      <PanelHeader title="Keyboard and mouse" titleId={titleId} />
      <div className="flex min-h-0 flex-1 flex-col gap-4 overflow-y-auto px-4 pb-4">
        <Section title="Mouse">
          <SegmentedControl<NavigationPreset>
            label="Navigation"
            options={PRESET_OPTIONS}
            value={navigation}
            onChange={(value) => {
              runCommand(app, `settings.navigation.${value}`, 'menu')
            }}
            className="self-start"
          />
          <KeyRows rows={mouseRows} />
        </Section>
        <Section title="Keys in the view and panels">
          <KeyRows rows={platformRows(COMPONENT_KEYS, app)} />
        </Section>
        <Section title="Typing values">
          <KeyRows rows={platformRows(TYPING_KEYS, app)} />
          <p className={clsx('text-sm', COLOR_CLASSES.textMuted.foreground)}>{TYPING_NOTE}</p>
        </Section>
        <p className={clsx('text-sm', COLOR_CLASSES.textMuted.foreground)}>
          {`Every command and its shortcut: Search (${formatShortcut('Mod+K', app.platform)}).`}
        </p>
      </div>
    </section>
  )
}
