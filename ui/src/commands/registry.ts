import type { LucideIcon } from 'lucide-react'
import { notify } from '../app/actions/notifications'
import type { App, Platform } from '../app/createApp'
import type { AppContext } from '../state/store'
import { chordToString, parseShortcut, type Chord } from './shortcuts'

export type CommandCategory =
  | 'File'
  | 'Edit'
  | 'View'
  | 'Create'
  | 'Modify'
  | 'Pattern'
  | 'Construct'
  | 'Sketch'
  | 'Constrain'
  | 'Print'
  | 'Settings'

export type Availability =
  | { readonly state: 'enabled' }
  | { readonly state: 'disabled'; readonly reason: string }
  | { readonly state: 'hidden' }

export type InvocationSource = 'toolbar' | 'palette' | 'shortcut' | 'menu' | 'inspector'

export type CommandDefinition = {
  /** Unique id, for example 'file.save' or 'feature.extrude'. */
  readonly id: string
  readonly title: string
  readonly category: CommandCategory
  /** lucide-react icon component, or null. */
  readonly icon: LucideIcon | null
  /** Extra search words, for example ['pad', 'pocket']. */
  readonly keywords: readonly string[]
  /** Shortcut specs (ui.md 2.10); the first is the one displayed. */
  readonly shortcuts: readonly string[]
  /** Contexts in which the command is listed in the palette and its shortcuts are active. */
  readonly contexts: readonly AppContext[]
  /** Only on this host; absent means every host. */
  readonly host?: 'web' | 'desktop'
  availability(app: App): Availability
  /** For toggle commands: the current state, shown as aria-pressed and a check mark. */
  checked?(app: App): boolean
  /** Secondary text in the palette, for example the undo label. */
  detail?(app: App): string | null
  run(app: App, source: InvocationSource): void | Promise<void>
}

export type ShortcutConflict = {
  readonly chord: string
  readonly context: AppContext
  readonly commands: readonly string[]
}

export type CommandRegistry = {
  /** Throws Error(`Command ${id} is already registered.`) on a duplicate id. */
  register(definition: CommandDefinition): () => void
  get(id: string): CommandDefinition | undefined
  /** Every registered command in registration order; the same array until the next change. */
  list(): readonly CommandDefinition[]
  /** Runs the command if enabled; a disabled command shows its reason as an info notification. */
  run(app: App, id: string, source: InvocationSource): Promise<void>
  /** The command bound to a chord in a context, considering host and availability 'hidden'. */
  forChord(app: App, chord: string, context: AppContext): CommandDefinition | undefined
  conflicts(platform: Platform): readonly ShortcutConflict[]
  subscribe(listener: () => void): () => void
}

type Registration = {
  readonly definition: CommandDefinition
  readonly chords: readonly Chord[]
}

/**
 * The chord as the platform's keyboard produces it: outside macOS the primary modifier is the
 * Control key, so `Ctrl` and `Mod` are one key there.
 */
function platformChord(chord: Chord, platform: Platform): string {
  if (platform.operatingSystem !== 'mac' && chord.control) {
    return chordToString({ ...chord, primaryModifier: true, control: false })
  }
  return chordToString(chord)
}

function onHost(definition: CommandDefinition, platform: Platform): boolean {
  return definition.host === undefined || definition.host === platform.host
}

export function createCommandRegistry(): CommandRegistry {
  const registrations = new Map<string, Registration>()
  const listeners = new Set<() => void>()
  // A stable snapshot, so subscribers that compare lists (useSyncExternalStore) see changes only.
  let definitions: readonly CommandDefinition[] = []

  function changed(): void {
    definitions = Array.from(registrations.values(), (registration) => registration.definition)
    // A copy, so a listener that subscribes or unsubscribes does not change this round.
    for (const listener of Array.from(listeners)) {
      listener()
    }
  }

  return {
    register(definition) {
      if (registrations.has(definition.id)) {
        throw new Error(`Command ${definition.id} is already registered.`)
      }
      // Parsing up front rejects a malformed shortcut at registration, not at the first key press.
      const registration: Registration = {
        definition,
        chords: definition.shortcuts.map(parseShortcut),
      }
      registrations.set(definition.id, registration)
      changed()
      return () => {
        if (registrations.get(definition.id) === registration) {
          registrations.delete(definition.id)
          changed()
        }
      }
    },

    get(id) {
      return registrations.get(id)?.definition
    },

    list() {
      return definitions
    },

    async run(app, id, source) {
      const definition = registrations.get(id)?.definition
      if (definition === undefined) {
        throw new Error(`Command ${id} is not registered.`)
      }
      if (!onHost(definition, app.platform)) {
        return
      }
      const availability = definition.availability(app)
      if (availability.state === 'disabled') {
        notify(app, { severity: 'info', message: availability.reason, actions: [] })
        return
      }
      if (availability.state === 'enabled') {
        await definition.run(app, source)
      }
    },

    forChord(app, chord, context) {
      for (const { definition, chords } of registrations.values()) {
        if (
          onHost(definition, app.platform) &&
          definition.contexts.includes(context) &&
          chords.some((bound) => platformChord(bound, app.platform) === chord) &&
          definition.availability(app).state !== 'hidden'
        ) {
          return definition
        }
      }
      return undefined
    },

    conflicts(platform) {
      // Command ids bound to each chord in each context, keyed by context and chord.
      const bindings = new Map<string, { chord: string; context: AppContext; ids: Set<string> }>()
      for (const { definition, chords } of registrations.values()) {
        if (!onHost(definition, platform)) {
          continue
        }
        for (const bound of chords) {
          const chord = platformChord(bound, platform)
          for (const context of definition.contexts) {
            const key = `${context} ${chord}`
            const binding = bindings.get(key) ?? { chord, context, ids: new Set<string>() }
            binding.ids.add(definition.id)
            bindings.set(key, binding)
          }
        }
      }
      return Array.from(bindings.values())
        .filter((binding) => binding.ids.size > 1)
        .map((binding) => ({
          chord: binding.chord,
          context: binding.context,
          commands: [...binding.ids],
        }))
    },

    subscribe(listener) {
      listeners.add(listener)
      return () => {
        listeners.delete(listener)
      }
    },
  }
}
