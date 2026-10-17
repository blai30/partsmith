// Shape queries: indexing, validity, bounds, element geometry, topology, edge normals and widths
// across edges (kernel spec sections 3.2 and 3.6).
#include <algorithm>
#include <optional>
#include <utility>
#include <vector>

#include <BRepAdaptor_Curve.hxx>
#include <BRepAdaptor_Surface.hxx>
#include <BRepBndLib.hxx>
#include <BRepBuilderAPI_MakeVertex.hxx>
#include <BRepCheck_Analyzer.hxx>
#include <BRepClass_FaceClassifier.hxx>
#include <BRepExtrema_DistShapeShape.hxx>
#include <BRepGProp.hxx>
#include <BRepLProp_SLProps.hxx>
#include <BRepTools.hxx>
#include <BRep_CurveRepresentation.hxx>
#include <BRep_TEdge.hxx>
#include <BRep_Tool.hxx>
#include <Bnd_Box.hxx>
#include <GProp_GProps.hxx>
#include <Geom2d_Curve.hxx>
#include <GeomAPI_ProjectPointOnSurf.hxx>
#include <TopExp.hxx>
#include <TopExp_Explorer.hxx>
#include <TopoDS.hxx>
#include <TopoDS_Iterator.hxx>
#include <gp_Circ.hxx>
#include <gp_Cone.hxx>
#include <gp_Cylinder.hxx>
#include <gp_Elips.hxx>
#include <gp_Hypr.hxx>
#include <gp_Lin.hxx>
#include <gp_Parab.hxx>
#include <gp_Pln.hxx>
#include <gp_Pnt2d.hxx>
#include <gp_Sphere.hxx>
#include <gp_Torus.hxx>

#include "partsmith-occt-sys/cpp/internal.h"

