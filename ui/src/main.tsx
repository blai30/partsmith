import { clsx } from 'clsx/lite'
import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import { App } from './App'
import { startCore } from './app/actions/document'
import { AppProvider } from './app/AppProvider'
import { installCloseGuard } from './app/close-guard'
import { createApp, detectPlatform, installUnloadGuard } from './app/createApp'
import { reportFailure } from './app/report'
import { builtinCommands } from './commands/builtin'
import { installShortcutDispatcher } from './commands/dispatcher'
import type { TauriGlobal } from './core/tauri-transport'
import { chooseTransport } from './core/transport'
import { chooseFileService } from './files/choose-file-service'
import { tauriGlobalOf } from './files/desktop-file-service'
import { createIndexedDbRecentFiles } from './files/recent'
import { createIndexedDbRecovery } from './files/recovery'
import { systemPrefersDark } from './layout/color-scheme'
import { MESSAGES } from './messages'
import { createAppStore } from './state/store'
import { COLOR_CLASSES, ROOT_CLASSES } from './theme/roles'
import { applyTheme, applyUiScale, resolveTheme } from './theme/theme'
import './index.css'

// Startup (ui.md 3.1): the capability check, the theme on the root element, the app and its
// services, the shortcut dispatcher, the first render, then the core.

/** Why this browser cannot run Partsmith, or null when it can. */
function missingCapability(): string | null {
  if (typeof WebAssembly !== 'object') {
    return MESSAGES.unsupported_wasm()
  }
  // A canvas that can draw WebGL2 says the viewport will work; the canvas is thrown away.
  const context = document.createElement('canvas').getContext('webgl2')
  return context === null || context === undefined ? MESSAGES.unsupported_webgl2() : null
}

/** The Tauri global of the desktop app, if this page runs there. */
function tauriEnvironment(): { readonly __TAURI__?: TauriGlobal } {
  const tauri: unknown = Reflect.get(window, '__TAURI__')
  // chooseTransport checks the shape before it uses it.
  return tauri === undefined ? {} : { __TAURI__: tauri as TauriGlobal }
}

const root = document.getElementById('root')
if (root === null) {
  throw new Error('index.html must contain a #root element')
}

const store = createAppStore()
const { settings } = store.getState()
const html = document.documentElement
html.classList.add(...ROOT_CLASSES.split(' '))
applyTheme(html, resolveTheme(settings.theme, systemPrefersDark()))
applyUiScale(html, settings.uiScale)

const missing = missingCapability()
if (missing !== null) {
  createRoot(root).render(
    <StrictMode>
      <main
        data-testid="unsupported"
        className={clsx(
          'flex h-dvh w-dvw items-center justify-center p-8 antialiased',
          COLOR_CLASSES.surface.background,
          COLOR_CLASSES.text.foreground,
        )}
      >
        <p className="max-w-xl text-base">{missing}</p>
      </main>
    </StrictMode>,
  )
} else {
  const environment = tauriEnvironment()
  const platform = detectPlatform(navigator, environment)
  const app = createApp({
    transport: chooseTransport(environment),
    files: chooseFileService(window),
    recent: createIndexedDbRecentFiles(indexedDB),
    // Both are missing outside secure contexts, and Web Locks in older browsers.
    recovery: createIndexedDbRecovery(indexedDB, {
      locks: navigator.locks ?? null,
      storage: navigator.storage ?? null,
    }),
    platform,
    store,
  })
  for (const command of builtinCommands(platform)) {
    app.commands.register(command)
  }
  installShortcutDispatcher(app, window)
  installUnloadGuard(app, window)
  // The desktop host asks before closing a window with unsaved changes (desktop.md 3.6).
  const tauri = tauriGlobalOf(window)
  if (tauri !== null) {
    installCloseGuard(app.store, tauri)
  }
  createRoot(root).render(
    <StrictMode>
      <AppProvider app={app}>
        <App />
      </AppProvider>
    </StrictMode>,
  )
  startCore(app).catch(reportFailure('Starting the geometry core'))
}
