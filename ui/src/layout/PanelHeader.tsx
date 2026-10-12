import { X } from 'lucide-react'
import type { ReactNode } from 'react'
import { useApp } from '../app/AppProvider'
import { IconButton } from '../components/IconButton'

/** The header of a side panel: its title, optional actions and a close button (3.2). */
export function PanelHeader({
  title,
  titleId,
  children,
}: {
  readonly title: string
  readonly titleId: string
  readonly children?: ReactNode
}) {
  const app = useApp()
  return (
    <div className="flex h-12 shrink-0 items-center gap-2 px-4">
      <h2 id={titleId} className="min-w-0 flex-1 truncate text-base font-semibold">
        {title}
      </h2>
      {children}
      <IconButton
        label={`Close ${title}`}
        icon={X}
        size="compact"
        onClick={() => {
          app.store.setState((state) => ({ panels: { ...state.panels, side: null } }))
        }}
      />
    </div>
  )
}
