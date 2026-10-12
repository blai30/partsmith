import { useRef, useState, type DragEvent } from 'react'
import { openFile } from '../app/actions/document'
import type { App } from '../app/createApp'

// Opening a file dropped on the window (ui.md 3.19); openFile explains a file that is not a model.

/** Whether a drag carries files, the only thing a drop opens. */
function carriesFiles(event: DragEvent): boolean {
  return event.dataTransfer.types.includes('Files')
}

/** Opening a .ptsm file dropped on the window, with an overlay while it is dragged over (3.19). */
export function useFileDrop(app: App) {
  const [over, setOver] = useState(false)
  // Nested elements fire their own enter and leave events; the depth tells when the window is left.
  const depth = useRef(0)
  return {
    over,
    handlers: {
      onDragEnter: (event: DragEvent) => {
        if (!carriesFiles(event)) {
          return
        }
        event.preventDefault()
        depth.current += 1
        setOver(true)
      },
      onDragOver: (event: DragEvent) => {
        if (carriesFiles(event)) {
          event.preventDefault()
          event.dataTransfer.dropEffect = 'copy'
        }
      },
      onDragLeave: (event: DragEvent) => {
        if (!carriesFiles(event)) {
          return
        }
        depth.current = Math.max(0, depth.current - 1)
        if (depth.current === 0) {
          setOver(false)
        }
      },
      onDrop: (event: DragEvent) => {
        if (!carriesFiles(event)) {
          return
        }
        event.preventDefault()
        depth.current = 0
        setOver(false)
        const [file] = event.dataTransfer.files
        if (file === undefined) {
          return
        }
        file.arrayBuffer().then(
          (buffer) =>
            openFile(app, { name: file.name, bytes: new Uint8Array(buffer), handle: null }),
          (error: unknown) => {
            console.error(`Reading ${file.name} failed.`, error)
          },
        )
      },
    },
  }
}
