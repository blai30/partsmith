import { clsx } from 'clsx/lite'
import { Download, Moon, PanelLeft, Search } from 'lucide-react'
import { useRef, useState } from 'react'
import { setContext } from '../app/actions/context'
import { setPaletteOpen } from '../app/actions/panels'
import { updateSettings } from '../app/actions/settings'
import { useApp, useAppState } from '../app/AppProvider'
import { reportFailure } from '../app/report'
import { runCommand } from '../commands/registry'
import { ariaKeyShortcut, formatShortcut } from '../commands/shortcuts'
import { IconButton } from '../components/IconButton'
import { SegmentedControl, type SegmentedOption } from '../components/SegmentedControl'
import { Tooltip } from '../components/Tooltip'
import { ExportPopover } from '../export/ExportPopover'
import { selectSketchRadioTooltip, selectUnsaved } from '../state/selectors'
import type { AppContext } from '../state/store'
import { COLOR_CLASSES } from '../theme/roles'
import { toggledTheme } from '../theme/theme'
import { useResolvedTheme } from './use-resolved-theme'
import { ContextToolbar } from './ContextToolbar'
import { DocumentMenu } from './DocumentMenu'
import { BROWSER_PANEL_ID } from './panel-ids'
import { useUiRequest } from './requests'
import { useCommandViews } from './use-commands'

// The top bar (ui.md 3.2): browser toggle, document name and save state, context switcher,
// context toolbar, palette button, Export button and theme toggle.

function BrowserToggle() {
  const app = useApp()
  const open = useAppState((state) => state.panels.browserOpen)
  return (
    <Tooltip label="Toggle browser" shortcut={formatShortcut('Mod+B', app.platform)}>
      <IconButton
        label="Toggle browser"
        icon={PanelLeft}
        pressed={open}
        aria-controls={BROWSER_PANEL_ID}
        onClick={() => {
          runCommand(app, 'view.browser', 'toolbar')
        }}
      />
    </Tooltip>
  )
}

function DocumentTitle() {
  const name = useAppState((state) => state.document?.name ?? 'Untitled')
  const edited = useAppState(selectUnsaved)
  const anchor = useRef<HTMLButtonElement>(null)
  const [open, setOpen] = useState(false)
  // The name keeps at least 5rem and truncates inside it, so it never collapses under the context
  // switcher in a narrow top bar (3.2).
  return (
    <div className="flex min-w-20 shrink items-center gap-2">
      <button
        ref={anchor}
        type="button"
        data-testid="document-name"
        data-dirty={edited ? 'true' : 'false'}
        aria-haspopup="menu"
        aria-expanded={open}
        onClick={() => {
          setOpen((current) => !current)
        }}
        className={clsx(
          'h-8 max-w-60 min-w-0 truncate rounded-md px-2 text-sm font-semibold transition-colors duration-150 motion-reduce:transition-none',
          COLOR_CLASSES.hover.background,
          COLOR_CLASSES.focusRing.outline,
          COLOR_CLASSES.accent.outline,
        )}
      >
        {name}
      </button>
      <span
        data-testid="save-state"
        className={clsx('shrink-0 text-xs', COLOR_CLASSES.textMuted.foreground)}
      >
        {edited ? 'Edited' : ''}
      </span>
      <DocumentMenu open={open} anchor={anchor} onOpenChange={setOpen} />
    </div>
  )
}

function ContextSwitcher() {
  const app = useApp()
  const context = useAppState((state) => state.context)
  // The Sketch radio edits the selected sketch or starts a new one, and says which (3.3).
  const sketchTooltip = useAppState(selectSketchRadioTooltip)
  const options: readonly SegmentedOption<AppContext>[] = [
    { value: 'model', label: 'Model' },
    { value: 'sketch', label: 'Sketch', tooltip: sketchTooltip },
    { value: 'print', label: 'Print' },
  ]
  return (
    <SegmentedControl
      label="Context"
      options={options}
      value={context}
      onChange={(next) => {
        setContext(app, next).catch(reportFailure('Switching the context'))
      }}
    />
  )
}

