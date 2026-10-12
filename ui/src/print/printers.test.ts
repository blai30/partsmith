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
import { healthProfileName, printerGroups, printerName, resolvedPrinter } from './printers'

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
