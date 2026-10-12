import { describe, expect, it, vi } from 'vitest'
import type { App, Platform } from '../app/createApp'
import type { AppContext } from '../state/store'
import { createTestApp as createFakeTestApp, LINUX_WEB, MAC_WEB, WINDOWS_DESKTOP } from '../test/fakes/app'
import {
  createCommandRegistry,
  type Availability,
  type CommandDefinition,
  type CommandRegistry,
} from './registry'

const ALL_CONTEXTS: readonly AppContext[] = ['model', 'sketch', 'print']

function createTestApp(commands: CommandRegistry, platform: Platform = LINUX_WEB): App {
  return createFakeTestApp({ commands, platform })
}

type CommandOptions = {
  readonly shortcuts?: readonly string[]
  readonly contexts?: readonly AppContext[]
  readonly host?: 'web' | 'desktop'
  readonly availability?: Availability
  readonly run?: CommandDefinition['run']
}

function command(id: string, options: CommandOptions = {}): CommandDefinition {
  const availability = options.availability ?? { state: 'enabled' }
  const definition: CommandDefinition = {
    id,
    title: id,
    category: 'Edit',
    icon: null,
    keywords: [],
    shortcuts: options.shortcuts ?? [],
    contexts: options.contexts ?? ALL_CONTEXTS,
    availability: () => availability,
    run: options.run ?? (() => undefined),
  }
  return options.host === undefined ? definition : { ...definition, host: options.host }
}

describe('register, get and list', () => {
  it('U-25 registering a duplicate id throws "Command x is already registered."', () => {
    const registry = createCommandRegistry()
    registry.register(command('x'))
    expect(() => registry.register(command('x'))).toThrow(
      new Error('Command x is already registered.'),
    )
  })

  it('register throws for a malformed shortcut and registers nothing', () => {
    const registry = createCommandRegistry()
    expect(() => registry.register(command('file.save', { shortcuts: ['Shift+Mod+S'] }))).toThrow(
      new Error('Invalid shortcut "Shift+Mod+S".'),
    )
    expect(registry.get('file.save')).toBeUndefined()
    expect(registry.list()).toEqual([])
  })

  it('get returns the registered definition and undefined for an unknown id', () => {
    const registry = createCommandRegistry()
    const save = command('file.save')
    registry.register(save)
    expect(registry.get('file.save')).toBe(save)
    expect(registry.get('file.open')).toBeUndefined()
  })

  it('list returns every registered definition', () => {
    const registry = createCommandRegistry()
    registry.register(command('file.save'))
    registry.register(command('edit.undo'))
    expect(registry.list().map((definition) => definition.id)).toEqual(
      expect.arrayContaining(['file.save', 'edit.undo']),
    )
    expect(registry.list()).toHaveLength(2)
  })

  it('the function register returns unregisters the command, so the id can be registered again', () => {
    const registry = createCommandRegistry()
    const unregister = registry.register(command('file.save'))
    unregister()
    expect(registry.get('file.save')).toBeUndefined()
    expect(registry.list()).toHaveLength(0)
    expect(() => registry.register(command('file.save'))).not.toThrow()
  })

  it('list returns the same array until the commands change', () => {
    const registry = createCommandRegistry()
    registry.register(command('file.save'))
    const first = registry.list()
    expect(registry.list()).toBe(first)
    const unregister = registry.register(command('file.open'))
    const second = registry.list()
    expect(second).not.toBe(first)
    expect(second.map((definition) => definition.id)).toEqual(['file.save', 'file.open'])
    unregister()
    expect(registry.list().map((definition) => definition.id)).toEqual(['file.save'])
  })

  it('notifies subscribers when commands are registered and unregistered, until they unsubscribe', () => {
    const registry = createCommandRegistry()
    const listener = vi.fn<() => void>()
    const unsubscribe = registry.subscribe(listener)
    const unregister = registry.register(command('file.save'))
    expect(listener).toHaveBeenCalledTimes(1)
    unregister()
    expect(listener).toHaveBeenCalledTimes(2)
    unsubscribe()
    registry.register(command('file.open'))
    expect(listener).toHaveBeenCalledTimes(2)
  })
})

