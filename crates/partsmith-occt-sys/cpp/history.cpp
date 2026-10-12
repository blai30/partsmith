// History normalization, copies, validity and repair (kernel spec sections 3.3 and 3.5).
#include <algorithm>
#include <utility>

#include <BRepBuilderAPI_Copy.hxx>
#include <BRepCheck_Analyzer.hxx>
#include <BRepLib.hxx>
#include <BRep_Builder.hxx>
#include <ShapeBuild_ReShape.hxx>
#include <ShapeFix_Shape.hxx>
#include <StdFail_NotDone.hxx>
#include <Standard_NullObject.hxx>
#include <TopExp.hxx>
#include <TopExp_Explorer.hxx>
#include <TopoDS.hxx>
#include <TopoDS_Compound.hxx>

#include "partsmith-occt-sys/cpp/internal.h"

namespace partsmith {
namespace occt {
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

void require_solid(const TopoDS_Shape &shape, const char *algorithm) {
  if (!TopExp_Explorer(shape, TopAbs_SOLID).More()) {
    raise_empty_result(std::string(algorithm) + " produced no solid");
  }
}

TopoDS_Shape solids_only(const TopoDS_Shape &shape) {
  TopTools_IndexedMapOfShape solids;
  TopExp::MapShapes(shape, TopAbs_SOLID, solids);
  if (solids.Extent() == 1) {
    return solids(1);
  }
  BRep_Builder builder;
  TopoDS_Compound compound;
  builder.MakeCompound(compound);
  for (int index = 1; index <= solids.Extent(); ++index) {
    builder.Add(compound, solids(index));
  }
  return compound;
}

Shape copy_shape(const Shape &shape) {
  BRepBuilderAPI_Copy copier(shape.get(), Standard_True, Standard_False);
  Shape copy(copier.Shape());
  for (ElementKind kind :
       {ElementKind::Solid, ElementKind::Face, ElementKind::Edge, ElementKind::Vertex}) {
    if (copy.map(kind).Extent() != shape.map(kind).Extent()) {
      raise_not_done("BRepBuilderAPI_Copy changed the element counts");
    }
  }
  return copy;
}

void store_plane_pcurves(const TopoDS_Shape &shape) {
  for (TopExp_Explorer faces(shape, TopAbs_FACE); faces.More(); faces.Next()) {
    const TopoDS_Face &face = TopoDS::Face(faces.Current());
    for (TopExp_Explorer edges(face, TopAbs_EDGE); edges.More(); edges.Next()) {
      BRepLib::BuildPCurveForEdgeOnPlane(TopoDS::Edge(edges.Current()), face);
    }
  }
}

void merge_history(const Handle(BRepTools_History) &history,
                   const Handle(BRepTools_History) &next) {
  if (!next.IsNull()) {
    history->Merge(next);
  }
}

void ensure_valid(TopoDS_Shape &shape, const Handle(BRepTools_History) &history,
                  const char *algorithm) {
  if (BRepCheck_Analyzer(shape).IsValid()) {
    return;
  }
  initialize();
  // Repair a copy: the result may share sub-shapes with the operation's inputs.
  BRepBuilderAPI_Copy copier(shape, Standard_True, Standard_False);
  TopTools_ListOfShape arguments;
  arguments.Append(shape);
  const Handle(BRepTools_History) copy_history = new BRepTools_History(arguments, copier);
  ShapeFix_Shape fixer(copier.Shape());
  fixer.SetPrecision(kLinear);
  fixer.Perform();
  const TopoDS_Shape fixed = fixer.Shape();
  const bool lost_solids = TopExp_Explorer(shape, TopAbs_SOLID).More() &&
                           !TopExp_Explorer(fixed, TopAbs_SOLID).More();
  if (lost_solids || !BRepCheck_Analyzer(fixed).IsValid()) {
    raise_invalid_result(std::string(algorithm) +
                         " produced a shape that stays invalid after ShapeFix_Shape");
  }
  merge_history(history, copy_history);
  merge_history(history, fixer.Context()->History());
  shape = fixed;
}

TopoDS_Shape finish_solid(const TopoDS_Shape &shape, const Handle(BRepTools_History) &history,
                          const char *algorithm) {
  TopoDS_Shape result = solids_only(shape);
  require_solid(result, algorithm);
  ensure_valid(result, history, algorithm);
  return solids_only(result);
}

namespace {

// The rank of a sub-shape type in history lists (faces, then edges, then vertices), or -1.
int element_rank(TopAbs_ShapeEnum type) {
  switch (type) {
    case TopAbs_FACE:
      return 0;
    case TopAbs_EDGE:
      return 1;
    case TopAbs_VERTEX:
      return 2;
    default:
      return -1;
  }
}

constexpr ElementKind kRankKinds[] = {ElementKind::Face, ElementKind::Edge, ElementKind::Vertex};

// The fate of one input element (kernel spec section 3.3, items 1 to 4).
FateData fate_of(const TopoDS_Shape &element, const TopTools_IndexedMapOfShape &same_kind,
                 const Handle(BRepTools_History) &history, const Shape &result) {
  std::vector<std::uint32_t> modified;
  const int own = same_kind.FindIndex(element);
  if (own > 0) {
    modified.push_back(static_cast<std::uint32_t>(own));
  } else if (!history.IsNull()) {
    for (const TopoDS_Shape &image : history->Modified(element)) {
      const int index = same_kind.FindIndex(image);
      if (index > 0) {
        modified.push_back(static_cast<std::uint32_t>(index));
      }
    }
  }
  std::sort(modified.begin(), modified.end());
  modified.erase(std::unique(modified.begin(), modified.end()), modified.end());

  std::vector<std::pair<int, std::uint32_t>> generated;
  if (!history.IsNull()) {
    const TopTools_IndexedMapOfShape *maps[] = {&result.faces(), &result.edges(),
                                                &result.vertices()};
    for (const TopoDS_Shape &shape : history->Generated(element)) {
      const int rank = element_rank(shape.ShapeType());
      if (rank < 0) {
        continue;
      }
      const int index = maps[rank]->FindIndex(shape);
      if (index > 0) {
        generated.emplace_back(rank, static_cast<std::uint32_t>(index));
      }
    }
  }
  std::sort(generated.begin(), generated.end());
  generated.erase(std::unique(generated.begin(), generated.end()), generated.end());

  FateData fate;
  for (std::uint32_t index : modified) {
    fate.modified.push_back(index);
  }
  for (const auto &[rank, index] : generated) {
    fate.generated.push_back(SubShapeData{kRankKinds[rank], index});
  }
  return fate;
}

// The fates of every input element of one type, in the input's index order.
rust::Vec<FateData> fates_of(const TopoDS_Shape &input, TopAbs_ShapeEnum type,
                             const TopTools_IndexedMapOfShape &same_kind,
                             const Handle(BRepTools_History) &history, const Shape &result) {
  TopTools_IndexedMapOfShape elements;
  TopExp::MapShapes(input, type, elements);
  rust::Vec<FateData> fates;
  fates.reserve(static_cast<std::size_t>(elements.Extent()));
  for (int index = 1; index <= elements.Extent(); ++index) {
    fates.push_back(fate_of(elements(index), same_kind, history, result));
  }
  return fates;
}

}  // namespace

rust::Vec<InputHistoryData> history_data(const Handle(BRepTools_History) &history,
                                         const std::vector<TopoDS_Shape> &inputs,
                                         const Shape &result) {
  rust::Vec<InputHistoryData> data;
  data.reserve(inputs.size());
  for (const TopoDS_Shape &input : inputs) {
    data.push_back(InputHistoryData{
        fates_of(input, TopAbs_FACE, result.faces(), history, result),
        fates_of(input, TopAbs_EDGE, result.edges(), history, result),
        fates_of(input, TopAbs_VERTEX, result.vertices(), history, result),
    });
  }
  return data;
}

OpResultData op_result(const TopoDS_Shape &result, const Handle(BRepTools_History) &history,
                       const std::vector<TopoDS_Shape> &inputs) {
  std::shared_ptr<Shape> shape = make_shape(result);
  rust::Vec<InputHistoryData> data = history_data(history, inputs, *shape);
  return OpResultData{std::move(shape), std::move(data)};
}

}  // namespace detail
}  // namespace occt
}  // namespace partsmith
