// Fuse, cut and common with unification of coplanar faces (kernel spec section 3.4.5).
#include <sstream>
#include <vector>

#include <BRepAlgoAPI_BooleanOperation.hxx>
#include <Precision.hxx>
#include <Standard_NullObject.hxx>
#include <TopExp_Explorer.hxx>

#include "partsmith-occt-sys/cpp/internal.h"

namespace partsmith {
namespace occt {

using namespace detail;

namespace {

// The name of a boolean operation in failure messages.
const char *operation_name(BOPAlgo_Operation operation) {
  switch (operation) {
    case BOPAlgo_FUSE:
      return "BRepAlgoAPI_Fuse";
    case BOPAlgo_CUT:
      return "BRepAlgoAPI_Cut";
    default:
      return "BRepAlgoAPI_Common";
  }
}

}  // namespace

TopoDS_Shape detail::run_boolean(BOPAlgo_Operation operation, const TopoDS_Shape &target,
                                 const TopTools_ListOfShape &tools, bool unify,
                                 const Handle(BRepTools_History) &history) {
  BRepAlgoAPI_BooleanOperation boolean;
  boolean.SetOperation(operation);
  TopTools_ListOfShape arguments;
  arguments.Append(target);
  boolean.SetArguments(arguments);
  boolean.SetTools(tools);
  boolean.SetNonDestructive(Standard_True);
  boolean.SetRunParallel(Standard_False);
  boolean.SetToFillHistory(Standard_True);
  boolean.Build();
  if (boolean.HasErrors()) {
    std::ostringstream alerts;
    boolean.DumpErrors(alerts);
    raise_not_done(std::string(operation_name(operation)) + " failed: " + alerts.str());
  }
  if (unify) {
    boolean.SimplifyResult(Standard_True, Standard_True, Precision::Angular());
  }
  merge_history(history, boolean.History());
  return boolean.Shape();
}

namespace {

// Runs a boolean and normalizes, checks and repairs the result. History inputs: [target,
// tools...].
OpResultData boolean_result(BOPAlgo_Operation operation, const Shape &target,
                            rust::Slice<const ShapeHandle> tools) {
  // The boolean runs on copies that share the inputs' geometry: unification writes curves on
  // faces and tolerances to the edges the result shares with its arguments, even when the
  // boolean then fails, and no facade function may change an input (kernel spec section 3.1).
  // A copy has the input's indices, so the history of a copy is the input's.
  std::vector<TopoDS_Shape> inputs{copy_elements(target).get()};
  for (const ShapeHandle &tool : tools) {
    if (!tool.shape) {
      Standard_NullObject::Raise("a shape handle is null");
    }
    inputs.push_back(copy_elements(*tool.shape).get());
  }
  CheckInputs check_inputs(inputs);
  const std::vector<TopoDS_Shape> tool_shapes(inputs.begin() + 1, inputs.end());
  Handle(BRepTools_History) history = new BRepTools_History;
  TopoDS_Shape result;
  if (operation == BOPAlgo_COMMON) {
    // The intersection with every tool: one common per tool, unified at the end.
    result = inputs.front();
    for (std::size_t position = 0; position < tool_shapes.size(); ++position) {
      if (!TopExp_Explorer(result, TopAbs_SOLID).More()) {
        break;
      }
      TopTools_ListOfShape tool;
      tool.Append(tool_shapes[position]);
      result = run_boolean(operation, result, tool, position + 1 == tool_shapes.size(), history);
    }
  } else {
    TopTools_ListOfShape all_tools;
    for (const TopoDS_Shape &tool : tool_shapes) {
      all_tools.Append(tool);
    }
    result = run_boolean(operation, inputs.front(), all_tools, true, history);
  }
  result = solids_only(result);
  if (TopExp_Explorer(result, TopAbs_SOLID).More()) {
    // Faces of the copies the result keeps are trusted unless unification changed them in place.
    // The result shares edges only with the copies, so the shim may store the curves of its edges
    // on planar faces, which the check would otherwise project again for every wire.
    check_inputs.settle();
    store_plane_pcurves(result);
    ensure_valid(result, history, check_inputs, operation_name(operation));
    result = solids_only(result);
  }
  return op_result(result, history, inputs);
}

}  // namespace

OpResultData fuse(const Shape &target, rust::Slice<const ShapeHandle> tools) {
  return boolean_result(BOPAlgo_FUSE, target, tools);
}

OpResultData cut(const Shape &target, rust::Slice<const ShapeHandle> tools) {
  return boolean_result(BOPAlgo_CUT, target, tools);
}

OpResultData common(const Shape &target, rust::Slice<const ShapeHandle> tools) {
  return boolean_result(BOPAlgo_COMMON, target, tools);
}

}  // namespace occt
}  // namespace partsmith
