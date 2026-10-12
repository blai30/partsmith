import { useRef, useState, type RefObject } from 'react'
import { useApp } from '../app/AppProvider'
import type { App } from '../app/createApp'
import { Menu, type MenuEntry } from '../components/Menu'
import { useUiRequest } from '../layout/requests'
import { viewportMenuItems } from './menu'
import { positionElement } from './overlay'

// The viewport context menu (ui.md 3.13, `viewport-menu`): opened at the pointer by a right click
// that does not move, or by Shift+F10 or the ContextMenu key on the focused viewport. Its items are
// `viewportMenuItems` when it opens.

export type ViewportMenuProps = {
  /** The focusable 3D view, which gets focus back when the menu closes. */
  readonly surface: RefObject<HTMLElement | null>
}

function entriesOf(app: App): readonly MenuEntry[] {
  return viewportMenuItems(app).map((item): MenuEntry => {
    if (item.kind === 'separator') {
      return item
    }
    return item.shortcut === null
      ? { kind: 'item', label: item.label, onSelect: item.run }
      : { kind: 'item', label: item.label, shortcut: item.shortcut, onSelect: item.run }
  })
}

export function ViewportMenu({ surface }: ViewportMenuProps) {
  const app = useApp()
  // A point-sized anchor at the pointer; the menu opens beside it (runtime position, 3.12).
  const anchor = useRef<HTMLSpanElement>(null)
  const [entries, setEntries] = useState<readonly MenuEntry[] | null>(null)
  useUiRequest(app, 'viewport_menu', (request) => {
    const element = anchor.current
    if (element === null) {
      return false
    }
    const opened = entriesOf(app)
    if (!opened.some((entry) => entry.kind !== 'separator')) {
      return true
    }
    positionElement(element, request.x, request.y)
    setEntries(opened)
    return true
  })
  return (
    <>
      <span
        ref={anchor}
        aria-hidden="true"
        className="pointer-events-none absolute top-0 left-0 size-0"
      />
      <Menu
        label="3D view"
        testId="viewport-menu"
        open={entries !== null}
        anchor={anchor}
        returnFocus={surface}
        entries={entries ?? []}
        onOpenChange={(open) => {
          if (!open) {
            setEntries(null)
          }
        }}
      />
    </>
  )
}