describe('run', () => {
  it('U-25 running a disabled command calls no run and shows an info notification with its reason', async () => {
    const registry = createCommandRegistry()
    const run = vi.fn<CommandDefinition['run']>()
    registry.register(
      command('edit.undo', {
        availability: { state: 'disabled', reason: 'Nothing to undo.' },
        run,
      }),
    )
    const app = createTestApp(registry)
    await registry.run(app, 'edit.undo', 'palette')
    expect(run).not.toHaveBeenCalled()
    expect(app.store.getState().notifications).toEqual([
      expect.objectContaining({ severity: 'info', message: 'Nothing to undo.' }),
    ])
  })

  it('runs an enabled command with the app and the invocation source', async () => {
    const registry = createCommandRegistry()
    const run = vi.fn<CommandDefinition['run']>()
    registry.register(command('file.save', { run }))
    const app = createTestApp(registry)
    await registry.run(app, 'file.save', 'toolbar')
    expect(run).toHaveBeenCalledTimes(1)
    expect(run).toHaveBeenCalledWith(app, 'toolbar')
    expect(app.store.getState().notifications).toEqual([])
  })

  it('resolves after an asynchronous command has finished', async () => {
    const registry = createCommandRegistry()
    let finished = false
    registry.register(
      command('file.save', {
        run: async () => {
          await Promise.resolve()
          finished = true
        },
      }),
    )
    const app = createTestApp(registry)
    await registry.run(app, 'file.save', 'shortcut')
    expect(finished).toBe(true)
  })
})

describe('forChord', () => {
  it('finds the command bound to a chord in a context', () => {
    const registry = createCommandRegistry()
    registry.register(command('file.save', { shortcuts: ['Mod+S'] }))
    const app = createTestApp(registry)
    expect(registry.forChord(app, 'Mod+S', 'model')?.id).toBe('file.save')
    expect(registry.forChord(app, 'Mod+O', 'model')).toBeUndefined()
  })

  it('matches every shortcut of a command, not only the displayed one', () => {
    const registry = createCommandRegistry()
    registry.register(command('edit.redo', { shortcuts: ['Mod+Shift+Z', 'Mod+Y'] }))
    const app = createTestApp(registry)
    expect(registry.forChord(app, 'Mod+Shift+Z', 'model')?.id).toBe('edit.redo')
    expect(registry.forChord(app, 'Mod+Y', 'print')?.id).toBe('edit.redo')
  })

  it('only finds a command in its contexts', () => {
    const registry = createCommandRegistry()
    registry.register(command('feature.hole', { shortcuts: ['H'], contexts: ['model', 'print'] }))
    registry.register(
      command('sketch.constrain_horizontal', { shortcuts: ['H'], contexts: ['sketch'] }),
    )
    const app = createTestApp(registry)
    expect(registry.forChord(app, 'H', 'model')?.id).toBe('feature.hole')
    expect(registry.forChord(app, 'H', 'print')?.id).toBe('feature.hole')
    expect(registry.forChord(app, 'H', 'sketch')?.id).toBe('sketch.constrain_horizontal')
  })

  it('skips a command for another host', () => {
    const registry = createCommandRegistry()
    registry.register(command('file.new', { shortcuts: ['Mod+N'], host: 'desktop' }))
    expect(registry.forChord(createTestApp(registry, LINUX_WEB), 'Mod+N', 'model')).toBeUndefined()
    expect(registry.forChord(createTestApp(registry, WINDOWS_DESKTOP), 'Mod+N', 'model')?.id).toBe(
      'file.new',
    )
  })

  it('skips a hidden command and finds another command bound to the same chord', () => {
    const registry = createCommandRegistry()
    registry.register(
      command('first.delete', { shortcuts: ['Delete'], availability: { state: 'hidden' } }),
    )
    registry.register(command('second.delete', { shortcuts: ['Delete'] }))
    const app = createTestApp(registry)
    expect(registry.forChord(app, 'Delete', 'model')?.id).toBe('second.delete')
  })

  it('keeps Ctrl and Mod apart on macOS, where Mod is Cmd', () => {
    const registry = createCommandRegistry()
    registry.register(command('view.browser', { shortcuts: ['Mod+B'] }))
    registry.register(command('control.command', { shortcuts: ['Ctrl+B'] }))
    const app = createTestApp(registry, MAC_WEB)
    expect(registry.forChord(app, 'Mod+B', 'model')?.id).toBe('view.browser')
    expect(registry.forChord(app, 'Ctrl+B', 'model')?.id).toBe('control.command')
  })

  it('finds a command bound to Ctrl by the chord the Control key gives outside macOS', () => {
    const registry = createCommandRegistry()
    registry.register(command('control.command', { shortcuts: ['Ctrl+B'] }))
    // chordFromEvent reports the Control key as the primary modifier outside macOS.
    expect(registry.forChord(createTestApp(registry, LINUX_WEB), 'Mod+B', 'model')?.id).toBe(
      'control.command',
    )
  })

  it('returns a disabled command so its reason can be shown', () => {
    const registry = createCommandRegistry()
    registry.register(
      command('edit.undo', {
        shortcuts: ['Mod+Z'],
        availability: { state: 'disabled', reason: 'Nothing to undo.' },
      }),
    )
    const app = createTestApp(registry)
    expect(registry.forChord(app, 'Mod+Z', 'model')?.id).toBe('edit.undo')
  })
})

