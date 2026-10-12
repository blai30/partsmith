import { clsx } from 'clsx/lite'
import { Shapes, Trash2, type LucideIcon } from 'lucide-react'
import { useEffect, useId, useState, useSyncExternalStore, type ReactNode } from 'react'
import { setBodyMaterial, setBodyVisible } from '../app/actions/bodies'
import { editFeature } from '../app/actions/command'
import { select, setHover } from '../app/actions/selection'
import {
  addSketchDimension,
  changeSketchDimension,
  commitSketchChange,
  createSketchIds,
  deleteSketchItems,
  sketchParamsOf,
} from '../app/actions/sketch'
import { setSuppressed } from '../app/actions/timeline'
import { useApp, useAppState } from '../app/AppProvider'
import { BodyColorButton } from '../browser/BodyColor'
import { FEATURE_ICONS, CONSTRAINT_ICONS } from '../commands/icons'
import { ariaKeyShortcut, formatShortcut } from '../commands/shortcuts'
import { Button } from '../components/Button'
import { Switch } from '../components/Switch'
import { Tooltip } from '../components/Tooltip'
import { featureLabel } from '../core/labels'
import { ExpressionInput } from '../expression/ExpressionInput'
import { formatPoint, formatQuantity, formatSize } from '../expression/format'
import type {
  CurveKind,
  SketchConstraint,
  SketchSolution,
  SurfaceKind,
  Units,
} from '../generated/protocol'
import { MESSAGES } from '../messages'
import {
  applicableConstraints,
  CONSTRAINT_TITLES,
  DIMENSION_TITLES,
  isDimension,
  type ConstraintType,
} from '../sketch/constraints'
import { selectEditLabel } from '../state/selectors'
import type { AppState, DocumentState, SelectionItem } from '../state/store'
import { COLOR_CLASSES, FONT_CLASSES } from '../theme/roles'
import { formatArea } from './area'
import { entitySize, entitySizeDimension, type EntitySize } from './entity-size'
import { measuredBody, measureSelection, selectionMeasurements, volumeMeter } from './measurements'
import { suggestedCommands } from './suggestions'
import { UseSuggestion } from './UseSuggestion'

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

function report(action: string): (error: unknown) => void {
  return (error) => {
    console.error(`${action} failed.`, error)
  }
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

/** A shortcut shown on a suggestion button: seen, but given to assistive technology through `aria-keyshortcuts`. */
function ShortcutHint({ text }: { readonly text: string }) {
  return (
    <span aria-hidden="true" className={clsx('text-xs', COLOR_CLASSES.textMuted.foreground)}>
      {text}
    </span>
  )
}

/** A command the selection suggests, run with the selection preselected (3.8). */
function SuggestedCommand({ id }: { readonly id: string }) {
  const app = useApp()
  const definition = app.commands.get(id)
  // A suggestion whose command is not available now is left out (2.10, 3.8).
  const available = useAppState(() => definition?.availability(app).state === 'enabled')
  if (definition === undefined || !available) {
    return null
  }
  const Icon: LucideIcon = definition.icon ?? Shapes
  const [shortcut] = definition.shortcuts
  return (
    <Button
      aria-keyshortcuts={
        shortcut === undefined ? undefined : ariaKeyShortcut(shortcut, app.platform)
      }
      onClick={() => {
        app.commands.run(app, id, 'inspector').catch(report(`The command ${id}`))
      }}
    >
      <Icon aria-hidden="true" strokeWidth={1.75} className="size-4" />
      {definition.title}
      {shortcut === undefined ? null : (
        <ShortcutHint text={formatShortcut(shortcut, app.platform)} />
      )}
    </Button>
  )
}

/** The suggested commands of 3.8 for the selection, in order. */
function Suggestions({ items }: { readonly items: readonly SelectionItem[] }) {
  const ids = suggestedCommands(useApp().store.getState(), items)
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

/**
 * The measure rows of the selection (3.8, `measure-row`): selecting measures, with no tool. A
 * body's volume comes from the core once per model mesh; its row is empty until then.
 */
function MeasureRows({ items }: { readonly items: readonly SelectionItem[] }) {
  const app = useApp()
  const meter = volumeMeter(app)
  const meshId = useAppState((state) => measuredBody(state, items)?.meshId ?? null)
  // Re-renders when the measured volume arrives.
  useSyncExternalStore(meter.subscribe, () => (meshId === null ? null : meter.volume(meshId)))
  useEffect(() => {
    if (meshId !== null) {
      measureSelection(app, items).catch(report('Measuring the selection'))
    }
  }, [app, items, meshId])
  const rows = selectionMeasurements(app, items)
  return rows.map((row) => (
    <div
      key={row.measure}
      data-testid="measure-row"
      data-measure={row.measure}
      className="flex min-h-8 items-center justify-between gap-3 text-sm"
    >
      <span className={COLOR_CLASSES.textMuted.foreground}>{row.label}</span>
      <span className={clsx('min-w-0 text-right tabular-nums select-text', FONT_CLASSES.values)}>
        {row.value}
      </span>
    </div>
  ))
}

function BodySection({
  document,
  bodyId,
  items,
}: {
  readonly document: DocumentState
  readonly bodyId: string
  readonly items: readonly SelectionItem[]
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
        <MeasureRows items={items} />
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
                setBodyMaterial(app, body.id, material).catch(report('Changing the material'))
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
              <option key={name} value={name}>
                {name}
              </option>
            ))}
          </datalist>
        </div>
        <Switch
          label="Visible"
          checked={body.visible !== false}
          onChange={(visible) => {
            setBodyVisible(app, body.id, visible).catch(report('Changing the visibility'))
          }}
        />
      </div>
      <Suggestions items={items} />
    </>
  )
}

