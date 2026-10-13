import { useEffect, useState, type RefObject } from 'react'
import { openRecent } from '../app/actions/document'
import { useApp, useAppState } from '../app/AppProvider'
import type { App } from '../app/createApp'
import { reportFailure } from '../app/report'
import { runCommand } from '../commands/registry'
import { Menu, type MenuEntry } from '../components/Menu'
import type { RecentFile } from '../files/recent'
import type { Settings } from '../settings/settings'
import { commandMenuItem, useCommandViews, type CommandView } from './use-commands'

// The Document menu under the document name (ui.md 3.2): New, Open..., Open recent, Save,
// Save as..., Export..., then the Theme, Navigation and UI scale settings.

const FILE_COMMANDS = ['file.new', 'file.open', 'file.save', 'file.save_as', 'file.export']

const THEMES: readonly { readonly value: Settings['theme']; readonly label: string }[] = [
  { value: 'system', label: 'System' },
  { value: 'light', label: 'Light' },
  { value: 'dark', label: 'Dark' },
]

const NAVIGATIONS: readonly { readonly value: Settings['navigation']; readonly label: string }[] = [
  { value: 'fusion', label: 'Fusion' },
  { value: 'freecad', label: 'FreeCAD' },
]

const SCALES: readonly Settings['uiScale'][] = [1, 1.25, 1.5, 2, 3]

/** How the menu shows when a recent file was opened, for example "Oct 12, 2026, 4:05 PM". */
function openedText(openedAt: number): string {
  return new Date(openedAt).toLocaleString('en-US', { dateStyle: 'medium', timeStyle: 'short' })
}

/** The menu item of a command, labeled as the menu words it; nothing for a hidden command. */
function commandItem(app: App, view: CommandView | undefined, label: string): MenuEntry[] {
  return view === undefined || view.availability.state === 'hidden'
    ? []
    : [commandMenuItem(app, view, label)]
}

export type DocumentMenuProps = {
  readonly open: boolean
  readonly anchor: RefObject<HTMLElement | null>
  readonly onOpenChange: (open: boolean) => void
}

export function DocumentMenu({ open, anchor, onOpenChange }: DocumentMenuProps) {
  const app = useApp()
  const views = useCommandViews(app, FILE_COMMANDS)
  const settings = useAppState((state) => state.settings)
  const coreReady = useAppState((state) => state.core.status === 'ready')
  const [recent, setRecent] = useState<readonly RecentFile[]>([])

  // The recent files are read each time the menu opens, newest first (3.19).
  useEffect(() => {
    if (!open) {
      return undefined
    }
    let current = true
    app.recent.list().then((files) => {
      if (current) {
        setRecent(files)
      }
    }, reportFailure('Reading the recent files'))
    return () => {
      current = false
    }
  }, [app, open])

  const byId = new Map(views.map((view) => [view.definition.id, view]))
  const recentEntries: MenuEntry[] =
    recent.length === 0
      ? [{ kind: 'item', label: 'No recent files', disabled: true, onSelect: () => undefined }]
      : recent.map((file) => ({
          kind: 'item',
          label: file.name,
          shortcut: openedText(file.openedAt),
          disabled: !coreReady,
          onSelect: () => {
            openRecent(app, file).catch(reportFailure('Opening the recent file'))
          },
        }))
  const entries: MenuEntry[] = [
    ...commandItem(app, byId.get('file.new'), 'New'),
    ...commandItem(app, byId.get('file.open'), 'Open...'),
    { kind: 'submenu', label: 'Open recent', entries: recentEntries },
    ...commandItem(app, byId.get('file.save'), 'Save'),
    ...commandItem(app, byId.get('file.save_as'), 'Save as...'),
    ...commandItem(app, byId.get('file.export'), 'Export...'),
    { kind: 'separator' },
    {
      kind: 'submenu',
      label: 'Theme',
      entries: THEMES.map((theme) => ({
        kind: 'radio',
        label: theme.label,
        checked: settings.theme === theme.value,
        onSelect: () => {
          runCommand(app, `settings.theme.${theme.value}`, 'menu')
        },
      })),
    },
    {
      kind: 'submenu',
      label: 'Navigation',
      entries: NAVIGATIONS.map((navigation) => ({
        kind: 'radio',
        label: navigation.label,
        checked: settings.navigation === navigation.value,
        onSelect: () => {
          runCommand(app, `settings.navigation.${navigation.value}`, 'menu')
        },
      })),
    },
    {
      kind: 'submenu',
      label: 'UI scale',
      entries: SCALES.map((scale) => {
        const percent = String(Math.round(scale * 100))
        return {
          kind: 'radio',
          label: `${percent}%`,
          checked: settings.uiScale === scale,
          onSelect: () => {
            runCommand(app, `settings.scale.${percent}`, 'menu')
          },
        }
      }),
    },
  ]
  return (
    <Menu
      label="Document"
      testId="document-menu"
      open={open}
      anchor={anchor}
      entries={entries}
      onOpenChange={onOpenChange}
    />
  )
}
