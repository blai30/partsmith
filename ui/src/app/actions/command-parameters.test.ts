import { describe, expect, it } from 'vitest'
import { documentState, featureItem, readyState } from '../../layout/fixtures.testing'
import { createModelApp, deferred } from '../../layout/test-app.testing'
import { MESSAGES } from '../../messages'
import type { ActiveCommand } from '../../state/store'
import { PARAMETRIC_DOCUMENT } from '../../test/fakes/documents'
import { settle } from '../../test/fakes/settle'
import { addParameter, deleteParameter, updateParameter } from './parameters'
import { setBodyVisible } from './bodies'
import { setSuppressed } from './timeline'

// Other edits during a command (ui.md 3.7): parameter changes join the command's transaction and
// the preview follows them; every other document edit waits for OK or Cancel.

const EXTRUDE_COMMAND: ActiveCommand = {
  featureType: 'extrude',
  featureId: 'f-ext9',
  editing: false,
  title: 'Extrude',
  params: {
    profile: { sketch: 'f-sk1', regions: [{ key: 'e-1,e-2,e-3,e-4', fp: 'c=0,0|a=400' }] },
    extent: { kind: 'distance', expr: 'size' },
    op: 'join',
    body: 'b-cube',
  },
  activeField: 'profile',
  fieldErrors: {},
  applied: true,
  newBody: null,
  helper: null,
  changedPaths: [],
  history: [],
  future: [],
  preview: 'shown',
}

function parametricApp(command: ActiveCommand | null) {
  return createModelApp({
    state: readyState(documentState(PARAMETRIC_DOCUMENT), {
      command,
      selection: [featureItem('f-ext1', 'Extrude 1')],
    }),
  })
}

describe('parameter edits during a command (U-108)', () => {
  it('U-108 sends a parameter change into the transaction with no label, pending until it lands', async () => {
    const { app, core } = parametricApp(EXTRUDE_COMMAND)
    const reply = deferred<number>()
    core.stub('applyEdits', () => reply.promise)
    const changed = updateParameter(app, 'p-size', { expression: '30 mm' })
    await settle()
    expect(core.callsTo('applyEdits')).toEqual([
      [
        1,
        null,
        [{ type: 'update_parameter', parameter: { id: 'p-size', name: 'size', expr: '30 mm' } }],
      ],
    ])
    expect(app.store.getState().command?.preview).toBe('pending')
    reply.resolve(2)
    expect(await changed).toBe(true)
    expect(app.store.getState().command?.preview).toBe('shown')
  })

  it('U-108 labels the change "Change size" without a command', async () => {
    const { app, core } = parametricApp(null)
    await updateParameter(app, 'p-size', { expression: '30 mm' })
    expect(core.callsTo('applyEdits')[0]?.[1]).toBe('Change size')
  })

  it('adds and deletes parameters inside the transaction too', async () => {
    const { app, core } = parametricApp(EXTRUDE_COMMAND)
    await addParameter(app, 'wall', '2 mm')
    await deleteParameter(app, 'p-size')
    expect(core.callsTo('applyEdits').map((call) => call[1])).toEqual([null, null])
  })

  it('U-108 disables Save, Suppress and the units during the command, and keeps the section view', () => {
    const { app } = parametricApp(EXTRUDE_COMMAND)
    const reason = {
      state: 'disabled',
      reason: 'Finish or cancel Extrude first.',
      code: 'finish_command_first',
    }
    expect(app.commands.get('file.save')?.availability(app)).toEqual(reason)
    expect(app.commands.get('edit.suppress')?.availability(app)).toEqual(reason)
    expect(app.commands.get('document.units.in')?.availability(app)).toEqual(reason)
    expect(app.commands.get('view.section')?.availability(app)).toEqual({ state: 'enabled' })
  })

  it('refuses other document edits during the command with finish_command_first', async () => {
    const { app, core } = parametricApp(EXTRUDE_COMMAND)
    await setBodyVisible(app, 'b-cube', false)
    await setSuppressed(app, ['f-ext1'], true)
    expect(core.callsTo('applyEdits')).toEqual([])
    expect(app.store.getState().notifications.map((notification) => notification.message)).toEqual([
      MESSAGES.finish_command_first('Extrude'),
    ])
  })
})
