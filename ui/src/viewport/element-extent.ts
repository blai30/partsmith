import type { BodyMeshData } from '../core/events'
import type { ElementDescriptor } from '../generated/protocol'
import { addScaled, boundsDiagonal, dot, subtract, type Vector3 } from './math'

// The points that span a model element on screen, so the inspector opens beside all of it rather
// than over half of it (ui.md 3.7).

/** Mesh points lie in a flat face's plane within this share of the body's diagonal. */
const PLANE_TOLERANCE_SHARE = 1e-4
/** A mesh normal belongs to a flat face when it is this close to the face's normal. */
const NORMAL_AGREEMENT = 0.999

/**
 * The vertices of `mesh` that lie in the plane of a flat face through `centroid` with `normal`
 * and face its way: the face's triangles, and those of any face of the body coplanar with it.
 */
function planeVertices(mesh: BodyMeshData, centroid: Vector3, normal: Vector3): Vector3[] {
  const tolerance = Math.max(PLANE_TOLERANCE_SHARE * boundsDiagonal(mesh.boundingBox), 1e-6)
  const points: Vector3[] = []
  for (let i = 0; i + 2 < mesh.positions.length; i += 3) {
    const point: Vector3 = [
      mesh.positions[i] ?? 0,
      mesh.positions[i + 1] ?? 0,
      mesh.positions[i + 2] ?? 0,
    ]
    const vertexNormal: Vector3 = [
      mesh.normals[i] ?? 0,
      mesh.normals[i + 1] ?? 0,
      mesh.normals[i + 2] ?? 0,
    ]
    if (
      Math.abs(dot(subtract(point, centroid), normal)) <= tolerance &&
      dot(vertexNormal, normal) >= NORMAL_AGREEMENT
    ) {
      points.push(point)
    }
  }
  return points
}

/**
 * The model points that span `element` (3.7): a flat face's vertices in its body's model `mesh`,
 * a straight edge's two ends, and otherwise, or while the mesh is unknown, its centroid.
 */
export function elementExtentPoints(
  element: ElementDescriptor,
  mesh: BodyMeshData | null,
): Vector3[] {
  const { centroid, direction, length, normal } = element
  if (element.kind === 'edge' && direction != null && length != null) {
    return [addScaled(centroid, direction, -length / 2), addScaled(centroid, direction, length / 2)]
  }
  if (element.kind === 'face' && normal != null && mesh !== null) {
    const points = planeVertices(mesh, centroid, normal)
    if (points.length > 0) {
      return points
    }
  }
  return [centroid]
}
