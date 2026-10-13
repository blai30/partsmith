import { openSidePanel } from '../app/actions/panels'
import { useApp, useAppState } from '../app/AppProvider'
import { Button } from '../components/Button'
import { counted } from '../messages'
import type { SelectionItem } from '../state/store'
import { BodySection } from './selection/BodySection'
import { ElementsSection } from './selection/ElementsSection'
import { FeatureSection } from './selection/FeatureSection'
import { Suggestions, Title } from './selection/parts'
import { SketchSection } from './selection/SketchSection'

// The selection inspector (ui.md 3.8): what the selection is, its measures, and the commands that
// fit it, so the next step is one click away.

/** Whether every item is a face, an edge or a vertex. */
function allElements(items: readonly SelectionItem[]): boolean {
  return items.every(
    (item) => item.kind === 'face' || item.kind === 'edge' || item.kind === 'vertex',
  )
}

/** The selection inspector's content for the shown selection. */
export function SelectionInspector({ items }: { readonly items: readonly SelectionItem[] }) {
  const app = useApp()
  const document = useAppState((state) => state.document)
  const context = useAppState((state) => state.context)
  useAppState((state) => state.catalog)
  useAppState((state) => state.sketch)
  const state = app.store.getState()
  if (document === null) {
    return null
  }
  if (context === 'sketch') {
    return <SketchSection state={state} document={document} items={items} />
  }
  const [only] = items
  if (items.length === 1 && only !== undefined) {
    if (only.kind === 'body' && only.body !== null) {
      return <BodySection key={only.body} document={document} bodyId={only.body} items={items} />
    }
    if ((only.kind === 'feature' || only.kind === 'datum_plane') && only.feature !== null) {
      return (
        <FeatureSection state={state} document={document} featureId={only.feature} items={items} />
      )
    }
    if (
      only.kind === 'origin_plane' ||
      only.kind === 'origin_axis' ||
      only.kind === 'origin_point'
    ) {
      return (
        <>
          <Title>{only.label}</Title>
          <Suggestions items={items} />
        </>
      )
    }
    if (only.kind === 'parameter') {
      return (
        <>
          <Title>{only.label}</Title>
          <div className="flex">
            <Button
              onClick={() => {
                openSidePanel(app, 'parameters')
              }}
            >
              Edit parameters
            </Button>
          </div>
        </>
      )
    }
  }
  if (allElements(items)) {
    return <ElementsSection state={state} document={document} items={items} />
  }
  const title = items.every((item) => item.kind === 'sketch_region')
    ? counted(items.length, 'region')
    : items.every((item) => item.kind === 'sketch_point')
      ? counted(items.length, 'point')
      : items.every((item) => item.kind === 'body')
        ? counted(items.length, 'body', 'bodies')
        : counted(items.length, 'item')
  return (
    <>
      <Title>{title}</Title>
      <Suggestions items={items} />
    </>
  )
}
