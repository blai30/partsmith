import { describe, expect, it } from 'vitest'
import type { ElementDescriptor } from '../../generated/protocol'
import { cubeDocumentState, readyState } from '../../layout/fixtures.testing'
import { escapeStep } from '../../layout/ladders'
import { createModelApp } from '../../layout/test-app.testing'
import type { FakeCoreApi } from '../../test/fakes/core-api'
import { deferred } from '../../test/fakes/deferred'
import { CUBE_TOP_FACE, CUBE_TOP_FRONT_EDGE } from '../../test/fakes/documents'
import { meshPick } from '../../test/fakes/selection'
import { settle } from '../../test/fakes/settle'
import { commandEdges, notificationCodes, selectionKeys } from '../../test/fakes/test-app'
import type { App } from '../createApp'
import { cancelCommand, startFeatureCommand } from './command'
import { setContext } from './context'
import { runFixAction } from './fixes'
import { pickAt } from './selection'
import { reattachSketch } from './sketch'

// What ends a feature command acts on the clicks made into it before (ui.md 3.6, 3.7 step 1,
// U-246): each case clicks into an open command, ends it, and answers the click's pick either
// after the end was asked for ("late") or before it ("at once", the guard), with the same outcome.

/** When the core answers the click's pick: after the end was asked for, or before it. */
type Timing = 'late' | 'at once'

const TIMINGS: readonly Timing[] = ['late', 'at once']

/** cube.ptsm with a new command of `featureType` open and nothing picked. */
async function withCommand(featureType: string): Promise<{ app: App; core: FakeCoreApi }> {
  const { app, core } = createModelApp({ state: readyState(cubeDocumentState()) })
  await startFeatureCommand(app, featureType)
  await settle()
  return { app, core }
}

/**
 * Clicks `element` into the open command, then runs `end`, with the core answering the click's
 * pick as `timing` says, and lets everything land.
 */
async function clickThenEnd(
  { app, core }: { app: App; core: FakeCoreApi },
  element: ElementDescriptor,
  timing: Timing,
  end: () => unknown,
): Promise<void> {
  const picked = deferred<ElementDescriptor>()
  core.stub('pickReference', () => picked.promise)
  const index = element.kind === 'face' ? 0 : 4
  const click = pickAt(app, meshPick(element, index), { toggle: false })
  if (timing === 'at once') {
    picked.resolve(element)
    await click
    await settle()
  }
  const ended = Promise.resolve(end())
  picked.resolve(element)
  await Promise.all([click, ended])
  await settle()
}

/** The labels of the core's `applyEdits` calls: null for a preview, a label for a change. */
function changeLabels(core: FakeCoreApi): unknown[] {
  return core.callsTo('applyEdits').map((call) => call[1])
}

