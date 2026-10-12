// Shapes, indexing, measurement, element geometry and topology (kernel spec sections 3.2 and
// 3.6).
#include <algorithm>
#include <cmath>
#include <optional>
#include <utility>
#include <vector>

#include <BRepAdaptor_Curve.hxx>
#include <BRepAdaptor_Surface.hxx>
#include <BRepBndLib.hxx>
#include <BRepCheck_Analyzer.hxx>
#include <BRepGProp.hxx>
#include <BRepLProp_SLProps.hxx>
#include <BRep_Builder.hxx>
#include <BRep_Tool.hxx>
#include <Bnd_Box.hxx>
#include <GProp_GProps.hxx>
#include <Geom2d_Circle.hxx>
#include <Geom2d_Curve.hxx>
#include <Geom2d_Line.hxx>
#include <Geom2d_TrimmedCurve.hxx>
#include <GeomAPI_ProjectPointOnSurf.hxx>
#include <TopExp.hxx>
#include <TopExp_Explorer.hxx>
#include <TopTools_IndexedDataMapOfShapeListOfShape.hxx>
#include <TopoDS.hxx>
#include <TopoDS_Compound.hxx>
#include <gp_Circ.hxx>
#include <gp_Circ2d.hxx>
#include <gp_Cone.hxx>
#include <gp_Cylinder.hxx>
#include <gp_Elips.hxx>
#include <gp_Hypr.hxx>
#include <gp_Lin.hxx>
#include <gp_Parab.hxx>
#include <gp_Pln.hxx>
#include <gp_Sphere.hxx>
#include <gp_Torus.hxx>

#include "partsmith-occt-sys/cpp/internal.h"

namespace partsmith {
namespace occt {

using namespace detail;

namespace {

// The sub-shapes of one type in TopExp::MapShapes order (kernel spec section 3.2, item 1).
TopTools_IndexedMapOfShape index_map(const TopoDS_Shape &shape, TopAbs_ShapeEnum type) {
  TopTools_IndexedMapOfShape map;
  TopExp::MapShapes(shape, type, map);
  return map;
}

}  // namespace

Shape::Shape(TopoDS_Shape shape)
    : shape_(std::move(shape)),
      solids_(index_map(shape_, TopAbs_SOLID)),
      faces_(index_map(shape_, TopAbs_FACE)),
      edges_(index_map(shape_, TopAbs_EDGE)),
      vertices_(index_map(shape_, TopAbs_VERTEX)) {}

const TopTools_IndexedMapOfShape &Shape::map(ElementKind kind) const {
  switch (kind) {
    case ElementKind::Solid:
      return solids_;
    case ElementKind::Face:
      return faces_;
    case ElementKind::Edge:
      return edges_;
    case ElementKind::Vertex:
      return vertices_;
  }
  raise_invalid_input("Unknown element kind.");
}

std::shared_ptr<Shape> empty_shape() {
  TopoDS_Compound compound;
  BRep_Builder().MakeCompound(compound);
  return make_shape(compound);
}

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

namespace {

// The sub-shape `index` of `kind`, checking the index.
const TopoDS_Shape &element_at(const Shape &shape, ElementKind kind, std::uint32_t index) {
  const TopTools_IndexedMapOfShape &map = shape.map(kind);
  if (index == 0 || index > static_cast<std::uint32_t>(map.Extent())) {
    raise_invalid_input("Element " + std::to_string(index) + " does not exist in this body.");
  }
  return map(static_cast<int>(index));
}

// Volume properties summed over the solids.
GProp_GProps solid_properties(const Shape &shape) {
  GProp_GProps total;
  for (int index = 1; index <= shape.solids().Extent(); ++index) {
    GProp_GProps properties;
    BRepGProp::VolumeProperties(shape.solids()(index), properties);
    total.Add(properties);
  }
  return total;
}

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

// The area of a face and its area centroid.
struct FaceMass {
  double area;
  gp_Pnt centroid;
};

// Integrals over a region of a plane's parameter plane (x, y), accumulated along its boundary
// by Green's theorem with the fields (-y / 2, x / 2), (0, x^2 / 2) and (-y^2 / 2, 0): `area`
// integrates 1, `first_x` integrates x and `first_y` integrates y. Every boundary piece uses
// the same fields, so the pieces of all loops add up to the exact integrals.
struct PlaneIntegrals {
  double area = 0.0;
  double first_x = 0.0;
  double first_y = 0.0;

