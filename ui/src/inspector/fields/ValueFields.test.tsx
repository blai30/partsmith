import { screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { createModelApp, renderWithApp } from '../../layout/test-app.testing'
import { hasTabularFigures, isCode } from '../../test/type-roles'
import type { InspectorField } from '../model'
import { NumberField, TextValueField } from './ValueFields'

// The type roles of the inspector's plain number and text fields (ui.md 2.16.7, 3.7).

function field(label: string, control: InspectorField['control'], value: unknown): InspectorField {
  return {
    path: label.toLowerCase(),
    label,
    help: null,
    essential: true,
    required: true,
    control,
    value,
    error: null,
    attachedTo: null,
    axis: null,
  }
}

describe('the type roles of number and text fields (2.16.7, U-205)', () => {
  it('U-205 sets a number field in tabular figures, not code', () => {
    const { app } = createModelApp()
    const count = field('Count', { type: 'number', integer: true }, 12)
    renderWithApp(app, <NumberField field={count} integer statusMessage={null} />)
    const input = screen.getByRole('textbox', { name: 'Count' })
    expect(input).toHaveValue('12')
    expect(hasTabularFigures(input)).toBe(true)
    expect(isCode(input)).toBe(false)
  })

  it('U-205 keeps a text field proportional, not code', () => {
    const { app } = createModelApp()
    const name = field('Name', { type: 'text' }, 'Bracket 10')
    renderWithApp(app, <TextValueField field={name} statusMessage={null} />)
    const input = screen.getByRole('textbox', { name: 'Name' })
    expect(hasTabularFigures(input)).toBe(false)
    expect(isCode(input)).toBe(false)
  })
})
