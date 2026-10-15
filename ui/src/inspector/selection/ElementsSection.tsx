import { useEffect } from 'react'
import { editFeature } from '../../app/actions/command'
import { select, setHover } from '../../app/actions/selection'
import { useApp } from '../../app/AppProvider'
import { reportFailure } from '../../app/report'
import { Button } from '../../components/Button'
import { Tooltip } from '../../components/Tooltip'
import { featureLabel } from '../../core/labels'
import { formatPoint, formatQuantity } from '../../expression/format'
import type { CurveKind, SurfaceKind, Units } from '../../generated/protocol'
import { counted } from '../../messages'
import type { AppState, DocumentState, SelectionItem } from '../../state/store'
import { COLOR_CLASSES } from '../../theme/roles'
import { formatArea } from '../area'
import { bodySelectionItem, featureKey } from '../selection-values'
import { CoordinateReadout, MeasureRows, ReadoutRow, Row, Suggestions, Title, Value } from './parts'

// The selection inspector of faces, edges and vertices (ui.md 3.8): their bodies, the feature that
// made one of them, and its measures.

const SURFACE_NAMES: Readonly<Record<SurfaceKind, string>> = {
  plane: 'Flat face',
  cylinder: 'Cylindrical face',
  cone: 'Conical face',
  sphere: 'Spherical face',
  torus: 'Toroidal face',
  bspline: 'Freeform face',
  other: 'Face',
}

const CURVE_NAMES: Readonly<Record<CurveKind, string>> = {
  line: 'Straight edge',
  circle: 'Circular edge',
  ellipse: 'Elliptical edge',
  bspline: 'Freeform edge',
  other: 'Edge',
}

/**
 * "Made by {label}" for one face, edge or vertex (3.8): "Edit {label}" opens the feature that made
 * it, and hovering the row lights up that feature's timeline item.
 */
function MadeBy({
  state,
  featureId,
  face,
}: {
  readonly state: AppState
  readonly featureId: string
  readonly face: boolean
}) {
  const app = useApp()
  const key = featureKey(featureId)
  // The row's hover ends with the row, so no timeline item stays lit after the selection changes.
  useEffect(
    () => () => {
      if (app.store.getState().hover?.key === key) {
        setHover(app, null)
      }
    },
    [app, key],
  )
  const document = state.document
  if (document === null) {
    return null
  }
  const label = featureLabel(document.recipe, featureId, state.catalog)
  const button = (
    <Button
      onClick={() => {
        setHover(app, null)
        editFeature(app, featureId).catch(reportFailure('Editing the feature'))
      }}
    >
      Edit {label}
    </Button>
  )
  return (
    // Hovering is a pointer convenience; the button carries the action for every input.
    // oxlint-disable-next-line jsx-a11y/no-static-element-interactions
    <div
      data-testid="made-by"
      onMouseEnter={() => {
        setHover(app, { key, kind: 'feature' })
      }}
      onMouseLeave={() => {
        if (app.store.getState().hover?.key === key) {
          setHover(app, null)
        }
      }}
      className="flex min-h-8 flex-wrap items-center justify-between gap-2 text-sm"
    >
      <span>Made by {label}</span>
      {face ? (
        <Tooltip label={`Edit ${label}`} description="Or double-click the face.">
          {button}
        </Tooltip>
      ) : (
        button
      )}
    </div>
  )
}

/** The section of selected faces, edges and vertices. */
export function ElementsSection({
  state,
  document,
  items,
}: {
  readonly state: AppState
  readonly document: DocumentState
  readonly items: readonly SelectionItem[]
}) {
  const app = useApp()
  const units = document.recipe.units
  const kinds = new Set(items.map((item) => item.kind))
  const [first] = items
  const element = first?.element ?? null
  const kind = first?.kind
  const single = kinds.size === 1 && (kind === 'face' || kind === 'edge' || kind === 'vertex')
  const noun = single ? kind : 'item'
  const title =
    noun === 'vertex' ? counted(items.length, noun, 'vertices') : counted(items.length, noun)
  const bodies = [...new Set(items.flatMap((item) => (item.body === null ? [] : [item.body])))]
    .map((id) => document.recipe.bodies?.find((candidate) => candidate.id === id))
    .filter((body) => body !== undefined)
  const madeBy =
    items.length === 1 &&
    first?.feature !== null &&
    first?.feature !== undefined &&
    document.recipe.features?.some((feature) => feature.id === first.feature) === true
      ? first.feature
      : null
  return (
    <>
      <Title>{title}</Title>
      <div className="flex flex-col">
        {bodies.length === 0 ? null : (
          <div className="flex min-h-8 flex-wrap items-center justify-between gap-2 text-sm">
            <span className={COLOR_CLASSES.textMuted.foreground}>
              {bodies.length === 1 ? 'Body' : 'Bodies'}
            </span>
            <span className="flex flex-wrap justify-end gap-2">
              {bodies.map((body) => (
                <Button
                  key={body.id}
                  onClick={() => {
                    select(app, [bodySelectionItem(body.id, body.name)], 'replace')
                  }}
                >
                  Select {body.name}
                </Button>
              ))}
            </span>
          </div>
        )}
        {madeBy === null ? null : (
          <MadeBy state={state} featureId={madeBy} face={kind === 'face'} />
        )}
        {items.length === 1 && element !== null ? (
          <ElementMeasures element={element} kind={kind} units={units} />
        ) : null}
        <MeasureRows items={items} />
      </div>
      <Suggestions items={items} />
    </>
  )
}

function ElementMeasures({
  element,
  kind,
  units,
}: {
  readonly element: NonNullable<SelectionItem['element']>
  readonly kind: SelectionItem['kind'] | undefined
  readonly units: Units
}) {
  if (kind === 'face') {
    return (
      <>
        <Row label="Surface">{SURFACE_NAMES[element.surface ?? 'other']}</Row>
        {element.area === null || element.area === undefined ? null : (
          <Row label="Area">
            <Value>{formatArea(element.area, units)}</Value>
          </Row>
        )}
      </>
    )
  }
  if (kind === 'edge') {
    return (
      <>
        <Row label="Curve">{CURVE_NAMES[element.curve ?? 'other']}</Row>
        {element.length === null || element.length === undefined ? null : (
          <Row label="Length">
            <Value>{formatQuantity(element.length, 'length', units)}</Value>
          </Row>
        )}
      </>
    )
  }
  return (
    <ReadoutRow label="Position">
      <CoordinateReadout
        testId="vertex-position"
        coordinates={element.centroid}
        units={units}
        text={formatPoint(element.centroid, units)}
      />
    </ReadoutRow>
  )
}
