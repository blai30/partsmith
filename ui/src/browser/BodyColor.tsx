import { clsx } from 'clsx/lite'
import { useId, useRef, useState } from 'react'
import { setBodyColor } from '../app/actions/bodies'
import { useApp, useAppState } from '../app/AppProvider'
import { Popover } from '../components/Popover'
import { resolvedTheme } from '../layout/color-scheme'
import type { Body } from '../generated/protocol'
import { BODY_SWATCHES, COLOR_CLASSES, FONT_CLASSES, VIEWPORT_COLORS } from '../theme/roles'

// A body's display color (ui.md 3.10): a swatch drawn from document data, and the popover that
// changes it. The stored hex is known only at runtime, so it goes into an SVG `fill` attribute.

const HEX_COLOR = /^#[0-9a-f]{6}$/i

/** The swatch of a body's current color: its stored hex, or the default body color. */
function SwatchGraphic({ hex }: { readonly hex: string }) {
  return (
    <svg aria-hidden="true" viewBox="0 0 16 16" className="size-full">
      <rect x="0" y="0" width="16" height="16" fill={hex} />
    </svg>
  )
}

export type BodyColorButtonProps = {
  readonly body: Body
  /** Rows keep their buttons out of the tab order (3.10). */
  readonly tabbable?: boolean
  readonly size?: 'row' | 'field'
}

/** The color swatch button of a body with its popover of swatches and a hex field. */
export function BodyColorButton({ body, tabbable = true, size = 'row' }: BodyColorButtonProps) {
  const app = useApp()
  const theme = useAppState((state) => resolvedTheme(state.settings))
  const anchor = useRef<HTMLButtonElement>(null)
  const [open, setOpen] = useState(false)
  const defaultHex = VIEWPORT_COLORS[theme].body
  const hex = body.color ?? defaultHex
  return (
    <>
      <button
        ref={anchor}
        type="button"
        tabIndex={tabbable ? 0 : -1}
        aria-label={`Color of ${body.name}`}
        aria-haspopup="dialog"
        aria-expanded={open}
        onClick={(event) => {
          event.stopPropagation()
          setOpen((current) => !current)
        }}
        onDoubleClick={(event) => {
          event.stopPropagation()
        }}
        className={clsx(
          'inline-flex shrink-0 overflow-hidden rounded-sm border',
          size === 'row' ? 'size-4' : 'size-6',
          COLOR_CLASSES.borderControl.border,
          COLOR_CLASSES.focusRing.outline,
        )}
      >
        <SwatchGraphic hex={hex} />
      </button>
      {open ? (
        <ColorPopover body={body} anchor={anchor} current={hex} onClose={() => setOpen(false)} />
      ) : null}
    </>
  )
}

type ColorPopoverProps = {
  readonly body: Body
  readonly anchor: React.RefObject<HTMLElement | null>
  readonly current: string
  readonly onClose: () => void
}

function ColorPopover({ body, anchor, current, onClose }: ColorPopoverProps) {
  const app = useApp()
  const hexId = useId()
  const [hex, setHex] = useState(current)
  const valid = HEX_COLOR.test(hex)

  function choose(color: string | null): void {
    onClose()
    setBodyColor(app, body.id, color).catch((error: unknown) => {
      console.error('Changing the body color failed.', error)
    })
  }

  return (
    <Popover
      label={`Color of ${body.name}`}
      open
      anchor={anchor}
      onOpenChange={(next) => {
        if (!next) {
          onClose()
        }
      }}
    >
      <div data-testid="color-popover" className="flex w-56 flex-col gap-3">
        <div className="grid grid-cols-7 gap-2">
          {BODY_SWATCHES.map((swatch) => (
            <button
              key={swatch.name}
              type="button"
              aria-label={swatch.name}
              aria-pressed={(swatch.hex ?? null) === (body.color ?? null)}
              onClick={() => {
                choose(swatch.hex)
              }}
              className={clsx(
                'size-6 rounded-sm border',
                swatch.background,
                COLOR_CLASSES.borderControl.border,
                COLOR_CLASSES.focusRing.outline,
              )}
            />
          ))}
        </div>
        <div className="flex flex-col gap-1">
          <label
            htmlFor={hexId}
            className={clsx('text-xs font-medium', COLOR_CLASSES.textMuted.foreground)}
          >
            Hex color
          </label>
          <input
            id={hexId}
            type="text"
            value={hex}
            spellCheck={false}
            autoComplete="off"
            aria-invalid={valid ? undefined : true}
            onChange={(event) => {
              setHex(event.currentTarget.value)
            }}
            onKeyDown={(event) => {
              if (event.key === 'Enter' && HEX_COLOR.test(hex)) {
                event.preventDefault()
                choose(hex.toLowerCase())
              }
            }}
            className={clsx(
              'h-8 w-full rounded-sm border px-2 text-sm',
              FONT_CLASSES.values,
              COLOR_CLASSES.field.background,
              COLOR_CLASSES.text.foreground,
              COLOR_CLASSES.focusRing.outline,
              valid ? COLOR_CLASSES.borderControl.border : COLOR_CLASSES.errorMark.border,
            )}
          />
        </div>
      </div>
    </Popover>
  )
}
