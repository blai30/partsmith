import { Trash2 } from 'lucide-react'
import { useState } from 'react'
import {
  addSketchDimension,
  changeSketchDimension,
  commitSketchChange,
  createSketchIds,
  deleteSketchItems,
  sketchParamsOf,
} from '../../app/actions/sketch'
import { useApp } from '../../app/AppProvider'
import { reportFailure } from '../../app/report'
import { CONSTRAINT_ICONS } from '../../commands/icons'
import { Button } from '../../components/Button'
import { Switch } from '../../components/Switch'
import { ExpressionInput } from '../../expression/ExpressionInput'
import type { SketchConstraint, SketchSolution } from '../../generated/protocol'
import { counted, MESSAGES } from '../../messages'
import {
  applicableConstraints,
  CONSTRAINT_TITLES,
  DIMENSION_TITLES,
  isDimension,
  type ConstraintType,
} from '../../sketch/constraints'
import type { AppState, DocumentState, SelectionItem } from '../../state/store'
import { entitySize, entitySizeDimension, type EntitySize } from '../entity-size'
import { InspectorCommandButton, Title } from './parts'

// The selection inspector in a sketch (ui.md 3.8): a dimension's value, a curve's size field, the
// constraints that fit the selection, construction and Delete.

const ENTITY_NOUNS: Readonly<
  Record<'point' | 'line' | 'circle' | 'arc', readonly [singular: string, plural: string]>
> = {
  point: ['point', 'points'],
  line: ['line', 'lines'],
  circle: ['circle', 'circles'],
  arc: ['arc', 'arcs'],
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
      .catch(reportFailure('Changing the dimension'))
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
      .catch(reportFailure('Changing the size'))
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

/** The section of the selection inside the edited sketch. */
export function SketchSection({
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
        deleteSketchItems(app, items).catch(reportFailure('Deleting the sketch items'))
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
          <Title>{counted(1, 'item')}</Title>
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
        {dimension ? <DimensionFields key={constraint.expr} constraint={constraint} /> : null}
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
  const title = uniform
    ? counted(items.length, ...ENTITY_NOUNS[firstType])
    : counted(items.length, 'item')
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
          {constraints.map((type) => (
            <InspectorCommandButton
              key={type}
              id={`sketch.constrain_${type}`}
              icon={CONSTRAINT_ICONS[type]}
              title={CONSTRAINT_TITLES[type]}
            />
          ))}
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
            }).catch(reportFailure('Changing construction'))
          }}
        />
      )}
      <div className="flex">{remove}</div>
    </>
  )
}
