import type { AppContext } from '../../state/store'
import type { App } from '../createApp'
import { cancelCommand } from './command'
import { finishSketch, startNewSketch } from './sketch'

// Switching between the Model, Sketch and Print contexts (ui.md 3.3).

export async function setContext(app: App, context: AppContext): Promise<void> {
  const state = app.store.getState()
  if (context === 'sketch') {
    // In sketch mode, choosing Sketch again changes nothing; elsewhere it starts a new sketch.
    if (state.context !== 'sketch') {
      await startNewSketch(app)
    }
    return
  }
  if (state.context === 'sketch') {
    // The Model radio selects what the sketch made (3.16.11); the Print radio keeps the selection.
    await finishSketch(app, { selectResult: context === 'model' })
  } else if (state.command !== null) {
    await cancelCommand(app)
  }
  app.store.setState({ context })
}
