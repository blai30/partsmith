// The C++ shim over OCCT 7.9.3 behind the cxx bridge in src/ffi.rs (kernel spec section 3.9).
#pragma once

#include <cstdint>
#include <memory>
#include <string>
#include <string_view>

#include <Standard_DefineException.hxx>
#include <Standard_Failure.hxx>
#include <Standard_Version.hxx>
#include <TopTools_IndexedMapOfShape.hxx>
#include <TopoDS_Shape.hxx>

#include "rust/cxx.h"

static_assert(std::string_view(OCC_VERSION_COMPLETE) == "7.9.3",
              "partsmith-occt-sys requires OCCT 7.9.3 (ADR 0003)");

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

// A geometry-dependent argument check failed; the message is the user-facing sentence.
DEFINE_STANDARD_HANDLE(Partsmith_InvalidInput, Standard_Failure)
DEFINE_STANDARD_EXCEPTION(Partsmith_InvalidInput, Standard_Failure)
// The result stayed invalid after repair.
DEFINE_STANDARD_HANDLE(Partsmith_InvalidResult, Standard_Failure)
DEFINE_STANDARD_EXCEPTION(Partsmith_InvalidResult, Standard_Failure)
// The result has no solid where one is required.
DEFINE_STANDARD_HANDLE(Partsmith_EmptyResult, Standard_Failure)
DEFINE_STANDARD_EXCEPTION(Partsmith_EmptyResult, Standard_Failure)
// Input bytes could not be parsed.
DEFINE_STANDARD_HANDLE(Partsmith_ReadError, Standard_Failure)
DEFINE_STANDARD_EXCEPTION(Partsmith_ReadError, Standard_Failure)

namespace partsmith {
namespace occt {

// Shared types defined by the generated bridge header.
struct Vec3;
struct Frame;
struct ShapeHandle;
struct OpResultData;
struct ElementData;
struct TopologyData;
struct EdgeNormalsData;
struct BoundsData;
struct ProfileCurveData;
struct ProfileData;
struct SweepData;
struct FilletEdgeData;
struct ChamferEdgeData;
struct SplitData;
struct MeshData;
struct StepBodyData;
struct StepImportData;
enum class ElementKind : std::uint8_t;
enum class ContinuityType : std::uint8_t;
enum class ShapeType : std::uint8_t;

// An immutable B-rep shape with its solid, shell, face, edge and vertex index maps, built once.
class Shape {
 public:
  explicit Shape(TopoDS_Shape shape);
  const TopoDS_Shape &get() const { return shape_; }
  // The index map of sub-shapes of one kind (TopExp::MapShapes order, 1-based).
  const TopTools_IndexedMapOfShape &map(ElementKind kind) const;
  const TopTools_IndexedMapOfShape &solids() const { return solids_; }
  const TopTools_IndexedMapOfShape &shells() const { return shells_; }
  const TopTools_IndexedMapOfShape &faces() const { return faces_; }
  const TopTools_IndexedMapOfShape &edges() const { return edges_; }
  const TopTools_IndexedMapOfShape &vertices() const { return vertices_; }

 private:
  const TopoDS_Shape shape_;
  const TopTools_IndexedMapOfShape solids_;
  const TopTools_IndexedMapOfShape shells_;
  const TopTools_IndexedMapOfShape faces_;
  const TopTools_IndexedMapOfShape edges_;
  const TopTools_IndexedMapOfShape vertices_;
};

// queries.cpp
std::shared_ptr<Shape> empty_shape();
ShapeType shape_type(const Shape &shape);
std::uint32_t shape_count(const Shape &shape, ElementKind kind);
std::shared_ptr<Shape> shape_solid(const Shape &shape, std::uint32_t index);
rust::Vec<std::uint32_t> shape_locate(const Shape &shape, const Shape &part, ElementKind kind);
bool shape_is_valid(const Shape &shape);
BoundsData shape_bounding_box(const Shape &shape);
ElementData shape_element(const Shape &shape, ElementKind kind, std::uint32_t index);
rust::Vec<ElementData> shape_elements(const Shape &shape, ElementKind kind);
TopologyData shape_topology(const Shape &shape);
EdgeNormalsData shape_edge_normals(const Shape &shape, std::uint32_t edge);
double shape_width_across(const Shape &shape, std::uint32_t face, std::uint32_t edge);

// measure.cpp
double shape_volume(const Shape &shape);
double shape_area(const Shape &shape);
Vec3 shape_center_of_mass(const Shape &shape);

// mesh.cpp
MeshData shape_tessellate(const Shape &shape, double chord_deviation, double angle);
rust::Vec<double> shape_element_display(const Shape &shape, ElementKind kind, std::uint32_t index,
                                        double chord_deviation, double angle);

// exchange.cpp
std::unique_ptr<std::string> shape_to_brep(const Shape &shape);
std::shared_ptr<Shape> shape_from_brep(rust::Slice<const std::uint8_t> bytes);
std::unique_ptr<std::string> write_step(rust::Slice<const StepBodyData> bodies);
StepImportData read_step(rust::Slice<const std::uint8_t> bytes);

// primitives.cpp
std::shared_ptr<Shape> make_box(const Frame &frame, double length, double width, double height);
std::shared_ptr<Shape> make_cylinder(const Frame &frame, double radius, double height);
std::shared_ptr<Shape> make_sphere(const Frame &frame, double radius);
std::shared_ptr<Shape> make_cone(const Frame &frame, double bottom_radius, double top_radius,
                                 double height);
std::shared_ptr<Shape> make_torus(const Frame &frame, double major_radius, double minor_radius);

// profiles.cpp
ProfileData make_profile(const Frame &frame, rust::Slice<const ProfileCurveData> curves);

// sweeps.cpp
SweepData extrude(const Shape &profile, const Vec3 &direction, double start, double end);
SweepData revolve(const Shape &profile, const Vec3 &origin, const Vec3 &direction, double angle,
                  bool full);

// booleans.cpp
OpResultData fuse(const Shape &target, rust::Slice<const ShapeHandle> tools);
OpResultData cut(const Shape &target, rust::Slice<const ShapeHandle> tools);
OpResultData common(const Shape &target, rust::Slice<const ShapeHandle> tools);

// blends.cpp
OpResultData fillet(const Shape &shape, rust::Slice<const FilletEdgeData> edges);
OpResultData chamfer(const Shape &shape, rust::Slice<const ChamferEdgeData> edges);
OpResultData shell(const Shape &shape, rust::Slice<const std::uint32_t> remove_faces,
                   double thickness);

// transforms.cpp
OpResultData transform(const Shape &shape, rust::Slice<const double> matrix);
SplitData split(const Shape &shape, const Frame &plane);
std::shared_ptr<Shape> section(const Shape &shape, const Frame &plane);
OpResultData compound(rust::Slice<const ShapeHandle> shapes);

}  // namespace occt
}  // namespace partsmith
