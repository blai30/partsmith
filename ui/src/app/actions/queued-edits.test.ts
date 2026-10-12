import { describe, expect, it } from 'vitest'
import { currentDocument, startWithDocument } from '../../test/fakes/app'
import { CUBE_DOCUMENT, PARAMETRIC_DOCUMENT } from '../../test/fakes/documents'
import { settle } from '../../test/fakes/settle'
import { renameBody, setBodyColor } from './bodies'
import { renameParameter, updateParameter } from './parameters'
import { renameFeature, setSuppressed } from './timeline'

// Two edits of one record queued behind each other: the later one is built from the record as the
// earlier one left it, so it changes only its own property and never undoes the earlier change.

describe('edits queued behind another edit of the same record', () => {
  it('a parameter renamed, then given a new expression, keeps its new name', async () => {
    const fake = await startWithDocument('parametric.ptsm', PARAMETRIC_DOCUMENT)
    fake.transport.pause()
    const renamed = renameParameter(fake.app, 'p-size', 'width')
    const changed = updateParameter(fake.app, 'p-size', { expression: '30 mm' })
    fake.transport.resume()
    expect(await renamed).toBe(true)
    expect(await changed).toBe(true)
    await settle()
    expect(currentDocument(fake.app).recipe.parameters).toEqual([
      { id: 'p-size', name: 'width', expr: '30 mm' },
    ])
  })

  it('a parameter given a new expression, then a comment, keeps its new expression', async () => {
    const fake = await startWithDocument('parametric.ptsm', PARAMETRIC_DOCUMENT)
    fake.transport.pause()
    const expression = updateParameter(fake.app, 'p-size', { expression: '30 mm' })
    const comment = updateParameter(fake.app, 'p-size', { comment: 'Outer size' })
    fake.transport.resume()
    await Promise.all([expression, comment])
    await settle()
    expect(currentDocument(fake.app).recipe.parameters).toEqual([
      { id: 'p-size', name: 'size', expr: '30 mm', comment: 'Outer size' },
    ])
  })

  it('a feature renamed, then suppressed, keeps its name', async () => {
    const fake = await startWithDocument('cube.ptsm', CUBE_DOCUMENT)
    fake.transport.pause()
    const renamed = renameFeature(fake.app, 'f-ext1', 'Base')
    const suppressed = setSuppressed(fake.app, ['f-ext1'], true)
    fake.transport.resume()
    await Promise.all([renamed, suppressed])
    await settle()
    const record = currentDocument(fake.app).recipe.features?.find(
      (feature) => feature.id === 'f-ext1',
    )
    expect(record).toMatchObject({ name: 'Base', suppressed: true })
    const labels = fake.transport.commandsOfType('apply_edits').map((command) => command.label)
    expect(labels).toEqual(['Rename', 'Suppress Base'])
  })

  it('a body renamed, then recolored, keeps its name', async () => {
    const fake = await startWithDocument('cube.ptsm', CUBE_DOCUMENT)
    fake.transport.pause()
    const renamed = renameBody(fake.app, 'b-cube', 'Box')
    const colored = setBodyColor(fake.app, 'b-cube', '#e7000b')
    fake.transport.resume()
    await Promise.all([renamed, colored])
    await settle()
    expect(currentDocument(fake.app).recipe.bodies).toEqual([
      { id: 'b-cube', name: 'Box', color: '#e7000b' },
    ])
  })
})
