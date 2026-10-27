// The face algorithms of the shim's mesher (kernel spec section 3.7, item 2): for each surface
// type the algorithm that OCCT 7.9.3's Delabella factory gives it, with the shim's copy of
// Delabella, in double-double arithmetic, wherever that factory uses OCCT's own copy.
#include <array>
#include <cstdlib>
#include <exception>
#include <memory>
#include <vector>

#include <BRepMesh_BoundaryParamsRangeSplitter.hxx>
#include <BRepMesh_ConeRangeSplitter.hxx>
#include <BRepMesh_CustomBaseMeshAlgo.hxx>
#include <BRepMesh_CustomDelaunayBaseMeshAlgo.hxx>
#include <BRepMesh_CylinderRangeSplitter.hxx>
#include <BRepMesh_DefaultRangeSplitter.hxx>
#include <BRepMesh_DelaunayBaseMeshAlgo.hxx>
#include <BRepMesh_DelaunayDeflectionControlMeshAlgo.hxx>
#include <BRepMesh_DelaunayNodeInsertionMeshAlgo.hxx>
#include <BRepMesh_NURBSRangeSplitter.hxx>
#include <BRepMesh_SphereRangeSplitter.hxx>
#include <BRepMesh_TorusRangeSplitter.hxx>
#include <Bnd_B2d.hxx>
#include <IMeshTools_MeshAlgoFactory.hxx>
#include <Standard_Failure.hxx>

#include "partsmith-occt-sys/cpp/delabella/delabella.h"
#include "partsmith-occt-sys/cpp/internal.h"

