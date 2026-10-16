import { screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { createModelApp, renderWithApp } from '../../layout/test-app.testing'
import { hasTabularFigures, isCode } from '../../test/type-roles'
import { COLOR_CLASSES } from '../../theme/roles'
import type { InspectorField } from '../model'
import { NumberField, TextValueField, UnsupportedField } from './ValueFields'

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

describe('the read-only JSON block of an unsupported field (2.12, U-211)', () => {
  it('U-211 has the shape of an input on the tonal fill, with a forcedEdge border', () => {
    const { app } = createModelApp()
    const unsupported = field('Profile', { type: 'unsupported' }, { kind: 'spline' })
    renderWithApp(app, <UnsupportedField field={unsupported} />)
    const block = screen.getByText('{"kind":"spline"}')
    expect(block).toHaveClass('rounded-md', 'border')
    expect(block).toHaveClass(COLOR_CLASSES.tonal.background)
    expect(block).toHaveClass(COLOR_CLASSES.forcedEdge.border)
    expect(block).not.toHaveClass(COLOR_CLASSES.field.background)
  })
})
