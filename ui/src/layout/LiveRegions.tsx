import { useEffect, useRef, useState } from 'react'
import { useAppState } from '../app/AppProvider'
import type { SelectionItem } from '../state/store'

// The polite live regions of the shell (ui.md 3.6, 3.20, 3.24): undo and redo announcements, and
// the selection, announced once it has settled.

/** How long the selection must stay unchanged before it is announced (3.6). */
const SELECTION_ANNOUNCE_DELAY_MILLISECONDS = 500

function selectionMessage(selection: readonly SelectionItem[]): string {
  const [only] = selection
  if (only === undefined) {
    return 'Selection cleared'
  }
  return selection.length === 1
    ? `${only.label} selected`
    : `${String(selection.length)} items selected`
}

/** The polite live regions: undo and redo announcements, and the debounced selection (3.6, 3.20). */
export function LiveRegions() {
  const announcement = useAppState((state) => state.announcement)
  const selection = useAppState((state) => state.selection)
  const [selectionText, setSelectionText] = useState('')
  const first = useRef(true)
  useEffect(() => {
    if (first.current) {
      // The selection the app starts with is not news.
      first.current = false
      return undefined
    }
    const timer = setTimeout(() => {
      setSelectionText(selectionMessage(selection))
    }, SELECTION_ANNOUNCE_DELAY_MILLISECONDS)
    return () => {
      clearTimeout(timer)
    }
  }, [selection])
  return (
    <>
      <div aria-live="polite" className="sr-only">
        {announcement === null ? null : <span key={announcement.id}>{announcement.message}</span>}
      </div>
      <div aria-live="polite" className="sr-only">
        {selectionText}
      </div>
    </>
  )
}