describe('conflicts', () => {
  it('is empty when no two commands share a chord in a context', () => {
    const registry = createCommandRegistry()
    registry.register(command('feature.hole', { shortcuts: ['H'], contexts: ['model', 'print'] }))
    registry.register(
      command('sketch.constrain_horizontal', { shortcuts: ['H'], contexts: ['sketch'] }),
    )
    registry.register(command('view.home', { shortcuts: ['Shift+H'] }))
    expect(registry.conflicts(LINUX_WEB)).toEqual([])
    expect(registry.conflicts(MAC_WEB)).toEqual([])
  })

  it('reports two commands bound to the same chord in a shared context, once per shared context', () => {
    const registry = createCommandRegistry()
    registry.register(command('first.command', { shortcuts: ['Shift+H'], contexts: ['model'] }))
    registry.register(
      command('second.command', { shortcuts: ['Shift+H'], contexts: ['model', 'sketch'] }),
    )
    const conflicts = registry.conflicts(LINUX_WEB)
    expect(conflicts).toHaveLength(1)
    expect(conflicts[0]?.chord).toBe('Shift+H')
    expect(conflicts[0]?.context).toBe('model')
    expect([...(conflicts[0]?.commands ?? [])].toSorted()).toEqual([
      'first.command',
      'second.command',
    ])
  })

  it('ignores commands of another host', () => {
    const registry = createCommandRegistry()
    registry.register(command('file.new', { shortcuts: ['Mod+N'], host: 'desktop' }))
    registry.register(command('web.only', { shortcuts: ['Mod+N'], host: 'web' }))
    expect(registry.conflicts(LINUX_WEB)).toEqual([])
    expect(registry.conflicts(WINDOWS_DESKTOP)).toEqual([])
  })

  it('treats Mod and Ctrl as the same key outside macOS only', () => {
    const registry = createCommandRegistry()
    registry.register(command('first.command', { shortcuts: ['Mod+B'], contexts: ['model'] }))
    registry.register(command('second.command', { shortcuts: ['Ctrl+B'], contexts: ['model'] }))
    expect(registry.conflicts(MAC_WEB)).toEqual([])
    const conflicts = registry.conflicts(LINUX_WEB)
    expect(conflicts).toHaveLength(1)
    expect(conflicts[0]?.context).toBe('model')
    expect([...(conflicts[0]?.commands ?? [])].toSorted()).toEqual([
      'first.command',
      'second.command',
    ])
  })
})
