import { cleanup, render, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { createRef } from 'react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { installPopoverPolyfill } from '../components/popover-polyfill.testing'
import type { SectionPlane } from './section'
import { SectionPopover, type SectionSettings } from './SectionPopover'

let uninstallPopovers: () => void

beforeEach(() => {
  uninstallPopovers = installPopoverPolyfill()
})

afterEach(() => {
  cleanup()
  uninstallPopovers()
})

function setUp(open: boolean, settings: SectionSettings = { plane: 'XY', flipped: false }) {
  const onPlaneChange = vi.fn<(plane: SectionPlane) => void>()
  const onFlippedChange = vi.fn<(flipped: boolean) => void>()
  const onOpenChange = vi.fn<(open: boolean) => void>()
  render(
    <SectionPopover
      open={open}
      anchor={createRef<HTMLElement>()}
      onOpenChange={onOpenChange}
      settings={settings}
      onPlaneChange={onPlaneChange}
      onFlippedChange={onFlippedChange}
      offsetField={<input aria-label="Offset" defaultValue="10" />}
    />,
  )
  return { onPlaneChange, onFlippedChange, onOpenChange }
}

describe('SectionPopover', () => {
  it('is hidden while closed', () => {
    setUp(false)
    expect(screen.queryByRole('dialog')).toBeNull()
  })

  it('is a dialog named "Section view" holding the section-popover content', () => {
    setUp(true)
    const dialog = screen.getByRole('dialog', { name: 'Section view' })
    expect(within(dialog).getByTestId('section-popover')).toBeInTheDocument()
  })

  it('offers the planes XY, XZ and YZ with the current one checked', () => {
    setUp(true, { plane: 'XZ', flipped: false })
    const plane = screen.getByRole('radiogroup', { name: 'Plane' })
    const radios = within(plane).getAllByRole('radio')
    expect(radios).toEqual(
      ['XY', 'XZ', 'YZ'].map((name) => within(plane).getByRole('radio', { name })),
    )
    expect(within(plane).getByRole('radio', { name: 'XZ' })).toBeChecked()
  })

  it('reports a plane choice', async () => {
    const { onPlaneChange } = setUp(true)
    await userEvent.click(screen.getByRole('radio', { name: 'YZ' }))
    expect(onPlaneChange).toHaveBeenCalledWith('YZ')
  })

  it('shows the Offset field between Plane and Flip', () => {
    setUp(true)
    const content = screen.getByTestId('section-popover')
    const offset = within(content).getByRole('textbox', { name: 'Offset' })
    const plane = within(content).getByRole('radiogroup', { name: 'Plane' })
    const flip = within(content).getByRole('switch', { name: 'Flip' })
    expect(plane.compareDocumentPosition(offset) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy()
    expect(offset.compareDocumentPosition(flip) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy()
  })

  it('reflects and reports Flip', async () => {
    const { onFlippedChange } = setUp(true, { plane: 'XY', flipped: true })
    const flip = screen.getByRole('switch', { name: 'Flip' })
    expect(flip).toBeChecked()
    await userEvent.click(flip)
    expect(onFlippedChange).toHaveBeenCalledWith(false)
  })

  it('moves focus into the popover when it opens, on the checked plane', () => {
    setUp(true)
    expect(screen.getByRole('radio', { name: 'XY' })).toHaveFocus()
  })
})
