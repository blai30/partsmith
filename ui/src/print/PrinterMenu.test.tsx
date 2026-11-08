import { act, render, screen, within } from '@testing-library/react'
import userEvent, { type UserEvent } from '@testing-library/user-event'
import { useRef, useState } from 'react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { openSidePanel } from '../app/actions/panels'
import { installPopoverPolyfill } from '../components/popover-polyfill.testing'
import type { Command, Document, PrinterProfile } from '../generated/protocol'
import {
  renderWithApp,
  stubAnimationFrames,
  stubElementSize,
  stubMatchMedia,
  type FrameQueue,
} from '../layout/test-app.testing'
import {
  currentDocument,
  labeledEdits,
  notificationCoded,
  printerLibraryHolding,
  startWithDocument,
  type FakeApp,
} from '../test/fakes/app'
import { CUBE_DOCUMENT } from '../test/fakes/documents'
import {
  GENERIC_FDM,
  GENERIC_RESIN,
  MK4,
  MK4B,
  unknownPrinterMessage,
  writtenProfile,
} from '../test/fakes/printers'
import { settle } from '../test/fakes/settle'
import { COLOR_CLASSES } from '../theme/roles'
import { PRINTERS } from './fixtures.testing'
import { createMemoryPrinterLibrary, type PrinterLibrary } from './library'
import { PrinterMenu } from './PrinterMenu'

beforeEach(installPopoverPolyfill)

type HarnessProps = {
  readonly printers?: readonly PrinterProfile[]
  readonly current: string | null
  readonly onChoose: (profileId: string | null) => void
  readonly onOpenChange?: (open: boolean) => void
}

// The menu under the toolbar's printer button, as the Print toolbar and the status bar use it.
function Harness({ printers = PRINTERS, current, onChoose, onOpenChange }: HarnessProps) {
  const anchor = useRef<HTMLButtonElement>(null)
  const [open, setOpen] = useState(false)
  return (
    <>
      <button ref={anchor} type="button" onClick={() => setOpen(true)}>
        Printer: none
      </button>
      <PrinterMenu
        open={open}
        anchor={anchor}
        onOpenChange={(next) => {
          onOpenChange?.(next)
          setOpen(next)
        }}
        printers={printers}
        current={current}
        onChoose={onChoose}
      />
    </>
  )
}

async function openMenu(props: Partial<HarnessProps> = {}) {
  const onChoose = vi.fn<(profileId: string | null) => void>()
  const onOpenChange = vi.fn<(open: boolean) => void>()
  render(<Harness current={null} onChoose={onChoose} onOpenChange={onOpenChange} {...props} />)
  await userEvent.click(screen.getByRole('button', { name: 'Printer: none' }))
  return { menu: screen.getByTestId('printer-menu'), onChoose, onOpenChange }
}

function radio(container: HTMLElement, name: string): HTMLElement {
  return within(container).getByRole('menuitemradio', { name })
}

