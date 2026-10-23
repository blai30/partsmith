import { fireEvent, screen } from '@testing-library/react'
import { vi } from 'vitest'
import type { Document } from '../generated/protocol'
import { featuresOf } from '../test/fakes/documents'

// The timeline strip under a mouse, for tests (ui.md 3.9). `stubItemRectangles` lays the items
// out 40 pixels wide and 4 apart from x = 100, so gap g lies at x = 98 + 44 g and item i's center
// at x = 120 + 44 i; the pointer moves at the items' height.

/** A mouse pointer with the primary button, at the items' height. */
const POINTER = { pointerId: 1, pointerType: 'mouse', button: 0, clientY: 28 }

/** The timeline item of the feature `id`. */
export function timelineItem(id: string): HTMLElement {
  const found = screen
    .getAllByTestId('timeline-item')
    .find((element) => element.getAttribute('data-feature-id') === id)
  if (found === undefined) {
    throw new Error(`No timeline item ${id}`)
  }
  return found
}

/** Lays out the items of `document`'s features in timeline order, in a strip 1000 pixels wide. */
export function stubItemRectangles(document: Document): void {
  const order = featuresOf(document).map((feature) => feature.id)
  vi.spyOn(Element.prototype, 'getBoundingClientRect').mockImplementation(function rectangle(
    this: Element,
  ) {
    const id = this.closest('[data-feature-id]')?.getAttribute('data-feature-id') ?? null
    const index = id === null ? -1 : order.indexOf(id)
    return index === -1
      ? DOMRect.fromRect({ x: 0, y: 0, width: 1000, height: 56 })
      : DOMRect.fromRect({ x: 100 + 44 * index, y: 8, width: 40, height: 40 })
  })
}

/** Presses the primary button over `target` at `x`. */
export function pressItem(target: HTMLElement, x: number): void {
  fireEvent.pointerDown(target, { ...POINTER, buttons: 1, clientX: x })
}

/** Moves the pointer over `target` to `x`, with the primary button held unless `buttons` is 0. */
export function movePointer(target: HTMLElement, x: number, buttons = 1): void {
  fireEvent.pointerMove(target, { ...POINTER, buttons, clientX: x })
}

/** Releases the primary button over `target` at `x`. */
export function releasePointer(target: HTMLElement, x: number): void {
  fireEvent.pointerUp(target, { ...POINTER, buttons: 0, clientX: x })
}

/** Drags `from` from `startX` to `endX`: the move 10 pixels left starts the drag (3.9). */
export function drag(from: HTMLElement, startX: number, endX: number): void {
  pressItem(from, startX)
  movePointer(from, startX - 10)
  movePointer(from, endX)
  releasePointer(from, endX)
}
