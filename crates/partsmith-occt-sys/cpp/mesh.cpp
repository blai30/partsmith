// Tessellation into flat mesh arrays, and the display geometry of one element (kernel spec section
// 3.7).
#include <algorithm>
#include <array>
#include <vector>

#include <BRepAdaptor_Curve.hxx>
#include <BRepBuilderAPI_Copy.hxx>
#include <BRepMesh_IncrementalMesh.hxx>
#include <BRep_Tool.hxx>
#include <GCPnts_TangentialDeflection.hxx>
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

// Appends a point's three coordinates to a flat list.
void push_point(rust::Vec<double> &values, const gp_XYZ &point) {
  values.push_back(point.X());
  values.push_back(point.Y());
  values.push_back(point.Z());
}

// The meshing parameters of kernel spec section 3.7, item 2.
IMeshTools_Parameters mesh_parameters(double chord_deviation, double angle) {
  IMeshTools_Parameters parameters;
  parameters.Deflection = chord_deviation;
  parameters.Angle = angle;
  parameters.Relative = Standard_False;
  parameters.InParallel = Standard_False;
  // Delabella triangulates each face's nodes at once, where OCCT's default inserts them one by
  // one, which took seconds on large spheres and on planar faces with many holes (kernel spec
  // section 3.7, item 2). Naming the algorithm also keeps OCCT from choosing it from the
  // CSF_MeshAlgo environment variable.
  parameters.MeshAlgo = IMeshTools_MeshAlgoType_Delabella;
  return parameters;
}

// The triangulation the mesher stored on face `index`, in the frame of the face without
// `location`; raises StdFail_NotDone when it stored none, as on a face without a wire (kernel
// spec section 3.7, items 7 and 9.2).
const Handle(Poly_Triangulation) &
    triangulation_of(const TopoDS_Face &face, std::uint32_t index, TopLoc_Location &location) {
  const Handle(Poly_Triangulation) &triangulation = BRep_Tool::Triangulation(face, location);
  if (triangulation.IsNull()) {
    raise_not_done("BRepMesh_IncrementalMesh gave face " + std::to_string(index) +
                   " no triangulation");
  }
  return triangulation;
}

// The nodes of a triangle wound counterclockwise seen from outside: the triangulation's order,
// with two nodes swapped on a reversed face (kernel spec section 3.7, item 3).
std::array<int, 3> outward_nodes(const Poly_Triangulation &triangulation, int triangle,
                                 bool reversed) {
  int first = 0, second = 0, third = 0;
  triangulation.Triangle(triangle).Get(first, second, third);
  if (reversed) {
    std::swap(second, third);
  }
  return {first, second, third};
}

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
    const Handle(Poly_Triangulation) &triangulation =
        triangulation_of(face, static_cast<std::uint32_t>(face_index), location);
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
      for (int node : outward_nodes(*triangulation, triangle, reversed)) {
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
  BRepMesh_IncrementalMesh mesher(copy.get(), mesh_parameters(chord_deviation, angle));
  return MeshBuilder(copy).build();
}

namespace {

// The triangles of one face, three points each, wound counterclockwise seen from outside (kernel
// spec section 3.7, item 9.2). Only a copy of the face alone is meshed, which shares its geometry
// and keeps its location and orientation, so the triangulation is stored on the copy.
rust::Vec<double> face_display(const TopoDS_Face &face, std::uint32_t index, double chord_deviation,
                               double angle) {
  BRepBuilderAPI_Copy copier(face, Standard_False, Standard_False);
  const TopoDS_Face copy = TopoDS::Face(copier.Shape());
  BRepMesh_IncrementalMesh mesher(copy, mesh_parameters(chord_deviation, angle));
  TopLoc_Location location;
  const Handle(Poly_Triangulation) &triangulation = triangulation_of(copy, index, location);
  // The triangulation lies in the frame of the face without its location, like a whole shape's.
  const gp_Trsf transformation = location.Transformation();
  const bool reversed = copy.Orientation() == TopAbs_REVERSED;
  rust::Vec<double> points;
  points.reserve(9 * static_cast<std::size_t>(triangulation->NbTriangles()));
  for (int triangle = 1; triangle <= triangulation->NbTriangles(); ++triangle) {
    for (int node : outward_nodes(*triangulation, triangle, reversed)) {
      push_point(points, triangulation->Node(node).Transformed(transformation).XYZ());
    }
  }
  return points;
}

// The polyline of one edge from its first vertex to its last, a closed edge ending exactly at its
// first point (kernel spec section 3.7, item 9.3).
rust::Vec<double> edge_display(const TopoDS_Edge &edge, std::uint32_t index, double chord_deviation,
                               double angle) {
  require_curve(edge, index);
  const BRepAdaptor_Curve curve(edge);
  const GCPnts_TangentialDeflection discretization(curve, angle, chord_deviation, 2);
  if (discretization.NbPoints() < 2) {
    raise_not_done("GCPnts_TangentialDeflection gave edge " + std::to_string(index) +
                   " fewer than 2 points");
  }
  std::vector<gp_XYZ> polyline;
  polyline.reserve(static_cast<std::size_t>(discretization.NbPoints()));
  for (int point = 1; point <= discretization.NbPoints(); ++point) {
    polyline.push_back(discretization.Value(point).XYZ());
  }
  // The curve runs along its parameter; a reversed edge runs the other way.
  if (edge.Orientation() == TopAbs_REVERSED) {
    std::reverse(polyline.begin(), polyline.end());
  }
  TopoDS_Vertex first_vertex, last_vertex;
  TopExp::Vertices(edge, first_vertex, last_vertex);
  if (!first_vertex.IsNull() && first_vertex.IsSame(last_vertex)) {
    polyline.back() = polyline.front();
  }
  rust::Vec<double> points;
  points.reserve(3 * polyline.size());
  for (const gp_XYZ &point : polyline) {
    push_point(points, point);
  }
  return points;
}

}  // namespace

rust::Vec<double> shape_element_display(const Shape &shape, ElementKind kind, std::uint32_t index,
                                        double chord_deviation, double angle) {
  const TopoDS_Shape &element = element_at(shape, kind, index);
  switch (element.ShapeType()) {
    case TopAbs_FACE:
      return face_display(TopoDS::Face(element), index, chord_deviation, angle);
    case TopAbs_EDGE:
      return edge_display(TopoDS::Edge(element), index, chord_deviation, angle);
    case TopAbs_VERTEX: {
      rust::Vec<double> position;
      push_point(position, BRep_Tool::Pnt(TopoDS::Vertex(element)).XYZ());
      return position;
    }
    default:
      raise_invalid_input("Only faces, edges and vertices can be queried.");
  }
}

}  // namespace occt
}  // namespace partsmith
