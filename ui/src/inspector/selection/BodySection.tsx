import { clsx } from 'clsx/lite'
import { useId, useState } from 'react'
import { setBodyMaterial, setBodyVisible } from '../../app/actions/bodies'
import { useApp } from '../../app/AppProvider'
import { reportFailure } from '../../app/report'
import { BodyColorButton } from '../../browser/BodyColor'
import { Switch } from '../../components/Switch'
import { formatPoint, formatSize } from '../../expression/format'
import type { DocumentState, SelectionItem } from '../../state/store'
import { COLOR_CLASSES } from '../../theme/roles'
import { MeasureRows, Row, Suggestions, Title, Value } from './parts'

// The selection inspector of one body (ui.md 3.8): its size, center and measures, color, material
// and visibility.

const MATERIALS = ['PLA', 'PETG', 'ABS', 'ASA', 'TPU', 'Resin'] as const

/** The section of one selected body. */
export function BodySection({
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
                setBodyMaterial(app, body.id, material).catch(
                  reportFailure('Changing the material'),
                )
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
            setBodyVisible(app, body.id, visible).catch(reportFailure('Changing the visibility'))
          }}
        />
      </div>
      <Suggestions items={items} />
    </>
  )
}
