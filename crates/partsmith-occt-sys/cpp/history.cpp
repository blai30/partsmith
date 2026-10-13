// History normalization: the fate of every input element (kernel spec section 3.3).
#include <algorithm>
#include <optional>
#include <utility>

#include <TopExp.hxx>
#include <TopTools_IndexedMapOfShape.hxx>

#include "partsmith-occt-sys/cpp/internal.h"

namespace partsmith {
namespace occt {
namespace detail {

void merge_history(const Handle(BRepTools_History) &history,
                   const Handle(BRepTools_History) &next) {
  if (!next.IsNull()) {
    history->Merge(next);
  }
}

namespace {

// The kind of a face, edge or vertex, the elements a history lists; nothing for other types.
std::optional<ElementKind> history_kind(TopAbs_ShapeEnum type) {
  switch (type) {
    case TopAbs_FACE:
      return ElementKind::Face;
    case TopAbs_EDGE:
      return ElementKind::Edge;
    case TopAbs_VERTEX:
      return ElementKind::Vertex;
    default:
      return std::nullopt;
  }
}

// The fate of one input element of `kind` (kernel spec section 3.3, items 1 to 4).
FateData fate_of(const TopoDS_Shape &element, ElementKind kind,
                 const Handle(BRepTools_History) &history, const Shape &result) {
  const TopTools_IndexedMapOfShape &same_kind = result.map(kind);
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

  // Sorted by kind (faces, then edges, then vertices) and index.
  std::vector<std::pair<ElementKind, std::uint32_t>> generated;
  if (!history.IsNull()) {
    for (const TopoDS_Shape &shape : history->Generated(element)) {
      const std::optional<ElementKind> generated_kind = history_kind(shape.ShapeType());
      if (!generated_kind) {
        continue;
      }
      const int index = result.map(*generated_kind).FindIndex(shape);
      if (index > 0) {
        generated.emplace_back(*generated_kind, static_cast<std::uint32_t>(index));
      }
    }
  }
  std::sort(generated.begin(), generated.end());
  generated.erase(std::unique(generated.begin(), generated.end()), generated.end());

  FateData fate;
  for (std::uint32_t index : modified) {
    fate.modified.push_back(index);
  }
  for (const auto &[generated_kind, index] : generated) {
    fate.generated.push_back(SubShapeData{generated_kind, index});
  }
  return fate;
}

// The fates of every input element of `kind`, in the input's index order.
rust::Vec<FateData> fates_of(const TopoDS_Shape &input, ElementKind kind,
                             const Handle(BRepTools_History) &history, const Shape &result) {
  TopTools_IndexedMapOfShape elements;
  TopExp::MapShapes(input, shape_enum(kind), elements);
  rust::Vec<FateData> fates;
  fates.reserve(static_cast<std::size_t>(elements.Extent()));
  for (int index = 1; index <= elements.Extent(); ++index) {
    fates.push_back(fate_of(elements(index), kind, history, result));
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
        fates_of(input, ElementKind::Face, history, result),
        fates_of(input, ElementKind::Edge, history, result),
        fates_of(input, ElementKind::Vertex, history, result),
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
