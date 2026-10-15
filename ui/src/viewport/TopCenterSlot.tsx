import type { ReactNode } from 'react'

/**
 * The free area's top-center slot (ui.md 3.2): a column between the viewport controls and the view
 * cube. While the slot is at least 36rem wide the column keeps 10rem from each side; in a narrower
 * slot it spans the slot and starts below the cube, where a spacer that shrinks first lets it rise
 * in a free area too short for both. Its cards and chips take pointer events themselves, so the slot
 * never blocks picking.
 */
export function TopCenterSlot({ children }: { readonly children: ReactNode }) {
  return (
    <div className="pointer-events-none absolute inset-4 z-10 flex flex-col @container/slot">
      <div aria-hidden="true" className="@max-xl/slot:h-44" />
      <div
        data-testid="top-center-slot"
        className="flex shrink-0 flex-col items-center gap-2 @xl/slot:mx-36"
      >
        {children}
      </div>
    </div>
  )
}
