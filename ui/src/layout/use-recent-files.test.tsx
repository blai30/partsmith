import { renderHook, waitFor } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import type { RecentFile, RecentFilesStore } from '../files/recent'
import { createTestApp } from '../test/fakes/test-app'
import { useRecentFiles } from './use-recent-files'

const BRACKET: RecentFile = {
  id: 'r-1',
  name: 'bracket.ptsm',
  openedAt: 1000,
  handle: null,
  text: '{}',
}

function recentStore(files: readonly RecentFile[]): RecentFilesStore {
  return {
    list: vi.fn<RecentFilesStore['list']>(() => Promise.resolve(files)),
    remember: () => Promise.resolve(),
    forget: () => Promise.resolve(),
  }
}

describe('useRecentFiles', () => {
  it('is empty until the files are read, then lists them', async () => {
    const app = createTestApp({ recent: recentStore([BRACKET]) })
    const { result } = renderHook(() => useRecentFiles(app, true))
    expect(result.current).toEqual([])
    await waitFor(() => {
      expect(result.current).toEqual([BRACKET])
    })
  })

  it('reads nothing while disabled, and reads each time it is enabled again', async () => {
    const recent = recentStore([BRACKET])
    const app = createTestApp({ recent })
    const { result, rerender } = renderHook(({ enabled }) => useRecentFiles(app, enabled), {
      initialProps: { enabled: false },
    })
    expect(recent.list).not.toHaveBeenCalled()
    rerender({ enabled: true })
    await waitFor(() => {
      expect(result.current).toEqual([BRACKET])
    })
    rerender({ enabled: false })
    rerender({ enabled: true })
    expect(recent.list).toHaveBeenCalledTimes(2)
  })

  it('keeps the same empty list when nothing was opened yet', async () => {
    const recent = recentStore([])
    const app = createTestApp({ recent })
    const { result } = renderHook(() => useRecentFiles(app, true))
    const before = result.current
    await waitFor(() => {
      expect(recent.list).toHaveBeenCalledTimes(1)
    })
    await Promise.resolve()
    expect(result.current).toBe(before)
  })
})
