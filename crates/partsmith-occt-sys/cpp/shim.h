#pragma once

#include <memory>

#include <Standard_Failure.hxx>
#include <TopoDS_Shape.hxx>

#include "rust/cxx.h"

// Translates every C++ exception into a Rust error. cxx picks this up instead of its default,
// which only catches std::exception, while OCCT throws Standard_Failure.
namespace rust {
namespace behavior {
template <typename Try, typename Fail>
static void trycatch(Try &&func, Fail &&fail) noexcept try {
  func();
} catch (const Standard_Failure &failure) {
  std::string message = failure.DynamicType()->Name();
  message += ": ";
  message += failure.GetMessageString();
  fail(message.c_str());
} catch (const std::exception &exception) {
  std::string message = "std::exception: ";
  message += exception.what();
  fail(message.c_str());
} catch (...) {
  fail("unknown: unknown C++ exception");
}
}  // namespace behavior
}  // namespace rust

namespace partsmith {
namespace occt {

// An immutable B-rep shape shared between Rust handles.
class Shape {
 public:
  explicit Shape(TopoDS_Shape shape) : shape_(std::move(shape)) {}
  const TopoDS_Shape &get() const { return shape_; }

 private:
  TopoDS_Shape shape_;
};

std::shared_ptr<Shape> make_box(double dx, double dy, double dz);
double shape_volume(const Shape &shape);

}  // namespace occt
}  // namespace partsmith
