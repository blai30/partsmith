import { afterEach, beforeEach, describe, expect, it, vi, type Mock } from 'vitest'
import type { App, Platform } from '../app/createApp'
import type { AppContext } from '../state/store'
import { createTestApp as createFakeTestApp, LINUX_WEB, MAC_WEB } from '../test/fakes/app'
import { installShortcutDispatcher } from './dispatcher'
import {
  createCommandRegistry,
  type Availability,
  type CommandDefinition,
  type CommandRegistry,
} from './registry'

const ALL_CONTEXTS: readonly AppContext[] = ['model', 'sketch', 'print']

function createTestApp(commands: CommandRegistry, platform: Platform): App {
  return createFakeTestApp({ commands, platform })
}

type Fixture = {
  readonly app: App
  /** The run function of each registered command, by id. */
  readonly runs: Readonly<Record<string, Mock<CommandDefinition['run']>>>
}

type Registration = {
  readonly id: string
  readonly shortcuts: readonly string[]
  readonly contexts?: readonly AppContext[]
  readonly availability?: Availability
}

const REGISTRATIONS: readonly Registration[] = [
  { id: 'app.palette', shortcuts: ['Mod+K', 'S'] },
  { id: 'edit.undo', shortcuts: ['Mod+Z'] },
  { id: 'edit.redo', shortcuts: ['Mod+Shift+Z', 'Mod+Y'] },
  { id: 'edit.select_all', shortcuts: ['Mod+A'] },
  { id: 'edit.copy', shortcuts: ['Mod+C'] },
  { id: 'edit.cut', shortcuts: ['Mod+X'] },
  { id: 'edit.paste', shortcuts: ['Mod+V'] },
  { id: 'file.save', shortcuts: ['Mod+S'] },
  { id: 'feature.extrude', shortcuts: ['E'] },
  { id: 'sketch.line', shortcuts: ['L'], contexts: ['sketch'] },
  {
    id: 'file.export',
    shortcuts: ['Mod+Shift+E'],
    availability: { state: 'disabled', reason: 'Finish or cancel Extrude first.' },
  },
]

function setUp(platform: Platform = LINUX_WEB): Fixture {
  const registry = createCommandRegistry()
  const runs: Record<string, Mock<CommandDefinition['run']>> = {}
  for (const registration of REGISTRATIONS) {
    const run = vi.fn<CommandDefinition['run']>()
    runs[registration.id] = run
    const availability = registration.availability ?? { state: 'enabled' }
    const definition: CommandDefinition = {
      id: registration.id,
      title: registration.id,
      category: 'Edit',
      icon: null,
      keywords: [],
      shortcuts: registration.shortcuts,
      contexts: registration.contexts ?? ALL_CONTEXTS,
      availability: () => availability,
      run,
    }
    registry.register(definition)
  }
  return { app: createTestApp(registry, platform), runs }
}

function keydown(target: EventTarget, init: KeyboardEventInit): KeyboardEvent {
  const event = new KeyboardEvent('keydown', { bubbles: true, cancelable: true, ...init })
  target.dispatchEvent(event)
  return event
}

// Lets a command started by the dispatcher reach its run function.
async function settle(): Promise<void> {
  await new Promise((resolve) => {
    setTimeout(resolve, 0)
  })
}

function ranCommands(runs: Fixture['runs']): string[] {
  return Object.entries(runs)
    .filter(([, run]) => run.mock.calls.length > 0)
    .map(([id]) => id)
}

let uninstall: (() => void) | null = null
let input: HTMLInputElement

beforeEach(() => {
  input = document.createElement('input')
  document.body.append(input)
})

afterEach(() => {
  uninstall?.()
  uninstall = null
  document.body.replaceChildren()
  vi.restoreAllMocks()
})

function install(fixture: Fixture): void {
  uninstall = installShortcutDispatcher(fixture.app, window)
}

