// The shim's Shape, value conversions, exceptions, compounds and copies (kernel spec sections
// 3.1 and 3.2).
#include <utility>

#include <BRepBuilderAPI_Copy.hxx>
#include <BRep_Builder.hxx>
#include <BRep_TFace.hxx>
#include <ShapeUpgrade_RemoveLocations.hxx>
#include <StdFail_NotDone.hxx>
#include <Standard_NullObject.hxx>
#include <TopExp.hxx>
#include <TopExp_Explorer.hxx>
#include <TopoDS_Iterator.hxx>

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
      shells_(index_map(shape_, TopAbs_SHELL)),
      faces_(index_map(shape_, TopAbs_FACE)),
      edges_(index_map(shape_, TopAbs_EDGE)),
      vertices_(index_map(shape_, TopAbs_VERTEX)) {}

const TopTools_IndexedMapOfShape &Shape::map(ElementKind kind) const {
  switch (kind) {
    case ElementKind::Solid:
      return solids_;
    case ElementKind::Shell:
      return shells_;
    case ElementKind::Face:
      return faces_;
    case ElementKind::Edge:
      return edges_;
    case ElementKind::Vertex:
      return vertices_;
  }
  raise_invalid_input("Unknown element kind.");
}

namespace detail {

gp_Pnt to_point(const Vec3 &point) { return gp_Pnt(point.x, point.y, point.z); }

gp_Dir to_direction(const Vec3 &direction) {
  return gp_Dir(direction.x, direction.y, direction.z);
}

gp_Ax2 to_ax2(const Frame &frame) {
  return gp_Ax2(to_point(frame.origin), to_direction(frame.normal), to_direction(frame.x_axis));
}

gp_Ax3 to_ax3(const Frame &frame) { return gp_Ax3(to_ax2(frame)); }

Vec3 to_vec3(const gp_XYZ &coordinates) {
  return Vec3{coordinates.X(), coordinates.Y(), coordinates.Z()};
}

TopAbs_ShapeEnum shape_enum(ElementKind kind) {
  switch (kind) {
    case ElementKind::Solid:
      return TopAbs_SOLID;
    case ElementKind::Shell:
      return TopAbs_SHELL;
    case ElementKind::Face:
      return TopAbs_FACE;
    case ElementKind::Edge:
      return TopAbs_EDGE;
    case ElementKind::Vertex:
      return TopAbs_VERTEX;
  }
  raise_invalid_input("Unknown element kind.");
}

std::shared_ptr<Shape> make_shape(const TopoDS_Shape &shape) {
  // Freeze the root like every sub-shape inside a container already is. TopoDS_Builder::Add
  // freezes each shape it adds, so a later compound holding this shape cannot change it (or
  // its BRep bytes); only roots no other Shape holds are still free here.
  TopoDS_Shape root = shape;
  if (root.Free()) {
    root.Free(Standard_False);
  }
  return std::make_shared<Shape>(root);
}

std::vector<TopoDS_Shape> shapes_of(rust::Slice<const ShapeHandle> handles) {
  std::vector<TopoDS_Shape> shapes;
  shapes.reserve(handles.size());
  for (const ShapeHandle &handle : handles) {
    if (!handle.shape) {
      Standard_NullObject::Raise("a shape handle is null");
    }
    shapes.push_back(handle.shape->get());
  }
  return shapes;
}

void raise_invalid_input(const std::string &message) {
  throw Partsmith_InvalidInput(message.c_str());
}

void raise_invalid_result(const std::string &message) {
  throw Partsmith_InvalidResult(message.c_str());
}

void raise_empty_result(const std::string &message) {
  throw Partsmith_EmptyResult(message.c_str());
}

void raise_read_error(const std::string &message) { throw Partsmith_ReadError(message.c_str()); }

void raise_not_done(const std::string &message) { throw StdFail_NotDone(message.c_str()); }

void raise_not_an_element() {
  raise_invalid_input("Only faces, edges and vertices can be queried.");
}

bool has_solid(const TopoDS_Shape &shape) { return TopExp_Explorer(shape, TopAbs_SOLID).More(); }

void require_solid(const TopoDS_Shape &shape, const char *algorithm) {
  if (!has_solid(shape)) {
    raise_empty_result(std::string(algorithm) + " produced no solid");
  }
}

TopoDS_Compound compound_of(const std::vector<TopoDS_Shape> &shapes) {
  BRep_Builder builder;
  TopoDS_Compound compound;
  builder.MakeCompound(compound);
  for (const TopoDS_Shape &shape : shapes) {
    builder.Add(compound, shape);
  }
  return compound;
}

TopoDS_Shape single_or_compound(const std::vector<TopoDS_Shape> &shapes) {
  if (shapes.size() == 1) {
    return shapes.front();
  }
  return compound_of(shapes);
}

TopoDS_Shape solids_only(const TopoDS_Shape &shape) {
  const TopTools_IndexedMapOfShape solids = index_map(shape, TopAbs_SOLID);
  return single_or_compound(std::vector<TopoDS_Shape>(solids.cbegin(), solids.cend()));
}

namespace {

// Raises StdFail_NotDone unless `copy` has as many elements of each kind as `shape`.
void require_same_counts(const Shape &shape, const Shape &copy, const char *algorithm) {
  for (ElementKind kind : {ElementKind::Solid, ElementKind::Shell, ElementKind::Face,
                           ElementKind::Edge, ElementKind::Vertex}) {
    if (copy.map(kind).Extent() != shape.map(kind).Extent()) {
      raise_not_done(std::string(algorithm) + " changed the element counts");
    }
  }
}

// Copies a shape with BRepBuilderAPI_Copy, without triangulation, and its surfaces and curves
// too when `copy_geometry` is set.
Shape copy_with(const Shape &shape, bool copy_geometry) {
  BRepBuilderAPI_Copy copier(shape.get(), copy_geometry, Standard_False);
  Shape copy(copier.Shape());
  require_same_counts(shape, copy, "BRepBuilderAPI_Copy");
  return copy;
}

// True when `shape` or one of its sub-shapes has a location, or a face of it a located surface.
bool has_location(const TopoDS_Shape &shape) {
  if (!shape.Location().IsIdentity()) {
    return true;
  }
  if (const Handle(BRep_TFace) face = Handle(BRep_TFace)::DownCast(shape.TShape());
      !face.IsNull() && !face->Location().IsIdentity()) {
    return true;
  }
  for (TopoDS_Iterator child(shape, Standard_False, Standard_False); child.More(); child.Next()) {
    if (has_location(child.Value())) {
      return true;
    }
  }
  return false;
}

}  // namespace

Shape copy_shape(const Shape &shape) { return copy_with(shape, true); }

Shape copy_elements(const Shape &shape) { return copy_with(shape, false); }

Shape copy_unlocated(const Shape &shape) {
  Shape copy = copy_shape(shape);
  if (!has_location(copy.get())) {
    return copy;
  }
  // Rebuilds every located element with its location applied to its geometry.
  ShapeUpgrade_RemoveLocations remover;
  remover.Remove(copy.get());
  Shape unlocated(remover.GetResult());
  require_same_counts(shape, unlocated, "ShapeUpgrade_RemoveLocations");
  return unlocated;
}

}  // namespace detail
}  // namespace occt
}  // namespace partsmith
