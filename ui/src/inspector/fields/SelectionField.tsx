import { clsx } from 'clsx/lite'
import { X } from 'lucide-react'
import { useEffect, useId, useRef, useState } from 'react'
import {
  capturePick,
  chooseNewBody,
  setActiveField,
  setCommandField,
  setHoveredField,
} from '../../app/actions/command'
import { useApp, useAppState } from '../../app/AppProvider'
import { featureLabel } from '../../core/labels'
import { Button } from '../../components/Button'
import { describedBy } from '../../components/described-by'
import { HelpLabel } from '../../components/HelpLabel'
import { IconButton } from '../../components/IconButton'
import { Menu, type MenuEntry } from '../../components/Menu'
import type { SelectionKind } from '../../generated/protocol'
import type { AppState, SelectionItem } from '../../state/store'
import { COLOR_CLASSES } from '../../theme/roles'
import { isSessionVisible } from '../../state/visibility'
import { formatArea } from '../area'
import { bodyCreatingTarget, type InspectorField } from '../model'
import { itemsFromValue, originItem } from '../selection-values'
import { fieldInputId } from './field-id'

// A selection param (3.7): a box that receives viewport and browser picks while it is the active
// field, a clear button, and a Choose menu so bodies, sketches, planes, axes and regions can be
// chosen without a pointer (3.24).

const NOUNS: Readonly<Record<SelectionKind, { readonly single: string; readonly plural: string }>> =
  {
    body: { single: 'a body', plural: 'bodies' },
    sketch: { single: 'a sketch', plural: 'sketches' },
    sketch_region: { single: 'a region', plural: 'regions' },
    sketch_point: { single: 'a point', plural: 'points' },
    face: { single: 'a face', plural: 'faces' },
    edge: { single: 'an edge', plural: 'edges' },
    plane: { single: 'a plane', plural: 'planes' },
    axis: { single: 'an axis', plural: 'axes' },
  }

/** Fields whose candidates can be listed, so they work without a pointer. */
const CHOOSABLE: ReadonlySet<SelectionKind> = new Set([
  'body',
  'sketch',
  'plane',
  'axis',
  'sketch_region',
])

/** A selection item a Choose menu offers: no element, and only the data its kind carries. */
function candidate(
  key: string,
  kind: SelectionItem['kind'],
  label: string,
  data: Partial<Pick<SelectionItem, 'body' | 'feature' | 'sketchItem' | 'origin'>> = {},
): SelectionItem {
  return {
    key,
    kind,
    label,
    body: data.body ?? null,
    feature: data.feature ?? null,
    element: null,
    sketchItem: data.sketchItem ?? null,
    origin: data.origin ?? null,
  }
}

/** The placeholder of an empty box (3.7); point and multiple edge fields also take faces. */
function placeholder(accepts: SelectionKind, multiple: boolean): string {
  if (accepts === 'sketch_point') {
    return 'Click a flat face or pick sketch points'
  }
  if (accepts === 'edge' && multiple) {
    return 'Select edges or a face'
  }
  const noun = NOUNS[accepts]
  return `Select ${multiple ? noun.plural : noun.single}`
}

/** The text of the box: the placeholder, the one item's label, or a count. */
function selectionText(
  accepts: SelectionKind,
  multiple: boolean,
  items: readonly SelectionItem[],
): string {
  const [first] = items
  if (items.length === 0 || first === undefined) {
    return placeholder(accepts, multiple)
  }
  return items.length === 1 ? first.label : `${String(items.length)} ${NOUNS[accepts].plural}`
}