  // Adds the segment from `start` to `end`.
  void add_segment(const gp_XY &start, const gp_XY &end) {
    const double x0 = start.X(), y0 = start.Y(), x1 = end.X(), y1 = end.Y();
    area += 0.5 * (x0 * y1 - x1 * y0);
    first_x += (y1 - y0) * (x0 * x0 + x0 * x1 + x1 * x1) / 6.0;
    first_y -= (x1 - x0) * (y0 * y0 + y0 * y1 + y1 * y1) / 6.0;
  }

  // Adds the arc of the circle with `center` and `radius` from `start` to `end` through
  // `middle`, turning by `angle` (counterclockwise when positive): its chord, plus the loop of
  // the arc and the chord back, which bounds a circular segment with closed-form integrals.
  void add_arc(const gp_XY &start, const gp_XY &end, const gp_XY &middle, const gp_XY &center,
               double radius, double angle) {
    add_segment(start, end);
    const double turn = std::abs(angle);
    const double sign = angle < 0.0 ? -1.0 : 1.0;
    const double segment_area = 0.5 * radius * radius * (turn - std::sin(turn));
    const double half_sine = std::sin(0.5 * turn);
    // The segment's first moment about the center, along the bisector toward `middle`.
    const double offset = 2.0 / 3.0 * radius * radius * radius * half_sine * half_sine * half_sine;
    const gp_XY bisector = (middle - center) / radius;
    area += sign * segment_area;
    first_x += sign * (segment_area * center.X() + offset * bisector.X());
    first_y += sign * (segment_area * center.Y() + offset * bisector.Y());
  }
};

// Integrals over a region of a cylinder's parameter plane (u, v), accumulated along its
// boundary by Green's theorem with the fields (0, u), (0, sin u), (0, -cos u) and
// (-v^2 / 2, 0): `area` integrates 1, `cosine` cos u, `sine` sin u and `height` v.
struct CylinderIntegrals {
  double area = 0.0;
  double cosine = 0.0;
  double sine = 0.0;
  double height = 0.0;

