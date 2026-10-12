import { clsx } from 'clsx/lite'
import { X } from 'lucide-react'
import { useId, useRef, useState, type KeyboardEvent } from 'react'
import { capturePick, setActiveField, setCommandField } from '../../app/actions/command'
import { useApp, useAppState } from '../../app/AppProvider'
import { featureLabel } from '../../core/labels'
import { Button } from '../../components/Button'
import { IconButton } from '../../components/IconButton'
import { Menu, type MenuEntry } from '../../components/Menu'
import type { SelectionKind } from '../../generated/protocol'
import type { AppState, OriginName, SelectionItem } from '../../state/store'
import { COLOR_CLASSES } from '../../theme/roles'
import { formatArea } from '../area'
import type { InspectorField } from '../model'
import { itemsFromValue } from '../selection-values'
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

const NO_ITEM_DATA = {
  body: null,
  feature: null,
  element: null,
  sketchItem: null,
  origin: null,
} as const

function originItem(name: OriginName, kind: SelectionItem['kind'], label: string): SelectionItem {
  return { ...NO_ITEM_DATA, key: `origin:${name}`, kind, label, origin: name }
}

/** The text of the box: the placeholder, the one item's label, or a count. */
export function selectionText(
  accepts: SelectionKind,
  multiple: boolean,
  items: readonly SelectionItem[],
): string {
  const noun = NOUNS[accepts]
  const [first] = items
  if (items.length === 0 || first === undefined) {
    return `Select ${multiple ? noun.plural : noun.single}`
  }
  return items.length === 1 ? first.label : `${String(items.length)} ${noun.plural}`
}

/** The items a field can choose from without a pointer, in a stable order. */
function candidatesFor(
  state: AppState,
  accepts: SelectionKind,
  value: unknown,
): readonly SelectionItem[] {
  const document = state.document
  if (document === null) {
    return []
  }
  const recipe = document.recipe
  const features = recipe.features ?? []
  switch (accepts) {
    case 'body':
      return (recipe.bodies ?? [])
        .filter((body) => document.meshes[body.id] !== undefined)
        .map((body) => ({
          ...NO_ITEM_DATA,
          key: `body:${body.id}`,
          kind: 'body',
          label: body.name,
          body: body.id,
        }))
    case 'sketch':
      return features
        .filter((feature) => feature.type === 'sketch')
        .map((feature) => ({
          ...NO_ITEM_DATA,
          key: `feature:${feature.id}`,
          kind: 'feature',
          label: featureLabel(recipe, feature.id, state.catalog),
          feature: feature.id,
        }))
    case 'plane':
      return [
        originItem('XY', 'origin_plane', 'XY plane'),
        originItem('XZ', 'origin_plane', 'XZ plane'),
        originItem('YZ', 'origin_plane', 'YZ plane'),
        ...features
          .filter((feature) => feature.type === 'datum_plane')
          .map((feature): SelectionItem => ({
            ...NO_ITEM_DATA,
            key: `feature:${feature.id}`,
            kind: 'datum_plane',
            label: featureLabel(recipe, feature.id, state.catalog),
            feature: feature.id,
          })),
      ]
    case 'axis':
      return [
        originItem('X', 'origin_axis', 'X axis'),
        originItem('Y', 'origin_axis', 'Y axis'),
        originItem('Z', 'origin_axis', 'Z axis'),
      ]
    case 'sketch_region': {
      const current =
        typeof value === 'object' && value !== null ? Reflect.get(value, 'sketch') : undefined
      const sketches =
        typeof current === 'string'
          ? [current]
          : features
              .filter(
                (feature) =>
                  feature.type === 'sketch' &&
                  state.view.sessionVisible[`feature:${feature.id}`] !== false,
              )
              .map((feature) => feature.id)
      const units = recipe.units
      return sketches.flatMap((sketch) =>
        (document.sketches[sketch]?.solution.regions ?? []).map((region, index): SelectionItem => ({
          ...NO_ITEM_DATA,
          key: `region:${sketch}:${region.reference.key}`,
          kind: 'sketch_region',
          label: `Region ${String(index + 1)} (${formatArea(region.area, units)})`,
          feature: sketch,
          sketchItem: { kind: 'region', sketch, region },
        })),
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
  const chooseButton = useRef<HTMLButtonElement>(null)
  const [choosing, setChoosing] = useState(false)
  const active = useAppState((state) => state.command?.activeField === field.path)
  // Items and candidates read the document, the catalog and session visibility.
  useAppState((state) => state.document)
  useAppState((state) => state.catalog)
  useAppState((state) => state.view.sessionVisible)
  const state = app.store.getState()
  const items = itemsFromValue(accepts, field.value, state, siblings)
  const empty = items.length === 0
  const text = selectionText(accepts, multiple, items)
  const invalid = statusMessage !== null || field.error !== null
  const message = statusMessage ?? field.error ?? ''

  function activate(): void {
    setActiveField(app, field.path)
  }

  function onKeyDown(event: KeyboardEvent<HTMLDivElement>): void {
    if (event.key === 'Enter' || event.key === ' ') {
      event.preventDefault()
      activate()
    }
  }

  const choices: readonly MenuEntry[] = CHOOSABLE.has(accepts)
    ? candidatesFor(state, accepts, field.value).map((item) => ({
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

  return (
    <div className="flex min-w-0 flex-col gap-1">
      <span
        id={labelId}
        className={clsx('text-xs font-medium', COLOR_CLASSES.textMuted.foreground)}
      >
        {field.label}
      </span>
      <div className="flex min-w-0 items-center gap-2">
        <div
          id={fieldInputId(field.path)}
          role="button"
          tabIndex={0}
          data-testid="selection-field"
          data-active={active}
          data-count={items.length}
          aria-labelledby={`${labelId} ${textId}`}
          aria-describedby={message === '' ? undefined : errorId}
          aria-pressed={active}
          onClick={activate}
          onKeyDown={onKeyDown}
          className={clsx(
            'flex h-8 min-w-0 flex-1 cursor-pointer items-center gap-2 rounded-sm border px-2 text-sm',
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
        </div>
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
      <p
        id={errorId}
        data-testid="field-error"
        aria-live="polite"
        className={clsx('text-xs', COLOR_CLASSES.errorText.foreground)}
      >
        {message}
      </p>
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
