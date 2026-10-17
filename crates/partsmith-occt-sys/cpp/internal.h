// Helpers shared by the shim's source files; not part of the bridge.
#pragma once

#include <memory>
#include <optional>
#include <sstream>
#include <string>
#include <utility>
#include <vector>

#include <BOPAlgo_Operation.hxx>
#include <BRepAdaptor_Surface.hxx>
#include <BRepTools_History.hxx>
#include <TopAbs_ShapeEnum.hxx>
#include <TopLoc_Location.hxx>
#include <TopTools_ListOfShape.hxx>
#include <TopTools_MapOfShape.hxx>
#include <TopoDS_Compound.hxx>
#include <TopoDS_Edge.hxx>
#include <TopoDS_Face.hxx>
#include <TopoDS_Shape.hxx>
#include <gp_Ax2.hxx>
#include <gp_Ax3.hxx>
#include <gp_Dir.hxx>
#include <gp_Pnt.hxx>

#include "partsmith-occt-sys/src/ffi.rs.h"

namespace partsmith {
namespace occt {
namespace detail {

// Modeling tolerance in millimeters (kernel spec section 3.1).
constexpr double kLinear = 1e-6;

// shape.cpp: the shim's Shape, conversions, exceptions, compounds and copies.

// Value conversions between bridge types and OCCT types.
gp_Pnt to_point(const Vec3 &point);
gp_Dir to_direction(const Vec3 &direction);
gp_Ax2 to_ax2(const Frame &frame);
gp_Ax3 to_ax3(const Frame &frame);
Vec3 to_vec3(const gp_XYZ &coordinates);

// The OCCT shape type of an element kind.
TopAbs_ShapeEnum shape_enum(ElementKind kind);

// Wraps a shape for the facade, freezing its root (see shape.cpp).
std::shared_ptr<Shape> make_shape(const TopoDS_Shape &shape);

// The shapes of a bridge list, in order.
std::vector<TopoDS_Shape> shapes_of(rust::Slice<const ShapeHandle> handles);

// Throws the shim's exception types with a message.
[[noreturn]] void raise_invalid_input(const std::string &message);
[[noreturn]] void raise_invalid_result(const std::string &message);
[[noreturn]] void raise_empty_result(const std::string &message);
[[noreturn]] void raise_read_error(const std::string &message);
// Throws StdFail_NotDone for an algorithm that reported failure without throwing.
[[noreturn]] void raise_not_done(const std::string &message);

// True when the shape contains a solid.
bool has_solid(const TopoDS_Shape &shape);

// Throws Partsmith_EmptyResult unless the shape contains a solid.
void require_solid(const TopoDS_Shape &shape, const char *algorithm);

// A compound of the shapes, in order.
TopoDS_Compound compound_of(const std::vector<TopoDS_Shape> &shapes);

// The single shape, or a compound of all the shapes (possibly empty).
TopoDS_Shape single_or_compound(const std::vector<TopoDS_Shape> &shapes);

// The shape's solids alone: the single solid, or a compound of all solids (possibly empty).
TopoDS_Shape solids_only(const TopoDS_Shape &shape);

// Copies a shape (geometry included, no triangulation) for algorithms that may write to their
// input. The copy has the same structure, so sub-shape `i` of the copy is the copy of sub-shape
// `i` of the input and the copy's history is the input's (kernel spec section 3.1).
Shape copy_shape(const Shape &shape);

// Copies a shape's solids, shells, faces, edges and vertices, sharing its surfaces and curves,
// for algorithms that write only to sub-shapes (curves on faces, ranges, tolerances). Same
// structure and indices as `copy_shape`.
Shape copy_elements(const Shape &shape);

// Copies a shape like `copy_shape`, with the location of every located element applied to its
// copied geometry instead of kept, for algorithms that fail on located geometry (kernel spec
// section 3.4.8). Same structure and indices as `copy_shape`.
Shape copy_unlocated(const Shape &shape);

// Header-only helpers for OCCT algorithms.

// Raises StdFail_NotDone with the alerts of a Boolean-family algorithm that reported errors.
template <class Algorithm>
void require_no_errors(const Algorithm &algorithm, const char *name) {
  if (algorithm.HasErrors()) {
    std::ostringstream alerts;
    algorithm.DumpErrors(alerts);
    raise_not_done(std::string(name) + " failed: " + alerts.str());
  }
}

// Sets the options every Boolean-family algorithm of the shim runs with (kernel spec sections
// 3.1 item 3 and 3.10 item 3): non-destructive, sequential and with history.
template <class Algorithm>
void set_boolean_options(Algorithm &algorithm) {
  algorithm.SetNonDestructive(Standard_True);
  algorithm.SetRunParallel(Standard_False);
  algorithm.SetToFillHistory(Standard_True);
}

// The history of an algorithm that ran on the single shape `argument`.
template <class Algorithm>
Handle(BRepTools_History) history_of(const TopoDS_Shape &argument, Algorithm &algorithm) {
  TopTools_ListOfShape arguments;
  arguments.Append(argument);
  return new BRepTools_History(arguments, algorithm);
}

// validity.cpp: the state of an operation's inputs, the validity check and repair.

// The state of one face, with its edges and vertices as the face sees them, that an algorithm
// can change in place: the addresses of geometry and curve representations, tolerances, flags,
// parameter ranges and points.
struct ElementState {
  std::vector<const void *> parts;
  std::vector<double> values;
  std::vector<TopLoc_Location> locations;