  // Adds the segment from `start` to `end`, both relative to `reference`; `height` stays
  // relative to the reference's v.
  void add_segment(const gp_XY &start, const gp_XY &end, const gp_XY &reference) {
    const double du = end.X() - start.X();
    const double dv = end.Y() - start.Y();
    const double mean_u = reference.X() + 0.5 * (start.X() + end.X());
    const double half = 0.5 * du;
    // sin(half) / half, with its series where the quotient loses precision.
    const double sinc = std::abs(half) < 1e-4 ? 1.0 - half * half / 6.0 : std::sin(half) / half;
    area += dv * 0.5 * (start.X() + end.X());
    cosine += dv * std::sin(mean_u) * sinc;
    sine -= dv * std::cos(mean_u) * sinc;
    const double v0 = start.Y(), v1 = end.Y();
    height -= du * (v0 * v0 + v0 * v1 + v1 * v1) / 6.0;
  }
};

// A parameter-plane curve without its trimming wrappers, which keep the parametrization.
Handle(Geom2d_Curve) basis_curve(Handle(Geom2d_Curve) curve) {
  while (!curve.IsNull() && curve->IsKind(STANDARD_TYPE(Geom2d_TrimmedCurve))) {
    curve = Handle(Geom2d_TrimmedCurve)::DownCast(curve)->BasisCurve();
  }
  return curve;
}

// The exact area and centroid of a face on a plane bounded by lines and circular arcs in its
// parameter plane, or on a cylinder bounded by lines, from the boundary integrals above;
// nothing for any other face. BRepGProp integrates numerically with 18 x 18 points per
// boundary edge, which made element queries of faces with many edges slow (kernel spec
// section 5).
std::optional<FaceMass> analytic_face_mass(const TopoDS_Face &face,
                                           const BRepAdaptor_Surface &surface) {
  const GeomAbs_SurfaceType type = surface.GetType();
  // A scaled location would scale the parameter plane as well.
  if ((type != GeomAbs_Plane && type != GeomAbs_Cylinder) || face.NbChildren() == 0 ||
      std::abs(std::abs(surface.Trsf().ScaleFactor()) - 1.0) > 1e-12) {
    return std::nullopt;
  }
  const bool planar = type == GeomAbs_Plane;
  // The forward face has its material on the left of its oriented boundary.
  const TopoDS_Face forward = TopoDS::Face(face.Oriented(TopAbs_FORWARD));
  bool has_reference = false;
  // Coordinates are taken relative to the first boundary point to keep the sums accurate.
  gp_XY reference(0.0, 0.0);
  PlaneIntegrals plane;
  CylinderIntegrals cylinder;
  for (TopExp_Explorer edges(forward, TopAbs_EDGE); edges.More(); edges.Next()) {
    const TopoDS_Edge &edge = TopoDS::Edge(edges.Current());
    const TopAbs_Orientation orientation = edge.Orientation();
    // Internal and external edges bound nothing.
    if (orientation != TopAbs_FORWARD && orientation != TopAbs_REVERSED) {
      continue;
    }
    double first = 0.0, last = 0.0;
    const Handle(Geom2d_Curve) curve =
        basis_curve(BRep_Tool::CurveOnSurface(edge, forward, first, last));
    if (curve.IsNull()) {
      return std::nullopt;
    }
    const bool reversed = orientation == TopAbs_REVERSED;
    gp_XY start = curve->Value(reversed ? last : first).XY();
    gp_XY end = curve->Value(reversed ? first : last).XY();
    if (!has_reference) {
      has_reference = true;
      reference = start;
    }
    start -= reference;
    end -= reference;
    if (curve->IsKind(STANDARD_TYPE(Geom2d_Line))) {
      if (planar) {
        plane.add_segment(start, end);
      } else {
        cylinder.add_segment(start, end, reference);
      }
    } else if (planar && curve->IsKind(STANDARD_TYPE(Geom2d_Circle))) {
      const gp_Circ2d circle = Handle(Geom2d_Circle)::DownCast(curve)->Circ2d();
      const gp_Ax22d &axes = circle.Position();
      const double sense = axes.XDirection().Crossed(axes.YDirection()) < 0.0 ? -1.0 : 1.0;
      const double angle = (last - first) * sense * (reversed ? -1.0 : 1.0);
      const gp_XY middle = curve->Value(0.5 * (first + last)).XY() - reference;
      plane.add_arc(start, end, middle, circle.Location().XY() - reference, circle.Radius(),
                    angle);
    } else {
      return std::nullopt;
    }
  }
  if (!has_reference) {
    return std::nullopt;
  }
  // The integrals assume the parametrization below; the face's own surface must agree.
  const gp_XYZ surface_point = surface.Value(reference.X(), reference.Y()).XYZ();
  if (planar) {
    const gp_Pln pln = surface.Plane();
    const gp_XYZ origin = pln.Location().XYZ();
    const gp_XYZ x_axis = pln.XAxis().Direction().XYZ();
    const gp_XYZ y_axis = pln.YAxis().Direction().XYZ();
    const gp_XYZ expected = origin + x_axis * reference.X() + y_axis * reference.Y();
    if ((surface_point - expected).Modulus() > kLinear || !(plane.area > 0.0)) {
      return std::nullopt;
    }
    const double x = reference.X() + plane.first_x / plane.area;
    const double y = reference.Y() + plane.first_y / plane.area;
    return FaceMass{plane.area, gp_Pnt(origin + x_axis * x + y_axis * y)};
  }
  const gp_Cylinder cyl = surface.Cylinder();
  const gp_Ax3 &frame = cyl.Position();
  const double radius = cyl.Radius();
  const gp_XYZ origin = frame.Location().XYZ();
  const gp_XYZ x_axis = frame.XDirection().XYZ();
  const gp_XYZ y_axis = frame.YDirection().XYZ();
  const gp_XYZ z_axis = frame.Direction().XYZ();
  const gp_XYZ expected = origin + x_axis * (radius * std::cos(reference.X())) +
                          y_axis * (radius * std::sin(reference.X())) + z_axis * reference.Y();
  if ((surface_point - expected).Modulus() > kLinear || !(cylinder.area > 0.0)) {
    return std::nullopt;
  }
  // The area element is radius du dv, and a point is origin + radius (cos u x + sin u y) + v z.
  const gp_XYZ moment = x_axis * (radius * cylinder.cosine) + y_axis * (radius * cylinder.sine) +
                        z_axis * cylinder.height;
  return FaceMass{radius * cylinder.area,
                  gp_Pnt(origin + z_axis * reference.Y() + moment / cylinder.area)};
}

// The area and area centroid of a face: exact where `analytic_face_mass` applies, else from
// BRepGProp (kernel spec section 3.6, items 2 and 3).
FaceMass face_mass(const TopoDS_Face &face, const BRepAdaptor_Surface &surface) {
  if (const std::optional<FaceMass> exact = analytic_face_mass(face, surface)) {
    return *exact;
  }
  GProp_GProps properties;
  BRepGProp::SurfaceProperties(face, properties);
  return FaceMass{properties.Mass(), properties.CentreOfMass()};
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

// Appends one list made of `indices`, distinct and ascending, and clears `indices` for the next
// list.
void append_list(IndexLists &lists, std::vector<std::uint32_t> &indices) {
  std::sort(indices.begin(), indices.end());
  indices.erase(std::unique(indices.begin(), indices.end()), indices.end());
  for (std::uint32_t index : indices) {
    lists.values.push_back(index);
  }
  lists.offsets.push_back(static_cast<std::uint32_t>(lists.values.size()));
  indices.clear();
}

// Adds the index in `map` of `element`, if it is there.
void add_index(std::vector<std::uint32_t> &indices, const TopTools_IndexedMapOfShape &map,
               const TopoDS_Shape &element) {
  const int index = map.FindIndex(element);
  if (index > 0) {
    indices.push_back(static_cast<std::uint32_t>(index));
  }
}

// Appends the list of the indices in `map` of the sub-shapes of `shape` of one type.
void append_sub_shapes(IndexLists &lists, std::vector<std::uint32_t> &indices,
                       const TopTools_IndexedMapOfShape &map, const TopoDS_Shape &shape,
                       TopAbs_ShapeEnum type) {
  for (TopExp_Explorer explorer(shape, type); explorer.More(); explorer.Next()) {
    add_index(indices, map, explorer.Current());
  }
  append_list(lists, indices);
}

// Appends the list of the indices in `map` of the ancestors of `element` found in `ancestors`.
void append_ancestors(IndexLists &lists, std::vector<std::uint32_t> &indices,
                      const TopTools_IndexedMapOfShape &map,
                      const TopTools_IndexedDataMapOfShapeListOfShape &ancestors,
                      const TopoDS_Shape &element) {
  if (const TopTools_ListOfShape *list = ancestors.Seek(element)) {
    for (const TopoDS_Shape &ancestor : *list) {
      add_index(indices, map, ancestor);
    }
  }
  append_list(lists, indices);
}

// Empty compressed lists.
IndexLists empty_lists() {
  IndexLists lists;
  lists.offsets.push_back(0);
  return lists;
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

double shape_volume(const Shape &shape) { return solid_properties(shape).Mass(); }

double shape_area(const Shape &shape) {
  double area = 0.0;
  for (int index = 1; index <= shape.faces().Extent(); ++index) {
    const TopoDS_Face &face = TopoDS::Face(shape.faces()(index));
    area += face_mass(face, BRepAdaptor_Surface(face, Standard_False)).area;
  }
  return area;
}

Vec3 shape_center_of_mass(const Shape &shape) {
  const GProp_GProps properties = solid_properties(shape);
  if (std::abs(properties.Mass()) < 1e-12) {
    raise_invalid_input("This shape has no volume, so it has no center of mass.");
  }
  return to_vec3(properties.CentreOfMass().XYZ());
}

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
  const TopoDS_Shape &whole = shape.get();
  TopTools_IndexedDataMapOfShapeListOfShape edge_faces;
  TopTools_IndexedDataMapOfShapeListOfShape vertex_edges;
  TopTools_IndexedDataMapOfShapeListOfShape vertex_faces;
  TopExp::MapShapesAndUniqueAncestors(whole, TopAbs_EDGE, TopAbs_FACE, edge_faces);
  TopExp::MapShapesAndUniqueAncestors(whole, TopAbs_VERTEX, TopAbs_EDGE, vertex_edges);
  TopExp::MapShapesAndUniqueAncestors(whole, TopAbs_VERTEX, TopAbs_FACE, vertex_faces);

  TopologyData data{empty_lists(), empty_lists(), empty_lists(), empty_lists(), {}, {},
                    empty_lists(), empty_lists()};
  // One buffer collects the indices of every list in turn.
  std::vector<std::uint32_t> indices;
  for (int index = 1; index <= shape.faces().Extent(); ++index) {
    const TopoDS_Shape &face = shape.faces()(index);
    append_sub_shapes(data.face_edges, indices, shape.edges(), face, TopAbs_EDGE);
    append_sub_shapes(data.face_vertices, indices, shape.vertices(), face, TopAbs_VERTEX);
  }
  for (int index = 1; index <= shape.edges().Extent(); ++index) {
    const TopoDS_Edge &edge = TopoDS::Edge(shape.edges()(index));
    append_ancestors(data.edge_faces, indices, shape.faces(), edge_faces, edge);
    append_sub_shapes(data.edge_vertices, indices, shape.vertices(), edge, TopAbs_VERTEX);
    data.edge_degenerate.push_back(BRep_Tool::Degenerated(edge));
    bool seam = false;
    if (const TopTools_ListOfShape *faces = edge_faces.Seek(edge)) {
      for (const TopoDS_Shape &face : *faces) {
        seam = seam || BRep_Tool::IsClosed(edge, TopoDS::Face(face));
      }
    }
    data.edge_seam.push_back(seam);
  }
  for (int index = 1; index <= shape.vertices().Extent(); ++index) {
    const TopoDS_Shape &vertex = shape.vertices()(index);
    append_ancestors(data.vertex_edges, indices, shape.edges(), vertex_edges, vertex);
    append_ancestors(data.vertex_faces, indices, shape.faces(), vertex_faces, vertex);
  }
  return data;
}

namespace {

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
    double u = 0.0, v = 0.0;
    projection.LowerDistanceParameters(u, v);
    parameters.SetCoord(u, v);
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
  const double middle = 0.5 * (curve.FirstParameter() + curve.LastParameter());
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
  EdgeNormalsData data{to_vec3(point.XYZ()), to_vec3(tangent.XYZ()), {}};
  // Faces in index order give the ascending order of the topology's face lists.
  for (int face_index = 1; face_index <= shape.faces().Extent(); ++face_index) {
    const TopoDS_Face &face = TopoDS::Face(shape.faces()(face_index));
    if (bounds_face(face, edge)) {
      const gp_Dir normal = normal_at_edge(face, edge, middle, point, edge_index, face_index);
      data.faces.push_back(
          FaceNormalData{static_cast<std::uint32_t>(face_index), to_vec3(normal.XYZ())});
    }
  }
  return data;
}

}  // namespace occt
}  // namespace partsmith