describe('PrinterMenu', () => {
  it('is not shown while closed', () => {
    render(<Harness current={null} onChoose={vi.fn<(profileId: string | null) => void>()} />)
    expect(screen.queryByRole('menu')).toBeNull()
  })

  it('opens as the menu printer-menu', async () => {
    const { menu } = await openMenu()
    expect(screen.getByRole('menu')).toBe(menu)
  })

  it('lists the FDM profiles under the heading "FDM" and the resin profiles under "Resin"', async () => {
    const { menu } = await openMenu()
    const order = [
      within(menu).getByText('FDM'),
      radio(menu, 'Generic FDM 0.4 mm'),
      radio(menu, 'My MK4'),
      within(menu).getByText('Resin'),
      radio(menu, 'Generic resin MSLA'),
    ]
    // Each element comes after the one before it.
    order.slice(1).forEach((element, index) => {
      expect(order[index]?.compareDocumentPosition(element)).toBe(Node.DOCUMENT_POSITION_FOLLOWING)
    })
  })

  it('keeps every radio item in one set: no groups and no separators between them', async () => {
    const { menu } = await openMenu()
    expect(within(menu).queryByRole('group')).toBeNull()
    expect(within(menu).queryByRole('separator')).toBeNull()
  })

  it('describes each profile with its technology and key numbers, outside its name', async () => {
    const { menu } = await openMenu()
    expect(radio(menu, 'Generic FDM 0.4 mm')).toHaveAccessibleDescription(
      'FDM 256 × 256 × 256 mm bed · 0.4 mm nozzle',
    )
    expect(radio(menu, 'My MK4')).toHaveAccessibleDescription(
      'FDM 250 × 210 × 220 mm bed · 0.4 mm nozzle',
    )
    expect(radio(menu, 'Generic resin MSLA')).toHaveAccessibleDescription('Resin 130 × 80 × 160 mm')
    expect(within(menu).getByText('130 × 80 × 160 mm')).toHaveClass(
      COLOR_CLASSES.textMuted.foreground,
    )
  })

  it('offers "Default ({default name})" after the profiles', async () => {
    const { menu } = await openMenu()
    expect(within(menu).getAllByRole('menuitemradio')).toEqual([
      radio(menu, 'Generic FDM 0.4 mm'),
      radio(menu, 'My MK4'),
      radio(menu, 'Generic resin MSLA'),
      radio(menu, 'Default (Generic FDM 0.4 mm)'),
    ])
    expect(radio(menu, 'Default (Generic FDM 0.4 mm)')).not.toHaveAccessibleDescription()
  })

  it('leaves out a heading without profiles', async () => {
    const { menu } = await openMenu({ printers: [GENERIC_FDM] })
    expect(within(menu).getByText('FDM')).toBeInTheDocument()
    expect(within(menu).queryByText('Resin')).toBeNull()
  })

  it("checks the document's printer", async () => {
    const { menu } = await openMenu({ current: 'my-mk4' })
    expect(radio(menu, 'My MK4')).toBeChecked()
    expect(radio(menu, 'Generic FDM 0.4 mm')).not.toBeChecked()
    expect(radio(menu, 'Generic resin MSLA')).not.toBeChecked()
    expect(radio(menu, 'Default (Generic FDM 0.4 mm)')).not.toBeChecked()
  })

  it('checks the default item when the document has no printer', async () => {
    const { menu } = await openMenu({ current: null })
    expect(radio(menu, 'Default (Generic FDM 0.4 mm)')).toBeChecked()
    expect(radio(menu, 'Generic FDM 0.4 mm')).not.toBeChecked()
  })

  it('chooses a profile by its id and closes', async () => {
    const { menu, onChoose, onOpenChange } = await openMenu()
    await userEvent.click(radio(menu, 'Generic resin MSLA'))
    expect(onChoose).toHaveBeenCalledTimes(1)
    expect(onChoose).toHaveBeenCalledWith('generic-resin')
    expect(onOpenChange).toHaveBeenLastCalledWith(false)
    expect(screen.queryByRole('menu')).toBeNull()
  })

  it('chooses null for the default item, which clears the printer', async () => {
    const { menu, onChoose } = await openMenu({ current: 'generic-fdm' })
    await userEvent.click(radio(menu, 'Default (Generic FDM 0.4 mm)'))
    expect(onChoose).toHaveBeenCalledWith(null)
  })

  it('focuses the first profile on opening, moves with the arrow keys and chooses with Enter', async () => {
    const { menu, onChoose } = await openMenu()
    expect(radio(menu, 'Generic FDM 0.4 mm')).toHaveFocus()
    await userEvent.keyboard('{ArrowDown}')
    expect(radio(menu, 'My MK4')).toHaveFocus()
    await userEvent.keyboard('{ArrowDown}')
    expect(radio(menu, 'Generic resin MSLA')).toHaveFocus()
    await userEvent.keyboard('{Enter}')
    expect(onChoose).toHaveBeenCalledWith('generic-resin')
  })

  it('closes on Escape and returns focus to its button', async () => {
    const { onChoose, onOpenChange } = await openMenu()
    await userEvent.keyboard('{Escape}')
    expect(onOpenChange).toHaveBeenLastCalledWith(false)
    expect(screen.queryByRole('menu')).toBeNull()
    expect(screen.getByRole('button', { name: 'Printer: none' })).toHaveFocus()
    expect(onChoose).not.toHaveBeenCalled()
  })
})

