import { CoreError } from '../../core/client'
import type { Document, PrinterProfile } from '../../generated/protocol'
import { MESSAGES } from '../../messages'
import { libraryOffer } from '../../print/library'
import type { PrinterDialogRequest } from '../../state/store'
import type { App } from '../createApp'
import { notify } from './notifications'
import { commitPrinter } from './print'

// The printer library's actions (ui.md 3.17, ADR 0062): the printer menu and dialog, the dialog's
// Save and Delete, and the offers a document that stores a profile gets when it opens.

/** Opens the status bar's printer menu, or closes it (3.17). */
export function setPrinterMenuOpen(app: App, open: boolean): void {
  app.store.setState({ printerMenuOpen: open })
}

/** Opens the printer dialog for `request`, or closes it with null (3.17). */
export function setPrinterDialog(app: App, request: PrinterDialogRequest | null): void {
  app.store.setState({ printerDialog: request })
}

/**
 * Puts a profile into the session and the library, then lists the session's profiles again (3.17);
 * resolves the profile the core returned. A library that does not keep it warns `printer_not_kept`.
 */
async function keepPrinter(app: App, profile: PrinterProfile): Promise<PrinterProfile> {
  const kept = await app.mutations.run(() => app.core.putPrinterProfile(profile), 'other')
  if (!(await app.printerLibrary.put(kept))) {
    notify(app, {
      severity: 'warning',
      message: MESSAGES.printer_not_kept(kept.name),
      code: 'printer_not_kept',
      actions: [],
    })
  }
  await refreshPrinters(app)
  return kept
}

/** Lists the session's profiles again into the store (3.17). */
async function refreshPrinters(app: App): Promise<void> {
  const printers = await app.mutations.run(() => app.core.listPrinterProfiles(), 'other')
  app.store.setState({ printers })
}

/**
 * The printer dialog's Save (3.17): keeps the profile and picks it for the document, which then
 * stores it. Resolves the core's message when it refuses the profile, which the dialog shows, else
 * null.
 */
export async function savePrinter(app: App, profile: PrinterProfile): Promise<string | null> {
  let kept: PrinterProfile
  try {
    kept = await keepPrinter(app, profile)
  } catch (error) {
    if (error instanceof CoreError) {
      return error.message
    }
    throw error
  }
  setPrinterDialog(app, null)
  await commitPrinter(app, kept.id)
  return null
}

/**
 * The printer dialog's "Delete printer" (3.17): removes the profile from the library and the
 * session. The document keeps the copy it stores, and its printer stays.
 */
export async function deletePrinter(app: App, id: string): Promise<void> {
  setPrinterDialog(app, null)
  await app.printerLibrary.remove(id)
  await app.mutations.run(() => app.core.removePrinterProfile(id), 'other')
  await refreshPrinters(app)
}

/**
 * Offers what the profile an opened document stores calls for (3.17): adding it to the library
 * when no listed profile has its id, or using the listed one when that one has other values.
 */
export function offerStoredPrinter(app: App, fileName: string, recipe: Document): void {
  const offer = libraryOffer(recipe, app.store.getState().printers)
  if (offer === null) {
    return
  }
  const { name, id } = offer.profile
  if (offer.kind === 'add') {
    notify(app, {
      severity: 'info',
      message: MESSAGES.printer_offer_add(fileName, name),
      code: 'printer_offer_add',
      actions: [
        {
          label: `Add '${name}' to my printers`,
          run: () => {
            void addOfferedPrinter(app, offer.profile)
          },
        },
      ],
    })
    return
  }
  notify(app, {
    severity: 'info',
    message: MESSAGES.printer_offer_differs(fileName, name),
    code: 'printer_offer_differs',
    actions: [
      {
        label: `Use my '${name}'`,
        editsDocument: true,
        run: () => {
          void commitPrinter(app, id)
        },
      },
    ],
  })
}

/** "Add '{name}' to my printers": keeps the stored profile without changing the document (3.17). */
async function addOfferedPrinter(app: App, profile: PrinterProfile): Promise<void> {
  try {
    await keepPrinter(app, profile)
  } catch (error) {
    if (!(error instanceof CoreError)) {
      throw error
    }
    notify(app, { severity: 'warning', message: error.message, code: error.code, actions: [] })
    return
  }
  notify(app, {
    severity: 'success',
    message: MESSAGES.printer_added(profile.name),
    code: 'printer_added',
    actions: [],
  })
}