namespace partsmith {
namespace occt {
namespace detail {

namespace {

// Destroys a triangulator that Delabella created.
struct DestroyTriangulator {
  void operator()(delabella::IDelaBella *triangulator) const { triangulator->Destroy(); }
};

// Triangulates the nodes of a face at once with the shim's copy of Delabella. The OCCT code it
// derives from then recovers the face's boundary edges and removes the auxiliary nodes with their
// triangles and the triangles outside the face.
class DelabellaBaseAlgorithm : public BRepMesh_CustomBaseMeshAlgo {
 protected:
  void buildBaseTriangulation() override;
};

void DelabellaBaseAlgorithm::buildBaseTriangulation() {
  const Handle(BRepMesh_DataStructureOfDelaun) &structure = getStructure();

  // The face's nodes, and the four corners of their bounding box grown by a tenth of its
  // diagonal as auxiliary nodes.
  std::vector<gp_XY> positions;
  positions.reserve(static_cast<std::size_t>(structure->NbNodes()) + 4);
  Bnd_B2d box;
  for (int node = 1; node <= structure->NbNodes(); ++node) {
    positions.push_back(structure->GetNode(node).Coord());
    box.Add(positions.back());
  }
  box.Enlarge(0.1 * (box.CornerMax() - box.CornerMin()).Modulus());
  const gp_XY minimum = box.CornerMin();
  const gp_XY maximum = box.CornerMax();
  const std::array<gp_XY, 4> corners = {minimum, gp_XY(maximum.X(), minimum.Y()), maximum,
                                        gp_XY(minimum.X(), maximum.Y())};
  for (const gp_XY &corner : corners) {
    structure->AddNode(BRepMesh_Vertex(corner.X(), corner.Y(), BRepMesh_Free));
    positions.push_back(corner);
  }

  // Every node in the unit square of the grown box, as OCCT's own base algorithm gives them.
  const double width = maximum.X() - minimum.X();
  const double height = maximum.Y() - minimum.Y();
  std::vector<double> points;
  points.reserve(2 * positions.size());
  for (const gp_XY &position : positions) {
    points.push_back((position.X() - minimum.X()) / width - 0.5);
    points.push_back((position.Y() - minimum.Y()) / height - 0.5);
  }

  const std::unique_ptr<delabella::IDelaBella, DestroyTriangulator> triangulator(
      delabella::IDelaBella::Create());
  int triangulated = 0;
  try {
    triangulated =
        triangulator->Triangulate(static_cast<int>(positions.size()), &points[0], &points[1],
                                  static_cast<int>(2 * sizeof(double)));
  } catch (const std::exception &exception) {
    // A check of the copy failed. OCCT's mesher gives a face whose algorithm raises
    // Standard_Failure no triangulation (kernel spec sections 3.7 item 7 and 3.9 item 7).
    throw Standard_Failure(exception.what());
  }
  // Nodes all on one line give no triangles, and the face then no triangulation.
  if (triangulated <= 0) {
    return;
  }
  for (const delabella::DelaBella_Triangle *triangle = triangulator->GetFirstDelaunayTriangle();
       triangle != nullptr; triangle = triangle->next) {
    // Delabella winds its triangles clockwise, the mesher counterclockwise.
    const std::array<int, 3> nodes = {triangle->v[0]->i + 1, triangle->v[2]->i + 1,
                                      triangle->v[1]->i + 1};
    int links[3];
    bool orientations[3];
    for (int side = 0; side < 3; ++side) {
      const int link =
          structure->AddLink(BRepMesh_Edge(nodes[side], nodes[(side + 1) % 3], BRepMesh_Free));
      links[side] = std::abs(link);
      orientations[side] = link > 0;
    }
    structure->AddElement(BRepMesh_Triangle(links, orientations, BRepMesh_Free));
  }
}

// OCCT's algorithms that add nodes inside a face and refine its triangulation, over the shim's
// base algorithm, as OCCT's Delabella factory composes them over its own.
using DelabellaDelaunay = BRepMesh_CustomDelaunayBaseMeshAlgo<DelabellaBaseAlgorithm>;
template <class RangeSplitter>
using NodeInsertion = BRepMesh_DelaunayNodeInsertionMeshAlgo<RangeSplitter, DelabellaDelaunay>;
template <class RangeSplitter>
using DeflectionControl =
    BRepMesh_DelaunayDeflectionControlMeshAlgo<RangeSplitter, DelabellaDelaunay>;
// OCCT's node insertion over its Watson algorithm, for cylinders.
template <class RangeSplitter>
using WatsonNodeInsertion =
    BRepMesh_DelaunayNodeInsertionMeshAlgo<RangeSplitter, BRepMesh_DelaunayBaseMeshAlgo>;

// An algorithm that registers the surface's nodes before the face is triangulated.
template <class Algorithm>
Handle(IMeshTools_MeshAlgo) with_surface_nodes_first() {
  Algorithm *algorithm = new Algorithm;
  const Handle(IMeshTools_MeshAlgo) owner = algorithm;
  algorithm->SetPreProcessSurfaceNodes(Standard_True);
  return owner;
}

// Gives each face the algorithm OCCT 7.9.3's BRepMesh_DelabellaMeshAlgoFactory gives it, by
// surface type, with the shim's base algorithm in place of OCCT's.
class MeshAlgorithmFactory : public IMeshTools_MeshAlgoFactory {
 public:
  Handle(IMeshTools_MeshAlgo) GetAlgo(const GeomAbs_SurfaceType surface,
                                      const IMeshTools_Parameters &parameters) const override {
    switch (surface) {
      case GeomAbs_Plane:
        if (parameters.InternalVerticesMode) {
          return new NodeInsertion<BRepMesh_DefaultRangeSplitter>;
        }
        return new DelabellaBaseAlgorithm;
      case GeomAbs_Sphere:
        return with_surface_nodes_first<NodeInsertion<BRepMesh_SphereRangeSplitter>>();
      case GeomAbs_Cylinder:
        if (parameters.InternalVerticesMode) {
          return new WatsonNodeInsertion<BRepMesh_CylinderRangeSplitter>;
        }
        return new BRepMesh_DelaunayBaseMeshAlgo;
      case GeomAbs_Cone:
        return with_surface_nodes_first<NodeInsertion<BRepMesh_ConeRangeSplitter>>();
      case GeomAbs_Torus:
        return with_surface_nodes_first<NodeInsertion<BRepMesh_TorusRangeSplitter>>();
      case GeomAbs_SurfaceOfRevolution:
        return with_surface_nodes_first<
            DeflectionControl<BRepMesh_BoundaryParamsRangeSplitter>>();
      default:
        return with_surface_nodes_first<DeflectionControl<BRepMesh_NURBSRangeSplitter>>();
    }
  }
};

}  // namespace

Handle(IMeshTools_MeshAlgoFactory) mesh_algorithm_factory() { return new MeshAlgorithmFactory; }

}  // namespace detail
}  // namespace occt
}  // namespace partsmith
