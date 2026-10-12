// Tessellation into flat mesh arrays (kernel spec section 3.7).
#include <algorithm>
#include <vector>

#include <BRepMesh_IncrementalMesh.hxx>
#include <BRep_Tool.hxx>
#include <GeomLib.hxx>
#include <Geom_Surface.hxx>
#include <IMeshTools_Parameters.hxx>
#include <Poly_PolygonOnTriangulation.hxx>
#include <Poly_Triangulation.hxx>
#include <Precision.hxx>
#include <TopExp.hxx>
#include <TopExp_Explorer.hxx>
#include <TopoDS.hxx>
#include <TopoDS_Edge.hxx>
#include <TopoDS_Face.hxx>
#include <TopoDS_Vertex.hxx>

#include "partsmith-occt-sys/cpp/internal.h"

namespace partsmith {
namespace occt {

using namespace detail;

namespace {

// The mesh vertices one face's triangulation gives an edge, in increasing edge parameter.
using EdgeUse = std::vector<std::uint32_t>;

// Builds the mesh of a triangulated copy of a shape.
class MeshBuilder {
 public:
  explicit MeshBuilder(const Shape &shape)
      : shape_(shape),
        uses_(static_cast<std::size_t>(shape.edges().Extent()) + 1),
        seams_(static_cast<std::size_t>(shape.edges().Extent()) + 1, false),
        discretizations_(static_cast<std::size_t>(shape.edges().Extent()) + 1) {}

  MeshData build() {
    for (int face = 1; face <= shape_.faces().Extent(); ++face) {
      add_face(face);
    }
    for (int edge = 1; edge <= shape_.edges().Extent(); ++edge) {
      unify_edge(edge);
    }
    add_polylines_and_markers();
    return std::move(data_);
  }

 private:
  // The number of mesh vertices so far.
  std::uint32_t vertex_count() const {
    return static_cast<std::uint32_t>(data_.positions.size() / 3);
  }

  void set_position(std::uint32_t vertex, const gp_XYZ &point) {
    data_.positions[3 * vertex] = point.X();
    data_.positions[3 * vertex + 1] = point.Y();
    data_.positions[3 * vertex + 2] = point.Z();
  }

  gp_XYZ position(std::uint32_t vertex) const {
    return gp_XYZ(data_.positions[3 * vertex], data_.positions[3 * vertex + 1],
                  data_.positions[3 * vertex + 2]);
  }

  static void push_point(rust::Vec<double> &values, const gp_XYZ &point) {
    values.push_back(point.X());
    values.push_back(point.Y());
    values.push_back(point.Z());
  }

  // Unit normals of the triangulation nodes in the triangulation's own frame: from the surface
  // at each node's (u, v), or, where the surface normal is undefined, from the adjacent
  // triangles (wound like the surface).
  static std::vector<gp_Dir> node_normals(const TopoDS_Face &face,
                                          const Handle(Poly_Triangulation) & triangulation) {
    const int count = triangulation->NbNodes();
    std::vector<gp_XYZ> flat(static_cast<std::size_t>(count) + 1, gp_XYZ(0.0, 0.0, 0.0));
    for (int triangle = 1; triangle <= triangulation->NbTriangles(); ++triangle) {
      int first = 0, second = 0, third = 0;
      triangulation->Triangle(triangle).Get(first, second, third);
      const gp_XYZ origin = triangulation->Node(first).XYZ();
      const gp_XYZ normal = (triangulation->Node(second).XYZ() - origin)
                                .Crossed(triangulation->Node(third).XYZ() - origin);
      for (int node : {first, second, third}) {
        flat[static_cast<std::size_t>(node)] += normal;
      }
    }
    const Handle(Geom_Surface) surface =
        BRep_Tool::Surface(TopoDS::Face(face.Located(TopLoc_Location())));
    std::vector<gp_Dir> normals(static_cast<std::size_t>(count) + 1, gp::DZ());
    for (int node = 1; node <= count; ++node) {
      gp_Dir normal = gp::DZ();
      const bool estimated = triangulation->HasUVNodes() && !surface.IsNull() &&
                             GeomLib::NormEstim(surface, triangulation->UVNode(node),
                                                Precision::Confusion(), normal) < 2;
      if (estimated) {
        normals[static_cast<std::size_t>(node)] = normal;
      } else if (flat[static_cast<std::size_t>(node)].Modulus() > gp::Resolution()) {
        normals[static_cast<std::size_t>(node)] = gp_Dir(flat[static_cast<std::size_t>(node)]);
      }
    }
    return normals;
  }