describe('PrinterMenu keyboard and dismissal', () => {
  it('checks a profile with Space and stays open', async () => {
    const { menu, onChoose } = await openMenu()
    await userEvent.keyboard('{ArrowDown}')
    await userEvent.keyboard(' ')
    expect(onChoose).toHaveBeenCalledWith('my-mk4')
    expect(screen.getByRole('menu')).toBe(menu)
  })

  it('wraps around with the arrow keys and jumps with Home and End', async () => {
    const { menu } = await openMenu()
    await userEvent.keyboard('{ArrowUp}')
    expect(radio(menu, 'Default (Generic FDM 0.4 mm)')).toHaveFocus()
    await userEvent.keyboard('{ArrowDown}')
    expect(radio(menu, 'Generic FDM 0.4 mm')).toHaveFocus()
    await userEvent.keyboard('{End}')
    expect(radio(menu, 'Default (Generic FDM 0.4 mm)')).toHaveFocus()
    await userEvent.keyboard('{Home}')
    expect(radio(menu, 'Generic FDM 0.4 mm')).toHaveFocus()
  })

  it('moves to the next item starting with a typed letter', async () => {
    const { menu } = await openMenu()
    await userEvent.keyboard('m')
    expect(radio(menu, 'My MK4')).toHaveFocus()
    await userEvent.keyboard('d')
    expect(radio(menu, 'Default (Generic FDM 0.4 mm)')).toHaveFocus()
  })

  it('closes on Tab without choosing', async () => {
    const { onChoose, onOpenChange } = await openMenu()
    await userEvent.keyboard('{Tab}')
    expect(onOpenChange).toHaveBeenLastCalledWith(false)
    expect(screen.queryByRole('menu')).toBeNull()
    expect(onChoose).not.toHaveBeenCalled()
  })

  it('closes on a press outside it', async () => {
    const { onChoose, onOpenChange } = await openMenu()
    await userEvent.click(document.body)
    expect(onOpenChange).toHaveBeenLastCalledWith(false)
    expect(screen.queryByRole('menu')).toBeNull()
    expect(onChoose).not.toHaveBeenCalled()
  })

  it('offers only the default item, named by its id, before the profiles are listed', async () => {
    const { menu } = await openMenu({ printers: [] })
    expect(within(menu).getAllByRole('menuitemradio')).toEqual([
      radio(menu, 'Default (generic-fdm)'),
    ])
    expect(within(menu).queryByText('FDM')).toBeNull()
  })
})

// The printer library in the shell (3.17, ADR 0062): the real app over the fake core, with a memory
// library, so a test reads the core's requests, the library and the document as the app leaves them.

vi.mock('../viewport/webgl2/backend', async () => {
  const fakes = await import('../test/fakes/backend')
  return { createWebGl2Backend: () => fakes.createFakeBackend() }
})

const TALL_LAYER =
  'Layer height 0.5 mm is larger than the 0.4 mm nozzle. Use a layer height of 0.4 mm or less.'

/** `cube.ptsm` at format version 3, naming `my-mk4` and storing `MK4`. */
const STORES_MK4: Document = {
  ...CUBE_DOCUMENT,
  format_version: 3,
  printer: 'my-mk4',
  printer_profile: MK4,
}

/** `cube.ptsm` at format version 3 naming `my-printer`, which no session holds (NC13). */
const NAMES_MY_PRINTER: Document = { ...CUBE_DOCUMENT, format_version: 3, printer: 'my-printer' }

/** The app over the fake core, rendered, with its library and the steps tests order. */
type Shell = {
  readonly fake: FakeApp
  readonly library: PrinterLibrary
  readonly user: UserEvent
  /** Library writes and printer requests, in the order they happened, since the shell rendered. */
  readonly steps: string[]
}

/** A library that adds "library.put {id}" and "library.remove {id}" to `steps` as it is called. */
function recordedLibrary(library: PrinterLibrary, steps: string[]): PrinterLibrary {
  return {
    list: () => library.list(),
    put: (profile) => {
      steps.push(`library.put ${profile.id}`)
      return library.put(profile)
    },
    remove: (id) => {
      steps.push(`library.remove ${id}`)
      return library.remove(id)
    },
  }
}

/** The profile the core returns for a put: its written form with a 12 mm bridge, which no test types. */
function returnedProfile(
  command: Extract<Command, { type: 'put_printer_profile' }>,
): PrinterProfile {
  const input = command.profile ?? null
  if (input === null) {
    throw new Error('The printer dialog sends a profile, not text.')
  }
  return { ...writtenProfile(input), max_bridge_length: 12 }
}

