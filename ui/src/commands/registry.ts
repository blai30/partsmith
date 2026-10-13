import type { LucideIcon } from 'lucide-react'
import { notify } from '../app/actions/notifications'
import { recordPaletteRecent } from '../app/actions/settings'
import type { App, Platform } from '../app/createApp'
import { reportFailure } from '../app/report'
import type { MessageId } from '../messages'
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
  | {
      readonly state: 'disabled'
      readonly reason: string
      /** The 4.3 message id of the reason, which its notification carries as `data-code` (2.8). */
      readonly code?: MessageId
    }
  | { readonly state: 'hidden' }

/** An availability that says why its command cannot run. */
export type DisabledAvailability = Extract<Availability, { readonly state: 'disabled' }>

/** A disabled availability whose reason is the 4.3 message `code`. */
export function disabledWith(code: MessageId, reason: string): DisabledAvailability {
  return { state: 'disabled', reason, code }
}

export type InvocationSource = 'toolbar' | 'palette' | 'shortcut' | 'menu' | 'inspector'

/**
 * A search word. A tagged word is another app's name for the command (`COMMAND_KEYWORDS`, 2.10),
 * shown as "{app}: {word}" in the palette when it matched.
 */
export type Keyword = string | { readonly word: string; readonly app: 'Fusion' | 'FreeCAD' }

export type CommandDefinition = {
  /** Unique id, for example 'file.save' or 'feature.extrude'. */
  readonly id: string
  readonly title: string
  /** The title as shown now when it names the state, such as "Repeat Fillet" (3.5); absent means `title`. */
  label?(app: App): string
  readonly category: CommandCategory
  /** lucide-react icon component, or null. */
  readonly icon: LucideIcon | null
  /** Extra search words from `COMMAND_KEYWORDS`; a tagged word is shown as "{app}: {word}" when it matched. */
  readonly keywords: readonly Keyword[]
  /**
   * At most two short plain sentences, 120 characters in all, ending with a period, saying what the
   * command does (2.10). Required for feature commands, sketch tools, Finish sketch and constraints.
   */
  readonly description?: string
  /** Shortcut specs (ui.md 2.10); the first is the one displayed. */
  readonly shortcuts: readonly string[]
  /** Contexts in which the command is listed in the palette and its shortcuts are active. */
  readonly contexts: readonly AppContext[]
  /**
   * Contexts in which the shortcuts are active when that is fewer than `contexts` (3.5): Hole,
   * Fillet and Move body are listed everywhere, but H, F and M work in Model and Print only.
   */
  readonly shortcutContexts?: readonly AppContext[]
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

/** Says why a command or control cannot act now, as an info notification carrying the reason's code (2.8). */
export function explainDisabled(
  app: App,
  disabled: Pick<DisabledAvailability, 'reason' | 'code'>,
): void {
  notify(app, {
    severity: 'info',
    message: disabled.reason,
    actions: [],
    ...(disabled.code === undefined ? {} : { code: disabled.code }),
  })
}

/** Runs a command from a control, logging a failure for reports. */
export function runCommand(app: App, id: string, source: InvocationSource): void {
  app.commands.run(app, id, source).catch(reportFailure(`The command ${id}`))
}

/** Whether a command's shortcuts are active in a context (3.5). */
export function shortcutsActiveIn(definition: CommandDefinition, context: AppContext): boolean {
  return (definition.shortcutContexts ?? definition.contexts).includes(context)
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
        explainDisabled(app, availability)
        return
      }
      if (availability.state === 'enabled') {
        // A feature command run from anywhere is a recent palette item too; the palette records its own (3.4).
        if (source !== 'palette' && definition.id.startsWith('feature.')) {
          recordPaletteRecent(app, definition.id)
        }
        await definition.run(app, source)
      }
    },

    forChord(app, chord, context) {
      for (const { definition, chords } of registrations.values()) {
        if (
          onHost(definition, app.platform) &&
          shortcutsActiveIn(definition, context) &&
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
          for (const context of definition.shortcutContexts ?? definition.contexts) {
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