  // Adds a face's nodes and triangles and records the nodes on its edges.
  void add_face(int face_index) {
    const TopoDS_Face &face = TopoDS::Face(shape_.faces()(face_index));
    TopLoc_Location location;
    const Handle(Poly_Triangulation) &triangulation = BRep_Tool::Triangulation(face, location);
    if (triangulation.IsNull()) {
      raise_not_done("BRepMesh_IncrementalMesh gave face " + std::to_string(face_index) +
                     " no triangulation");
    }
    const gp_Trsf transformation = location.Transformation();
    const bool reversed = face.Orientation() == TopAbs_REVERSED;
    const std::uint32_t base = vertex_count();

    // The triangulation and the normals are in the frame of the face without its location;
    // the face location places both in model space.
    const std::vector<gp_Dir> normals = node_normals(face, triangulation);
    for (int node = 1; node <= triangulation->NbNodes(); ++node) {
      push_point(data_.positions, triangulation->Node(node).Transformed(transformation).XYZ());
      gp_Dir normal = normals[static_cast<std::size_t>(node)].Transformed(transformation);
      if (reversed) {
        normal.Reverse();
      }
      push_point(data_.normals, normal.XYZ());
    }

    for (int triangle = 1; triangle <= triangulation->NbTriangles(); ++triangle) {
      int first = 0, second = 0, third = 0;
      triangulation->Triangle(triangle).Get(first, second, third);
      if (reversed) {
        std::swap(second, third);
      }
      for (int node : {first, second, third}) {
        data_.triangles.push_back(base + static_cast<std::uint32_t>(node) - 1);
      }
      data_.face_ids.push_back(static_cast<std::uint32_t>(face_index));
    }

    for (TopExp_Explorer explorer(face, TopAbs_EDGE); explorer.More(); explorer.Next()) {
      const TopoDS_Edge &edge = TopoDS::Edge(explorer.Current());
      const int edge_index = shape_.edges().FindIndex(edge);
      const Handle(Poly_PolygonOnTriangulation) &polygon =
          BRep_Tool::PolygonOnTriangulation(edge, triangulation, location);
      if (polygon.IsNull() || edge_index == 0) {
        raise_not_done("BRepMesh_IncrementalMesh did not discretize edge " +
                       std::to_string(edge_index) + " on face " + std::to_string(face_index));
      }
      if (BRep_Tool::IsClosed(edge, face)) {
        seams_[static_cast<std::size_t>(edge_index)] = true;
      }
      EdgeUse use;
      use.reserve(static_cast<std::size_t>(polygon->NbNodes()));
      for (int node = 1; node <= polygon->NbNodes(); ++node) {
        use.push_back(base + static_cast<std::uint32_t>(polygon->Node(node)) - 1);
      }
      const int last = polygon->NbNodes();
      if (polygon->HasParameters() && last > 1 &&
          polygon->Parameter(1) > polygon->Parameter(last)) {
        std::reverse(use.begin(), use.end());
      }
      uses_[static_cast<std::size_t>(edge_index)].push_back(std::move(use));
    }
  }

