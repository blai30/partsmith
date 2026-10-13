import { describe, expect, it } from 'vitest'
import {
  currentDocument,
  type FakeApp,
  labeledEdits,
  startWithDocument,
} from '../../test/fakes/app'
import { PARAMETRIC_DOCUMENT, parametersOf } from '../../test/fakes/documents'
import { editsOf } from '../../test/fakes/protocol'
import { settle } from '../../test/fakes/settle'
import { addParameter, deleteParameter, renameParameter, updateParameter } from './parameters'

/** parametric.ptsm: parameter p-size "size" = 20 mm drives the cube. */
async function parametric(): Promise<FakeApp> {
  return startWithDocument('parametric.ptsm', PARAMETRIC_DOCUMENT)
}

describe('renameParameter (3.11)', () => {
  it('U-85 sends rename_parameter and no update_parameter, and resolves true', async () => {
    const fake = await parametric()
    await expect(renameParameter(fake.app, 'p-size', 'side')).resolves.toBe(true)
    await settle()
    expect(fake.transport.commandsOfType('rename_parameter')).toEqual([
      {
        type: 'rename_parameter',
        doc: currentDocument(fake.app).documentHandle,
        parameter: 'p-size',
        name: 'side',
      },
    ])
    const edits = fake.transport
      .commandsOfType('apply_edits')
      .flatMap((command) => editsOf(command))
    expect(edits.some((edit) => edit.type === 'update_parameter')).toBe(false)
    expect(parametersOf(currentDocument(fake.app).recipe)[0]?.name).toBe('side')
    expect(currentDocument(fake.app).undoLabel).toBe('Rename parameter size to side')
  })

  it('resolves false when the core refuses the name', async () => {
    const fake = await parametric()
    fake.transport.handle('rename_parameter', () => ({
      error: { code: 'edit_rejected', message: '"pi" is a reserved name. Choose another name.' },
    }))
    await expect(renameParameter(fake.app, 'p-size', 'pi')).resolves.toBe(false)
    expect(parametersOf(currentDocument(fake.app).recipe)[0]?.name).toBe('size')
  })
})

describe('addParameter (3.11)', () => {
  it('appends the parameter with a new p- id, labeled "Add parameter {name}"', async () => {
    const fake = await parametric()
    await expect(addParameter(fake.app, 'wall', '2 mm')).resolves.toBe(true)
    await settle()
    const [command] = fake.transport.commandsOfType('apply_edits')
    expect(command?.label).toBe('Add parameter wall')
    const [edit] = editsOf(command)
    expect(edit?.type).toBe('add_parameter')
    if (edit?.type !== 'add_parameter') {
      return
    }
    expect(edit.index).toBe(1)
    expect(edit.parameter).toMatchObject({ name: 'wall', expr: '2 mm' })
    expect(edit.parameter.id).toMatch(/^p-[0-9a-f]{12}$/)
    expect(
      parametersOf(currentDocument(fake.app).recipe).map((parameter) => parameter.name),
    ).toEqual(['size', 'wall'])
  })

  it('resolves false when the core rejects the parameter', async () => {
    const fake = await parametric()
    fake.transport.handle('apply_edits', () => ({
      error: {
        code: 'edit_rejected',
        message: 'A parameter named size already exists.',
        edit_index: 0,
      },
    }))
    await expect(addParameter(fake.app, 'size', '3 mm')).resolves.toBe(false)
  })
})

describe('updateParameter (3.11)', () => {
  it('commits a new expression with update_parameter labeled "Change {name}"', async () => {
    const fake = await parametric()
    await expect(updateParameter(fake.app, 'p-size', { expression: '30 mm' })).resolves.toBe(true)
    await settle()
    expect(labeledEdits(fake)).toEqual([
      {
        label: 'Change size',
        edits: [
          { type: 'update_parameter', parameter: { id: 'p-size', name: 'size', expr: '30 mm' } },
        ],
      },
    ])
  })

  it('commits a comment labeled "Change comment of {name}"', async () => {
    const fake = await parametric()
    await updateParameter(fake.app, 'p-size', { comment: 'Edge length of the cube' })
    await settle()
    expect(labeledEdits(fake)).toEqual([
      {
        label: 'Change comment of size',
        edits: [
          {
            type: 'update_parameter',
            parameter: {
              id: 'p-size',
              name: 'size',
              expr: '20 mm',
              comment: 'Edge length of the cube',
            },
          },
        ],
      },
    ])
  })

  it('an empty or null comment removes the comment', async () => {
    const fake = await startWithDocument('parametric.ptsm', {
      ...PARAMETRIC_DOCUMENT,
      parameters: [{ id: 'p-size', name: 'size', expr: '20 mm', comment: 'Edge length' }],
    })
    await updateParameter(fake.app, 'p-size', { comment: '' })
    await updateParameter(fake.app, 'p-size', { comment: null })
    await settle()
    expect(fake.transport.commandsOfType('apply_edits').length).toBeGreaterThan(0)
    for (const command of fake.transport.commandsOfType('apply_edits')) {
      const [edit] = editsOf(command)
      expect(
        edit?.type === 'update_parameter' ? (edit.parameter.comment ?? null) : 'not an update',
      ).toBeNull()
    }
  })
})

describe('deleteParameter (3.11)', () => {
  it('commits remove_parameter labeled "Delete parameter {name}"', async () => {
    const fake = await parametric()
    await expect(deleteParameter(fake.app, 'p-size')).resolves.toBe(true)
    await settle()
    expect(labeledEdits(fake)).toEqual([
      { label: 'Delete parameter size', edits: [{ type: 'remove_parameter', id: 'p-size' }] },
    ])
    expect(parametersOf(currentDocument(fake.app).recipe)).toEqual([])
  })
})
