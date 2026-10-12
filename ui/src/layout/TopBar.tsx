import { clsx } from 'clsx/lite'
import { Download, Moon, PanelLeft, Search } from 'lucide-react'
import { useRef, useState } from 'react'
import { setContext } from '../app/actions/context'
import { updateSettings } from '../app/actions/settings'
import { useApp, useAppState } from '../app/AppProvider'
import { formatShortcut } from '../commands/shortcuts'
import { IconButton } from '../components/IconButton'
import { SegmentedControl, type SegmentedOption } from '../components/SegmentedControl'
import { Tooltip } from '../components/Tooltip'
import { ExportPopover } from '../export/ExportPopover'
import type { AppContext, AppState } from '../state/store'
import { COLOR_CLASSES } from '../theme/roles'
import { toggledTheme } from '../theme/theme'
import { useResolvedTheme } from './use-resolved-theme'
import { ContextToolbar } from './ContextToolbar'
import { DocumentMenu } from './DocumentMenu'
import { BROWSER_PANEL_ID } from './panel-ids'
import { useUiRequest } from './requests'
import { runCommand, useCommandViews } from './use-commands'

// The top bar (ui.md 3.2): browser toggle, document name and save state, context switcher,
// context toolbar, palette button, Export button and theme toggle.

const CONTEXTS: readonly SegmentedOption<AppContext>[] = [
  { value: 'model', label: 'Model' },
  { value: 'sketch', label: 'Sketch' },
  { value: 'print', label: 'Print' },
]

function report(action: string): (error: unknown) => void {
  return (error) => {
    console.error(`${action} failed.`, error)
  }
}

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

/** Whether the document has changes that are not in its file: the core's flag or a recovered copy. */
function selectEdited(state: AppState): boolean {
  const document = state.document
  return document !== null && (document.dirty || document.localUnsaved)
}

function DocumentTitle() {
  const name = useAppState((state) => state.document?.name ?? 'Untitled')
  const edited = useAppState(selectEdited)
  const anchor = useRef<HTMLButtonElement>(null)
  const [open, setOpen] = useState(false)
  return (
    <div className="flex min-w-0 shrink items-center gap-2">
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
          'h-8 max-w-60 truncate rounded-sm px-2 text-sm font-semibold transition-colors duration-150 motion-reduce:transition-none',
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
  return (
    <SegmentedControl
      label="Context"
      options={CONTEXTS}
      value={context}
      onChange={(next) => {
        setContext(app, next).catch(report('Switching the context'))
      }}
    />
  )
}

function PaletteButton() {
  const app = useApp()
  const hint = formatShortcut('Mod+K', app.platform)
  return (
    <button
      type="button"
      data-testid="palette-button"
      aria-keyshortcuts={app.platform.operatingSystem === 'mac' ? 'Meta+K' : 'Control+K'}
      onClick={() => {
        app.store.setState((state) => ({ panels: { ...state.panels, paletteOpen: true } }))
      }}
      className={clsx(
        'inline-flex h-8 shrink-0 items-center gap-2 rounded-sm border px-2 text-sm transition-colors duration-150 motion-reduce:transition-none',
        COLOR_CLASSES.field.background,
        COLOR_CLASSES.border.border,
        COLOR_CLASSES.textMuted.foreground,
        COLOR_CLASSES.hover.background,
        COLOR_CLASSES.focusRing.outline,
        COLOR_CLASSES.accent.outline,
      )}
    >
      <Search aria-hidden="true" strokeWidth={1.75} className="size-4" />
      <span>Search</span>
      <span className="hidden text-xs @3xl:inline">{hint}</span>
    </button>
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
            'inline-flex h-8 shrink-0 items-center gap-2 rounded-sm border px-3 text-sm font-medium whitespace-nowrap transition-colors duration-150 motion-reduce:transition-none',
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
          Export
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
      className={clsx(
        '@container flex h-12 shrink-0 items-center gap-2 border-b px-2',
        COLOR_CLASSES.surface.background,
        COLOR_CLASSES.border.border,
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
