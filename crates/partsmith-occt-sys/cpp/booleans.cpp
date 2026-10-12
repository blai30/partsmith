// Fuse, cut and common with unification of coplanar faces (kernel spec section 3.4.5).
#include <sstream>
#include <vector>

#include <BRepAlgoAPI_BooleanOperation.hxx>
#include <Precision.hxx>
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

// Runs one boolean of `target` with `tools` in non-destructive mode, optionally unifying
// coplanar faces and collinear edges, and merges its history.
TopoDS_Shape run(BOPAlgo_Operation operation, const TopoDS_Shape &target,
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

// Runs a boolean and normalizes, checks and repairs the result. History inputs: [target,
// tools...].
OpResultData boolean_result(BOPAlgo_Operation operation, const Shape &target,
                            rust::Slice<const ShapeHandle> tools) {
  const std::vector<TopoDS_Shape> tool_shapes = shapes_of(tools);
  Handle(BRepTools_History) history = new BRepTools_History;
  TopoDS_Shape result;
  if (operation == BOPAlgo_COMMON) {
    // The intersection with every tool: one common per tool, unified at the end.
    result = target.get();
    for (std::size_t position = 0; position < tool_shapes.size(); ++position) {
      if (!TopExp_Explorer(result, TopAbs_SOLID).More()) {
        break;
      }
      TopTools_ListOfShape tool;
      tool.Append(tool_shapes[position]);
      result = run(operation, result, tool, position + 1 == tool_shapes.size(), history);
    }
  } else {
    TopTools_ListOfShape all_tools;
    for (const TopoDS_Shape &tool : tool_shapes) {
      all_tools.Append(tool);
    }
    result = run(operation, target.get(), all_tools, true, history);
  }
  result = solids_only(result);
  if (TopExp_Explorer(result, TopAbs_SOLID).More()) {
    ensure_valid(result, history, operation_name(operation));
    result = solids_only(result);
  }
  std::vector<TopoDS_Shape> inputs{target.get()};
  inputs.insert(inputs.end(), tool_shapes.begin(), tool_shapes.end());
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
