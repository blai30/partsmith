import { render, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { createRef } from 'react'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { installPopoverPolyfill } from '../components/popover-polyfill.testing'
import { ChecksPopover } from './ChecksPopover'

beforeEach(installPopoverPolyfill)

// The switches the user can turn off, in display order (ui.md 3.17).
const SWITCHABLE: readonly { readonly title: string; readonly check: string }[] = [
  { title: 'Overhangs', check: 'overhangs' },
  { title: 'Bed fit', check: 'bed_fit' },
  { title: 'First layer contact', check: 'first_layer_contact' },
  { title: 'Thin walls', check: 'thin_walls' },
  { title: 'Small features', check: 'small_features' },
  { title: 'Islands', check: 'islands' },
  { title: 'Trapped volumes', check: 'trapped_volumes' },
]

function setUp(open: boolean, hiddenChecks: readonly string[] = []) {
  const onHiddenChange = vi.fn<(check: string, hidden: boolean) => void>()
  const onOpenChange = vi.fn<(open: boolean) => void>()
  render(
    <ChecksPopover
      open={open}
      anchor={createRef<HTMLElement>()}
      onOpenChange={onOpenChange}
      hiddenChecks={hiddenChecks}
      onHiddenChange={onHiddenChange}
    />,
  )
  return { onHiddenChange, onOpenChange }
}

function checkSwitch(name: string): HTMLElement {
  return within(screen.getByTestId('checks-popover')).getByRole('switch', { name })
}

describe('ChecksPopover', () => {
  it('is hidden while closed', () => {
    setUp(false)
    expect(screen.queryByRole('dialog')).toBeNull()
  })

  it('is a dialog named "Checks" holding the checks-popover content', () => {
    setUp(true)
    const dialog = screen.getByRole('dialog', { name: 'Checks' })
    expect(within(dialog).getByTestId('checks-popover')).toBeInTheDocument()
  })

  it('has one switch per check in the order and wording of the spec, Mesh validity last', () => {
    setUp(true)
    const switches = within(screen.getByTestId('checks-popover')).getAllByRole('switch')
    expect(switches).toEqual([
      ...SWITCHABLE.map(({ title }) => checkSwitch(title)),
      checkSwitch('Mesh validity'),
    ])
  })

  it('shows a check as on unless it is hidden', () => {
    setUp(true, ['islands', 'thin_walls'])
    expect(checkSwitch('Islands')).not.toBeChecked()
    expect(checkSwitch('Thin walls')).not.toBeChecked()
    expect(checkSwitch('Overhangs')).toBeChecked()
    expect(checkSwitch('Bed fit')).toBeChecked()
    expect(checkSwitch('Trapped volumes')).toBeChecked()
  })

  it.each(SWITCHABLE)('switching $title off hides $check', async ({ title, check }) => {
    const { onHiddenChange } = setUp(true)
    await userEvent.click(checkSwitch(title))
    expect(onHiddenChange).toHaveBeenCalledTimes(1)
    expect(onHiddenChange).toHaveBeenCalledWith(check, true)
  })

  it('switching a hidden check on shows it again', async () => {
    const { onHiddenChange } = setUp(true, ['islands'])
    await userEvent.click(checkSwitch('Islands'))
    expect(onHiddenChange).toHaveBeenCalledWith('islands', false)
  })

  it('keeps Mesh validity on and disabled, with the help "Always checked before export"', () => {
    setUp(true)
    const meshValidity = checkSwitch('Mesh validity')
    expect(meshValidity).toBeChecked()
    expect(meshValidity).toBeDisabled()
    expect(meshValidity).toHaveAccessibleDescription('Always checked before export')
  })

  it('keeps Mesh validity on even when the settings list it as hidden', async () => {
    const { onHiddenChange } = setUp(true, ['mesh_validity'])
    const meshValidity = checkSwitch('Mesh validity')
    expect(meshValidity).toBeChecked()
    await userEvent.click(meshValidity)
    expect(onHiddenChange).not.toHaveBeenCalled()
  })
})
