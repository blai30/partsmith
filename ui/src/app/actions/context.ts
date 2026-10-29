import { selectSketchToEdit } from '../../state/selectors'
import type { AppContext } from '../../state/store'
import type { App } from '../createApp'
import { cancelCommandImplicitly, startFeatureCommand } from './command'
import { enterSketch, finishSketch } from './sketch'

// Switching between the Model, Sketch and Print contexts (ui.md 3.3).

export async function setContext(app: App, context: AppContext): Promise<void> {
  const state = app.store.getState()
  if (context === 'sketch') {
    // In sketch mode, choosing Sketch again changes nothing. Elsewhere it edits the selected
    // sketch, or starts a new one as the Sketch command does.
    if (state.context !== 'sketch') {
      const sketchId = selectSketchToEdit(state)
      await (sketchId === null ? startFeatureCommand(app, 'sketch') : enterSketch(app, sketchId))
    }
    return
  }
  if (state.context === 'sketch') {
    // The Model radio selects what the sketch made (3.16.11); the Print radio keeps the selection.
    await finishSketch(app, { selectResult: context === 'model' })
  } else if (state.command !== null) {
    await cancelCommandImplicitly(app)
  }
  app.store.setState({ context })
}
