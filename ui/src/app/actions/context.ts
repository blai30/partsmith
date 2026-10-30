import type { AppContext } from '../../state/store'
import type { App } from '../createApp'
import { cancelCommandImplicitly } from './command'
import { editOrStartSketch, finishSketch } from './sketch'
import { afterStarts } from './starts'

// Switching between the Model, Sketch and Print contexts (ui.md 3.3).

export async function setContext(app: App, context: AppContext): Promise<void> {
  if (context === 'sketch') {
    // In sketch mode, choosing Sketch again changes nothing. Elsewhere it edits the selected
    // sketch, or starts a new one as the Sketch command does.
    if (app.store.getState().context !== 'sketch') {
      await editOrStartSketch(app)
    }
    return
  }
  // Model and Print act on what a start in progress opens, such as a sketch entered (3.16.1).
  await afterStarts(app, async () => {
    const state = app.store.getState()
    if (state.context === 'sketch') {
      // The Model radio selects what the sketch made (3.16.11); the Print radio keeps the selection.
      await finishSketch(app, { selectResult: context === 'model' })
    } else if (state.command !== null) {
      await cancelCommandImplicitly(app)
    }
    app.store.setState({ context })
  })
}
