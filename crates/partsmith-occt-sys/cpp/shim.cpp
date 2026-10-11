#include "partsmith-occt-sys/cpp/shim.h"

#include <BRepGProp.hxx>
#include <BRepPrimAPI_MakeBox.hxx>
#include <GProp_GProps.hxx>

namespace partsmith {
namespace occt {

std::shared_ptr<Shape> make_box(double dx, double dy, double dz) {
  BRepPrimAPI_MakeBox maker(dx, dy, dz);
  return std::make_shared<Shape>(maker.Shape());
}

double shape_volume(const Shape &shape) {
  GProp_GProps properties;
  BRepGProp::VolumeProperties(shape.get(), properties);
  return properties.Mass();
}

}  // namespace occt
}  // namespace partsmith