/** The items a field can choose from without a pointer, in a stable order. */
function candidatesFor(
  state: AppState,
  accepts: SelectionKind,
  value: unknown,
  createsBody: boolean,
): readonly SelectionItem[] {
  const document = state.document
  if (document === null) {
    return []
  }
  const recipe = document.recipe
  const features = recipe.features ?? []
  switch (accepts) {
    case 'body':
      // A body-creating target must be empty, so it lists bodies without geometry (2.12).
      return (recipe.bodies ?? [])
        .filter((body) => (document.meshes[body.id] !== undefined) !== createsBody)
        .filter((body) => body.id !== state.command?.newBody?.id)
        .map((body) => candidate(`body:${body.id}`, 'body', body.name, { body: body.id }))
    case 'sketch':
      return features
        .filter((feature) => feature.type === 'sketch')
        .map((feature) =>
          candidate(
            `feature:${feature.id}`,
            'feature',
            featureLabel(recipe, feature.id, state.catalog),
            {
              feature: feature.id,
            },
          ),
        )
    case 'plane':
      return [
        originItem('XY'),
        originItem('XZ'),
        originItem('YZ'),
        ...features
          .filter((feature) => feature.type === 'datum_plane')
          .map((feature) =>
            candidate(
              `feature:${feature.id}`,
              'datum_plane',
              featureLabel(recipe, feature.id, state.catalog),
              { feature: feature.id },
            ),
          ),
      ]
    case 'axis':
      return [originItem('X'), originItem('Y'), originItem('Z')]
    case 'sketch_region': {
      const current =
        typeof value === 'object' && value !== null ? Reflect.get(value, 'sketch') : undefined
      const sketches =
        typeof current === 'string'
          ? [current]
          : features
              .filter(
                (feature) =>
                  feature.type === 'sketch' && isSessionVisible(state, `feature:${feature.id}`),
              )
              .map((feature) => feature.id)
      const units = recipe.units
      return sketches.flatMap((sketch) =>
        (document.sketches[sketch]?.solution.regions ?? []).map((region, index) =>
          candidate(
            `region:${sketch}:${region.reference.key}`,
            'sketch_region',
            `Region ${String(index + 1)} (${formatArea(region.area, units)})`,
            { feature: sketch, sketchItem: { kind: 'region', sketch, region } },
          ),
        ),
      )
    }
    default:
      return []
  }
}

export type SelectionFieldProps = {
  readonly field: InspectorField
  readonly accepts: SelectionKind
  readonly multiple: boolean
  /** The params object that holds the field, whose `body` or `sketch` qualifies its value. */
  readonly siblings: Readonly<Record<string, unknown>>
  readonly statusMessage: string | null
}

