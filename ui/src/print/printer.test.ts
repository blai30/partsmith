import { describe, expect, it } from 'vitest'
import { GENERIC_FDM, GENERIC_RESIN } from '../test/fakes/printers'
import {
  CUBE,
  MY_MK4,
  PRINTERS,
  bodyPrint,
  documentState,
  printReport,
  printState,
  stateWith,
} from './fixtures.testing'
import {
  defaultPrinterName,
  healthProfileName,
  printerButtonText,
  printerDetail,
  printerGroups,
  printerLabel,
  printerName,
  resolvedPrinter,
} from './printer'

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

describe('printerName', () => {
  it('names a profile, or shows the id of one that is not installed', () => {
    expect(printerName(PRINTERS, 'my-mk4')).toBe('My MK4')
    expect(printerName(PRINTERS, 'retired-printer')).toBe('retired-printer')
  })
})

describe('resolvedPrinter', () => {
  it('is the named profile, else the generic FDM profile', () => {
    expect(resolvedPrinter(PRINTERS, 'generic-resin')).toBe(GENERIC_RESIN)
    expect(resolvedPrinter(PRINTERS, 'retired-printer')).toBe(GENERIC_FDM)
    expect(resolvedPrinter(PRINTERS, undefined)).toBe(GENERIC_FDM)
  })

  it('is null without the generic FDM profile', () => {
    expect(resolvedPrinter([MY_MK4], null)).toBeNull()
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

  it('leaves out values the profile does not have', () => {
    expect(printerDetail({ ...GENERIC_FDM, bed: null })).toBe('0.4 mm nozzle')
    expect(printerDetail({ ...GENERIC_FDM, nozzle_diameter: null })).toBe('256 × 256 × 256 mm bed')
    expect(printerDetail({ ...GENERIC_RESIN, bed: null })).toBe('')
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
    expect(printerButtonText(PRINTERS, null)).toBe('Printer: Generic FDM 0.4 mm (default)')
  })

  it("name the document's printer", () => {
    expect(printerLabel(PRINTERS, 'my-mk4')).toBe('My MK4')
    expect(printerButtonText(PRINTERS, 'my-mk4')).toBe('Printer: My MK4')
  })

  it('name the default profile, marked as the default, when the document has no printer', () => {
    expect(printerLabel(PRINTERS, null)).toBe('Generic FDM 0.4 mm (default)')
    expect(printerLabel(PRINTERS, undefined)).toBe('Generic FDM 0.4 mm (default)')
    expect(printerButtonText(PRINTERS, null)).toBe('Printer: Generic FDM 0.4 mm (default)')
  })

  it('show the id of a printer that is not installed', () => {
    expect(printerButtonText(PRINTERS, 'retired-printer')).toBe('Printer: retired-printer')
  })

  it('show the default id before the profiles are listed', () => {
    expect(defaultPrinterName([])).toBe('generic-fdm')
    expect(printerButtonText([], null)).toBe('Printer: generic-fdm (default)')
  })
})
