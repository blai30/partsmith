import { ScrollText } from 'lucide-react'
import { afterEach, beforeEach, describe, expect, it, vi, type MockInstance } from 'vitest'
import { rankPaletteItems } from '../../commands/fuzzy'
import { paletteItems } from '../../commands/palette'
import type { CommandDefinition } from '../../commands/registry'
import { cubeDocumentState, FILLET_EDGES_COMMAND, readyState } from '../../layout/fixtures.testing'
import { registerMissingCommands } from '../../layout/test-app.testing'
import { createAppStore, type AppState, type CoreState } from '../../state/store'
import { createFakeApp, onlyNotification, type FakeApp } from '../../test/fakes/app'
import type { App, Platform } from '../createApp'
import { showNotices } from './notices'
import { dismiss } from './notifications'

// Third-party notices (ui.md 3.5, 6.4 U-248 and U-249, delivery.md open issue 11).

const WINDOWS_WEB: Platform = { operatingSystem: 'windows', host: 'web' }
const LINUX_DESKTOP: Platform = { operatingSystem: 'linux', host: 'desktop' }

/** The arguments of the one `window.open` call that opens the deployment's notices. */
const OPEN_NOTICES = ['/THIRD-PARTY-NOTICES.txt', '_blank', 'noopener'] as const

const FAILED_CORE: CoreState = {
  status: 'failed',
  engineVersion: null,
  fatal: 'HTTP 404',
  pendingMutations: 0,
  stillWorking: false,
  stopRequested: false,
  stopPending: false,
}

/** The message on each operating system of the desktop (4.3 `notices_installed`, the places of 3.5). */
const DESKTOP_MESSAGES: readonly (readonly [Platform['operatingSystem'], string])[] = [
  [
    'mac',
    'The licenses of Open CASCADE Technology and the other software in Partsmith are in Partsmith.app/Contents/Resources/THIRD-PARTY-NOTICES.txt.',
  ],
  [
    'linux',
    'The licenses of Open CASCADE Technology and the other software in Partsmith are in /usr/lib/Partsmith/THIRD-PARTY-NOTICES.txt.',
  ],
  [
    'windows',
    'The licenses of Open CASCADE Technology and the other software in Partsmith are in %LOCALAPPDATA%\\Partsmith\\THIRD-PARTY-NOTICES.txt, or in THIRD-PARTY-NOTICES.txt next to partsmith-desktop.exe if Partsmith was installed in another folder.',
  ],
  [
    'other',
    'The licenses of Open CASCADE Technology and the other software in Partsmith are in THIRD-PARTY-NOTICES.txt in the folder Partsmith is installed in.',
  ],
]

let openTab: MockInstance<typeof window.open>

beforeEach(() => {
  openTab = vi.spyOn(window, 'open').mockReturnValue(null)
})

afterEach(() => {
  vi.restoreAllMocks()
  vi.unstubAllEnvs()
  vi.useRealTimers()
})

/** The fake app on a platform over a store with `state`, with the built-in and catalog commands registered. */
function noticesApp(
  platform: Platform,
  state: Partial<AppState> = readyState(cubeDocumentState()),
): FakeApp {
  const fake = createFakeApp({ platform, store: createAppStore(state) })
  registerMissingCommands(fake.app)
  return fake
}

function noticesCommand(app: App): CommandDefinition {
  const definition = app.commands.get('app.notices')
  if (definition === undefined) {
    throw new Error('app.notices is not registered.')
  }
  return definition
}

function runNotices(app: App): Promise<void> {
  return app.commands.run(app, 'app.notices', 'palette')
}

