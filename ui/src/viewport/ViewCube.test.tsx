import { cleanup, fireEvent, render, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { createRef } from 'react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { COLOR_CLASSES } from '../theme/roles'
import { standardViewRotation } from './camera'
import type { Quaternion } from './math'
import { ViewCube } from './ViewCube'
import { VIEW_CUBE_PARTS } from './view-cube'

afterEach(() => {
  cleanup()
})

function setUp() {
  const onChooseView = vi.fn<(rotation: Quaternion) => void>()
  const onHome = vi.fn<() => void>()
  const onOrbit = vi.fn<(deltaX: number, deltaY: number) => void>()
  const rotorRef = createRef<HTMLDivElement>()
  render(
    <ViewCube rotorRef={rotorRef} onChooseView={onChooseView} onHome={onHome} onOrbit={onOrbit} />,
  )
  return { onChooseView, onHome, onOrbit, rotorRef, cube: screen.getByTestId('view-cube') }
}

describe('ViewCube', () => {
  it('is a group named "View cube" at the top right of the viewport, size-24', () => {
    const { cube } = setUp()
    expect(screen.getByRole('group', { name: 'View cube' })).toBe(cube)
    expect(cube).toHaveClass('absolute', 'top-4', 'right-4', 'size-24')
  })

  it('has a button for each of its 6 faces, 12 edges and 8 corners, and the Home view button', () => {
    const { cube } = setUp()
    for (const cubePart of VIEW_CUBE_PARTS) {
      expect(within(cube).getByRole('button', { name: cubePart.label })).toBeInTheDocument()
    }
    expect(within(cube).getByRole('button', { name: 'Home view' })).toBeInTheDocument()
    expect(within(cube).getAllByRole('button')).toHaveLength(VIEW_CUBE_PARTS.length + 1)
  })

  it('labels its faces Top, Bottom, Front, Back, Left and Right', () => {
    const { cube } = setUp()
    for (const label of ['Top', 'Bottom', 'Front', 'Back', 'Left', 'Right']) {
      expect(within(cube).getByText(label, { exact: true })).toBeInTheDocument()
    }
  })

  it('attaches the rotor that the controller turns to match the camera', () => {
    const { rotorRef, cube } = setUp()
    expect(rotorRef.current).not.toBeNull()
    expect(cube).toContainElement(rotorRef.current)
    expect(rotorRef.current).toHaveClass('transform-3d')
  })

  it('a click on a face chooses its standard view', async () => {
    const { onChooseView } = setUp()
    await userEvent.click(screen.getByRole('button', { name: 'Top view' }))
    expect(onChooseView).toHaveBeenCalledTimes(1)
    expect(onChooseView).toHaveBeenCalledWith(standardViewRotation('top'))
  })

  it('a click on the front right top corner chooses the iso view', async () => {
    const { onChooseView } = setUp()
    await userEvent.click(screen.getByRole('button', { name: 'Front right top corner view' }))
    const [rotation] = onChooseView.mock.calls[0] ?? []
    const iso = standardViewRotation('iso')
    rotation?.forEach((component, index) => {
      expect(component).toBeCloseTo(iso[index] ?? Number.NaN, 12)
    })
  })

  it('a click on an edge band shown on another face chooses the same view as its button', () => {
    const { onChooseView, cube } = setUp()
    const copies = cube.querySelectorAll('[data-view-cube-part="front-top"]')
    expect(copies).toHaveLength(2)
    for (const copy of copies) {
      fireEvent.click(copy)
    }
    expect(onChooseView).toHaveBeenCalledTimes(2)
    expect(onChooseView.mock.calls[0]).toEqual(onChooseView.mock.calls[1])
  })

  it('the Home view button asks for the home view', async () => {
    const { onHome, onChooseView } = setUp()
    await userEvent.click(screen.getByRole('button', { name: 'Home view' }))
    expect(onHome).toHaveBeenCalledTimes(1)
    expect(onChooseView).not.toHaveBeenCalled()
  })

  it('a drag of more than 3 pixels orbits by the pointer motion and chooses no view', () => {
    const { onChooseView, onOrbit } = setUp()
    const face = screen.getByRole('button', { name: 'Front view' })
    fireEvent.pointerDown(face, { pointerId: 1, button: 0, clientX: 100, clientY: 100 })
    fireEvent.pointerMove(face, { pointerId: 1, clientX: 102, clientY: 101 })
    expect(onOrbit).not.toHaveBeenCalled()
    fireEvent.pointerMove(face, { pointerId: 1, clientX: 110, clientY: 104 })
    fireEvent.pointerMove(face, { pointerId: 1, clientX: 115, clientY: 100 })
    fireEvent.pointerUp(face, { pointerId: 1, button: 0, clientX: 115, clientY: 100 })
    fireEvent.click(face)
    expect(onOrbit.mock.calls).toEqual([
      [10, 4],
      [5, -4],
    ])
    expect(onChooseView).not.toHaveBeenCalled()
  })

  it('a press that moves 3 pixels or less is a click', () => {
    const { onChooseView, onOrbit } = setUp()
    const face = screen.getByRole('button', { name: 'Right view' })
    fireEvent.pointerDown(face, { pointerId: 1, button: 0, clientX: 100, clientY: 100 })
    fireEvent.pointerMove(face, { pointerId: 1, clientX: 102, clientY: 102 })
    fireEvent.pointerUp(face, { pointerId: 1, button: 0, clientX: 102, clientY: 102 })
    fireEvent.click(face)
    expect(onOrbit).not.toHaveBeenCalled()
    expect(onChooseView).toHaveBeenCalledWith(standardViewRotation('right'))
  })

  it('ignores presses of other buttons and other pointers', () => {
    const { onOrbit } = setUp()
    const face = screen.getByRole('button', { name: 'Front view' })
    fireEvent.pointerDown(face, { pointerId: 1, button: 1, clientX: 100, clientY: 100 })
    fireEvent.pointerMove(face, { pointerId: 1, clientX: 140, clientY: 100 })
    fireEvent.pointerDown(face, { pointerId: 2, button: 0, clientX: 100, clientY: 100 })
    fireEvent.pointerMove(face, { pointerId: 3, clientX: 140, clientY: 100 })
    expect(onOrbit).not.toHaveBeenCalled()
  })

  it('highlights every copy of the hovered part with the hover role', () => {
    const { cube } = setUp()
    const corner = cube.querySelectorAll('[data-view-cube-part="front-right-top"]')
    expect(corner).toHaveLength(3)
    fireEvent.pointerOver(screen.getByRole('button', { name: 'Front right top corner view' }))
    for (const copy of corner) {
      expect(copy).toHaveClass(COLOR_CLASSES.hover.active)
      expect(copy).not.toHaveClass(COLOR_CLASSES.surfaceRaised.background)
    }
    const front = screen.getByRole('button', { name: 'Front view' })
    expect(front).toHaveClass(COLOR_CLASSES.surfaceRaised.background)
    fireEvent.pointerLeave(cube)
    for (const copy of corner) {
      expect(copy).toHaveClass(COLOR_CLASSES.surfaceRaised.background)
    }
  })

  it('keeps the cube out of the tab order: the viewport is one tab stop', () => {
    const { cube } = setUp()
    for (const button of within(cube).getAllByRole('button')) {
      expect(button).toHaveAttribute('tabindex', '-1')
    }
  })
})
