import { render, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { installPopoverPolyfill } from '../components/popover-polyfill.testing'
import { COLOR_CLASSES } from '../theme/roles'
import type { SectionPlane } from './section'
import type { SectionSettings } from './SectionPopover'
import {
  ViewportControls,
  type SelectionFilter,
  type SelectionFilterKind,
  type ViewportControlsProps,
} from './ViewportControls'

beforeEach(installPopoverPolyfill)

const DEFAULT_FILTER: SelectionFilter = { body: false, face: true, edge: true, vertex: true }
const SECTION_ON: SectionSettings = { plane: 'XY', flipped: false }

function handlers() {
  return {
    onFit: vi.fn<() => void>(),
    onToggleProjection: vi.fn<() => void>(),
    onToggleSection: vi.fn<() => void>(),
    onToggleFilter: vi.fn<(kind: SelectionFilterKind) => void>(),
    onSectionPlaneChange: vi.fn<(plane: SectionPlane) => void>(),
    onSectionFlippedChange: vi.fn<(flipped: boolean) => void>(),
  }
}

function setUp(overrides: Partial<ViewportControlsProps> = {}) {
  const on = handlers()
  const props: ViewportControlsProps = {
    orthographic: false,
    section: null,
    filter: DEFAULT_FILTER,
    sectionOffsetField: <input aria-label="Offset" defaultValue="0" />,
    ...on,
    ...overrides,
  }
  const view = render(<ViewportControls {...props} />)
  const rerender = (next: Partial<ViewportControlsProps>): void => {
    view.rerender(<ViewportControls {...props} {...next} />)
  }
  return { on, rerender, controls: screen.getByTestId('viewport-controls') }
}

function button(name: string): HTMLElement {
  return screen.getByRole('button', { name })
}

describe('ViewportControls layout', () => {
  it('sits under the view cube, below the reach of its corners in every orientation', () => {
    const { controls } = setUp()
    const box = controls.parentElement
    // Centered under the cube, which is as wide and as far from the right edge.
    expect(box).toHaveClass('absolute', 'right-6', 'w-24')
    const top = /\btop-(\d+)\b/.exec(box?.className ?? '')?.[1]
    // The 6rem cube at top-8 is centered 5rem down; its corners turn up to 3√3 rem from the center, a little more under the cube's perspective.
    const cubeReach = 5 + 3 * Math.sqrt(3) * 1.01
    expect(Number(top) * 0.25).toBeGreaterThan(cubeReach)
  })
})

// The tooltip that describes a control, through its aria-describedby.
function tooltipOf(name: string): HTMLElement | null {
  return document.getElementById(button(name).getAttribute('aria-describedby') ?? '')
}

describe('ViewportControls', () => {
  it('is a group of icon buttons: Fit to view, Orthographic, Section view and Selection filter', () => {
    const { controls } = setUp()
    expect(screen.getByRole('group', { name: 'View controls' })).toBe(controls)
    expect(
      within(controls)
        .getAllByRole('button')
        .map((element) => element.getAttribute('aria-label')),
    ).toEqual(['Fit to view', 'Orthographic', 'Section view', 'Selection filter'])
  })

  it('draws size-8 buttons with decorative lucide icons of stroke width 1.75', () => {
    const { controls } = setUp()
    for (const element of within(controls).getAllByRole('button')) {
      expect(element).toHaveClass('size-8', COLOR_CLASSES.focusRing.outline)
      expect(element).not.toHaveClass('rounded-full')
      const icon = element.querySelector('svg')
      expect(icon).toHaveAttribute('aria-hidden', 'true')
      expect(icon).toHaveAttribute('stroke-width', '1.75')
    }
  })

  it('uses the lucide icons maximize, cuboid, scan-line and mouse-pointer-2', () => {
    setUp()
    expect(button('Fit to view').querySelector('svg')).toHaveClass('lucide-maximize')
    expect(button('Orthographic').querySelector('svg')).toHaveClass('lucide-cuboid')
    expect(button('Section view').querySelector('svg')).toHaveClass('lucide-scan-line')
    expect(button('Selection filter').querySelector('svg')).toHaveClass('lucide-mouse-pointer-2')
  })

  it('Fit to view asks to fit', async () => {
    const { on } = setUp()
    await userEvent.click(button('Fit to view'))
    expect(on.onFit).toHaveBeenCalledTimes(1)
  })

  it('Orthographic is pressed in orthographic and toggles the projection', async () => {
    const { on, rerender } = setUp()
    expect(button('Orthographic')).toHaveAttribute('aria-pressed', 'false')
    await userEvent.click(button('Orthographic'))
    expect(on.onToggleProjection).toHaveBeenCalledTimes(1)
    rerender({ orthographic: true })
    expect(button('Orthographic')).toHaveAttribute('aria-pressed', 'true')
    expect(button('Orthographic')).toHaveClass(COLOR_CLASSES.selection.background)
  })

  it('Section view is pressed while the section is on and toggles it', async () => {
    const { on, rerender } = setUp()
    expect(button('Section view')).toHaveAttribute('aria-pressed', 'false')
    await userEvent.click(button('Section view'))
    expect(on.onToggleSection).toHaveBeenCalledTimes(1)
    rerender({ section: SECTION_ON })
    expect(button('Section view')).toHaveAttribute('aria-pressed', 'true')
  })

  it('turning the section on opens the section popover with its fields', () => {
    const { rerender } = setUp()
    expect(screen.queryByTestId('section-popover')).toBeNull()
    rerender({ section: SECTION_ON })
    const dialog = screen.getByRole('dialog', { name: 'Section view' })
    const content = within(dialog).getByTestId('section-popover')
    expect(within(content).getByRole('radio', { name: 'XY' })).toBeChecked()
    expect(within(content).getByRole('textbox', { name: 'Offset' })).toBeInTheDocument()
    expect(within(content).getByRole('switch', { name: 'Flip' })).not.toBeChecked()
  })

  it('reports plane and flip changes from the popover', async () => {
    const { on, rerender } = setUp()
    rerender({ section: SECTION_ON })
    await userEvent.click(screen.getByRole('radio', { name: 'XZ' }))
    await userEvent.click(screen.getByRole('switch', { name: 'Flip' }))
    expect(on.onSectionPlaneChange).toHaveBeenCalledWith('XZ')
    expect(on.onSectionFlippedChange).toHaveBeenCalledWith(true)
  })

  it('closing the popover with Esc keeps the section on and returns focus to its control', async () => {
    const { on, rerender } = setUp()
    await userEvent.click(button('Section view'))
    rerender({ section: SECTION_ON })
    expect(screen.getByRole('radio', { name: 'XY' })).toHaveFocus()
    await userEvent.keyboard('{Escape}')
    expect(screen.queryByRole('dialog', { name: 'Section view' })).toBeNull()
    expect(button('Section view')).toHaveAttribute('aria-pressed', 'true')
    expect(button('Section view')).toHaveFocus()
    expect(on.onToggleSection).toHaveBeenCalledTimes(1)
  })

  it('turning the section off removes the popover', () => {
    const { rerender } = setUp()
    rerender({ section: SECTION_ON })
    rerender({ section: null })
    expect(screen.queryByRole('dialog', { name: 'Section view' })).toBeNull()
    expect(screen.queryByTestId('section-popover')).toBeNull()
  })

  it('does not open the popover for a section that was already on when it mounted', () => {
    setUp({ section: SECTION_ON })
    expect(screen.queryByRole('dialog', { name: 'Section view' })).toBeNull()
  })

  it('opens the Selection filter menu of Bodies, Faces, Edges and Vertices', async () => {
    setUp()
    const trigger = button('Selection filter')
    expect(trigger).toHaveAttribute('aria-haspopup', 'menu')
    expect(trigger).toHaveAttribute('aria-expanded', 'false')
    await userEvent.click(trigger)
    expect(trigger).toHaveAttribute('aria-expanded', 'true')
    const menu = screen.getByRole('menu', { name: 'Selection filter' })
    const items = within(menu).getAllByRole('menuitemcheckbox')
    expect(items.map((item) => item.textContent)).toEqual(['Bodies', 'Faces', 'Edges', 'Vertices'])
    expect(items.map((item) => item.getAttribute('aria-checked'))).toEqual([
      'false',
      'true',
      'true',
      'true',
    ])
  })

  it('a filter item toggles its kind and closes the menu', async () => {
    const { on } = setUp()
    await userEvent.click(button('Selection filter'))
    await userEvent.click(screen.getByRole('menuitemcheckbox', { name: 'Edges' }))
    expect(on.onToggleFilter).toHaveBeenCalledWith('edge')
    expect(screen.queryByRole('menu')).toBeNull()
    expect(button('Selection filter')).toHaveAttribute('aria-expanded', 'false')
  })

  it('hides the Selection filter without a filter (sketch mode)', () => {
    setUp({ filter: null })
    expect(screen.queryByRole('button', { name: 'Selection filter' })).toBeNull()
  })

  it('closes the Selection filter menu when sketch mode hides its control', async () => {
    const { rerender } = setUp()
    await userEvent.click(button('Selection filter'))
    rerender({ filter: null })
    expect(screen.queryByRole('menu')).toBeNull()
    rerender({ filter: DEFAULT_FILTER })
    expect(screen.queryByRole('menu')).toBeNull()
    expect(button('Selection filter')).toHaveAttribute('aria-expanded', 'false')
  })

  it('describes each control with a tooltip of its title and shortcut', () => {
    setUp({ shortcuts: { fit: 'F6', projection: 'Shift+P', section: 'Shift+X' } })
    expect(tooltipOf('Fit to view')).toHaveTextContent('Fit to viewF6')
    expect(tooltipOf('Orthographic')).toHaveTextContent('OrthographicShift+P')
    expect(tooltipOf('Section view')).toHaveTextContent('Section viewShift+X')
    expect(tooltipOf('Selection filter')).toHaveTextContent(/^Selection filter$/)
    expect(tooltipOf('Fit to view')).toHaveAttribute('role', 'tooltip')
  })

  it('keeps the controls out of the tab order: the viewport is one tab stop', () => {
    const { controls } = setUp()
    for (const element of within(controls).getAllByRole('button')) {
      expect(element).toHaveAttribute('tabindex', '-1')
    }
  })
})
