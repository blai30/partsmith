import { clsx } from 'clsx/lite'
import { Shapes, Trash2, type LucideIcon } from 'lucide-react'
import { useId, useState, type ReactNode } from 'react'
import { setBodyMaterial, setBodyVisible } from '../app/actions/bodies'
import { editFeature } from '../app/actions/command'
import { commitSketchChange, deleteSketchItems, sketchParamsOf } from '../app/actions/sketch'
import { setSuppressed } from '../app/actions/timeline'
import { useApp, useAppState } from '../app/AppProvider'
import type { App } from '../app/createApp'
import { BodyColorButton } from '../browser/BodyColor'
import { FEATURE_ICONS, CONSTRAINT_ICONS } from '../commands/icons'
import { formatShortcut } from '../commands/shortcuts'
import { Button } from '../components/Button'
import { Switch } from '../components/Switch'
import { Tooltip } from '../components/Tooltip'
import { featureLabel } from '../core/labels'
import { ExpressionInput } from '../expression/ExpressionInput'
import { formatPoint, formatQuantity, formatSize } from '../expression/format'
import type { CurveKind, SketchConstraint, SurfaceKind, Units } from '../generated/protocol'
import { MESSAGES } from '../messages'
import {
  applicableConstraints,
  CONSTRAINT_TITLES,
  DIMENSION_TITLES,
  isDimension,
  type ConstraintType,
} from '../sketch/constraints'
import type { AppState, DocumentState, SelectionItem } from '../state/store'
import { COLOR_CLASSES, FONT_CLASSES } from '../theme/roles'
import { formatArea } from './area'

// The selection inspector (ui.md 3.8): what the selection is, its measures, and the commands that
// fit it, so the next step is one click away.

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

const ELEMENT_NOUNS = {
  face: ['face', 'faces'],
  edge: ['edge', 'edges'],
  vertex: ['vertex', 'vertices'],
} as const

const ENTITY_NOUNS = {
  point: ['point', 'points'],
  line: ['line', 'lines'],
  circle: ['circle', 'circles'],
  arc: ['arc', 'arcs'],
} as const

const MATERIALS = ['PLA', 'PETG', 'ABS', 'ASA', 'TPU', 'Resin'] as const

/** "1 face", "3 edges": a count with its noun. */
function counted(count: number, nouns: readonly [string, string]): string {
  return `${String(count)} ${count === 1 ? nouns[0] : nouns[1]}`
}

function itemsCount(count: number): string {
  return `${String(count)} ${count === 1 ? 'item' : 'items'}`
}

function Title({ children }: { readonly children: ReactNode }) {
  return (
    <h2 data-testid="selection-title" className="text-base font-semibold tabular-nums">
      {children}
    </h2>
  )
}

function Row({ label, children }: { readonly label: string; readonly children: ReactNode }) {
  return (
    <div className="flex min-h-8 items-center justify-between gap-3 text-sm">
      <span className={COLOR_CLASSES.textMuted.foreground}>{label}</span>
      <span className="min-w-0 text-right">{children}</span>
    </div>
  )
}

function Value({ testId, children }: { readonly testId?: string; readonly children: ReactNode }) {
  return (
    <span data-testid={testId} className={clsx('tabular-nums', FONT_CLASSES.values)}>
      {children}
    </span>
  )
}

/** A command the selection suggests, run with the selection preselected (3.8). */
function SuggestedCommand({ id }: { readonly id: string }) {
  const app = useApp()
  const definition = app.commands.get(id)
  if (definition === undefined) {
    return null
  }
  const Icon: LucideIcon = definition.icon ?? Shapes
  const [shortcut] = definition.shortcuts
  return (
    <Button
      onClick={() => {
        app.commands.run(app, id, 'inspector').catch((error: unknown) => {
          console.error(`The command ${id} failed.`, error)
        })
      }}
    >
      <Icon aria-hidden="true" strokeWidth={1.75} className="size-4" />
      {definition.title}
      {shortcut === undefined ? null : (
        <span className={clsx('text-xs', COLOR_CLASSES.textMuted.foreground)}>
          {formatShortcut(shortcut, app.platform)}
        </span>
      )}
    </Button>
  )
}

function Suggestions({ ids }: { readonly ids: readonly string[] }) {
  if (ids.length === 0) {
    return null
  }
  return (
    <div className="flex flex-wrap gap-2">
      {ids.map((id) => (
        <SuggestedCommand key={id} id={id} />
      ))}
    </div>
  )
}