export function SelectionField({
  field,
  accepts,
  multiple,
  siblings,
  statusMessage,
}: SelectionFieldProps) {
  const app = useApp()
  const labelId = useId()
  const textId = useId()
  const errorId = useId()
  const hintId = useId()
  const helpId = useId()
  const chooseButton = useRef<HTMLButtonElement>(null)
  const [choosing, setChoosing] = useState(false)
  const active = useAppState((state) => state.command?.activeField === field.path)
  const path = field.path
  // A box that goes away under the pointer (More options closing) stops highlighting its items.
  useEffect(
    () => () => {
      if (app.store.getState().command?.hoveredField === path) {
        setHoveredField(app, null)
      }
    },
    [app, path],
  )
  const newBody = useAppState((state) => state.command?.newBody ?? null)
  // A body-creating target lists "New body" first in its Choose menu (2.12, 3.7).
  const createsBody = useAppState((state) => {
    const feature =
      state.command === null ? undefined : state.catalog?.byType[state.command.featureType]
    return field.path === 'target' && feature !== undefined && bodyCreatingTarget(feature) !== null
  })
  // Why the field refused the last pick, for 3 s (3.7).
  const pickHint = useAppState((state) =>
    state.pickHint?.field === field.path ? state.pickHint.message : null,
  )
  // Items and candidates read the document, the catalog and session visibility.
  useAppState((state) => state.document)
  useAppState((state) => state.catalog)
  useAppState((state) => state.view.sessionVisible)
  const state = app.store.getState()
  const items = itemsFromValue(accepts, field.value, state, siblings)
  const empty = items.length === 0
  // A field set to the body this command creates names it as new (3.7).
  const text =
    newBody !== null && field.value === newBody.id
      ? `${newBody.name} (new)`
      : selectionText(accepts, multiple, items)
  const invalid = statusMessage !== null || field.error !== null
  const message = statusMessage ?? field.error ?? ''

  function activate(): void {
    setActiveField(app, field.path)
  }

  const candidates: readonly MenuEntry[] = CHOOSABLE.has(accepts)
    ? candidatesFor(state, accepts, field.value, createsBody).map((item) => ({
        kind: 'item',
        label: item.label,
        onSelect: () => {
          setActiveField(app, field.path)
          capturePick(app, item).catch((error: unknown) => {
            console.error('Choosing a selection failed.', error)
          })
        },
      }))
    : []
  const choices: readonly MenuEntry[] = createsBody
    ? [
        {
          kind: 'item',
          label: 'New body',
          onSelect: () => {
            chooseNewBody(app, field.path)
          },
        },
        ...candidates,
      ]
    : candidates

  return (
    <div className="flex min-w-0 flex-col gap-1">
      <HelpLabel help={field.help} helpId={helpId}>
        <span
          id={labelId}
          className={clsx(
            'self-start text-xs font-medium',
            COLOR_CLASSES.textMuted.foreground,
            field.help !== null && 'cursor-help',
          )}
        >
          {field.label}
        </span>
      </HelpLabel>
      <div className="flex min-w-0 items-center gap-2">
        <button
          type="button"
          id={fieldInputId(field.path)}
          // ui.md 3.7 and 6.1 name the button role on the box.
          // oxlint-disable-next-line jsx-a11y/no-redundant-roles
          role="button"
          data-testid="selection-field"
          data-active={active}
          data-count={items.length}
          aria-labelledby={`${labelId} ${textId}`}
          aria-describedby={describedBy(
            message === '' ? null : errorId,
            pickHint === null ? null : hintId,
            field.help === null ? null : helpId,
          )}
          aria-pressed={active}
          onClick={activate}
          onPointerEnter={() => {
            // Hovering the box highlights its items in the viewport (3.7).
            setHoveredField(app, field.path)
          }}
          onPointerLeave={() => {
            setHoveredField(app, null)
          }}
          className={clsx(
            'flex h-8 min-w-0 flex-1 cursor-pointer items-center gap-2 rounded-sm border px-2 text-left text-sm',
            COLOR_CLASSES.field.background,
            COLOR_CLASSES.focusRing.outline,
            empty ? COLOR_CLASSES.textMuted.foreground : COLOR_CLASSES.text.foreground,
            invalid && COLOR_CLASSES.errorMark.border,
            !invalid && (active || (empty && field.required)) && COLOR_CLASSES.accent.border,
            !invalid && !active && !(empty && field.required) && COLOR_CLASSES.borderControl.border,
            empty && field.required && 'border-dashed',
          )}
        >
          <span id={textId} className="min-w-0 truncate tabular-nums">
            {text}
          </span>
        </button>
        {empty ? null : (
          <IconButton
            label={`Clear ${field.label}`}
            icon={X}
            size="compact"
            onClick={() => {
              setCommandField(app, field.path, undefined)
            }}
          />
        )}
        {choices.length === 0 ? null : (
          <Button
            ref={chooseButton}
            aria-label={`Choose ${field.label}`}
            aria-haspopup="menu"
            aria-expanded={choosing}
            onClick={() => {
              setChoosing((open) => !open)
            }}
          >
            Choose
          </Button>
        )}
      </div>
      <div className="flex flex-col">
        <p
          id={errorId}
          data-testid="field-error"
          aria-live="polite"
          className={clsx('text-xs', COLOR_CLASSES.errorText.foreground)}
        >
          {message}
        </p>
        <p
          id={hintId}
          aria-live="polite"
          className={clsx('text-xs', COLOR_CLASSES.textMuted.foreground)}
        >
          {pickHint}
        </p>
      </div>
      <Menu
        label={`Choose ${field.label}`}
        open={choosing}
        anchor={chooseButton}
        entries={choices}
        onOpenChange={setChoosing}
      />
    </div>
  )
}
