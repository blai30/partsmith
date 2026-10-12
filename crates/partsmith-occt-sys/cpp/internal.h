// Helpers shared by the shim's source files; not part of the bridge.
#pragma once

#include <memory>
#include <string>
#include <vector>

#include <BOPAlgo_Operation.hxx>
#include <BRepTools_History.hxx>
#include <TopAbs_ShapeEnum.hxx>
#include <TopTools_ListOfShape.hxx>
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

// Value conversions between bridge types and OCCT types.
gp_Pnt to_point(const Vec3 &point);
gp_Dir to_direction(const Vec3 &direction);
gp_Ax2 to_ax2(const Frame &frame);
gp_Ax3 to_ax3(const Frame &frame);
Vec3 to_vec3(const gp_XYZ &coordinates);

// The OCCT shape type of an element kind.
TopAbs_ShapeEnum shape_enum(ElementKind kind);

// Wraps a shape for the facade, freezing its root (see history.cpp).
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

// Throws Partsmith_EmptyResult unless the shape contains a solid.
void require_solid(const TopoDS_Shape &shape, const char *algorithm);

// The shape's solids alone: the single solid, or a compound of all solids (possibly empty).
TopoDS_Shape solids_only(const TopoDS_Shape &shape);

// Copies a shape (geometry included, no triangulation) for algorithms that may write to their
// input. The copy has the same structure, so sub-shape `i` of the copy is the copy of sub-shape
// `i` of the input and the copy's history is the input's (kernel spec section 3.1).
Shape copy_shape(const Shape &shape);

// Stores the curve of every edge on every planar face of `shape`, which OCCT otherwise projects
// again on each use; checks and queries of the shape get much cheaper. Only for shapes the
// operation built itself: it writes to the edges.
void store_plane_pcurves(const TopoDS_Shape &shape);

// Runs one boolean of `target` with `tools` with the settings of kernel spec section 3.4.5
// (non-destructive, sequential, history filled), unifying coplanar faces and collinear edges
// when `unify` is set, and merges its history into `history`. Raises StdFail_NotDone with the
// algorithm's alerts when it fails.
TopoDS_Shape run_boolean(BOPAlgo_Operation operation, const TopoDS_Shape &target,
                         const TopTools_ListOfShape &tools, bool unify,
                         const Handle(BRepTools_History) &history);

// Appends the next step of an operation to its history.
void merge_history(const Handle(BRepTools_History) &history,
                   const Handle(BRepTools_History) &next);

// Checks what the operation produced or changed in its result relative to `inputs`, the shapes
// it ran on, with the checks of BRepCheck_Analyzer (the whole result when no face of an input
// is in it). An invalid result is repaired with ShapeFix_Shape (on a copy, so shared inputs
// stay untouched), the repair is merged into the history and the whole repaired result is
// checked; a result still invalid raises Partsmith_InvalidResult (kernel spec section 3.5).
void ensure_valid(TopoDS_Shape &shape, const Handle(BRepTools_History) &history,
                  const std::vector<TopoDS_Shape> &inputs, const char *algorithm);

// Normalizes the result of an operation that must produce a solid: keeps its solids, checks
// and repairs it relative to `inputs` (see ensure_valid), and raises Partsmith_EmptyResult
// when no solid is left.
TopoDS_Shape finish_solid(const TopoDS_Shape &shape, const Handle(BRepTools_History) &history,
                          const std::vector<TopoDS_Shape> &inputs, const char *algorithm);

// The history of every input relative to the result (kernel spec section 3.3). `history` may
// be null when the result contains the inputs' sub-shapes unchanged.
rust::Vec<InputHistoryData> history_data(const Handle(BRepTools_History) &history,
                                         const std::vector<TopoDS_Shape> &inputs,
                                         const Shape &result);

// The result shape and the history of every input.
OpResultData op_result(const TopoDS_Shape &result, const Handle(BRepTools_History) &history,
                       const std::vector<TopoDS_Shape> &inputs);

// Runs the one-time initialization of OCCT's process-wide state: removes the default message
// printers so OCCT writes nothing to standard output (kernel spec section 3.10).
void initialize();

}  // namespace detail
}  // namespace occt
}  // namespace partsmith
