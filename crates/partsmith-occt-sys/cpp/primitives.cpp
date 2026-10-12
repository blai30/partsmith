// Primitive solids in a placement frame (kernel spec section 3.4.1).
#include <BRepPrimAPI_MakeBox.hxx>
#include <BRepPrimAPI_MakeCone.hxx>
#include <BRepPrimAPI_MakeCylinder.hxx>
#include <BRepPrimAPI_MakeSphere.hxx>
#include <BRepPrimAPI_MakeTorus.hxx>

#include "partsmith-occt-sys/cpp/internal.h"

namespace partsmith {
namespace occt {

using namespace detail;

namespace {

// Wraps a primitive's solid, which must exist.
std::shared_ptr<Shape> primitive(const TopoDS_Shape &shape, const char *algorithm) {
  require_solid(shape, algorithm);
  return make_shape(shape);
}

}  // namespace

std::shared_ptr<Shape> make_box(const Frame &frame, double length, double width, double height) {
  BRepPrimAPI_MakeBox maker(to_ax2(frame), length, width, height);
  return primitive(maker.Shape(), "BRepPrimAPI_MakeBox");
}

std::shared_ptr<Shape> make_cylinder(const Frame &frame, double radius, double height) {
  BRepPrimAPI_MakeCylinder maker(to_ax2(frame), radius, height);
  return primitive(maker.Shape(), "BRepPrimAPI_MakeCylinder");
}

std::shared_ptr<Shape> make_sphere(const Frame &frame, double radius) {
  BRepPrimAPI_MakeSphere maker(to_ax2(frame), radius);
  return primitive(maker.Shape(), "BRepPrimAPI_MakeSphere");
}

std::shared_ptr<Shape> make_cone(const Frame &frame, double bottom_radius, double top_radius,
                                 double height) {
  BRepPrimAPI_MakeCone maker(to_ax2(frame), bottom_radius, top_radius, height);
  return primitive(maker.Shape(), "BRepPrimAPI_MakeCone");
}

std::shared_ptr<Shape> make_torus(const Frame &frame, double major_radius, double minor_radius) {
  BRepPrimAPI_MakeTorus maker(to_ax2(frame), major_radius, minor_radius);
  return primitive(maker.Shape(), "BRepPrimAPI_MakeTorus");
}

}  // namespace occt
}  // namespace partsmith
