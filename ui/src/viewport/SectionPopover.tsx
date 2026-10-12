import type { ReactNode, RefObject } from 'react'
import { Popover } from '../components/Popover'
import { SegmentedControl } from '../components/SegmentedControl'
import { Switch } from '../components/Switch'
import { SECTION_PLANES, type SectionPlane } from './section'

/** The plane and side of the section view (3.15). Its offset is an expression the Offset field edits. */
export type SectionSettings = { readonly plane: SectionPlane; readonly flipped: boolean }

const PLANE_OPTIONS = SECTION_PLANES.map((plane) => ({ value: plane, label: plane }))

export type SectionPopoverProps = {
  readonly open: boolean
  /** The "Section view" control the popover is anchored to. */
  readonly anchor: RefObject<HTMLElement | null>
  readonly onOpenChange: (open: boolean) => void
  readonly settings: SectionSettings
  readonly onPlaneChange: (plane: SectionPlane) => void
  readonly onFlippedChange: (flipped: boolean) => void
  /**
   * The Offset field: an `ExpressionInput` labeled "Offset" with unit length (2.13). The viewport renders it because it evaluates through the core.
   */
  readonly offsetField: ReactNode
}

/** The section popover (3.15): Plane (XY, XZ, YZ), Offset and Flip. */
export function SectionPopover({
  open,
  anchor,
  onOpenChange,
  settings,
  onPlaneChange,
  onFlippedChange,
  offsetField,
}: SectionPopoverProps) {
  return (
    <Popover label="Section view" open={open} anchor={anchor} onOpenChange={onOpenChange}>
      <div data-testid="section-popover" className="flex w-60 flex-col gap-3">
        <div className="flex min-h-8 items-center justify-between gap-3">
          <span aria-hidden="true">Plane</span>
          <SegmentedControl
            label="Plane"
            options={PLANE_OPTIONS}
            value={settings.plane}
            onChange={onPlaneChange}
          />
        </div>
        {offsetField}
        <Switch label="Flip" checked={settings.flipped} onChange={onFlippedChange} />
      </div>
    </Popover>
  )
}
