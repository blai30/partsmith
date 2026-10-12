import { describe, expect, it } from 'vitest'
import { isCheckHidden } from './checks'

describe('isCheckHidden', () => {
  it('hides the checks settings.hiddenChecks lists', () => {
    expect(isCheckHidden(['overhangs', 'islands'], 'islands')).toBe(true)
    expect(isCheckHidden(['overhangs', 'islands'], 'bed_fit')).toBe(false)
  })

  it('never hides mesh validity, even when the stored list has it', () => {
    expect(isCheckHidden(['mesh_validity'], 'mesh_validity')).toBe(false)
  })
})