describe('U-248 Third-party notices on the web', () => {
  it('opens the deployment notices in a new tab and adds no notification', async () => {
    const { app } = noticesApp(WINDOWS_WEB)
    await runNotices(app)
    expect(openTab.mock.calls).toEqual([OPEN_NOTICES])
    expect(app.store.getState().notifications).toEqual([])
  })

  it('opens the tab before the run is awaited, while the click or key press is still going on', async () => {
    const { app } = noticesApp(WINDOWS_WEB)
    const running = runNotices(app)
    expect(openTab.mock.calls).toEqual([OPEN_NOTICES])
    await running
  })

  it('opens the tab at once from the run of its palette item', () => {
    const { app } = noticesApp(WINDOWS_WEB)
    const item = paletteItems(app, []).find((candidate) => candidate.id === 'app.notices')
    if (item === undefined) {
      throw new Error('The palette lists no app.notices.')
    }
    item.run()
    expect(openTab.mock.calls).toEqual([OPEN_NOTICES])
  })

  it.each([
    ['the core failed', readyState(cubeDocumentState(), { core: FAILED_CORE })],
    ['the core ready and no document', readyState(null)],
    ['Sketch context', readyState(cubeDocumentState(), { context: 'sketch' })],
    ['a feature command open', readyState(cubeDocumentState(), { command: FILLET_EDGES_COMMAND })],
  ] as const)('is enabled and does the same with %s', async (_state, state) => {
    const { app } = noticesApp(WINDOWS_WEB, state)
    expect(noticesCommand(app).availability(app)).toEqual({ state: 'enabled' })
    await runNotices(app)
    expect(openTab.mock.calls).toEqual([OPEN_NOTICES])
    expect(app.store.getState().notifications).toEqual([])
    expect(app.store.getState().command).toEqual(state.command ?? null)
  })

  it('is Third-party notices under View with ScrollText, no shortcut, every context and every host', () => {
    const { app } = noticesApp(WINDOWS_WEB)
    const definition = noticesCommand(app)
    expect(definition.title).toBe('Third-party notices')
    expect(definition.category).toBe('View')
    expect(definition.icon).toBe(ScrollText)
    expect(definition.shortcuts).toEqual([])
    expect([...definition.contexts].toSorted()).toEqual(['model', 'print', 'sketch'])
    expect(definition.host).toBeUndefined()
  })

  it.each(['license', 'legal', 'Open CASCADE'])('ranks first in the palette for "%s"', (query) => {
    const { app } = noticesApp(WINDOWS_WEB)
    const [first] = rankPaletteItems(query, paletteItems(app, []), [], 'model')
    expect(first?.id).toBe('app.notices')
  })

  it('is listed among the View commands with the empty query', () => {
    const { app } = noticesApp(WINDOWS_WEB)
    const browsed = rankPaletteItems('', paletteItems(app, []), [], 'model')
    const viewPlaces = browsed.flatMap((item, index) => (item.category === 'View' ? [index] : []))
    const firstViewPlace = viewPlaces[0] ?? -1
    // The View group is one run of items, and the notices are in it.
    expect(viewPlaces).toEqual(viewPlaces.map((_place, offset) => firstViewPlace + offset))
    expect(viewPlaces).toContain(browsed.findIndex((item) => item.id === 'app.notices'))
  })
})

describe('U-249 Third-party notices on the desktop', () => {
  it.each(DESKTOP_MESSAGES)(
    'on %s names where the bundle installed the notices and keeps it shown',
    async (operatingSystem, message) => {
      vi.useFakeTimers()
      const { app } = noticesApp({ operatingSystem, host: 'desktop' })
      await runNotices(app)
      expect(openTab).not.toHaveBeenCalled()
      expect(onlyNotification(app)).toMatchObject({
        severity: 'info',
        code: 'notices_installed',
        message,
        actions: [],
      })

      vi.advanceTimersByTime(60_000)
      expect(onlyNotification(app).message).toBe(message)

      await runNotices(app)
      expect(onlyNotification(app).message).toBe(message)
    },
  )
})

describe('the notices command', () => {
  it('has the keywords of ui.md 2.10', () => {
    const { app } = noticesApp(WINDOWS_WEB)
    expect(noticesCommand(app).keywords).toEqual([
      'licenses',
      'legal',
      'copyright',
      'about',
      'credits',
      'open source',
      'Open CASCADE',
    ])
  })

  it.each([WINDOWS_WEB, LINUX_DESKTOP])(
    'reads no file and sends the core nothing on $operatingSystem $host',
    async (platform) => {
      const fake = noticesApp(platform)
      await runNotices(fake.app)
      expect(fake.journal).toEqual([])
    },
  )
})

describe('showNotices', () => {
  it('opens the tab on the web as it returns', () => {
    const { app } = noticesApp(WINDOWS_WEB)
    showNotices(app)
    expect(openTab.mock.calls).toEqual([OPEN_NOTICES])
  })

  it("opens the notices under Vite's base, as the core's path follows it", async () => {
    vi.stubEnv('BASE_URL', '/partsmith/')
    vi.resetModules()
    const notices = await import('./notices')
    const { app } = noticesApp(WINDOWS_WEB)
    notices.showNotices(app)
    expect(openTab.mock.calls).toEqual([
      ['/partsmith/THIRD-PARTY-NOTICES.txt', '_blank', 'noopener'],
    ])
  })

  it('shows the notification on the desktop as it returns, and dismissing it removes it', () => {
    const { app } = noticesApp(LINUX_DESKTOP)
    showNotices(app)
    const shown = onlyNotification(app)
    expect(shown.code).toBe('notices_installed')
    dismiss(app, shown.id)
    expect(app.store.getState().notifications).toEqual([])
  })
})