  bool operator==(const ElementState &other) const;
};

// The shapes an operation runs on, as the validity check of kernel spec section 3.5 sees them.
// Faces of the inputs that the result keeps are trusted unless the operation changed them in
// place: some OCCT algorithms write to the sub-shapes their result shares with their arguments
// (unification adds and removes curves on faces), so the state of every face, edge and vertex
// is recorded when the inputs are given, before the operation runs.
class CheckInputs {
 public:
  // No inputs: the whole result is checked.
  CheckInputs() = default;
  explicit CheckInputs(std::vector<TopoDS_Shape> shapes);

  const std::vector<TopoDS_Shape> &shapes() const { return shapes_; }

  // Fixes the faces the operation changed in place, before the shim writes to the result itself
  // (curves stored on planes), which then does not count as a change.
  void settle();

  // The faces of the inputs whose state, or the state of one of their edges or vertices, differs
  // from the recorded one: the faces fixed by `settle`, or else compared now.
  TopTools_MapOfShape altered_faces() const;

 private:
  // The faces whose state differs from the recorded one now.
  TopTools_MapOfShape compare() const;

  std::vector<TopoDS_Shape> shapes_;
  std::optional<TopTools_MapOfShape> settled_;
  // Each face of the inputs (one occurrence of each) with its recorded state.
  std::vector<std::pair<TopoDS_Shape, ElementState>> states_;
};

// Stores the curve of every edge on every planar face of `shape`, which OCCT otherwise projects
// again on each use; checks and queries of the shape get much cheaper. Only for shapes the
// operation built itself: it writes to the edges.
void store_plane_pcurves(const TopoDS_Shape &shape);

// Fixes the input faces the operation changed in place (`CheckInputs::settle`), then stores the
// curves on planes of `result` (`store_plane_pcurves`), which then do not count as changes. Only
// for results that share edges with nothing but the copies the operation ran on.
void store_result_pcurves(CheckInputs &inputs, const TopoDS_Shape &result);

// True when the result passes the checks of kernel spec section 3.5 relative to `inputs`, the
// shapes the operation ran on: the checks of BRepCheck_Analyzer limited to the faces that are
// not faces of an input or were changed in place, their wires, edges and vertices, and the
// shells and solids that are not an input's; the whole result when none of its faces is an
// unchanged face of an input.
bool changed_part_valid(const TopoDS_Shape &shape, const CheckInputs &inputs);

// Checks what the operation produced or changed in its result relative to `inputs`, the shapes
// it ran on, with the checks of BRepCheck_Analyzer (the whole result when no unchanged face of
// an input is in it). An invalid result is repaired with ShapeFix_Shape (on a copy, so shared
// inputs stay untouched), the repair is merged into the history and the whole repaired result
// is checked; a result still invalid raises Partsmith_InvalidResult (kernel spec section 3.5).
void ensure_valid(TopoDS_Shape &shape, const Handle(BRepTools_History) &history,
                  const CheckInputs &inputs, const char *algorithm);

// Normalizes the result of an operation that must produce a solid: keeps its solids, checks
// and repairs it relative to `inputs` (see ensure_valid), and raises Partsmith_EmptyResult
// when no solid is left.
TopoDS_Shape finish_solid(const TopoDS_Shape &shape, const Handle(BRepTools_History) &history,
                          const CheckInputs &inputs, const char *algorithm);

// history.cpp: the history of every input relative to the result.

// Appends the next step of an operation to its history.
void merge_history(const Handle(BRepTools_History) &history,
                   const Handle(BRepTools_History) &next);

// The history of every input relative to the result (kernel spec section 3.3). `history` may
// be null when the result contains the inputs' sub-shapes unchanged.
rust::Vec<InputHistoryData> history_data(const Handle(BRepTools_History) &history,
                                         const std::vector<TopoDS_Shape> &inputs,
                                         const Shape &result);

// The result shape and the history of every input.
OpResultData op_result(const TopoDS_Shape &result, const Handle(BRepTools_History) &history,
                       const std::vector<TopoDS_Shape> &inputs);

// queries.cpp

// The sub-shape `index` of `kind`, checking the index.
const TopoDS_Shape &element_at(const Shape &shape, ElementKind kind, std::uint32_t index);

// Raises Partsmith_InvalidInput for a degenerated edge, which has no curve to measure or draw
// (kernel spec sections 3.6 item 7 and 3.7 item 9); `index` names it in the message.
void require_curve(const TopoDS_Edge &edge, std::uint32_t index);

// measure.cpp

// The area of a face and its area centroid.
struct FaceMass {
  double area;
  gp_Pnt centroid;
};

// The area and area centroid of a face (kernel spec section 3.6, items 2 and 3): exact from its
// boundary for planes bounded by lines and circular arcs and cylinders bounded by lines, from
// BRepGProp for other faces. `surface` is the face's surface, without restriction to the face.
FaceMass face_mass(const TopoDS_Face &face, const BRepAdaptor_Surface &surface);

// The volume of the solids of `shape` (kernel spec section 3.6, item 1), each solid once.
double solids_volume(const TopoDS_Shape &shape);

// booleans.cpp

// Runs one boolean of `target` with `tools` with the settings of kernel spec section 3.4.5
// (non-destructive, sequential, history filled), unifying coplanar faces and collinear edges
// when `unify` is set, and merges its history into `history`. Raises StdFail_NotDone with the
// algorithm's alerts when it fails.
TopoDS_Shape run_boolean(BOPAlgo_Operation operation, const TopoDS_Shape &target,
                         const TopTools_ListOfShape &tools, bool unify,
                         const Handle(BRepTools_History) &history);

// exchange.cpp

// Runs the one-time initialization of OCCT's process-wide state: removes the default message
// printers so OCCT writes nothing to standard output (kernel spec section 3.10).
void initialize();

}  // namespace detail
}  // namespace occt
}  // namespace partsmith
