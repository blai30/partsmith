import type { ReactNode } from 'react'
import { Button } from '../components/Button'

export type ReferenceOfferProps = {
  /** The sentence that says why the dimension cannot drive the sketch. */
  readonly children: ReactNode
  readonly onAddReference: () => void
  readonly onCancel: () => void
}

/**
 * The offer of a dimension the sketch cannot take as driving (ui.md 3.16.7): an alert with why,
 * Add reference and Cancel. The dimension editor and the selection inspector's size field show it.
 */
export function ReferenceOffer({ children, onAddReference, onCancel }: ReferenceOfferProps) {
  return (
    <div role="alert" className="flex flex-col gap-2 text-sm">
      {children}
      <div className="flex justify-end gap-2">
        <Button variant="primary" onClick={onAddReference}>
          Add reference
        </Button>
        <Button onClick={onCancel}>Cancel</Button>
      </div>
    </div>
  )
}