describe('installShortcutDispatcher', () => {
  it('U-24 e with an <input> target runs nothing', async () => {
    const fixture = setUp()
    install(fixture)
    const event = keydown(input, { key: 'e', code: 'KeyE' })
    await settle()
    expect(ranCommands(fixture.runs)).toEqual([])
    expect(event.defaultPrevented).toBe(false)
  })

  it('U-24 e with document.body runs feature.extrude', async () => {
    const fixture = setUp()
    install(fixture)
    const event = keydown(document.body, { key: 'e', code: 'KeyE' })
    await settle()
    expect(ranCommands(fixture.runs)).toEqual(['feature.extrude'])
    expect(fixture.runs['feature.extrude']).toHaveBeenCalledWith(fixture.app, 'shortcut')
    expect(event.defaultPrevented).toBe(true)
  })

  it('U-24 Mod+S in an input runs file.save', async () => {
    const fixture = setUp()
    install(fixture)
    const event = keydown(input, { key: 's', code: 'KeyS', ctrlKey: true })
    await settle()
    expect(ranCommands(fixture.runs)).toEqual(['file.save'])
    expect(event.defaultPrevented).toBe(true)
  })

  it('U-24 Mod+Z in an input runs nothing and keeps native undo', async () => {
    const fixture = setUp()
    install(fixture)
    const event = keydown(input, { key: 'z', code: 'KeyZ', ctrlKey: true })
    await settle()
    expect(ranCommands(fixture.runs)).toEqual([])
    expect(event.defaultPrevented).toBe(false)
  })

  it.each([
    ['Mod+Shift+Z', { key: 'Z', code: 'KeyZ', ctrlKey: true, shiftKey: true }],
    ['Mod+Y', { key: 'y', code: 'KeyY', ctrlKey: true }],
    ['Mod+A', { key: 'a', code: 'KeyA', ctrlKey: true }],
    ['Mod+C', { key: 'c', code: 'KeyC', ctrlKey: true }],
    ['Mod+X', { key: 'x', code: 'KeyX', ctrlKey: true }],
    ['Mod+V', { key: 'v', code: 'KeyV', ctrlKey: true }],
  ])('%s in an input runs nothing (native text editing keeps it)', async (_chord, init) => {
    const fixture = setUp()
    install(fixture)
    keydown(input, init)
    await settle()
    expect(ranCommands(fixture.runs)).toEqual([])
  })

  it('Mod+Z outside an editable target runs edit.undo', async () => {
    const fixture = setUp()
    install(fixture)
    keydown(document.body, { key: 'z', code: 'KeyZ', ctrlKey: true })
    await settle()
    expect(ranCommands(fixture.runs)).toEqual(['edit.undo'])
  })

  it('U-24 a defaultPrevented event runs nothing', async () => {
    const fixture = setUp()
    install(fixture)
    const button = document.createElement('button')
    document.body.append(button)
    button.addEventListener('keydown', (event) => {
      event.preventDefault()
    })
    keydown(button, { key: 'e', code: 'KeyE' })
    await settle()
    expect(ranCommands(fixture.runs)).toEqual([])
  })

  it('U-24 a repeat event runs nothing', async () => {
    const fixture = setUp()
    install(fixture)
    keydown(document.body, { key: 'e', code: 'KeyE', repeat: true })
    await settle()
    expect(ranCommands(fixture.runs)).toEqual([])
  })

  it('an isComposing event runs nothing', async () => {
    const fixture = setUp()
    install(fixture)
    keydown(document.body, { key: 'e', code: 'KeyE', isComposing: true })
    await settle()
    expect(ranCommands(fixture.runs)).toEqual([])
  })

  it('U-24 with the palette open only Mod+K runs', async () => {
    const fixture = setUp()
    const { store } = fixture.app
    store.setState({ panels: { ...store.getState().panels, paletteOpen: true } })
    install(fixture)
    keydown(document.body, { key: 'e', code: 'KeyE' })
    keydown(document.body, { key: 's', code: 'KeyS', ctrlKey: true })
    keydown(document.body, { key: 's', code: 'KeyS' })
    await settle()
    expect(ranCommands(fixture.runs)).toEqual([])
    keydown(document.body, { key: 'k', code: 'KeyK', ctrlKey: true })
    await settle()
    expect(ranCommands(fixture.runs)).toEqual(['app.palette'])
  })

  it('with a menu or popover open only Mod+K runs', async () => {
    const fixture = setUp()
    install(fixture)
    const menu = document.createElement('div')
    menu.setAttribute('popover', 'auto')
    document.body.append(menu)
    // jsdom has no Popover API and never matches :popover-open, so the document reports the menu
    // as an open popover the way a browser does.
    const querySelector = document.querySelector.bind(document)
    vi.spyOn(document, 'querySelector').mockImplementation((selectors: string) =>
      selectors.includes(':popover-open') ? menu : querySelector(selectors),
    )
    keydown(document.body, { key: 'e', code: 'KeyE' })
    keydown(document.body, { key: 's', code: 'KeyS', ctrlKey: true })
    await settle()
    expect(ranCommands(fixture.runs)).toEqual([])
    keydown(document.body, { key: 'k', code: 'KeyK', ctrlKey: true })
    await settle()
    expect(ranCommands(fixture.runs)).toEqual(['app.palette'])
  })

  it('Mod+K runs the palette command from a text field', async () => {
    const fixture = setUp()
    install(fixture)
    keydown(input, { key: 'k', code: 'KeyK', ctrlKey: true })
    await settle()
    expect(ranCommands(fixture.runs)).toEqual(['app.palette'])
  })

  it('treats a textarea and a select as editable targets', async () => {
    const fixture = setUp()
    install(fixture)
    const textarea = document.createElement('textarea')
    const select = document.createElement('select')
    document.body.append(textarea, select)
    keydown(textarea, { key: 'e', code: 'KeyE' })
    keydown(select, { key: 'e', code: 'KeyE' })
    await settle()
    expect(ranCommands(fixture.runs)).toEqual([])
  })

  it('treats a content-editable element as an editable target', async () => {
    const fixture = setUp()
    install(fixture)
    const editor = document.createElement('div')
    editor.setAttribute('contenteditable', 'true')
    // jsdom does not compute isContentEditable; a browser gives true here.
    Object.defineProperty(editor, 'isContentEditable', { value: true })
    document.body.append(editor)
    keydown(editor, { key: 'e', code: 'KeyE' })
    keydown(editor, { key: 'z', code: 'KeyZ', ctrlKey: true })
    await settle()
    expect(ranCommands(fixture.runs)).toEqual([])
    keydown(editor, { key: 's', code: 'KeyS', ctrlKey: true })
    await settle()
    expect(ranCommands(fixture.runs)).toEqual(['file.save'])
  })

  it('treats a checkbox as not editable', async () => {
    const fixture = setUp()
    install(fixture)
    const checkbox = document.createElement('input')
    checkbox.type = 'checkbox'
    document.body.append(checkbox)
    keydown(checkbox, { key: 'e', code: 'KeyE' })
    await settle()
    expect(ranCommands(fixture.runs)).toEqual(['feature.extrude'])
  })

  it('uses the current context: a sketch tool key runs only in Sketch', async () => {
    const fixture = setUp()
    install(fixture)
    keydown(document.body, { key: 'l', code: 'KeyL' })
    await settle()
    expect(ranCommands(fixture.runs)).toEqual([])
    fixture.app.store.setState({ context: 'sketch' })
    keydown(document.body, { key: 'l', code: 'KeyL' })
    await settle()
    expect(ranCommands(fixture.runs)).toEqual(['sketch.line'])
  })

  it('uses Cmd as the primary modifier on macOS', async () => {
    const fixture = setUp(MAC_WEB)
    install(fixture)
    keydown(document.body, { key: 's', code: 'KeyS', ctrlKey: true })
    await settle()
    expect(ranCommands(fixture.runs)).toEqual([])
    keydown(document.body, { key: 's', code: 'KeyS', metaKey: true })
    await settle()
    expect(ranCommands(fixture.runs)).toEqual(['file.save'])
  })

  it('shows the reason of a disabled command as an info notification instead of running it', async () => {
    const fixture = setUp()
    install(fixture)
    const event = keydown(document.body, { key: 'E', code: 'KeyE', ctrlKey: true, shiftKey: true })
    await settle()
    expect(ranCommands(fixture.runs)).toEqual([])
    expect(event.defaultPrevented).toBe(true)
    expect(fixture.app.store.getState().notifications).toEqual([
      expect.objectContaining({ severity: 'info', message: 'Finish or cancel Extrude first.' }),
    ])
  })

  it('leaves keys with no command alone', async () => {
    const fixture = setUp()
    install(fixture)
    const event = keydown(document.body, { key: 'q', code: 'KeyQ' })
    await settle()
    expect(ranCommands(fixture.runs)).toEqual([])
    expect(event.defaultPrevented).toBe(false)
  })

  it('stops dispatching once uninstalled', async () => {
    const fixture = setUp()
    install(fixture)
    uninstall?.()
    uninstall = null
    keydown(document.body, { key: 'e', code: 'KeyE' })
    await settle()
    expect(ranCommands(fixture.runs)).toEqual([])
  })
})
