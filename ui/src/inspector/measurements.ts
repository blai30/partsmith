import type { App } from '../app/createApp'
import { createLatestOnly, type LatestOnly } from '../core/queue'
import { formatNumber, formatQuantity, MILLIMETERS_PER_UNIT } from '../expression/format'
import type { DocHandle, Units } from '../generated/protocol'
import type { AppState, AppStore, SelectionItem } from '../state/store'

// Measurements of the selection (ui.md 3.8): selecting something measures it, with no tool and no
// mode. Diameters come from the element descriptors; a body's volume comes from the core's
// `measure`, once per model-layer mesh.

/** One measure row of the selection inspector (`measure-row`). */
export type MeasureRow = {
  /** The row's `data-measure`. */
  readonly measure: 'diameter' | 'volume'
  readonly label: string
  /** The value with its unit; empty while the core measures it. */
  readonly value: string
}

/** A body whose volume the selection shows, with the model-layer mesh the volume belongs to. */
export type MeasuredBody = { readonly body: string; readonly meshId: number }

/** A volume in mm³ written in the document unit cubed: `8000 mm³`, `0.4882 in³` (rounded as a length, 2.13). */
export function formatVolume(volume: number, units: Units): string {
  // formatNumber divides a length by the unit once; dividing twice first makes it the unit cubed.
  const perUnit = MILLIMETERS_PER_UNIT[units]
  return `${formatNumber(volume / (perUnit * perUnit), 'length', units)} ${units}³`
}

/** The one selected body with model-layer geometry, whose volume is measured; null otherwise. */
export function measuredBody(
  state: AppState,
  items: readonly SelectionItem[],
): MeasuredBody | null {
  const [only] = items
  if (items.length !== 1 || only?.kind !== 'body' || only.body === null) {
    return null
  }
  const mesh = state.document?.meshes[only.body]
  return mesh === undefined ? null : { body: only.body, meshId: mesh.meshId }
}

/** Body volumes by model-layer mesh id, measured through the core once per mesh (3.8). */
export type VolumeMeter = {
  /** The volume in mm³ of a measured mesh, or undefined while it is not known. */
  volume(meshId: number): number | undefined
  /**
   * Measures a body's mesh unless it is known; while one measurement is in flight a newer request
   * replaces the waiting one (`LatestOnly`).
   */
  request(documentHandle: DocHandle, measured: MeasuredBody): void
  /** Resolves when no measurement is waiting or in flight. */
  flush(): Promise<void>
  /** Hears every newly known volume; returns the unsubscriber. */
  subscribe(listener: () => void): () => void
}

type VolumeRequest = { readonly documentHandle: DocHandle; readonly measured: MeasuredBody }

const metersByStore = new WeakMap<AppStore, VolumeMeter>()

function createVolumeMeter(app: App): VolumeMeter {
  const volumes = new Map<number, number>()
  const listeners = new Set<() => void>()
  const latest: LatestOnly<VolumeRequest> = createLatestOnly(
    ({ documentHandle, measured }: VolumeRequest) =>
      app.core.measure(documentHandle, { kind: 'body', body: measured.body }),
    (measurement, { measured }) => {
      const volume = measurement.volume ?? null
      if (volume === null) {
        return
      }
      volumes.set(measured.meshId, volume)
      for (const listener of Array.from(listeners)) {
        listener()
      }
    },
    (error) => {
      // A body that changed or emptied meanwhile is measured again once its new mesh is selected.
      console.info('Partsmith could not measure the body.', error)
    },
  )
  return {
    volume: (meshId) => volumes.get(meshId),
    request(documentHandle, measured) {
      if (!volumes.has(measured.meshId)) {
        latest.submit({ documentHandle, measured })
      }
    },
    flush: () => latest.flush(),
    subscribe(listener) {
      listeners.add(listener)
      return () => {
        listeners.delete(listener)
      }
    },
  }
}

/** The app's volume meter. */
export function volumeMeter(app: App): VolumeMeter {
  let meter = metersByStore.get(app.store)
  if (meter === undefined) {
    meter = createVolumeMeter(app)
    metersByStore.set(app.store, meter)
  }
  return meter
}

/**
 * The measure rows of a selection (3.8): Diameter for one circular edge, cylindrical face or
 * spherical face (twice the descriptor's radius), and Volume for one body with geometry, empty
 * until the core has measured it. Two or more items have no rows.
 */
export function selectionMeasurements(
  app: App,
  items: readonly SelectionItem[],
): readonly MeasureRow[] {
  const state = app.store.getState()
  const units = state.document?.recipe.units ?? 'mm'
  const [only] = items
  if (items.length !== 1 || only === undefined) {
    return []
  }
  const element = only.element
  const radius = element?.radius ?? null
  const round =
    (only.kind === 'edge' && element?.curve === 'circle') ||
    (only.kind === 'face' && (element?.surface === 'cylinder' || element?.surface === 'sphere'))
  if (round && radius !== null) {
    return [
      {
        measure: 'diameter',
        label: 'Diameter',
        value: formatQuantity(2 * radius, 'length', units),
      },
    ]
  }
  const measured = measuredBody(state, items)
  if (measured === null) {
    return []
  }
  const volume = volumeMeter(app).volume(measured.meshId)
  return [
    {
      measure: 'volume',
      label: 'Volume',
      value: volume === undefined ? '' : formatVolume(volume, units),
    },
  ]
}

/** Asks the core for what the selection's rows still need; resolves when it has answered. */
export async function measureSelection(app: App, items: readonly SelectionItem[]): Promise<void> {
  const state = app.store.getState()
  const measured = measuredBody(state, items)
  const document = state.document
  if (measured === null || document === null) {
    return
  }
  const meter = volumeMeter(app)
  meter.request(document.documentHandle, measured)
  await meter.flush()
}
