import { clsx } from 'clsx/lite'
import { Cuboid, Maximize, MousePointer2, ScanLine, type LucideIcon } from 'lucide-react'
import { useRef, useState, type ComponentProps, type ReactNode } from 'react'
import { Menu, type MenuEntry } from '../components/Menu'
import { Tooltip } from '../components/Tooltip'
import { COLOR_CLASSES } from '../theme/roles'
import type { SectionPlane } from './section'
import { SectionPopover, type SectionSettings } from './SectionPopover'

/** Model element kinds the selection filter turns on and off (3.6). */
export type SelectionFilterKind = 'body' | 'face' | 'edge' | 'vertex'
/** Which model elements the pointer picks in the Model and Print contexts (3.6). */
export type SelectionFilter = Readonly<Record<SelectionFilterKind, boolean>>

/** Formatted shortcuts (formatShortcut) the tooltips show, for the controls that have one. */
export type ViewportControlShortcuts = {
  readonly fit?: string
  readonly projection?: string
  readonly section?: string
}

export type ViewportControlsProps = {
  readonly orthographic: boolean
  /** The section's plane and side, or null while the section view is off. */
  readonly section: SectionSettings | null
  /** The selection filter, or null in sketch mode, where its control is hidden. */
  readonly filter: SelectionFilter | null
  readonly shortcuts?: ViewportControlShortcuts
  readonly onFit: () => void
  readonly onToggleProjection: () => void
  readonly onToggleSection: () => void
  readonly onToggleFilter: (kind: SelectionFilterKind) => void
  readonly onSectionPlaneChange: (plane: SectionPlane) => void
  readonly onSectionFlippedChange: (flipped: boolean) => void
  /** The section popover's Offset field (see `SectionPopoverProps.offsetField`). */
  readonly sectionOffsetField: ReactNode
}

const FILTER_LABELS: Readonly<Record<SelectionFilterKind, string>> = {
  body: 'Bodies',
  face: 'Faces',
  edge: 'Edges',
  vertex: 'Vertices',
}
const FILTER_KINDS: readonly SelectionFilterKind[] = ['body', 'face', 'edge', 'vertex']

type ControlButtonProps = Omit<
  ComponentProps<'button'>,
  'type' | 'children' | 'aria-label' | 'className'
> & {
  readonly label: string
  readonly icon: LucideIcon
}

// A size-8 icon button of the stack; pressed toggles use the selection background (as IconButton does).
function ControlButton({
  label,
  icon: Icon,
  'aria-pressed': pressed,
  ...rest
}: ControlButtonProps) {
  const selected = pressed === true
  return (
    <button
      {...rest}
      type="button"
      tabIndex={-1}
      aria-label={label}
      aria-pressed={pressed}
      className={clsx(
        'inline-flex size-8 items-center justify-center rounded-md transition-colors duration-150 motion-reduce:transition-none',
        COLOR_CLASSES.focusRing.outline,
        selected && COLOR_CLASSES.selection.background,
        selected && COLOR_CLASSES.text.foreground,
        !selected && COLOR_CLASSES.hover.background,
        !selected && COLOR_CLASSES.textMuted.foreground,
      )}
    >
      <Icon aria-hidden="true" strokeWidth={1.75} className="size-4" />
    </button>
  )
}

/**
 * The viewport controls (3.14): Fit to view, Orthographic, Section view and the Selection filter, a vertical stack in a card at the free area's top-left corner, the first child of its left column. Turning the section on opens the section popover anchored to its control. The stack is not in the tab order, since the viewport is one tab stop (3.24): every control is also a command with a shortcut (`view.fit`, `view.projection`, `view.section`) or a palette entry (`view.filter.*`).
 */
export function ViewportControls({
  orthographic,
  section,
  filter,
  shortcuts,
  onFit,
  onToggleProjection,
  onToggleSection,
  onToggleFilter,
  onSectionPlaneChange,
  onSectionFlippedChange,
  sectionOffsetField,
}: ViewportControlsProps) {
  const sectionButton = useRef<HTMLButtonElement>(null)
  const filterButton = useRef<HTMLButtonElement>(null)
  const [filterMenuOpen, setFilterMenuOpen] = useState(false)
  const [sectionPopoverOpen, setSectionPopoverOpen] = useState(false)
  // Turning the section on (here or with Shift+X) opens its popover; turning it off closes it.
  const sectionOn = section !== null
  const [sectionWasOn, setSectionWasOn] = useState(sectionOn)
  if (sectionOn !== sectionWasOn) {
    setSectionWasOn(sectionOn)
    setSectionPopoverOpen(sectionOn)
  }
  // Sketch mode hides the filter control, and with it any open filter menu.
  if (filter === null && filterMenuOpen) {
    setFilterMenuOpen(false)
  }

  const filterEntries: readonly MenuEntry[] =
    filter === null
      ? []
      : FILTER_KINDS.map((kind) => ({
          kind: 'checkbox',
          label: FILTER_LABELS[kind],
          checked: filter[kind],
          onSelect: () => {
            onToggleFilter(kind)
          },
        }))

  // The menu and the popover open in the top layer, beside the card rather than inside it.
  return (
    <>
      <fieldset
        aria-label="View controls"
        data-testid="viewport-controls"
        className={clsx(
          'pointer-events-auto flex flex-col gap-1 rounded-xl border p-1 shadow-md',
          COLOR_CLASSES.surfaceRaised.background,
          COLOR_CLASSES.border.border,
          COLOR_CLASSES.elevation.shadow,
        )}
      >
        <Tooltip label="Fit to view" shortcut={shortcuts?.fit ?? null}>
          <ControlButton label="Fit to view" icon={Maximize} onClick={onFit} />
        </Tooltip>
        <Tooltip label="Orthographic" shortcut={shortcuts?.projection ?? null}>
          <ControlButton
            label="Orthographic"
            icon={Cuboid}
            aria-pressed={orthographic}
            onClick={onToggleProjection}
          />
        </Tooltip>
        <Tooltip label="Section view" shortcut={shortcuts?.section ?? null}>
          <ControlButton
            label="Section view"
            icon={ScanLine}
            ref={sectionButton}
            aria-pressed={sectionOn}
            onClick={onToggleSection}
          />
        </Tooltip>
        {filter === null ? null : (
          <Tooltip label="Selection filter">
            <ControlButton
              label="Selection filter"
              icon={MousePointer2}
              ref={filterButton}
              aria-haspopup="menu"
              aria-expanded={filterMenuOpen}
              onClick={() => {
                setFilterMenuOpen((open) => !open)
              }}
            />
          </Tooltip>
        )}
      </fieldset>
      <Menu
        label="Selection filter"
        open={filterMenuOpen}
        anchor={filterButton}
        entries={filterEntries}
        onOpenChange={setFilterMenuOpen}
      />
      {section === null ? null : (
        <SectionPopover
          open={sectionPopoverOpen}
          anchor={sectionButton}
          onOpenChange={setSectionPopoverOpen}
          settings={section}
          onPlaneChange={onSectionPlaneChange}
          onFlippedChange={onSectionFlippedChange}
          offsetField={sectionOffsetField}
        />
      )}
    </>
  )
}