function PaletteButton() {
  const app = useApp()
  const hint = formatShortcut('Mod+K', app.platform)
  // In a top bar under 48rem the label is visually hidden, keeping the accessible name (3.2).
  return (
    <Tooltip label="Search" shortcut={hint}>
      <button
        type="button"
        data-testid="palette-button"
        aria-keyshortcuts={ariaKeyShortcut('Mod+K', app.platform)}
        onClick={() => {
          setPaletteOpen(app, true)
        }}
        className={clsx(
          'inline-flex h-8 shrink-0 items-center gap-2 rounded-md border px-2 text-sm transition-colors duration-150 motion-reduce:transition-none',
          COLOR_CLASSES.field.background,
          COLOR_CLASSES.border.border,
          COLOR_CLASSES.textMuted.foreground,
          COLOR_CLASSES.hover.background,
          COLOR_CLASSES.focusRing.outline,
          COLOR_CLASSES.accent.outline,
        )}
      >
        <Search aria-hidden="true" strokeWidth={1.75} className="size-4" />
        <span className="@max-3xl:sr-only">Search</span>
        <span className="hidden text-xs @3xl:inline">{hint}</span>
      </button>
    </Tooltip>
  )
}

const EXPORT_COMMAND = ['file.export']

function ExportButton() {
  const app = useApp()
  const [view] = useCommandViews(app, EXPORT_COMMAND)
  const anchor = useRef<HTMLButtonElement>(null)
  const [open, setOpen] = useState(false)
  const reason = view?.availability.state === 'disabled' ? view.availability.reason : null
  useUiRequest(app, 'export', () => {
    setOpen(true)
    return true
  })
  return (
    <>
      <Tooltip label={reason ?? 'Export'} shortcut={view?.shortcut ?? null}>
        <button
          ref={anchor}
          type="button"
          aria-disabled={reason === null ? undefined : true}
          aria-haspopup="dialog"
          aria-expanded={open}
          onClick={() => {
            if (reason === null) {
              setOpen((current) => !current)
            } else {
              // A disabled command explains itself (2.10).
              runCommand(app, 'file.export', 'toolbar')
            }
          }}
          className={clsx(
            'inline-flex h-8 shrink-0 items-center gap-2 rounded-md border px-3 text-sm font-medium whitespace-nowrap transition-colors duration-150 @max-3xl:px-2 motion-reduce:transition-none',
            COLOR_CLASSES.border.border,
            COLOR_CLASSES.focusRing.outline,
            COLOR_CLASSES.accent.outline,
            reason === null && COLOR_CLASSES.text.foreground,
            reason === null && COLOR_CLASSES.hover.background,
            reason !== null && COLOR_CLASSES.textDisabled.foreground,
            reason !== null && 'cursor-not-allowed',
          )}
        >
          <Download aria-hidden="true" strokeWidth={1.75} className="size-4" />
          <span className="@max-3xl:sr-only">Export</span>
        </button>
      </Tooltip>
      {open && reason === null ? (
        <ExportPopover anchor={anchor} onClose={() => setOpen(false)} />
      ) : null}
    </>
  )
}

function ThemeToggle() {
  const app = useApp()
  const theme = useResolvedTheme()
  return (
    <Tooltip label="Dark theme">
      <IconButton
        data-testid="theme-toggle"
        label="Dark theme"
        icon={Moon}
        pressed={theme === 'dark'}
        onClick={() => {
          // The toggle stores an explicit choice, the opposite of what is shown (2.16.8).
          updateSettings(app, { theme: toggledTheme(theme) })
        }}
      />
    </Tooltip>
  )
}

export function TopBar() {
  return (
    <header
      data-testid="top-bar"
      // A one-row island: the menus and tooltips of its controls keep clear of it (3.2).
      data-bar=""
      className={clsx(
        '@container relative z-10 col-span-3 row-start-1 mb-2 flex h-12 min-w-0 items-center gap-2 rounded-xl border px-2 shadow-md',
        COLOR_CLASSES.surface.background,
        COLOR_CLASSES.border.border,
        COLOR_CLASSES.elevation.shadow,
      )}
    >
      <BrowserToggle />
      <DocumentTitle />
      <ContextSwitcher />
      <ContextToolbar />
      <PaletteButton />
      <ExportButton />
      <ThemeToggle />
    </header>
  )
}