function BodySection({
  document,
  bodyId,
}: {
  readonly document: DocumentState
  readonly bodyId: string
}) {
  const app = useApp()
  const materialId = useId()
  const listId = useId()
  const body = document.recipe.bodies?.find((candidate) => candidate.id === bodyId)
  const [material, setMaterial] = useState(body?.material ?? '')
  if (body === undefined) {
    return <Title>{bodyId}</Title>
  }
  const units = document.recipe.units
  const mesh = document.meshes[body.id]
  const box = mesh?.boundingBox
  const center =
    box === undefined
      ? null
      : formatPoint(
          [
            (box.min[0] + box.max[0]) / 2,
            (box.min[1] + box.max[1]) / 2,
            (box.min[2] + box.max[2]) / 2,
          ],
          units,
        )
  return (
    <>
      <Title>{body.name}</Title>
      <div className="flex flex-col">
        <Row label="Size">
          <Value testId="body-size">{box === undefined ? 'Empty' : formatSize(box, units)}</Value>
        </Row>
        {center === null ? null : (
          <Row label="Center">
            <Value testId="body-center">{center}</Value>
          </Row>
        )}
        <Row label="Color">
          <BodyColorButton body={body} size="field" />
        </Row>
        <div className="flex min-h-8 items-center justify-between gap-3 text-sm">
          <label htmlFor={materialId} className={COLOR_CLASSES.textMuted.foreground}>
            Material
          </label>
          <input
            id={materialId}
            list={listId}
            value={material}
            autoComplete="off"
            onChange={(event) => {
              setMaterial(event.currentTarget.value)
            }}
            onKeyDown={(event) => {
              if (event.key === 'Enter') {
                event.preventDefault()
                setBodyMaterial(app, body.id, material).catch((error: unknown) => {
                  console.error('Changing the material failed.', error)
                })
              }
            }}
            className={clsx(
              'h-8 w-36 rounded-sm border px-2 text-sm',
              COLOR_CLASSES.field.background,
              COLOR_CLASSES.text.foreground,
              COLOR_CLASSES.borderControl.border,
              COLOR_CLASSES.focusRing.outline,
            )}
          />
          <datalist id={listId}>
            {MATERIALS.map((name) => (
              <option key={name} value={name} />
            ))}
          </datalist>
        </div>
        <Switch
          label="Visible"
          checked={body.visible !== false}
          onChange={(visible) => {
            setBodyVisible(app, body.id, visible).catch((error: unknown) => {
              console.error('Changing the visibility failed.', error)
            })
          }}
        />
      </div>
    </>
  )
}

function FeatureSection({
  state,
  document,
  featureId,
  suggestions,
}: {
  readonly state: AppState
  readonly document: DocumentState
  readonly featureId: string
  readonly suggestions: readonly string[]
}) {
  const app = useApp()
  const record = document.recipe.features?.find((feature) => feature.id === featureId)
  const label = featureLabel(document.recipe, featureId, state.catalog)
  if (record === undefined) {
    return <Title>{label}</Title>
  }
  const manifest = state.catalog?.byType[record.type]?.manifest
  const Icon = FEATURE_ICONS[manifest?.icon ?? ''] ?? Shapes
  const status = document.featureStatus[featureId]
  const messages = (status?.messages ?? []).filter(
    (message) => message.severity === 'error' || message.severity === 'warning',
  )
  const suppressed = record.suppressed === true
  const importFeature = record.type === 'import'
  const edit = () => {
    editFeature(app, featureId).catch((error: unknown) => {
      console.error('Editing the feature failed.', error)
    })
  }
  return (
    <>
      <Title>{label}</Title>
      <Row label="Type">
        <span className="inline-flex items-center gap-2">
          <Icon aria-hidden="true" strokeWidth={1.75} className="size-4" />
          {manifest?.label ?? record.type}
        </span>
      </Row>
      {messages.length === 0 ? null : (
        <ul className="flex flex-col gap-2 text-sm">
          {messages.map((message, index) => (
            // Messages carry no id; their order is the core's.
            // oxlint-disable-next-line react/no-array-index-key
            <li
              key={index}
              className={
                message.severity === 'error'
                  ? COLOR_CLASSES.errorText.foreground
                  : COLOR_CLASSES.warningText.foreground
              }
            >
              {message.message}
            </li>
          ))}
        </ul>
      )}
      <div className="flex flex-wrap gap-2">
        {importFeature ? (
          <Tooltip label={MESSAGES.import_not_editable()}>
            <Button aria-disabled="true">Edit</Button>
          </Tooltip>
        ) : (
          <Button onClick={edit}>Edit</Button>
        )}
        <Button
          onClick={() => {
            setSuppressed(app, [featureId], !suppressed).catch((error: unknown) => {
              console.error('Suppressing the feature failed.', error)
            })
          }}
        >
          {suppressed ? 'Unsuppress' : 'Suppress'}
        </Button>
      </div>
      <Suggestions ids={suggestions} />
    </>
  )
}