  // Gives every node on an edge, in every face, the same coordinates: the edge's single
  // discretization from its first use, with its ends exactly at its vertices (kernel spec
  // section 3.7, item 4).
  void unify_edge(int edge_index) {
    std::vector<EdgeUse> &uses = uses_[static_cast<std::size_t>(edge_index)];
    if (uses.empty()) {
      return;
    }
    const TopoDS_Edge &edge = TopoDS::Edge(shape_.edges()(edge_index));
    TopoDS_Vertex first_vertex, last_vertex;
    TopExp::Vertices(edge, first_vertex, last_vertex);
    std::vector<gp_XYZ> points;
    points.reserve(uses.front().size());
    for (std::uint32_t vertex : uses.front()) {
      points.push_back(position(vertex));
    }
    if (BRep_Tool::Degenerated(edge)) {
      std::fill(points.begin(), points.end(), BRep_Tool::Pnt(first_vertex).XYZ());
    } else if (!points.empty()) {
      points.front() = BRep_Tool::Pnt(first_vertex).XYZ();
      points.back() = BRep_Tool::Pnt(last_vertex).XYZ();
    }
    for (const EdgeUse &use : uses) {
      if (use.size() != points.size()) {
        raise_not_done("BRepMesh_IncrementalMesh discretized edge " + std::to_string(edge_index) +
                       " differently on its faces");
      }
      for (std::size_t position = 0; position < use.size(); ++position) {
        set_position(use[position], points[position]);
      }
    }
    discretizations_[static_cast<std::size_t>(edge_index)] = std::move(points);
  }

  // Adds a polyline per drawn edge and a marker per vertex shared by two drawn edges (kernel
  // spec section 3.7, items 5 and 6).
  void add_polylines_and_markers() {
    std::vector<int> drawn_edges_at(static_cast<std::size_t>(shape_.vertices().Extent()) + 1, 0);
    data_.edge_offsets.push_back(0);
    for (int edge_index = 1; edge_index <= shape_.edges().Extent(); ++edge_index) {
      const TopoDS_Edge &edge = TopoDS::Edge(shape_.edges()(edge_index));
      const std::vector<gp_XYZ> &points = discretizations_[static_cast<std::size_t>(edge_index)];
      if (points.empty() || BRep_Tool::Degenerated(edge) ||
          seams_[static_cast<std::size_t>(edge_index)]) {
        continue;
      }
      std::vector<gp_XYZ> ordered = points;
      if (edge.Orientation() == TopAbs_REVERSED) {
        std::reverse(ordered.begin(), ordered.end());
      }
      for (const gp_XYZ &point : ordered) {
        push_point(data_.edge_points, point);
      }
      data_.edge_ids.push_back(static_cast<std::uint32_t>(edge_index));
      data_.edge_offsets.push_back(static_cast<std::uint32_t>(data_.edge_points.size() / 3));
      TopTools_IndexedMapOfShape vertices;
      TopExp::MapShapes(edge, TopAbs_VERTEX, vertices);
      for (int position = 1; position <= vertices.Extent(); ++position) {
        const int vertex = shape_.vertices().FindIndex(vertices(position));
        drawn_edges_at[static_cast<std::size_t>(vertex)] += 1;
      }
    }
    for (int vertex = 1; vertex <= shape_.vertices().Extent(); ++vertex) {
      if (drawn_edges_at[static_cast<std::size_t>(vertex)] >= 2) {
        data_.marker_ids.push_back(static_cast<std::uint32_t>(vertex));
        push_point(data_.marker_positions,
                   BRep_Tool::Pnt(TopoDS::Vertex(shape_.vertices()(vertex))).XYZ());
      }
    }
  }

  const Shape &shape_;
  std::vector<std::vector<EdgeUse>> uses_;
  std::vector<bool> seams_;
  // Each edge's discretization in increasing parameter; empty for edges without faces.
  std::vector<std::vector<gp_XYZ>> discretizations_;
  MeshData data_;
};

}  // namespace

MeshData shape_tessellate(const Shape &shape, double chord_deviation, double angle) {
  // Mesh a copy so the input never carries a triangulation (kernel spec section 3.7, item 2).
  const Shape copy = copy_shape(shape);
  IMeshTools_Parameters parameters;
  parameters.Deflection = chord_deviation;
  parameters.Angle = angle;
  parameters.Relative = Standard_False;
  parameters.InParallel = Standard_False;
  parameters.MeshAlgo = IMeshTools_MeshAlgoType_Delabella;
  BRepMesh_IncrementalMesh mesher(copy.get(), parameters);
  return MeshBuilder(copy).build();
}

}  // namespace occt
}  // namespace partsmith
