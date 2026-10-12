import { beforeEach, describe, expect, it } from 'vitest'
import type { Edit } from '../generated/protocol'
import { TIMING_NAMES } from '../performance/marks'
import { startFakeApp, startWithDocument } from '../test/fakes/app'
import {
  CUBE_DOCUMENT,
  CUBE_REGION,
  CUBE_SKETCH_SOLUTION,
  documentFile,
} from '../test/fakes/documents'
import { regionItem } from '../test/fakes/selection'
import { settle } from '../test/fakes/settle'
import { setCommandField, startFeatureCommand } from './actions/command'
import { applyEdits, openFile } from './actions/document'
import { select } from './actions/selection'
import { measureToNextFrame } from './timing'

// The User Timing measures of ui.md 2.18 that end once the viewport drew a result: ps:open,
// ps:rebuild and ps:preview.

const RENAME: readonly Edit[] = [{ type: 'update_body', body: { id: 'b-cube', name: 'Box' } }]

/** Resolves in the animation frame after every frame requested so far. */
function nextFrame(): Promise<void> {
  return new Promise((resolve) => {
    requestAnimationFrame(() => {
      resolve()
    })
  })
}

function durations(name: string): number[] {
  return performance.getEntriesByName(name, 'measure').map((entry) => entry.duration)
}

beforeEach(() => {
  performance.clearMeasures()
})

describe('measureToNextFrame', () => {
  it('records the measure from the start time to the next animation frame', async () => {
    const start = performance.now() - 5
    measureToNextFrame(TIMING_NAMES.rebuild, start)
    expect(durations(TIMING_NAMES.rebuild)).toEqual([])
    await nextFrame()
    const [duration] = durations(TIMING_NAMES.rebuild)
    expect(duration).toBeGreaterThanOrEqual(5)
  })
})

describe('timing marks of the actions (2.18)', () => {
  it('ps:open runs from the read file to the frame after the opened document arrived', async () => {
    const fake = await startFakeApp()
    await openFile(fake.app, documentFile('cube.ptsm', CUBE_DOCUMENT))
    await settle()
    await nextFrame()
    expect(durations(TIMING_NAMES.open)).toHaveLength(1)
  })

  it('ps:open is not recorded for a file that does not open', async () => {
    const fake = await startFakeApp()
    await openFile(fake.app, documentFile('cube.stl', CUBE_DOCUMENT))
    await settle()
    await nextFrame()
    expect(durations(TIMING_NAMES.open)).toEqual([])
  })

  it('ps:rebuild runs from sending an edit to the frame after its response', async () => {
    const fake = await startWithDocument('cube.ptsm', CUBE_DOCUMENT)
    await nextFrame()
    performance.clearMeasures()
    await applyEdits(fake.app, 'Rename', RENAME)
    await settle()
    await nextFrame()
    expect(durations(TIMING_NAMES.rebuild)).toHaveLength(1)
  })

  it('ps:preview runs from an inspector change to the frame after its preview', async () => {
    const fake = await startWithDocument('cube.ptsm', CUBE_DOCUMENT, {
      core: { solutions: { 'f-sk1': CUBE_SKETCH_SOLUTION } },
    })
    select(fake.app, [regionItem('f-sk1', CUBE_REGION)], 'replace')
    await startFeatureCommand(fake.app, 'extrude')
    await settle()
    await nextFrame()
    // The first preview of a new command follows no change by the user.
    expect(durations(TIMING_NAMES.preview)).toEqual([])
    setCommandField(fake.app, 'extent.expr', '25 mm')
    await settle()
    await nextFrame()
    expect(durations(TIMING_NAMES.preview)).toHaveLength(1)
  })
})