describe('U-246 what ends a feature command acts on the clicks made into it (3.6, 3.7)', () => {
  it.each(TIMINGS)(
    'U-246 (a) a click on a face into a Shell, then Sketch, cancels the Shell with Restore for plane picking (%s)',
    async (timing) => {
      const fake = await withCommand('shell')
      await clickThenEnd(fake, CUBE_TOP_FACE, timing, () => startFeatureCommand(fake.app, 'sketch'))
      expect(changeLabels(fake.core)).not.toContain('New sketch')
      expect(fake.app.store.getState()).toMatchObject({
        context: 'sketch',
        sketch: { sketchId: null },
        command: null,
        selection: [],
      })
      expect(notificationCodes(fake.app)).toEqual(['command_canceled'])
    },
  )

  it.each(TIMINGS)(
    'U-246 (b) a click on an edge into a Chamfer, then F, cancels the Chamfer with Restore and opens an empty Fillet (%s)',
    async (timing) => {
      const fake = await withCommand('chamfer')
      await clickThenEnd(fake, CUBE_TOP_FRONT_EDGE, timing, () =>
        startFeatureCommand(fake.app, 'fillet'),
      )
      expect(fake.app.store.getState().command?.featureType).toBe('fillet')
      expect(commandEdges(fake.app)).toEqual([])
      expect(notificationCodes(fake.app)).toEqual(['command_canceled'])
    },
  )

  it.each(TIMINGS)(
    'U-246 (c) a click on an edge into a Fillet, then F again, cancels it with Restore and opens it empty (%s)',
    async (timing) => {
      const fake = await withCommand('fillet')
      await clickThenEnd(fake, CUBE_TOP_FRONT_EDGE, timing, () =>
        startFeatureCommand(fake.app, 'fillet'),
      )
      expect(fake.core.callsTo('cancelTransaction')).toHaveLength(1)
      expect(fake.core.callsTo('beginTransaction')).toEqual([
        [1, 'Fillet'],
        [1, 'Fillet'],
      ])
      expect(commandEdges(fake.app)).toEqual([])
      expect(notificationCodes(fake.app)).toEqual(['command_canceled'])
    },
  )

  it.each(TIMINGS)(
    'U-246 (d) a click on an edge into a Fillet, then re-attaching a sketch, cancels the Fillet with Restore (%s)',
    async (timing) => {
      const fake = await withCommand('fillet')
      await clickThenEnd(fake, CUBE_TOP_FRONT_EDGE, timing, () => reattachSketch(fake.app, 'f-sk1'))
      expect(fake.app.store.getState()).toMatchObject({
        context: 'sketch',
        sketch: { sketchId: null, reattaching: 'f-sk1' },
        command: null,
        selection: [],
      })
      expect(notificationCodes(fake.app)).toEqual(['command_canceled'])
    },
  )

  it.each(TIMINGS)(
    'U-246 (e) a click on an edge into a Fillet, then Esc, ends with nothing selected (%s)',
    async (timing) => {
      const fake = await withCommand('fillet')
      await clickThenEnd(fake, CUBE_TOP_FRONT_EDGE, timing, () => {
        expect(escapeStep(fake.app)).toBe(true)
      })
      expect(fake.app.store.getState()).toMatchObject({ command: null, selection: [] })
      expect(fake.core.callsTo('cancelTransaction')).toHaveLength(1)
      expect(notificationCodes(fake.app)).toEqual([])
    },
  )

  it.each(TIMINGS)(
    'U-246 (f) a click on an edge into a Fillet, then Cancel, ends with nothing selected (%s)',
    async (timing) => {
      const fake = await withCommand('fillet')
      await clickThenEnd(fake, CUBE_TOP_FRONT_EDGE, timing, () => cancelCommand(fake.app))
      expect(fake.app.store.getState()).toMatchObject({ command: null, selection: [] })
      expect(notificationCodes(fake.app)).toEqual([])
    },
  )

  it.each(
    (['model', 'print'] as const).flatMap((context) =>
      TIMINGS.map((timing) => [context, timing] as const),
    ),
  )(
    'U-246 (g) a click on an edge into a Fillet, then the %s context, cancels it with Restore and selects nothing (%s)',
    async (context, timing) => {
      const fake = await withCommand('fillet')
      await clickThenEnd(fake, CUBE_TOP_FRONT_EDGE, timing, () => setContext(fake.app, context))
      expect(fake.app.store.getState()).toMatchObject({ context, command: null, selection: [] })
      expect(notificationCodes(fake.app)).toEqual(['command_canceled'])
    },
  )

  it.each(TIMINGS)(
    'U-246 (h) a click on an edge into a Fillet, then a show of another feature, cancels it with Restore (%s)',
    async (timing) => {
      const fake = await withCommand('fillet')
      await clickThenEnd(fake, CUBE_TOP_FRONT_EDGE, timing, () =>
        runFixAction(
          fake.app,
          { label: "Show 'Sketch 1'", run: { kind: 'show', feature: 'f-sk1' } },
          'panel',
        ),
      )
      expect(fake.app.store.getState().command).toBeNull()
      expect(selectionKeys(fake.app)).toEqual(['feature:f-sk1'])
      expect(notificationCodes(fake.app)).toEqual(['command_canceled'])
    },
  )
})
