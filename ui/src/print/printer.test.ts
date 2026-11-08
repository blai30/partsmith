import { describe, expect, it } from 'vitest'
import type { Document } from '../generated/protocol'
import { GENERIC_FDM, GENERIC_RESIN, MK4, MK4B, TEST_PRINTERS } from '../test/fakes/printers'
import {
  CUBE,
  MY_MK4,
  PRINTERS,
  bodyPrint,
  documentState,
  printReport,
  printState,
  recipe,
  stateWith,
} from './fixtures.testing'
import {
  defaultPrinterName,
  documentPrinter,
  editablePrinter,
  findPrinter,
  healthProfileName,
  numberFieldProblem,
  printerButtonText,
  printerDetail,
  printerForm,
  printerFromForm,
  printerGroups,
  printerIdFromName,
  printerLabel,
  printerName,
  reportPrinters,
} from './printer'

/** The cube recipe naming `my-mk4` and storing `MK4` (U-258). */
const STORES_MK4: Document = { ...recipe([CUBE], 'my-mk4'), printer_profile: MK4 }

describe('printerGroups', () => {
  it('groups the profiles by technology, FDM first, keeping their order', () => {
    expect(printerGroups(PRINTERS)).toEqual([
      { technology: 'fdm', title: 'FDM', printers: [GENERIC_FDM, MY_MK4] },
      { technology: 'resin', title: 'Resin', printers: [GENERIC_RESIN] },
    ])
  })

  it('leaves out a technology without profiles', () => {
    expect(printerGroups([GENERIC_RESIN]).map((group) => group.title)).toEqual(['Resin'])
    expect(printerGroups([])).toEqual([])
  })
})

describe('findPrinter', () => {
  it('finds an installed profile by id', () => {
    expect(findPrinter(PRINTERS, 'my-mk4')).toBe(MY_MK4)
    expect(findPrinter(PRINTERS, 'retired-printer')).toBeUndefined()
    expect(findPrinter(PRINTERS, null)).toBeUndefined()
  })
})

describe('printerName', () => {
  it('names a profile, or shows the id of one that is not installed', () => {
    expect(printerName(PRINTERS, 'my-mk4')).toBe('My MK4')
    expect(printerName(PRINTERS, 'retired-printer')).toBe('retired-printer')
  })
})

describe('documentPrinter (3.17)', () => {
  const printers = [...TEST_PRINTERS, MK4B]

  it('U-258 is the stored profile, else the listed profile with the id, else generic-fdm', () => {
    expect(documentPrinter(STORES_MK4, printers)).toEqual(MK4)
    expect(documentPrinter(recipe([CUBE], 'my-mk4'), printers)).toBe(MK4B)
    expect(documentPrinter(recipe([CUBE], 'nope'), printers)).toBe(GENERIC_FDM)
    expect(printerButtonText(STORES_MK4, printers)).toBe('Printer: My MK4')
  })

  it('is the named profile, else the generic FDM profile', () => {
    expect(documentPrinter(recipe([CUBE], 'generic-resin'), PRINTERS)).toBe(GENERIC_RESIN)
    expect(documentPrinter(recipe([CUBE]), PRINTERS)).toBe(GENERIC_FDM)
  })

  it('uses the stored profile before a listed profile with its id', () => {
    expect(documentPrinter(STORES_MK4, [...TEST_PRINTERS, MY_MK4])).toEqual(MK4)
  })
})

describe('printerIdFromName (3.17)', () => {
  it('U-260 writes the name in lowercase with runs of other characters as one dash', () => {
    expect(printerIdFromName('My MK4', [])).toBe('my-mk4')
    expect(printerIdFromName('Prusa MK4 0.4 mm', [])).toBe('prusa-mk4-0-4-mm')
  })

  it('U-260 appends the first free number while the id is taken', () => {
    expect(printerIdFromName('My MK4', ['my-mk4', 'my-mk4-2'])).toBe('my-mk4-3')
  })

  it('U-260 gives printer when nothing is left, and cuts a long name to 56 characters', () => {
    expect(printerIdFromName('Ößé', [])).toBe('printer')
    expect(printerIdFromName('a'.repeat(80), [])).toBe('a'.repeat(56))
  })

  it('trims dashes from both ends', () => {
    expect(printerIdFromName('  (My MK4)  ', [])).toBe('my-mk4')
  })
})

describe('healthProfileName', () => {
  it("names the report's profile", () => {
    const report = printReport([bodyPrint({ body: CUBE.id, meshId: 1 })], 'generic-resin')
    expect(
      healthProfileName(
        stateWith({
          document: documentState({ printer: 'my-mk4' }),
          print: printState({ report }),
        }),
      ),
    ).toBe('Generic resin MSLA')
  })

  it('names the profile the next analysis uses before there is a report', () => {
    expect(healthProfileName(stateWith({ document: documentState({ printer: 'my-mk4' }) }))).toBe(
      'My MK4',
    )
    expect(healthProfileName(stateWith({ document: documentState() }))).toBe('Generic FDM 0.4 mm')
  })

  it('names the profile the document stores before there is a report', () => {
    const stored = { ...documentState(), recipe: STORES_MK4 }
    expect(healthProfileName(stateWith({ printers: TEST_PRINTERS, document: stored }))).toBe(
      'My MK4',
    )
  })

  it('shows the id of the profile the next analysis uses when no profile is listed', () => {
    const printer = documentState({ printer: 'my-mk4' })
    expect(healthProfileName(stateWith({ printers: [], document: printer }))).toBe('my-mk4')
    expect(healthProfileName(stateWith({ printers: [], document: documentState() }))).toBe(
      'generic-fdm',
    )
  })
})