function ElementsSection({
  document,
  items,
}: {
  readonly document: DocumentState
  readonly items: readonly SelectionItem[]
}) {
  const units = document.recipe.units
  const kinds = new Set(items.map((item) => item.kind))
  const [first] = items
  const element = first?.element ?? null
  const kind = first?.kind
  const single = kinds.size === 1 && (kind === 'face' || kind === 'edge' || kind === 'vertex')
  const title = single ? counted(items.length, ELEMENT_NOUNS[kind]) : itemsCount(items.length)
  const bodies = [
    ...new Set(
      items.flatMap((item) => {
        const body = document.recipe.bodies?.find((candidate) => candidate.id === item.body)
        return body === undefined ? [] : [body.name]
      }),
    ),
  ]
  const allEdges = items.every((item) => item.kind === 'edge')
  const allFaces = items.every((item) => item.kind === 'face')
  const flatFace = items.length === 1 && allFaces && element?.surface === 'plane'
  const suggestions = allEdges
    ? ['feature.fillet', 'feature.chamfer']
    : flatFace
      ? ['feature.sketch', 'feature.hole', 'feature.shell']
      : allFaces
        ? ['feature.shell']
        : []
  return (
    <>
      <Title>{title}</Title>
      <div className="flex flex-col">
        {bodies.length === 0 ? null : (
          <Row label={bodies.length === 1 ? 'Body' : 'Bodies'}>{bodies.join(', ')}</Row>
        )}
        {items.length === 1 && element !== null ? (
          <ElementMeasures element={element} kind={kind} units={units} />
        ) : null}
      </div>
      <Suggestions ids={suggestions} />
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
    <Row label="Position">
      <Value>{formatPoint(element.centroid, units)}</Value>
    </Row>
  )
}

function DimensionValue({
  constraint,
  onCommit,
}: {
  readonly constraint: Extract<SketchConstraint, { expr: string }>
  readonly onCommit: (expression: string) => void
}) {
  const [text, setText] = useState(constraint.expr)
  return (
    <ExpressionInput
      id={`dimension-${constraint.id}`}
      label="Dimension value"
      value={text}
      unit={constraint.type === 'angle' ? 'angle' : 'length'}
      onChange={(next) => {
        setText(next)
      }}
      onCommit={(next) => {
        onCommit(next)
      }}
      onCancel={() => {
        setText(constraint.expr)
      }}
    />
  )
}

function replaceConstraint(app: App, label: string, constraint: SketchConstraint): void {
  commitSketchChange(app, label, { replaceConstraint: constraint, requested: [], inferred: [] })
    .then((result) => {
      if (result.outcome === 'failed') {
        console.info('The sketch change was refused.', result.message)
      }
    })
    .catch((error: unknown) => {
      console.error('Changing the dimension failed.', error)
    })
}