function FeatureSection({
  state,
  document,
  featureId,
  items,
}: {
  readonly state: AppState
  readonly document: DocumentState
  readonly featureId: string
  readonly items: readonly SelectionItem[]
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
            <li
              // Messages carry no id; their order is the core's.
              // oxlint-disable-next-line react/no-array-index-key
              key={index}
              className="flex flex-col gap-1"
            >
              <span
                className={
                  message.severity === 'error'
                    ? COLOR_CLASSES.errorText.foreground
                    : COLOR_CLASSES.warningText.foreground
                }
              >
                {message.message}
              </span>
              <UseSuggestion record={record} message={message} />
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
          <Button
            onClick={() => {
              editFeature(app, featureId).catch(report('Editing the feature'))
            }}
          >
            {selectEditLabel(state, featureId)}
          </Button>
        )}
        <Button
          onClick={() => {
            setSuppressed(app, [featureId], !suppressed).catch(report('Suppressing the feature'))
          }}
        >
          {suppressed ? 'Unsuppress' : 'Suppress'}
        </Button>
      </div>
      <Suggestions items={items} />
    </>
  )
}

/** The selection item of a body, as its browser row selects it (3.6 keys). */
function bodySelectionItem(id: string, name: string): SelectionItem {
  return {
    key: `body:${id}`,
    kind: 'body',
    label: name,
    body: id,
    feature: null,
    element: null,
    sketchItem: null,
    origin: null,
  }
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
  const key = `feature:${featureId}`
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
        editFeature(app, featureId).catch(report('Editing the feature'))
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

function ElementsSection({
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
  const title = single ? counted(items.length, ELEMENT_NOUNS[kind]) : itemsCount(items.length)
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
    <Row label="Position">
      <Value>{formatPoint(element.centroid, units)}</Value>
    </Row>
  )
}

/**
 * The selection inspector's fields of one dimension (3.8): its value, committed on Enter, and the
 * Reference switch, both as "Change dimension". A driving value is probed first (3.16.7): when the
 * sketch cannot take it, the text stays and the field shows `dimension_unsolvable`.
 */
function DimensionFields({
  constraint,
}: {
  readonly constraint: Extract<SketchConstraint, { expr: string }>
}) {
  const app = useApp()
  const [text, setText] = useState(constraint.expr)
  const [error, setError] = useState<string | null>(null)
  const change = (changed: SketchConstraint): void => {
    changeSketchDimension(app, changed)
      .then((result) => {
        setError(result.outcome === 'committed' ? null : MESSAGES.dimension_unsolvable())
      })
      .catch(report('Changing the dimension'))
  }
  return (
    <>
      <ExpressionInput
        id={`dimension-${constraint.id}`}
        label="Dimension value"
        value={text}
        offerParameter
        unit={constraint.type === 'angle' ? 'angle' : 'length'}
        error={error}
        onChange={(next) => {
          setText(next)
        }}
        onCommit={(next) => {
          change({ ...constraint, expr: next })
        }}
        onCancel={() => {
          setText(constraint.expr)
          setError(null)
        }}
      />
      <Switch
        label="Reference"
        checked={constraint.driving === false}
        onChange={(reference) => {
          change(withDriving(constraint, !reference))
        }}
      />
    </>
  )
}

/** A dimension with its driving flag; sketch.md omits `driving` when true. */
function withDriving(
  constraint: Extract<SketchConstraint, { expr: string }>,
  driving: boolean,
): SketchConstraint {
  if (driving) {
    const { driving: _driving, ...rest } = constraint
    return rest
  }
  return { ...constraint, driving: false }
}

/** Why the size field did not commit: a new dimension would over-constrain, or the sketch has no solution. */
type SizeProblem =
  | { readonly kind: 'over_constrained' }
  | { readonly kind: 'message'; readonly message: string }

/**
 * The size field of one line, circle or arc (3.8, `entity-size`): the driving dimension's
 * expression or the solved size. Enter with a changed valid text replaces that dimension ("Change
 * dimension") or adds a driving one ("Dimension"), which then shows on the canvas.
 */
function SizeField({
  item,
  size,
  solution,
}: {
  readonly item: SelectionItem
  readonly size: EntitySize
  readonly solution: SketchSolution
}) {
  const app = useApp()
  const [text, setText] = useState(size.text)
  const [problem, setProblem] = useState<SizeProblem | null>(null)
  const commit = (expression: string, reference: boolean): void => {
    const dimension = entitySizeDimension(size, item, expression, solution, createSketchIds(app), {
      reference,
    })
    const committed =
      size.dimension === null
        ? addSketchDimension(app, dimension)
        : changeSketchDimension(app, dimension)
    committed
      .then((result) => {
        if (result.outcome === 'committed') {
          setProblem(null)
        } else if (result.outcome === 'over_constrained' && size.dimension === null) {
          setProblem({ kind: 'over_constrained' })
        } else {
          setProblem({
            kind: 'message',
            message: result.outcome === 'failed' ? result.message : MESSAGES.dimension_unsolvable(),
          })
        }
      })
      .catch(report('Changing the size'))
  }
  return (
    <div className="flex flex-col gap-2">
      <ExpressionInput
        id={`entity-size-${size.entity}`}
        testId="entity-size"
        label={size.label}
        value={text}
        unit="length"
        offerParameter
        error={problem?.kind === 'message' ? problem.message : null}
        onChange={(next) => {
          setText(next)
        }}
        onCommit={(next, state) => {
          if (state.state === 'valid' && next.trim() !== size.text) {
            commit(next, false)
          }
        }}
        onCancel={() => {
          setText(size.text)
          setProblem(null)
        }}
      />
      {problem?.kind === 'over_constrained' ? (
        <div role="alert" className="flex flex-col gap-2 text-sm">
          <p>{MESSAGES.dimension_over_constrained()}</p>
          <div className="flex justify-end gap-2">
            <Button
              variant="primary"
              onClick={() => {
                commit(text, true)
              }}
            >
              Add reference
            </Button>
            <Button
              onClick={() => {
                setText(size.text)
                setProblem(null)
              }}
            >
              Cancel
            </Button>
          </div>
        </div>
      ) : null}
    </div>
  )
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
        deleteSketchItems(app, items).catch(report('Deleting the sketch items'))
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
            <DimensionFields key={constraint.expr} constraint={constraint} />
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
  const entities = items.flatMap((item) => {
    const reference = item.sketchItem
    if (reference?.kind === 'entity') {
      return [reference.entity]
    }
    // A point entity is selected by its point reference, which is its bare id.
    return reference?.kind === 'point' &&
      solution?.entities.some((entity) => entity.id === reference.point) === true
      ? [reference.point]
      : []
  })
  const construction =
    entities.length > 0 &&
    entities.every(
      (entity) =>
        solution?.entities.find((candidate) => candidate.id === entity)?.construction === true,
    )
  const constraints: readonly ConstraintType[] =
    solution === null ? [] : applicableConstraints(items, solution)
  const size =
    solution === null || params === null
      ? null
      : entitySize(items, params, solution, document.recipe.units)
  return (
    <>
      <Title>{title}</Title>
      {size === null || solution === null || only === undefined ? null : (
        <SizeField
          key={`${size.entity} ${size.text}`}
          item={only}
          size={size}
          solution={solution}
        />
      )}
      {constraints.length === 0 ? null : (
        <div className="flex flex-wrap gap-2">
          {constraints.map((type) => {
            const Glyph = CONSTRAINT_ICONS[type]
            const id = `sketch.constrain_${type}`
            const [shortcut] = app.commands.get(id)?.shortcuts ?? []
            return (
              <Button
                key={type}
                aria-keyshortcuts={
                  shortcut === undefined ? undefined : ariaKeyShortcut(shortcut, app.platform)
                }
                onClick={() => {
                  app.commands.run(app, id, 'inspector').catch(report(`The command ${id}`))
                }}
              >
                <Glyph aria-hidden="true" strokeWidth={1.75} className="size-4" />
                {CONSTRAINT_TITLES[type]}
                {shortcut === undefined ? null : (
                  <ShortcutHint text={formatShortcut(shortcut, app.platform)} />
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
            }).catch(report('Changing construction'))
          }}
        />
      )}
      <div className="flex">{remove}</div>
    </>
  )
}

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
  if (allElements(items)) {
    return <ElementsSection state={state} document={document} items={items} />
  }
  const title = items.every((item) => item.kind === 'sketch_region')
    ? counted(items.length, ['region', 'regions'])
    : items.every((item) => item.kind === 'sketch_point')
      ? counted(items.length, ['point', 'points'])
      : items.every((item) => item.kind === 'body')
        ? counted(items.length, ['body', 'bodies'])
        : itemsCount(items.length)
  return (
    <>
      <Title>{title}</Title>
      <Suggestions items={items} />
    </>
  )
}