describe('printerDetail', () => {
  it('U-121 gives generic-fdm "256 × 256 × 256 mm bed · 0.4 mm nozzle" and a round bed its diameter', () => {
    expect(printerDetail(GENERIC_FDM)).toBe('256 × 256 × 256 mm bed · 0.4 mm nozzle')
    const round = { ...GENERIC_FDM, bed: { kind: 'circle', diameter: 180, height: 180 } } as const
    expect(printerDetail(round)).toBe('⌀180 × 180 mm bed · 0.4 mm nozzle')
  })

  it('gives an FDM profile its bed and nozzle', () => {
    expect(printerDetail(GENERIC_FDM)).toBe('256 × 256 × 256 mm bed · 0.4 mm nozzle')
    expect(printerDetail(MY_MK4)).toBe('250 × 210 × 220 mm bed · 0.4 mm nozzle')
  })

  it('gives a round FDM bed its diameter and height', () => {
    const delta = {
      ...GENERIC_FDM,
      bed: { kind: 'circle', diameter: 170, height: 240 },
      nozzle_diameter: 0.6,
    } as const
    expect(printerDetail(delta)).toBe('⌀170 × 240 mm bed · 0.6 mm nozzle')
  })

  it('gives a resin profile its build volume only', () => {
    expect(printerDetail(GENERIC_RESIN)).toBe('130 × 80 × 160 mm')
  })

  it('leaves out a nozzle the profile does not have', () => {
    expect(printerDetail({ ...GENERIC_FDM, nozzle_diameter: null })).toBe('256 × 256 × 256 mm bed')
  })

  it('writes numbers with formatNumber in mm', () => {
    const fine = {
      ...GENERIC_FDM,
      bed: { kind: 'rectangle', width: 180.25, depth: 180, height: 180 },
      nozzle_diameter: 0.25,
    } as const
    expect(printerDetail(fine)).toBe('180.25 × 180 × 180 mm bed · 0.25 mm nozzle')
  })
})

describe('printerLabel and printerButtonText', () => {
  it('U-121 printerButtonText for a document without a printer is "Printer: Generic FDM 0.4 mm (default)"', () => {
    expect(printerButtonText(recipe([CUBE]), PRINTERS)).toBe(
      'Printer: Generic FDM 0.4 mm (default)',
    )
  })

  it("name the document's printer", () => {
    expect(printerLabel(recipe([CUBE], 'my-mk4'), PRINTERS)).toBe('My MK4')
    expect(printerButtonText(recipe([CUBE], 'my-mk4'), PRINTERS)).toBe('Printer: My MK4')
  })

  it('name the profile the document stores', () => {
    expect(printerLabel(STORES_MK4, TEST_PRINTERS)).toBe('My MK4')
  })

  it('name the default profile, marked as the default, when the document has no printer or none is open', () => {
    expect(printerLabel(recipe([CUBE]), PRINTERS)).toBe('Generic FDM 0.4 mm (default)')
    expect(printerLabel(null, PRINTERS)).toBe('Generic FDM 0.4 mm (default)')
    expect(printerButtonText(null, PRINTERS)).toBe('Printer: Generic FDM 0.4 mm (default)')
  })

  it('show the id of a printer that is not installed', () => {
    expect(printerButtonText(recipe([CUBE], 'retired-printer'), PRINTERS)).toBe(
      'Printer: retired-printer',
    )
  })

  it('show the default id before the profiles are listed', () => {
    expect(defaultPrinterName([])).toBe('generic-fdm')
    expect(printerButtonText(recipe([CUBE]), [])).toBe('Printer: generic-fdm (default)')
  })
})

describe('reportPrinters (3.17)', () => {
  it('puts the stored profile in place of the listed one with its id', () => {
    expect(reportPrinters(MK4, [...TEST_PRINTERS, MK4B])).toEqual([MK4, GENERIC_FDM, GENERIC_RESIN])
    expect(reportPrinters(null, TEST_PRINTERS)).toBe(TEST_PRINTERS)
  })
})

describe('editablePrinter (3.17)', () => {
  it('is the listed profile the document names, but never a built-in one', () => {
    expect(editablePrinter(recipe([CUBE], 'my-mk4'), [...TEST_PRINTERS, MK4B])).toBe(MK4B)
    expect(editablePrinter(recipe([CUBE], 'generic-resin'), TEST_PRINTERS)).toBeNull()
    expect(editablePrinter(recipe([CUBE]), TEST_PRINTERS)).toBeNull()
    expect(editablePrinter(null, TEST_PRINTERS)).toBeNull()
  })
})

describe('the printer dialog fields (3.17)', () => {
  it('take plain decimal numbers only', () => {
    expect(numberFieldProblem(' 0.4 ')).toBeNull()
    expect(numberFieldProblem('-0.05')).toBeNull()
    expect(numberFieldProblem('.5')).toBeNull()
    expect(numberFieldProblem('')).toBe('field_required')
    expect(numberFieldProblem('2 mm')).toBe('field_not_number')
    expect(numberFieldProblem('1e3')).toBe('field_not_number')
  })

  it('give back the profile they opened with when nothing was edited', () => {
    expect(printerFromForm(printerForm(MK4), MK4, 'my-mk4')).toEqual(MK4)
    expect(printerFromForm(printerForm(GENERIC_RESIN), GENERIC_RESIN, 'r')).toEqual({
      ...GENERIC_RESIN,
      id: 'r',
    })
  })

  it('start a round bed from the width of a rectangle', () => {
    const form = { ...printerForm(MK4), bed: 'circle' as const }
    expect(printerFromForm(form, MK4, 'my-mk4').bed).toEqual({
      kind: 'circle',
      diameter: 250,
      height: 220,
    })
  })
})