/** Records the printer requests in `steps`; puts answer `returnedProfile` after the core keeps the profile. */
function recordRequests(fake: FakeApp, steps: string[]): void {
  fake.transport.handle('put_printer_profile', (command, core) => {
    steps.push('put_printer_profile')
    core.reply(command)
    return { result: { type: 'put_printer_profile', profile: returnedProfile(command) } }
  })
  fake.transport.handle('remove_printer_profile', (command, core) => {
    steps.push('remove_printer_profile')
    return core.reply(command)
  })
  fake.transport.handle('list_printer_profiles', (command, core) => {
    steps.push('list_printer_profiles')
    return core.reply(command)
  })
  fake.transport.handle('apply_edits', (command, core) => {
    steps.push('apply_edits')
    return core.reply(command)
  })
}

let restoreSize: () => void = () => undefined
let frames: FrameQueue | null = null

async function settleShell(): Promise<void> {
  await act(async () => {
    await settle()
  })
  act(() => {
    frames?.flush()
  })
}

/** Opens `document` in the real app over the fake core with `library`, and renders the shell. */
async function openShell(
  document: Document,
  library: PrinterLibrary = createMemoryPrinterLibrary(),
): Promise<Shell> {
  const steps: string[] = []
  const fake = await startWithDocument('cube.ptsm', document, {
    printerLibrary: recordedLibrary(library, steps),
  })
  recordRequests(fake, steps)
  renderWithApp(fake.app)
  await settleShell()
  return { fake, library, user: userEvent.setup(), steps }
}

/** Opens the printer menu from the status bar's printer button. */
async function openPrinterMenu(shell: Shell): Promise<HTMLElement> {
  await shell.user.click(screen.getByTestId('status-printer'))
  return screen.getByTestId('printer-menu')
}

/** Opens the printer dialog with "Add printer…" of the printer menu. */
async function addPrinter(shell: Shell): Promise<HTMLElement> {
  const menu = await openPrinterMenu(shell)
  await shell.user.click(within(menu).getByTestId('printer-add'))
  return screen.getByTestId('printer-dialog')
}

/** A number or text field of the printer dialog, by its label. */
function dialogField(dialog: HTMLElement, label: string): HTMLInputElement {
  return within(dialog).getByRole<HTMLInputElement>('textbox', { name: label })
}

/** Replaces the text of a dialog field. */
async function fill(shell: Shell, dialog: HTMLElement, label: string, text: string): Promise<void> {
  const field = dialogField(dialog, label)
  await shell.user.clear(field)
  await shell.user.type(field, text)
}

/** Clicks Save in the printer dialog and lets what it started settle. */
async function save(shell: Shell, dialog: HTMLElement): Promise<void> {
  await shell.user.click(within(dialog).getByRole('button', { name: 'Save' }))
  await settleShell()
}

/** The profiles the `put_printer_profile` requests carried, in order. */
function putProfiles(fake: FakeApp): unknown[] {
  return fake.transport.commandsOfType('put_printer_profile').map((command) => command.profile)
}

