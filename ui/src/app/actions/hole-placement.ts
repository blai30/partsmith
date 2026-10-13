import type { ElementDescriptor, Placement } from '../../generated/protocol'
import { getParam, setParam } from '../../inspector/model'
import { worldToSketch } from '../../sketch/placement'
import type { CatalogFeature, HelperSketch } from '../../state/store'
import { addScaled, dot, length, subtract } from '../../viewport/math'
import type { PointerRay } from '../../viewport/picking'
import { sameValue, siblingPath, type Params } from './command-values'

// Hole centers placed on a flat face through the command's helper sketch (ui.md 3.7).

/** A hole center snaps to the face's centroid within this many CSS pixels (3.7). */
const CENTROID_SNAP_PIXELS = 8

/** Sketch coordinates of hole centers are kept to this many decimals of a millimeter. */
const CENTER_DECIMALS = 6

/** A world point rounded into a placement's sketch coordinates. */
function sketchCoordinates(placement: Placement, point: readonly [number, number, number]) {
  const [x, y] = worldToSketch(placement, point)
  const factor = 10 ** CENTER_DECIMALS
  const rounded = (value: number) => Math.round(value * factor) / factor + 0
  return [rounded(x), rounded(y)] as [number, number]
}

/** Where a click's ray meets a face's plane, snapped to the centroid within 8 CSS pixels (3.7). */
export function holeCenter(element: ElementDescriptor, placement: Placement, pointer?: PointerRay) {
  const centroid = element.centroid
  if (pointer === undefined) {
    return sketchCoordinates(placement, centroid)
  }
  const along = dot(pointer.direction, placement.normal)
  const hit =
    Math.abs(along) < 1e-12
      ? centroid
      : addScaled(
          pointer.origin,
          pointer.direction,
          dot(subtract(placement.origin, pointer.origin), placement.normal) / along,
        )
  const snapped =
    length(subtract(hit, centroid)) <= CENTROID_SNAP_PIXELS * pointer.millimetersPerPixel
      ? centroid
      : hit
  return sketchCoordinates(placement, snapped)
}

/** Whether a helper sketch lies on this face. */
export function helperOnFace(helper: HelperSketch, element: ElementDescriptor): boolean {
  const plane = helper.params.plane
  return (
    plane.kind === 'face' && plane.body === element.body && sameValue(plane.face, element.reference)
  )
}

/** The helper after a change of the params: dropped once the point field holds none of its points. */
export function helperFor(
  feature: CatalogFeature,
  params: Params,
  helper: HelperSketch | null,
): { readonly params: Params; readonly helper: HelperSketch | null } {
  if (helper === null) {
    return { params, helper }
  }
  const pointField = feature.manifest.fields.find((hint) => hint.selection === 'sketch_point')
  if (pointField === undefined) {
    return { params, helper: null }
  }
  const sketchPath = siblingPath(pointField.path, 'sketch')
  const points = getParam(params, pointField.path)
  const held = new Set(
    getParam(params, sketchPath) === helper.id && Array.isArray(points) ? points : [],
  )
  const all = helper.params.entities ?? []
  const entities = all.filter((entity) => held.has(entity.id))
  if (entities.length === 0) {
    // The helper goes; a sketch field still naming it is cleared with it.
    const cleared =
      getParam(params, sketchPath) === helper.id
        ? setParam(feature, params, sketchPath, undefined)
        : params
    return { params: cleared, helper: null }
  }
  return entities.length === all.length
    ? { params, helper }
    : { params, helper: { ...helper, params: { ...helper.params, entities } } }
}