namespace partsmith {
namespace occt {

using namespace detail;

std::shared_ptr<Shape> empty_shape() { return make_shape(compound_of({})); }

ShapeType shape_type(const Shape &shape) {
  switch (shape.get().ShapeType()) {
    case TopAbs_COMPOUND:
      return ShapeType::Compound;
    case TopAbs_COMPSOLID:
      return ShapeType::CompSolid;
    case TopAbs_SOLID:
      return ShapeType::Solid;
    case TopAbs_SHELL:
      return ShapeType::Shell;
    case TopAbs_FACE:
      return ShapeType::Face;
    case TopAbs_WIRE:
      return ShapeType::Wire;
    case TopAbs_EDGE:
      return ShapeType::Edge;
    case TopAbs_VERTEX:
    case TopAbs_SHAPE:
      break;
  }
  // TopAbs_SHAPE never describes a concrete shape, so only vertices reach this point.
  return ShapeType::Vertex;
}

std::uint32_t shape_count(const Shape &shape, ElementKind kind) {
  return static_cast<std::uint32_t>(shape.map(kind).Extent());
}

namespace detail {

const TopoDS_Shape &element_at(const Shape &shape, ElementKind kind, std::uint32_t index) {
  const TopTools_IndexedMapOfShape &map = shape.map(kind);
  if (index == 0 || index > static_cast<std::uint32_t>(map.Extent())) {
    raise_invalid_input("Element " + std::to_string(index) + " does not exist in this body.");
  }
  return map(static_cast<int>(index));
}

void require_curve(const TopoDS_Edge &edge, std::uint32_t index) {
  if (BRep_Tool::Degenerated(edge)) {
    raise_invalid_input("Edge " + std::to_string(index) +
                        " is a degenerated edge, so it has no curve to measure or draw.");
  }
}

}  // namespace detail

namespace {

// The point on an axis closest to a point.
gp_Pnt closest_on_axis(const gp_Ax1 &axis, const gp_Pnt &point) {
  const gp_XYZ origin = axis.Location().XYZ();
  const gp_XYZ direction = axis.Direction().XYZ();
  return gp_Pnt(origin + direction * (point.XYZ() - origin).Dot(direction));
}

// Sets the axis direction and the point on it closest to the centroid.
void set_axis(ElementData &data, const gp_Ax1 &axis) {
  data.has_direction = true;
  data.direction = to_vec3(axis.Direction().XYZ());
  data.has_location = true;
  data.location = to_vec3(closest_on_axis(axis, to_point(data.centroid)).XYZ());
}

// Geometry of a face (kernel spec section 3.6, item 3).
ElementData face_element(const TopoDS_Face &face) {
  const BRepAdaptor_Surface surface(face, Standard_False);
  const FaceMass mass = face_mass(face, surface);
  ElementData data{};
  data.centroid = to_vec3(mass.centroid.XYZ());
  data.measure = mass.area;
  switch (surface.GetType()) {
    case GeomAbs_Plane: {
      const gp_Pln plane = surface.Plane();
      gp_Dir normal = plane.Axis().Direction();
      if (!plane.Position().Direct()) {
        normal.Reverse();
      }
      if (face.Orientation() == TopAbs_REVERSED) {
        normal.Reverse();
      }
      data.kind = GeometryType::Plane;
      data.has_direction = true;
      data.direction = to_vec3(normal.XYZ());
      data.has_location = true;
      data.location = data.centroid;
      break;
    }
    case GeomAbs_Cylinder: {
      const gp_Cylinder cylinder = surface.Cylinder();
      data.kind = GeometryType::Cylinder;
      set_axis(data, cylinder.Axis());
      data.has_radius = true;
      data.radius = cylinder.Radius();
      break;
    }
    case GeomAbs_Cone:
      data.kind = GeometryType::Cone;
      set_axis(data, surface.Cone().Axis());
      break;
    case GeomAbs_Sphere: {
      const gp_Sphere sphere = surface.Sphere();
      data.kind = GeometryType::Sphere;
      data.has_location = true;
      data.location = to_vec3(sphere.Location().XYZ());
      data.has_radius = true;
      data.radius = sphere.Radius();
      break;
    }
    case GeomAbs_Torus: {
      const gp_Torus torus = surface.Torus();
      data.kind = GeometryType::Torus;
      set_axis(data, torus.Axis());
      data.has_radius = true;
      data.radius = torus.MajorRadius();
      break;
    }
    case GeomAbs_SurfaceOfRevolution:
      data.kind = GeometryType::Revolution;
      set_axis(data, surface.AxeOfRevolution());
      break;
    case GeomAbs_SurfaceOfExtrusion:
      data.kind = GeometryType::Extrusion;
      data.has_direction = true;
      data.direction = to_vec3(surface.Direction().XYZ());
      break;
    case GeomAbs_BSplineSurface:
      data.kind = GeometryType::BsplineSurface;
      break;
    case GeomAbs_BezierSurface:
      data.kind = GeometryType::BezierSurface;
      break;
    case GeomAbs_OffsetSurface:
      data.kind = GeometryType::OffsetSurface;
      break;
    case GeomAbs_OtherSurface:
      data.kind = GeometryType::OtherSurface;
      break;
  }
  return data;
}

// Sets the axis direction and center of a conic.
void set_conic(ElementData &data, const gp_Ax1 &axis, const gp_Pnt &center) {
  data.has_direction = true;
  data.direction = to_vec3(axis.Direction().XYZ());
  data.has_location = true;
  data.location = to_vec3(center.XYZ());
}

// Geometry of an edge (kernel spec section 3.6, item 3).
ElementData edge_element(const TopoDS_Edge &edge) {
  ElementData data{};
  if (BRep_Tool::Degenerated(edge)) {
    data.kind = GeometryType::OtherCurve;
    data.centroid = to_vec3(BRep_Tool::Pnt(TopExp::FirstVertex(edge)).XYZ());
    return data;
  }
  GProp_GProps properties;
  BRepGProp::LinearProperties(edge, properties);
  data.centroid = to_vec3(properties.CentreOfMass().XYZ());
  data.measure = properties.Mass();
  const BRepAdaptor_Curve curve(edge);
  switch (curve.GetType()) {
    case GeomAbs_Line: {
      data.kind = GeometryType::Line;
      const gp_Pnt first = BRep_Tool::Pnt(TopExp::FirstVertex(edge, Standard_True));
      const gp_Pnt last = BRep_Tool::Pnt(TopExp::LastVertex(edge, Standard_True));
      gp_Dir direction = curve.Line().Direction();
      if (first.Distance(last) > gp::Resolution()) {
        direction = gp_Dir(gp_Vec(first, last));
      }
      data.has_direction = true;
      data.direction = to_vec3(direction.XYZ());
      break;
    }
    case GeomAbs_Circle: {
      const gp_Circ circle = curve.Circle();
      data.kind = GeometryType::Circle;
      set_conic(data, circle.Axis(), circle.Location());
      data.has_radius = true;
      data.radius = circle.Radius();
      break;
    }
    case GeomAbs_Ellipse: {
      const gp_Elips ellipse = curve.Ellipse();
      data.kind = GeometryType::Ellipse;
      set_conic(data, ellipse.Axis(), ellipse.Location());
      break;
    }
    case GeomAbs_Hyperbola: {
      const gp_Hypr hyperbola = curve.Hyperbola();
      data.kind = GeometryType::Hyperbola;
      set_conic(data, hyperbola.Axis(), hyperbola.Location());
      break;
    }
    case GeomAbs_Parabola: {
      const gp_Parab parabola = curve.Parabola();
      data.kind = GeometryType::Parabola;
      set_conic(data, parabola.Axis(), parabola.Location());
      break;
    }
    case GeomAbs_BSplineCurve:
      data.kind = GeometryType::BsplineCurve;
      break;
    case GeomAbs_BezierCurve:
      data.kind = GeometryType::BezierCurve;
      break;
    case GeomAbs_OffsetCurve:
      data.kind = GeometryType::OffsetCurve;
      break;
    case GeomAbs_OtherCurve:
      data.kind = GeometryType::OtherCurve;
      break;
  }
  return data;
}

// Geometry of a vertex.
ElementData vertex_element(const TopoDS_Vertex &vertex) {
  ElementData data{};
  data.kind = GeometryType::Point;
  data.centroid = to_vec3(BRep_Tool::Pnt(vertex).XYZ());
  return data;
}

// Geometry of a face, edge or vertex.
ElementData element_data(const TopoDS_Shape &element) {
  switch (element.ShapeType()) {
    case TopAbs_FACE:
      return face_element(TopoDS::Face(element));
    case TopAbs_EDGE:
      return edge_element(TopoDS::Edge(element));
    case TopAbs_VERTEX:
      return vertex_element(TopoDS::Vertex(element));
    default:
      raise_invalid_input("Only faces, edges and vertices can be queried.");
  }
}

// Appends one list made of `indices`, distinct and ascending, to `lists`.
void append_list(IndexLists &lists, std::vector<std::uint32_t> &indices) {
  std::sort(indices.begin(), indices.end());
  indices.erase(std::unique(indices.begin(), indices.end()), indices.end());
  for (std::uint32_t index : indices) {
    lists.values.push_back(index);
  }
  lists.offsets.push_back(static_cast<std::uint32_t>(lists.values.size()));
}

// Adds the index in `map` of `element`, if it is there.
void add_index(std::vector<std::uint32_t> &indices, const TopTools_IndexedMapOfShape &map,
               const TopoDS_Shape &element) {
  const int index = map.FindIndex(element);
  if (index > 0) {
    indices.push_back(static_cast<std::uint32_t>(index));
  }
}

// Adds the indices of the edges and vertices at any depth below `shape`, as TopExp_Explorer
// finds them (orientation and location composed) but without its allocations.
void add_edges_and_vertices(const Shape &whole, const TopoDS_Shape &shape,
                            std::vector<std::uint32_t> &edges,
                            std::vector<std::uint32_t> &vertices) {
  for (TopoDS_Iterator child(shape); child.More(); child.Next()) {
    const TopoDS_Shape &sub_shape = child.Value();
    if (sub_shape.ShapeType() == TopAbs_VERTEX) {
      add_index(vertices, whole.vertices(), sub_shape);
      continue;
    }
    if (sub_shape.ShapeType() == TopAbs_EDGE) {
      add_index(edges, whole.edges(), sub_shape);
    }
    add_edges_and_vertices(whole, sub_shape, edges, vertices);
  }
}

// One entry of an inverted relation: an element index and a related index.
using Pair = std::pair<std::uint32_t, std::uint32_t>;

// The compressed lists of elements 1 to `count` from pairs (element, related), each pair once
// and the related indices of each element ascending in the order of `pairs` (a stable counting
// sort by element).
IndexLists invert(std::size_t count, const std::vector<Pair> &pairs) {
  // starts[i] is where the list of element i + 1 starts; starts[count] is the end.
  std::vector<std::uint32_t> starts(count + 1, 0);
  for (const Pair &pair : pairs) {
    ++starts[pair.first];
  }
  for (std::size_t position = 1; position <= count; ++position) {
    starts[position] += starts[position - 1];
  }
  // Now starts[i] is where the list of element i ends; filling from the back of each list
  // keeps the order of `pairs` once the ends have moved back to the starts.
  std::vector<std::uint32_t> values(starts[count]);
  for (auto pair = pairs.rbegin(); pair != pairs.rend(); ++pair) {
    values[--starts[pair->first]] = pair->second;
  }
  IndexLists lists;
  lists.offsets.reserve(count + 1);
  for (std::size_t position = 1; position <= count; ++position) {
    lists.offsets.push_back(starts[position]);
  }
  lists.offsets.push_back(static_cast<std::uint32_t>(values.size()));
  lists.values.reserve(values.size());
  for (std::uint32_t value : values) {
    lists.values.push_back(value);
  }
  return lists;
}

// True when the edge has a curve representation that makes it closed on some surface or
// triangulation; only such an edge can be a seam (BRep_Tool::IsClosed looks for one).
bool has_closed_representation(const TopoDS_Edge &edge) {
  const Handle(BRep_TEdge) storage = Handle(BRep_TEdge)::DownCast(edge.TShape());
  if (storage.IsNull()) {
    return false;
  }
  for (const Handle(BRep_CurveRepresentation) &representation : storage->Curves()) {
    if (representation->IsCurveOnClosedSurface() ||
        representation->IsPolygonOnClosedTriangulation()) {
      return true;
    }
  }
  return false;
}

}  // namespace

std::shared_ptr<Shape> shape_solid(const Shape &shape, std::uint32_t index) {
  return make_shape(element_at(shape, ElementKind::Solid, index));
}

rust::Vec<std::uint32_t> shape_locate(const Shape &shape, const Shape &part, ElementKind kind) {
  const TopTools_IndexedMapOfShape &parts = part.map(kind);
  const TopTools_IndexedMapOfShape &own = shape.map(kind);
  rust::Vec<std::uint32_t> indices;
  indices.reserve(static_cast<std::size_t>(parts.Extent()));
  for (int position = 1; position <= parts.Extent(); ++position) {
    indices.push_back(static_cast<std::uint32_t>(own.FindIndex(parts(position))));
  }
  return indices;
}

bool shape_is_valid(const Shape &shape) { return BRepCheck_Analyzer(shape.get()).IsValid(); }

BoundsData shape_bounding_box(const Shape &shape) {
  Bnd_Box box;
  BRepBndLib::AddOptimal(shape.get(), box, Standard_False, Standard_False);
  if (box.IsVoid()) {
    raise_invalid_input("This shape is empty, so it has no bounding box.");
  }
  double min_x = 0.0, min_y = 0.0, min_z = 0.0, max_x = 0.0, max_y = 0.0, max_z = 0.0;
  box.Get(min_x, min_y, min_z, max_x, max_y, max_z);
  return BoundsData{Vec3{min_x, min_y, min_z}, Vec3{max_x, max_y, max_z}};
}

ElementData shape_element(const Shape &shape, ElementKind kind, std::uint32_t index) {
  return element_data(element_at(shape, kind, index));
}

rust::Vec<ElementData> shape_elements(const Shape &shape, ElementKind kind) {
  const TopTools_IndexedMapOfShape &map = shape.map(kind);
  rust::Vec<ElementData> elements;
  elements.reserve(static_cast<std::size_t>(map.Extent()));
  for (int index = 1; index <= map.Extent(); ++index) {
    elements.push_back(element_data(map(index)));
  }
  return elements;
}

TopologyData shape_topology(const Shape &shape) {
  const std::size_t face_count = static_cast<std::size_t>(shape.faces().Extent());
  const std::size_t edge_count = static_cast<std::size_t>(shape.edges().Extent());
  const std::size_t vertex_count = static_cast<std::size_t>(shape.vertices().Extent());
  TopologyData data;
  data.face_edges.offsets.push_back(0);
  data.face_vertices.offsets.push_back(0);
  data.edge_vertices.offsets.push_back(0);
  // One walk over the faces and one over the edges give every relation; the inverse ones are
  // collected as pairs in ascending order of the related element.
  std::vector<Pair> edge_faces;
  std::vector<Pair> vertex_faces;
  std::vector<Pair> vertex_edges;
  std::vector<std::uint32_t> edges;
  std::vector<std::uint32_t> vertices;
  for (std::uint32_t face = 1; face <= face_count; ++face) {
    edges.clear();
    vertices.clear();
    add_edges_and_vertices(shape, shape.faces()(static_cast<int>(face)), edges, vertices);
    append_list(data.face_edges, edges);
    append_list(data.face_vertices, vertices);
    for (std::uint32_t edge : edges) {
      edge_faces.emplace_back(edge, face);
    }
    for (std::uint32_t vertex : vertices) {
      vertex_faces.emplace_back(vertex, face);
    }
  }
  data.edge_faces = invert(edge_count, edge_faces);
  data.vertex_faces = invert(vertex_count, vertex_faces);
  for (std::uint32_t edge = 1; edge <= edge_count; ++edge) {
    const TopoDS_Edge &stored = TopoDS::Edge(shape.edges()(static_cast<int>(edge)));
    edges.clear();
    vertices.clear();
    add_edges_and_vertices(shape, stored, edges, vertices);
    append_list(data.edge_vertices, vertices);
    for (std::uint32_t vertex : vertices) {
      vertex_edges.emplace_back(vertex, edge);
    }
    data.edge_degenerate.push_back(BRep_Tool::Degenerated(stored));
    bool seam = false;
    if (has_closed_representation(stored)) {
      const std::uint32_t first = data.edge_faces.offsets[edge - 1];
      const std::uint32_t last = data.edge_faces.offsets[edge];
      for (std::uint32_t position = first; position < last && !seam; ++position) {
        const int face = static_cast<int>(data.edge_faces.values[position]);
        seam = BRep_Tool::IsClosed(stored, TopoDS::Face(shape.faces()(face)));
      }
    }
    data.edge_seam.push_back(seam);
  }
  data.vertex_edges = invert(vertex_count, vertex_edges);
  return data;
}

namespace {

// The middle of the parameter range of an edge's curve (kernel spec section 3.6, item 5.2).
double middle_parameter(const BRepAdaptor_Curve &curve) {
  return 0.5 * (curve.FirstParameter() + curve.LastParameter());
}

// True when `edge` bounds `face`.
bool bounds_face(const TopoDS_Face &face, const TopoDS_Edge &edge) {
  for (TopExp_Explorer edges(face, TopAbs_EDGE); edges.More(); edges.Next()) {
    if (edges.Current().IsSame(edge)) {
      return true;
    }
  }
  return false;
}

// The outward unit normal of `face` at `point`, the point of `edge` at its parameter `middle`
// (kernel spec section 3.6, item 5). `edge_index` and `face_index` name them in messages.
gp_Dir normal_at_edge(const TopoDS_Face &face, const TopoDS_Edge &edge, double middle,
                      const gp_Pnt &point, std::uint32_t edge_index, int face_index) {
  TopLoc_Location location;
  if (BRep_Tool::Surface(face, location).IsNull()) {
    raise_not_done("face " + std::to_string(face_index) + " has no surface");
  }
  double first = 0.0, last = 0.0;
  const Handle(Geom2d_Curve) curve = BRep_Tool::CurveOnSurface(edge, face, first, last);
  gp_Pnt2d parameters;
  if (!curve.IsNull()) {
    parameters = curve->Value(middle);
  } else {
    GeomAPI_ProjectPointOnSurf projection(point, BRep_Tool::Surface(face));
    if (projection.NbPoints() == 0) {
      raise_not_done("GeomAPI_ProjectPointOnSurf found no point of face " +
                     std::to_string(face_index) + " at the middle of edge " +
                     std::to_string(edge_index));
    }
    double u_parameter = 0.0, v_parameter = 0.0;
    projection.LowerDistanceParameters(u_parameter, v_parameter);
    parameters.SetCoord(u_parameter, v_parameter);
  }
  const BRepAdaptor_Surface surface(face);
  BRepLProp_SLProps properties(surface, parameters.X(), parameters.Y(), 1, kLinear);
  if (!properties.IsNormalDefined()) {
    raise_invalid_input("Face " + std::to_string(face_index) +
                        " has no defined normal at the middle of edge " +
                        std::to_string(edge_index) + ".");
  }
  gp_Dir normal = properties.Normal();
  if (face.Orientation() == TopAbs_REVERSED) {
    normal.Reverse();
  }
  return normal;
}

}  // namespace

EdgeNormalsData shape_edge_normals(const Shape &shape, std::uint32_t edge_index) {
  const TopoDS_Edge &edge = TopoDS::Edge(element_at(shape, ElementKind::Edge, edge_index));
  const std::string name = "Edge " + std::to_string(edge_index);
  if (BRep_Tool::Degenerated(edge)) {
    raise_invalid_input(name + " is a degenerated edge, so it has no middle point.");
  }
  const BRepAdaptor_Curve curve(edge);
  const double middle = middle_parameter(curve);
  gp_Pnt point;
  gp_Vec derivative;
  curve.D1(middle, point, derivative);
  if (derivative.Magnitude() < gp::Resolution()) {
    raise_invalid_input(name + " has no direction at its middle.");
  }
  // The curve runs along its parameter; a reversed edge runs the other way.
  gp_Dir tangent(derivative);
  if (edge.Orientation() == TopAbs_REVERSED) {
    tangent.Reverse();
  }
  EdgeNormalsData data{to_vec3(point.XYZ()), to_vec3(tangent.XYZ()), {}, ContinuityType::Absent};
  // Faces in index order give the ascending order of the topology's face lists.
  std::vector<int> faces;
  for (int face_index = 1; face_index <= shape.faces().Extent(); ++face_index) {
    const TopoDS_Face &face = TopoDS::Face(shape.faces()(face_index));
    if (bounds_face(face, edge)) {
      const gp_Dir normal = normal_at_edge(face, edge, middle, point, edge_index, face_index);
      data.faces.push_back(
          FaceNormalData{static_cast<std::uint32_t>(face_index), to_vec3(normal.XYZ())});
      faces.push_back(face_index);
    }
  }
  // The continuity recorded across the edge (kernel spec section 3.6, item 5): between its two
  // faces, or for a seam between its face and itself. OCCT reports C0 when none is recorded.
  const auto face_at = [&shape](int index) { return TopoDS::Face(shape.faces()(index)); };
  std::optional<GeomAbs_Shape> continuity;
  if (faces.size() == 2) {
    continuity = BRep_Tool::Continuity(edge, face_at(faces[0]), face_at(faces[1]));
  } else if (faces.size() == 1 && BRep_Tool::IsClosed(edge, face_at(faces[0]))) {
    continuity = BRep_Tool::Continuity(edge, face_at(faces[0]), face_at(faces[0]));
  }
  if (continuity) {
    data.continuity = *continuity == GeomAbs_C0 ? ContinuityType::C0 : ContinuityType::G1OrBetter;
  }
  return data;
}

namespace {

// The cells per side of the parameter grid a width is sampled on (kernel spec section 3.6, item
// 7.2).
constexpr int kWidthGridCells = 8;

// The points of `face` a width across an edge is measured from, with the face location applied
// (kernel spec section 3.6, item 7.2): its vertices, the middles of its edges that are not
// degenerated, and the centers of the cells of a grid over its parameter bounds that lie on it.
std::vector<gp_Pnt> width_samples(const TopoDS_Face &face) {
  std::vector<gp_Pnt> samples;
  TopTools_IndexedMapOfShape vertices;
  TopExp::MapShapes(face, TopAbs_VERTEX, vertices);
  for (int vertex = 1; vertex <= vertices.Extent(); ++vertex) {
    samples.push_back(BRep_Tool::Pnt(TopoDS::Vertex(vertices(vertex))));
  }
  TopTools_IndexedMapOfShape edges;
  TopExp::MapShapes(face, TopAbs_EDGE, edges);
  for (int edge = 1; edge <= edges.Extent(); ++edge) {
    if (!BRep_Tool::Degenerated(TopoDS::Edge(edges(edge)))) {
      const BRepAdaptor_Curve curve(TopoDS::Edge(edges(edge)));
      samples.push_back(curve.Value(middle_parameter(curve)));
    }
  }
  double u_first = 0.0, u_last = 0.0, v_first = 0.0, v_last = 0.0;
  BRepTools::UVBounds(face, u_first, u_last, v_first, v_last);
  const double u_step = (u_last - u_first) / kWidthGridCells;
  const double v_step = (v_last - v_first) / kWidthGridCells;
  const BRepAdaptor_Surface surface(face, Standard_False);
  BRepClass_FaceClassifier classifier;
  for (int column = 0; column < kWidthGridCells; ++column) {
    const double u_parameter = u_first + (column + 0.5) * u_step;
    for (int row = 0; row < kWidthGridCells; ++row) {
      const double v_parameter = v_first + (row + 0.5) * v_step;
      classifier.Perform(face, gp_Pnt2d(u_parameter, v_parameter), kLinear);
      const TopAbs_State state = classifier.State();
      if (state == TopAbs_IN || state == TopAbs_ON) {
        samples.push_back(surface.Value(u_parameter, v_parameter));
      }
    }
  }
  return samples;
}

}  // namespace

double shape_width_across(const Shape &shape, std::uint32_t face_index, std::uint32_t edge_index) {
  const TopoDS_Face &face = TopoDS::Face(element_at(shape, ElementKind::Face, face_index));
  const TopoDS_Edge &edge = TopoDS::Edge(element_at(shape, ElementKind::Edge, edge_index));
  require_curve(edge, edge_index);
  // The edge is loaded once; each sample is a vertex measured to it (kernel spec section 3.6,
  // item 7.3).
  BRepExtrema_DistShapeShape distance;
  distance.SetFlag(Extrema_ExtFlag_MIN);
  distance.LoadS2(edge);
  double widest = 0.0;
  for (const gp_Pnt &sample : width_samples(face)) {
    distance.LoadS1(BRepBuilderAPI_MakeVertex(sample).Vertex());
    if (!distance.Perform()) {
      raise_not_done("BRepExtrema_DistShapeShape found no distance from a point of face " +
                     std::to_string(face_index) + " to edge " + std::to_string(edge_index));
    }
    widest = std::max(widest, distance.Value());
  }
  return widest;
}

}  // namespace occt
}  // namespace partsmith