function SketchSection({
  state,
  document,
  items,
}: {
  readonly state: AppState
  readonly document: DocumentState
  readonly items: readonly SelectionItem[]
}) {
  const app = useApp()
  const sketchId = state.sketch?.sketchId ?? null
  const solution = sketchId === null ? null : (document.sketches[sketchId]?.solution ?? null)
  const record = document.recipe.features?.find((feature) => feature.id === sketchId)
  const params = record === undefined ? null : sketchParamsOf(record)
  const [only] = items
  const constraintId =
    items.length === 1 && only?.sketchItem?.kind === 'constraint'
      ? only.sketchItem.constraint
      : null
  const remove = (
    <Button
      onClick={() => {
        deleteSketchItems(app, items).catch((error: unknown) => {
          console.error('Deleting the sketch items failed.', error)
        })
      }}
    >
      <Trash2 aria-hidden="true" strokeWidth={1.75} className="size-4" />
      Delete
    </Button>
  )
  if (constraintId !== null) {
    const constraint = params?.constraints?.find((candidate) => candidate.id === constraintId)
    if (constraint === undefined) {
      return (
        <>
          <Title>{itemsCount(1)}</Title>
          {remove}
        </>
      )
    }
    const dimension = isDimension(constraint)
    return (
      <>
        <Title>
          {dimension ? DIMENSION_TITLES[constraint.type] : CONSTRAINT_TITLES[constraint.type]}
        </Title>
        {dimension ? (
          <>
            <DimensionValue
              key={constraint.expr}
              constraint={constraint}
              onCommit={(expression) => {
                replaceConstraint(app, 'Change dimension', { ...constraint, expr: expression })
              }}
            />
            <Switch
              label="Reference"
              checked={constraint.driving === false}
              onChange={(reference) => {
                replaceConstraint(app, 'Change dimension', { ...constraint, driving: !reference })
              }}
            />
          </>
        ) : null}
        {remove}
      </>
    )
  }
  const entityTypes = items.map((item) => {
    const reference = item.sketchItem
    if (reference?.kind === 'entity') {
      return reference.entityType
    }
    return item.kind === 'sketch_point' ? 'point' : null
  })
  const [firstType] = entityTypes
  const uniform =
    firstType !== null && firstType !== undefined && entityTypes.every((type) => type === firstType)
  const title = uniform ? counted(items.length, ENTITY_NOUNS[firstType]) : itemsCount(items.length)
  const entities = items.flatMap((item) =>
    item.sketchItem?.kind === 'entity' ? [item.sketchItem.entity] : [],
  )
  const construction =
    entities.length > 0 &&
    entities.every(
      (entity) =>
        solution?.entities.find((candidate) => candidate.id === entity)?.construction === true,
    )
  const constraints: readonly ConstraintType[] =
    solution === null ? [] : applicableConstraints(items, solution)
  return (
    <>
      <Title>{title}</Title>
      {constraints.length === 0 ? null : (
        <div className="flex flex-wrap gap-2">
          {constraints.map((type) => {
            const Glyph = CONSTRAINT_ICONS[type]
            const id = `sketch.constrain_${type}`
            const [shortcut] = app.commands.get(id)?.shortcuts ?? []
            return (
              <Button
                key={type}
                onClick={() => {
                  app.commands.run(app, id, 'inspector').catch((error: unknown) => {
                    console.error(`The command ${id} failed.`, error)
                  })
                }}
              >
                <Glyph aria-hidden="true" strokeWidth={1.75} className="size-4" />
                {CONSTRAINT_TITLES[type]}
                {shortcut === undefined ? null : (
                  <span className={clsx('text-xs', COLOR_CLASSES.textMuted.foreground)}>
                    {formatShortcut(shortcut, app.platform)}
                  </span>
                )}
              </Button>
            )
          })}
        </div>
      )}
      {entities.length === 0 ? null : (
        <Switch
          label="Construction"
          checked={construction}
          onChange={() => {
            commitSketchChange(app, 'Construction', {
              toggleConstruction: entities,
              requested: [],
              inferred: [],
            }).catch((error: unknown) => {
              console.error('Changing construction failed.', error)
            })
          }}
        />
      )}
      <div className="flex">{remove}</div>
    </>
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
      return <BodySection key={only.body} document={document} bodyId={only.body} />
    }
    if ((only.kind === 'feature' || only.kind === 'datum_plane') && only.feature !== null) {
      return (
        <FeatureSection
          state={state}
          document={document}
          featureId={only.feature}
          suggestions={only.kind === 'datum_plane' ? ['feature.sketch'] : []}
        />
      )
    }
    if (only.kind === 'origin_plane') {
      return (
        <>
          <Title>{only.label}</Title>
          <Suggestions ids={['feature.sketch']} />
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
                app.store.setState((current) => ({
                  panels: { ...current.panels, side: 'parameters' },
                }))
              }}
            >
              Edit parameters
            </Button>
          </div>
        </>
      )
    }
  }
  if (
    items.every((item) => item.kind === 'face' || item.kind === 'edge' || item.kind === 'vertex')
  ) {
    return <ElementsSection document={document} items={items} />
  }
  if (items.every((item) => item.kind === 'sketch_region')) {
    return (
      <>
        <Title>{counted(items.length, ['region', 'regions'])}</Title>
        <Suggestions ids={['feature.extrude', 'feature.revolve']} />
      </>
    )
  }
  return <Title>{itemsCount(items.length)}</Title>
}
