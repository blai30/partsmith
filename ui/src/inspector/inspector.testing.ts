// Test support for the inspector: its fields by params path, the role classes they draw with,
// and the edges of cube-features.ptsm's fillet. Nothing here is part of the app.

import { screen, within } from '@testing-library/react'
import { CUBE_FEATURES_DOCUMENT, featuresOf } from '../test/fakes/documents'

/** The edges of cube-features.ptsm's fillet `f-fil1`. */
export const FILLET_EDGES = featuresOf(CUBE_FEATURES_DOCUMENT)[2]?.edges as readonly unknown[]

/** The inspector's field at a params path; throws when the inspector shows none. */
export function inspectorField(path: string): HTMLElement {
  const found = within(screen.getByTestId('inspector'))
    .getAllByTestId('inspector-field')
    .find((element) => element.getAttribute('data-path') === path)
  if (found === undefined) {
    throw new Error(`No inspector field ${path}`)
  }
  return found
}

/** Whether the element, or an element inside it, has every class of a role's classes. */
export function hasRoleClasses(element: Element, classes: string): boolean {
  const names = classes.split(' ')
  return [element, ...element.querySelectorAll('*')].some((candidate) =>
    names.every((name) => candidate.classList.contains(name)),
  )
}
