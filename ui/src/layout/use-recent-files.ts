import { useEffect, useState } from 'react'
import type { App } from '../app/createApp'
import { reportFailure } from '../app/report'
import type { RecentFile } from '../files/recent'

// The recent files a menu or the palette lists (ui.md 3.4, 3.19).

const NO_RECENT_FILES: readonly RecentFile[] = []

/**
 * The recent files, newest first, read each time `enabled` turns on; empty until they are read.
 * An empty read keeps an empty list as it is, so what a caller derives from it is not rebuilt.
 */
export function useRecentFiles(app: App, enabled: boolean): readonly RecentFile[] {
  const [recent, setRecent] = useState(NO_RECENT_FILES)
  useEffect(() => {
    if (!enabled) {
      return undefined
    }
    let current = true
    app.recent.list().then((files) => {
      if (current) {
        setRecent((previous) => (files.length === 0 && previous.length === 0 ? previous : files))
      }
    }, reportFailure('Reading the recent files'))
    return () => {
      current = false
    }
  }, [app, enabled])
  return recent
}
