import type { App, Platform } from '../../app/createApp'
import { createCommandRegistry, type CommandRegistry } from '../../commands/registry'
import type { CoreApi } from '../../core/api'
import { createMutationQueue } from '../../core/queue'
import type { FileService } from '../../files/file-service'
import { createMemoryRecentFiles, type RecentFilesStore } from '../../files/recent'
import { createMemoryRecovery, type RecoveryStore } from '../../files/recovery'
import {
  createAppStore,
  type ActiveCommand,
  type AppStore,
  type DocumentState,
  type Notification,
} from '../../state/store'
import type { ViewportHandle } from '../../viewport/controller'
import { createFakeCoreApi } from './core-api'
import { createFakeFileService } from './files'

// An App assembled by hand around a FakeCoreApi (no events reach the store), plus helpers to read any
// App's state. This module does not load createApp or the actions, so tests of low-level modules can
// use it. ./app re-exports it and adds the real app built by createApp over a FakeTransport.

export const LINUX_WEB: Platform = { operatingSystem: 'linux', host: 'web' }
export const MAC_WEB: Platform = { operatingSystem: 'mac', host: 'web' }
export const WINDOWS_DESKTOP: Platform = { operatingSystem: 'windows', host: 'desktop' }

export type TestAppOptions = {
  readonly store?: AppStore
  readonly core?: CoreApi
  readonly commands?: CommandRegistry
  readonly files?: FileService
  readonly recent?: RecentFilesStore
  readonly recovery?: RecoveryStore
  readonly platform?: Platform
  readonly viewport?: ViewportHandle | null
}

/** An App assembled by hand around a FakeCoreApi, a real mutation queue and in-memory stores. */
export function createTestApp(options: TestAppOptions = {}): App {
  let viewport = options.viewport ?? null
  return {
    store: options.store ?? createAppStore(),
    core: options.core ?? createFakeCoreApi(),
    mutations: createMutationQueue(),
    commands: options.commands ?? createCommandRegistry(),
    files: options.files ?? createFakeFileService(),
    recent: options.recent ?? createMemoryRecentFiles(),
    recovery: options.recovery ?? createMemoryRecovery(),
    platform: options.platform ?? LINUX_WEB,
    viewport: () => viewport,
    attachViewport: (handle) => {
      viewport = handle
    },
    restartCore: () => Promise.resolve(),
  }
}

/** The open document; throws when there is none. */
export function currentDocument(app: App): DocumentState {
  const document = app.store.getState().document
  if (document === null) {
    throw new Error('The app has no open document.')
  }
  return document
}

/** The active feature command; throws when there is none. */
export function activeCommand(app: App): ActiveCommand {
  const command = app.store.getState().command
  if (command === null) {
    throw new Error('No command is active.')
  }
  return command
}

/** The messages of the shown notifications, oldest first. */
export function notificationMessages(app: App): string[] {
  return app.store.getState().notifications.map((notification) => notification.message)
}

/** The shown notification whose message matches exactly (a string) or by pattern; throws when there is none. */
export function notificationWith(app: App, message: string | RegExp): Notification {
  const found = app.store
    .getState()
    .notifications.find((notification) =>
      typeof message === 'string'
        ? notification.message === message
        : message.test(notification.message),
    )
  if (found === undefined) {
    const shown = notificationMessages(app).join(' | ')
    throw new Error(`No notification matches ${String(message)}; shown: ${shown || 'none'}.`)
  }
  return found
}

/** The labels of a notification's actions, in order. */
export function actionLabels(notification: Notification): readonly string[] {
  return notification.actions.map((action) => action.label)
}

/** Runs the notification action with this label; throws when there is none. */
export function runAction(notification: Notification, label: string): void {
  const action = notification.actions.find((candidate) => candidate.label === label)
  if (action === undefined) {
    throw new Error(`The notification "${notification.message}" has no action ${label}.`)
  }
  action.run()
}