describe('the printer library in the shell (3.17)', () => {
  beforeEach(() => {
    restoreSize = stubElementSize(HTMLCanvasElement.prototype, { width: 800, height: 600 })
    stubMatchMedia({ reducedMotion: true })
    frames = stubAnimationFrames()
  })

  afterEach(() => {
    restoreSize()
    vi.unstubAllGlobals()
    vi.restoreAllMocks()
  })

  it('U-259 lists a stored profile that differs from the listed one first, checked, as "(this document)"', async () => {
    const shell = await openShell(STORES_MK4, await printerLibraryHolding([MK4B]))
    const menu = await openPrinterMenu(shell)
    const stored = radio(menu, 'My MK4 (this document)')
    expect(stored).toBeChecked()
    expect(stored).toHaveAccessibleDescription('FDM 250 × 210 × 220 mm bed · 0.4 mm nozzle')
    expect(within(menu).getAllByRole('menuitemradio').slice(0, 3)).toEqual([
      stored,
      radio(menu, 'Generic FDM 0.4 mm'),
      radio(menu, 'My MK4'),
    ])
    expect(radio(menu, 'My MK4')).not.toBeChecked()
    shell.fake.transport.clearRequests()
    await shell.user.click(radio(menu, 'My MK4'))
    await settleShell()
    expect(labeledEdits(shell.fake)).toEqual([
      { label: 'Change printer', edits: [{ type: 'set_printer', printer: 'my-mk4' }] },
    ])
  })

  it('U-259 checks the listed profile and shows no "(this document)" item when it equals the stored one', async () => {
    const shell = await openShell(STORES_MK4, await printerLibraryHolding([MK4]))
    const menu = await openPrinterMenu(shell)
    expect(within(menu).queryByRole('menuitemradio', { name: /\(this document\)$/ })).toBeNull()
    expect(radio(menu, 'My MK4')).toBeChecked()
    expect(within(menu).getAllByRole('menuitemradio')).toHaveLength(4)
  })

  it('opens the dialog "New printer" with the values of generic-fdm and the name "My printer"', async () => {
    const shell = await openShell(CUBE_DOCUMENT)
    const dialog = await addPrinter(shell)
    expect(dialog).toHaveAttribute('role', 'dialog')
    expect(dialog).toHaveAccessibleName('New printer')
    expect(dialogField(dialog, 'Name')).toHaveValue('My printer')
    expect(dialogField(dialog, 'Layer height')).toHaveValue('0.2')
    expect(dialogField(dialog, 'Nozzle')).toHaveValue('0.4')
    expect(dialogField(dialog, 'Steepest overhang')).toHaveValue('45')
    expect(dialogField(dialog, 'Slip fit')).toHaveValue('0.2')
    expect(within(dialog).getByRole('radio', { name: 'FDM' })).toBeChecked()
    expect(within(dialog).getByRole('radio', { name: 'Rectangle' })).toBeChecked()
  })

  it('names a new printer "My printer 2" while a listed profile is named "My printer"', async () => {
    const mine = { ...MK4, id: 'my-printer', name: 'My printer' }
    const shell = await openShell(CUBE_DOCUMENT, await printerLibraryHolding([mine]))
    const dialog = await addPrinter(shell)
    expect(dialogField(dialog, 'Name')).toHaveValue('My printer 2')
  })

  it('shows field_required for an empty number field and field_not_number for other text, and disables Save', async () => {
    const shell = await openShell(CUBE_DOCUMENT)
    const dialog = await addPrinter(shell)
    const field = dialogField(dialog, 'Layer height')
    await shell.user.clear(field)
    expect(dialog).toHaveTextContent('Layer height needs a value.')
    expect(within(dialog).getByRole('button', { name: 'Save' })).toBeDisabled()
    await shell.user.type(field, 'abc')
    expect(dialog).toHaveTextContent('Layer height must be a number.')
    expect(within(dialog).getByRole('button', { name: 'Save' })).toBeDisabled()
  })

  it('U-261 keeps what was not edited, takes the overhang in degrees, and a technology switch takes its generic values', async () => {
    const shell = await openShell(CUBE_DOCUMENT)
    let dialog = await addPrinter(shell)
    await fill(shell, dialog, 'Name', 'My MK4')
    await fill(shell, dialog, 'Layer height', '0.15')
    await save(shell, dialog)
    expect(putProfiles(shell.fake).at(-1)).toMatchObject({
      max_overhang_angle: 0.7853981633974483,
      nozzle_diameter: 0.4,
      layer_height: 0.15,
    })

    dialog = await addPrinter(shell)
    await fill(shell, dialog, 'Name', 'Steep')
    await fill(shell, dialog, 'Steepest overhang', '50')
    await save(shell, dialog)
    expect(putProfiles(shell.fake).at(-1)).toMatchObject({
      max_overhang_angle: (50 * Math.PI) / 180,
    })

    dialog = await addPrinter(shell)
    await fill(shell, dialog, 'Name', 'My resin')
    await shell.user.click(within(dialog).getByRole('radio', { name: 'Resin' }))
    expect(dialogField(dialog, 'Name')).toHaveValue('My resin')
    await save(shell, dialog)
    const resin = putProfiles(shell.fake).at(-1)
    expect(resin).not.toHaveProperty('nozzle_diameter')
    expect(resin).toMatchObject({
      name: 'My resin',
      technology: 'resin',
      bed: GENERIC_RESIN.bed,
      layer_height: GENERIC_RESIN.layer_height,
      max_overhang_angle: GENERIC_RESIN.max_overhang_angle,
      max_bridge_length: GENERIC_RESIN.max_bridge_length,
      fit: GENERIC_RESIN.fit,
    })
  })

  it('U-262 Save puts the profile, keeps what the core returned, lists the profiles, then picks it', async () => {
    const shell = await openShell(CUBE_DOCUMENT)
    const dialog = await addPrinter(shell)
    await fill(shell, dialog, 'Name', 'My MK4')
    await fill(shell, dialog, 'Layer height', '0.15')
    await fill(shell, dialog, 'Slip fit', '0.15')
    await fill(shell, dialog, 'Loose fit', '0.3')
    shell.fake.transport.clearRequests()
    await save(shell, dialog)
    const [sent] = shell.fake.transport.commandsOfType('put_printer_profile')
    expect(sent?.profile).toMatchObject({
      id: 'my-mk4',
      name: 'My MK4',
      layer_height: 0.15,
      fit: { slip: 0.15, loose: 0.3 },
    })
    expect(shell.steps).toEqual([
      'put_printer_profile',
      'library.put my-mk4',
      'list_printer_profiles',
      'apply_edits',
    ])
    if (sent === undefined) {
      throw new Error('No put_printer_profile was sent.')
    }
    await expect(shell.library.list()).resolves.toEqual([returnedProfile(sent)])
    expect(labeledEdits(shell.fake)).toEqual([
      { label: 'Change printer', edits: [{ type: 'set_printer', printer: 'my-mk4' }] },
    ])
    expect(screen.queryByTestId('printer-dialog')).toBeNull()
  })

  it('U-263 a profile the core refuses keeps the dialog open with its message and the typed values', async () => {
    const shell = await openShell(CUBE_DOCUMENT)
    shell.fake.transport.handle('put_printer_profile', () => ({
      error: { code: 'invalid_profile', message: TALL_LAYER },
    }))
    const dialog = await addPrinter(shell)
    await fill(shell, dialog, 'Name', 'My MK4')
    await fill(shell, dialog, 'Layer height', '0.5')
    shell.fake.transport.clearRequests()
    await save(shell, dialog)
    expect(within(dialog).getByTestId('printer-dialog-error')).toHaveTextContent(TALL_LAYER)
    expect(screen.getByTestId('printer-dialog')).toBe(dialog)
    expect(labeledEdits(shell.fake)).toEqual([])
    await expect(shell.library.list()).resolves.toEqual([])
    expect(dialogField(dialog, 'Name')).toHaveValue('My MK4')
    expect(dialogField(dialog, 'Layer height')).toHaveValue('0.5')
  })

  it('U-293 a library that does not keep the printer warns printer_not_kept and still picks it', async () => {
    const memory = createMemoryPrinterLibrary()
    const shell = await openShell(CUBE_DOCUMENT, { ...memory, put: () => Promise.resolve(false) })
    const dialog = await addPrinter(shell)
    await fill(shell, dialog, 'Name', 'My MK4')
    shell.fake.transport.clearRequests()
    await save(shell, dialog)
    const warning = notificationCoded(shell.fake.app, 'printer_not_kept')
    expect(warning.severity).toBe('warning')
    expect(warning.message).toBe(
      "'My MK4' could not be kept among your printers. This document stores it, and new documents can use it until Partsmith reloads.",
    )
    expect(labeledEdits(shell.fake)).toEqual([
      { label: 'Change printer', edits: [{ type: 'set_printer', printer: 'my-mk4' }] },
    ])
  })

  it('U-291 Edit printer puts the new values under the same id, keeps them and picks the printer again', async () => {
    const shell = await openShell(STORES_MK4, await printerLibraryHolding([MK4]))
    const menu = await openPrinterMenu(shell)
    await shell.user.click(within(menu).getByTestId('printer-edit'))
    const dialog = screen.getByTestId('printer-dialog')
    expect(dialog).toHaveAccessibleName('Edit printer')
    expect(dialogField(dialog, 'Name')).toHaveValue('My MK4')
    await fill(shell, dialog, 'Slip fit', '0.3')
    await fill(shell, dialog, 'Loose fit', '0.4')
    shell.fake.transport.clearRequests()
    await save(shell, dialog)
    const [sent] = shell.fake.transport.commandsOfType('put_printer_profile')
    expect(sent?.profile).toMatchObject({ id: 'my-mk4', fit: { slip: 0.3, loose: 0.4 } })
    expect(shell.steps).toEqual([
      'put_printer_profile',
      'library.put my-mk4',
      'list_printer_profiles',
      'apply_edits',
    ])
    if (sent === undefined) {
      throw new Error('No put_printer_profile was sent.')
    }
    await expect(shell.library.list()).resolves.toEqual([returnedProfile(sent)])
    expect(labeledEdits(shell.fake)).toEqual([
      { label: 'Change printer', edits: [{ type: 'set_printer', printer: 'my-mk4' }] },
    ])
  })

  it('U-291 names the library profile in effect in "Edit \'My MK4\'…"', async () => {
    const shell = await openShell(STORES_MK4, await printerLibraryHolding([MK4]))
    const menu = await openPrinterMenu(shell)
    expect(within(menu).getByTestId('printer-edit')).toHaveAccessibleName("Edit 'My MK4'…")
    expect(within(menu).getByTestId('printer-add')).toHaveAccessibleName('Add printer…')
  })

  it('offers no Edit item while the printer in effect is a built-in profile', async () => {
    const shell = await openShell(CUBE_DOCUMENT)
    const menu = await openPrinterMenu(shell)
    expect(within(menu).queryByTestId('printer-edit')).toBeNull()
  })

  it('U-292 Delete printer removes it from the library and the session, and the document keeps its copy', async () => {
    const shell = await openShell(STORES_MK4, await printerLibraryHolding([MK4]))
    const menu = await openPrinterMenu(shell)
    await shell.user.click(within(menu).getByTestId('printer-edit'))
    const dialog = screen.getByTestId('printer-dialog')
    shell.fake.transport.clearRequests()
    await shell.user.click(within(dialog).getByTestId('printer-delete'))
    await settleShell()
    expect(shell.steps).toEqual(
      expect.arrayContaining(['library.remove my-mk4', 'remove_printer_profile']),
    )
    expect(
      shell.fake.transport.commandsOfType('remove_printer_profile').map((command) => command.id),
    ).toEqual(['my-mk4'])
    await expect(shell.library.list()).resolves.toEqual([])
    expect(screen.queryByTestId('printer-dialog')).toBeNull()
    expect(labeledEdits(shell.fake)).toEqual([])
    const recipe = currentDocument(shell.fake.app).recipe
    expect(recipe.printer).toBe('my-mk4')
    expect(recipe.printer_profile).toEqual(MK4)
  })

  it('U-265 the unknown printer row offers "Choose printer" and "Add printer \'my-printer\'"', async () => {
    const shell = await openShell(NAMES_MY_PRINTER)
    act(() => {
      openSidePanel(shell.fake.app, 'problems')
    })
    const panel = screen.getByTestId('problems-panel')
    expect(panel).toHaveTextContent(unknownPrinterMessage('my-printer'))
    await shell.user.click(within(panel).getByRole('button', { name: 'Choose printer' }))
    expect(screen.getByTestId('printer-menu')).toBeVisible()
    await shell.user.keyboard('{Escape}')
    await shell.user.click(within(panel).getByRole('button', { name: "Add printer 'my-printer'" }))
    const dialog = screen.getByTestId('printer-dialog')
    expect(dialog).toHaveAccessibleName('New printer')
    expect(dialogField(dialog, 'Name')).toHaveValue('my-printer')
    await fill(shell, dialog, 'Name', 'Mine')
    shell.fake.transport.clearRequests()
    await save(shell, dialog)
    expect(putProfiles(shell.fake)).toEqual([
      expect.objectContaining({ id: 'my-printer', name: 'Mine' }),
    ])
    expect(labeledEdits(shell.fake)).toEqual([
      { label: 'Change printer', edits: [{ type: 'set_printer', printer: 'my-printer' }] },
    ])
    expect(screen.getByTestId('problems-panel')).not.toHaveTextContent(
      unknownPrinterMessage('my-printer'),
    )
  })
})
