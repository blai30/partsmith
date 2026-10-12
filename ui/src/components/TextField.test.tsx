import { cleanup, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { useState, type KeyboardEvent } from 'react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { COLOR_CLASSES } from '../theme/roles'
import { TextField } from './TextField'

afterEach(() => {
  cleanup()
})

function Controlled({ onChange }: { readonly onChange: (value: string) => void }) {
  const [value, setValue] = useState('size')
  return (
    <TextField
      label="Name of size"
      value={value}
      onChange={(next) => {
        onChange(next)
        setValue(next)
      }}
    />
  )
}

describe('TextField', () => {
  it('is a text box named by its label showing its value', () => {
    render(<TextField label="Name of size" value="size" onChange={() => undefined} />)
    expect(screen.getByRole('textbox', { name: 'Name of size' })).toHaveValue('size')
  })

  it('reports each edit as a string', async () => {
    const onChange = vi.fn<(value: string) => void>()
    render(<Controlled onChange={onChange} />)
    const input = screen.getByRole('textbox', { name: 'Name of size' })
    await userEvent.type(input, '_2')
    expect(onChange).toHaveBeenLastCalledWith('size_2')
    expect(input).toHaveValue('size_2')
  })

  it('passes keyboard handlers through to the input', async () => {
    const onKeyDown = vi.fn<(event: KeyboardEvent<HTMLInputElement>) => void>()
    render(
      <TextField
        label="Comment of size"
        value=""
        onChange={() => undefined}
        onKeyDown={onKeyDown}
      />,
    )
    await userEvent.type(screen.getByRole('textbox', { name: 'Comment of size' }), '{Enter}')
    expect(onKeyDown).toHaveBeenCalledWith(expect.objectContaining({ key: 'Enter' }))
  })

  it('uses the field background, the control border and the focus ring without an error', () => {
    render(<TextField label="Name of size" value="size" onChange={() => undefined} />)
    const input = screen.getByRole('textbox', { name: 'Name of size' })
    expect(input).toHaveClass(COLOR_CLASSES.field.background)
    expect(input).toHaveClass(COLOR_CLASSES.borderControl.border)
    expect(input).toHaveClass(COLOR_CLASSES.focusRing.outline)
    expect(input).not.toHaveAttribute('aria-invalid', 'true')
  })

  it('shows an error under the field, marks it invalid and describes the input with it', () => {
    const message = 'Names start with a letter or _ and use only letters, digits and _.'
    render(<TextField label="Name of size" value="2x" error={message} onChange={() => undefined} />)
    const input = screen.getByRole('textbox', { name: 'Name of size' })
    expect(input).toHaveAttribute('aria-invalid', 'true')
    expect(input).toHaveAccessibleDescription(message)
    expect(input).toHaveClass(COLOR_CLASSES.errorMark.border)
    expect(input).not.toHaveClass(COLOR_CLASSES.borderControl.border)
    expect(screen.getByText(message)).toHaveClass(COLOR_CLASSES.errorText.foreground)
  })

  it('removes the error when it is cleared', () => {
    const message = 'Names start with a letter or _ and use only letters, digits and _.'
    const { rerender } = render(
      <TextField label="Name of size" value="2x" error={message} onChange={() => undefined} />,
    )
    rerender(<TextField label="Name of size" value="x2" error={null} onChange={() => undefined} />)
    expect(screen.getByRole('textbox', { name: 'Name of size' })).not.toHaveAttribute(
      'aria-invalid',
      'true',
    )
    expect(screen.queryByText(message)).toBeNull()
  })
})

describe('TextField labels and descriptions', () => {
  it('keeps a hidden label as the accessible name', () => {
    render(<TextField label="Name of size" hideLabel value="size" onChange={() => undefined} />)
    expect(screen.getByRole('textbox', { name: 'Name of size' })).toBeInTheDocument()
    expect(screen.getByText('Name of size')).toHaveClass('sr-only')
  })

  it('adds the error to a description the caller gives', () => {
    render(
      <>
        <p id="hint">Used in expressions.</p>
        <TextField
          label="Name of size"
          value="2x"
          aria-describedby="hint"
          error="Names start with a letter or _ and use only letters, digits and _."
          onChange={() => undefined}
        />
      </>,
    )
    expect(screen.getByRole('textbox', { name: 'Name of size' })).toHaveAccessibleDescription(
      'Used in expressions. Names start with a letter or _ and use only letters, digits and _.',
    )
  })
})
